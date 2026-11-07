//! 把 [`vt100`] 解析出的终端 `Screen` 渲染到 [`embedded_graphics`] 的 `DrawTarget`。
//!
//! 字体走 [`u8g2_fonts`](纯 Rust 移植的 u8g2 字库),用 `U8g2TextStyle`(e-g
//! `MonoTextStyle` 的等价物)渲染——这样能挂带中文的 Unifont/GB2312 字体。
//! 桌面用 `embedded-graphics-simulator` 开发,以后换 ESP32 真屏只换 `DrawTarget` 后端。
//!
//! 建一个 [`TerminalRenderer`],喂字节给 `vt100::Parser`,
//! 再把 `parser.screen()` 交给 `renderer.render()`。

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;
#[cfg(test)]
extern crate std;

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use core::num::NonZeroUsize;
use embedded_graphics::Drawable;
use embedded_graphics::Pixel;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::{OriginDimensions, Point, Size};
use embedded_graphics::pixelcolor::{Rgb565, Rgb888, RgbColor};
use embedded_graphics::primitives::Rectangle;
use embedded_graphics::text::renderer::TextRenderer;
use embedded_graphics::text::{Baseline, Text};
use lru::LruCache;
#[cfg(feature = "timing")]
use std::time::Instant;
use u8g2_fonts::{Font, U8g2TextStyle};
use vt100::Screen;

mod surface;
mod theme;
pub use surface::Rgb565Surface;
pub use theme::Theme;
pub use vt100;

pub fn process_output(parser: &mut vt100::Parser, output: &[u8]) {
    for &byte in output {
        if byte == b'\n' {
            parser.process(b"\r");
        }
        parser.process(&[byte]);
    }
}

/// 256 色立方体每个通道的 6 级电平(16–231)。
const CUBE: [u8; 6] = [0, 95, 135, 175, 215, 255];

/// 把 xterm 256 色索引(仅 16–255)解析成 `Rgb888`。
/// 0–15(ANSI 16)由 [`TerminalRenderer`] 的当前主题决定,在 `resolve` 里取。
fn xterm256(i: u8) -> Rgb888 {
    match i {
        16..=231 => {
            let c = (i - 16) as usize;
            Rgb888::new(CUBE[(c / 36) % 6], CUBE[(c / 6) % 6], CUBE[c % 6])
        }
        _ => {
            let v = 8 + 10 * (i - 232);
            Rgb888::new(v, v, v)
        }
    }
}

/// 8 位 RGB 通道 → `Rgb565`。
///
/// 注意 e-g 的 `Rgb565::new(r,g,b)` 取的是每通道的**低位**(r&31、g&63、b&31),
/// 不是 8 位!所以必须先把 8 位值右移到 5/6/5 位再传,否则高位被截掉(比如 g=128 会变 0)。
fn rgb565(r8: u8, g8: u8, b8: u8) -> Rgb565 {
    Rgb565::new(r8 >> 3, g8 >> 2, b8 >> 3)
}

/// 把主题的 ANSI 16 色(`ansi` + `brights`)转成 `Rgb565` 调色板(0–15)。
fn theme_palette(theme: &Theme) -> [Rgb565; 16] {
    let mut a = [Rgb565::BLACK; 16];
    for i in 0..8 {
        a[i] = rgb565(theme.ansi[i].r(), theme.ansi[i].g(), theme.ansi[i].b());
        a[i + 8] = rgb565(
            theme.brights[i].r(),
            theme.brights[i].g(),
            theme.brights[i].b(),
        );
    }
    a
}

/// cell 大小的离屏缓冲:用来判断某字体对一个字有没有 ink(`pixels` 非空 = 有字模)。
/// 仅 `draw_glyph` 检测 fallback 时临时使用。
struct InkBuf {
    size: Size,
    pixels: Vec<Pixel<Rgb565>>,
}

impl DrawTarget for InkBuf {
    type Color = Rgb565;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        self.pixels.extend(pixels);
        Ok(())
    }
}

impl OriginDimensions for InkBuf {
    fn size(&self) -> Size {
        self.size
    }
}

/// 测一个 u8g2 字体的纵向过冲(下移量,像素):用几个满高字(`│ █ ▐ 中`)以 `Baseline::Top`
/// 画到 (0,0),取 ink 最小 y 的负值(钳到 ≤0)。Unifont 满高字会高出格子顶 ~3px → 返回 3;
/// 字体没有满高字(如纯 dingbat 子集 t_78_79,只有 `❯` 这类)→ 返回 0(不补正)。
/// 这样 gb2312/t_symbols 偏移 3,而 t_78_79 不偏移,避免把 `❯` 推得太低。
fn measure_offset(style: &U8g2TextStyle<Rgb565>) -> i32 {
    const PROBE: [char; 4] = ['\u{2502}', '\u{2588}', '\u{2590}', '\u{4E2D}']; // │ █ ▐ 中
    let mut min_y = 0_i32;
    for ch in PROBE {
        let mut buf = InkBuf {
            size: Size::new(16, 32),
            pixels: Vec::new(),
        };
        let s = ch.to_string();
        let _ = Text::with_baseline(&s, Point::zero(), style.clone(), Baseline::Top).draw(&mut buf);
        if let Some(m) = buf.pixels.iter().map(|Pixel(p, _)| p.y).min() {
            min_y = min_y.min(m);
        }
    }
    -(min_y.min(0))
}

/// 字形缓存的默认容量(条目数)。终端里不同字符天然有界,LRU 再兜底;
/// 单条约 20–40B 位图 + lru 节点开销 ~80–100B,1024 条上限 ≈ 80–100KB(全在 heap,
/// lru 渐进增长,实际只占命中过的不同字符数,常见也就一百多),可用
/// [`TerminalRenderer::with_glyph_cache_cap`] 调整。
const DEFAULT_GLYPH_CACHE_CAP: usize = 1024;

/// 一个字符解码后的形状缓存条目(只存形状不存颜色)。
///
/// - `font_idx`:画出此字用的字体(0 = 主字体,1.. = [`TerminalRenderer::fallbacks`] 序号);
/// - `width_cells`:字宽(1 = ASCII,2 = CJK),决定位图列数;
/// - `bits`:1bit 位图(行优先),绘制时用当前 fg 现着色。
///
/// 位图行数 = `cell_h + y_pad`,列数 = `cell_w * width_cells`。y 域统一加 `y_pad` 存放,
/// 这样 Unifont 满高字高出格子顶的过冲像素(实际 y < 0)也能落进非负索引;
/// blit 时还原 `actual_y = stored_y - y_pad` 再加上该字体自己的纵向补正。
struct CachedGlyph {
    font_idx: u8,
    width_cells: u8,
    bits: Box<[u8]>,
}

/// 位图 mask 每行的字节数(行优先,每行 ceil(列数/8) 字节)。
fn mask_bytes_per_row(cell_w: u32, width: u32) -> usize {
    ((cell_w * width) as usize + 7) / 8
}

/// 读位图 mask 的 (x, y) 位(调用方保证在 bounds 内)。
fn mask_get(bits: &[u8], bpr: usize, x: usize, y: usize) -> bool {
    bits[y * bpr + x / 8] & (1 << (x % 8)) != 0
}

/// 把相对字模原点的像素坐标打包成 1bit 位图 mask(行优先)。
/// 越界像素(不该出现)丢弃;全空返回 `None`(如该字体画不出此字)。
fn pack_mask(
    pixels: &[(i32, i32)],
    cell_w: u32,
    width: u32,
    cell_h: u32,
    y_pad: i32,
) -> Option<Box<[u8]>> {
    let bpr = mask_bytes_per_row(cell_w, width);
    let rows = cell_h as i32 + y_pad;
    let cols = cell_w * width;
    let mut bits = vec![0u8; bpr * rows as usize].into_boxed_slice();
    let mut any = false;
    for &(px, py) in pixels {
        let yy = py + y_pad;
        if px >= 0 && px < cols as i32 && yy >= 0 && yy < rows {
            let (x, y) = (px as usize, yy as usize);
            bits[y * bpr + x / 8] |= 1 << (x % 8);
            any = true;
        }
    }
    if any { Some(bits) } else { None }
}

/// 把位图 mask 的 set bits 填进复用缓冲 `out`(先清空):逐 set bit 用 `fg` 着色,
/// y 还原 `y_pad` 偏移再加 `offset`。调用方把 `out` 喂给 `draw_iter`(如 `drain`),
/// 之后容量保留供下次复用,零分配。
fn fill_mask_into(
    out: &mut Vec<Pixel<Rgb565>>,
    bits: &[u8],
    width_cells: u8,
    cell_w: u32,
    cell_h: u32,
    y_pad: i32,
    offset: i32,
    fg: Rgb565,
    origin: Point,
) {
    out.clear();
    let bpr = mask_bytes_per_row(cell_w, width_cells as u32);
    let rows = cell_h as i32 + y_pad;
    let cols = cell_w * width_cells as u32;
    for y in 0..rows {
        for x in 0..cols as i32 {
            if mask_get(bits, bpr, x as usize, y as usize) {
                let actual_y = y - y_pad;
                out.push(Pixel(
                    Point::new(origin.x + x, origin.y + actual_y + offset),
                    fg,
                ));
            }
        }
    }
}

/// 两个矩形的并集(增量渲染累积脏矩形包围盒)。`None` 视为空。
fn union_rect(a: Option<Rectangle>, r: Rectangle) -> Option<Rectangle> {
    match a {
        None => Some(r),
        Some(p) => {
            let px1 = p.top_left.x + p.size.width as i32 - 1;
            let py1 = p.top_left.y + p.size.height as i32 - 1;
            let rx1 = r.top_left.x + r.size.width as i32 - 1;
            let ry1 = r.top_left.y + r.size.height as i32 - 1;
            let x0 = p.top_left.x.min(r.top_left.x);
            let y0 = p.top_left.y.min(r.top_left.y);
            let x1 = px1.max(rx1);
            let y1 = py1.max(ry1);
            Some(Rectangle::new(
                Point::new(x0, y0),
                Size::new((x1 - x0 + 1) as u32, (y1 - y0 + 1) as u32),
            ))
        }
    }
}

/// 把 vt100 终端 `Screen` 的 cell 网格画到任意 `DrawTarget<Color = Rgb565>`。
///
/// 由显示像素尺寸 + 字体测量(`U8g2TextStyle` 的 `line_height` / `measure_string`)推算
/// 能容纳多少列/行;`render()` 遍历每个 cell,解析颜色与属性(反色/暗淡/下划线),
/// 先填背景再画字形,最后叠一个块光标。
///
/// 字体在构造时被"吸收"进一个 `U8g2TextStyle<Rgb565>`(它内部以静态引用持有字库数据,
/// 所以本结构体不必泛型于字体类型),逐格克隆一份、把 `text_color` 换成当前 fg 即可。
pub struct TerminalRenderer {
    style: U8g2TextStyle<Rgb565>,
    /// 主字体的纵向补正(像素)。
    style_offset: i32,
    /// 备用字体链:(style, 该字体的纵向补正)。各字体过冲不同,各用各的偏移。
    fallbacks: Vec<(U8g2TextStyle<Rgb565>, i32)>,
    cell_w: u32,
    cell_h: u32,
    default_fg: Rgb565,
    default_bg: Rgb565,
    /// ANSI 16 色(0–15),已转 `Rgb565`;来自当前主题(`Theme`)。
    ansi_16: [Rgb565; 16],
    cols: u16,
    rows: u16,
    /// 字形形状缓存:key = char(终端不同字符天然有界 + LRU 容量兜底,防内存膨胀)。
    /// 命中即跳过 u8g2 解码与 fallback 离屏检测,直接 blit。
    glyph_cache: LruCache<char, CachedGlyph>,
    /// 位图 mask 的 y 偏移上界(所有字体纵向补正的最大值):让过冲的负 y 也能非负存放。
    y_pad: i32,
    /// blit 命中字形时复用的像素缓冲:每帧每个字复用同一块,避免反复堆分配。
    scratch: Vec<Pixel<Rgb565>>,
    /// 字符替换表:源字符(通常是所有字体都画不出的符号)→ 替换字符(可见)。
    /// 在 `draw_glyph` 入口替换,字形缓存 key 用替换后的字符。
    substitutions: BTreeMap<char, char>,
    /// 增量渲染(`render_diff`)用的上一帧 cell 快照:逐格 `cell == prev` 即未变。
    /// 长度 = rows*cols;首次调用或尺寸变化时清空,触发全屏重画。
    prev_grid: Vec<Option<vt100::Cell>>,
    /// 上一帧的光标位置(已折回到宽字符前导格);增量时用来恢复旧位置。
    prev_cursor: (u16, u16),
    /// `timing` feature 开启时:render 各段累计纳秒(每帧开头清零)。
    #[cfg(feature = "timing")]
    t_bg: u64,
    #[cfg(feature = "timing")]
    t_hit: u64,
    #[cfg(feature = "timing")]
    t_miss: u64,
    #[cfg(feature = "timing")]
    t_cursor: u64,
}

impl TerminalRenderer {
    /// 按显示像素尺寸和 u8g2 字体构造。`cols`/`rows` 由尺寸整除字体单格大小得到
    /// (末行放不下一整行字模会被截掉,故调用方应先 `display.clear(default_bg)`)。
    /// 单格尺寸自动从字体测量:行高 = `line_height()`,列宽 = `measure_string("M")` 的横向 advance。
    pub fn new<F: Font>(
        display_size: Size,
        font: F,
        default_fg: Rgb565,
        default_bg: Rgb565,
    ) -> Self {
        let probe = U8g2TextStyle::new(font, default_fg);
        let cell_h = probe.line_height();
        let cell_w = probe
            .measure_string("M", Point::new(0, 0), Baseline::Top)
            .next_position
            .x as u32;
        assert!(cell_w > 0 && cell_h > 0, "font cell size must be non-zero");
        let cols = (display_size.width / cell_w) as u16;
        let rows = (display_size.height / cell_h) as u16;
        let style_offset = measure_offset(&probe);
        Self {
            style: probe,
            style_offset,
            fallbacks: Vec::new(),
            cell_w,
            cell_h,
            default_fg,
            default_bg,
            ansi_16: theme_palette(&Theme::DEFAULT),
            cols,
            rows,
            glyph_cache: LruCache::new(
                NonZeroUsize::new(DEFAULT_GLYPH_CACHE_CAP).expect("non-zero cache cap"),
            ),
            y_pad: style_offset,
            scratch: Vec::new(),
            substitutions: BTreeMap::new(),
            prev_grid: Vec::new(),
            prev_cursor: (0, 0),
            #[cfg(feature = "timing")]
            t_bg: 0,
            #[cfg(feature = "timing")]
            t_hit: 0,
            #[cfg(feature = "timing")]
            t_miss: 0,
            #[cfg(feature = "timing")]
            t_cursor: 0,
        }
    }

    /// 追加一个备用字体(可链式多次调用)。主字体画不出的字会依次试备用链。
    ///
    /// 备用字体的**行高**必须和主字体一致(否则 panic)。列宽不单独校验——备用字体可能不含
    /// ASCII(如纯符号子集,`measure_string("M")` 会返回 0),所以列宽沿用主字体的;
    /// 所有 unifont 变体都是 8 宽,备用字形画在主字体定义的格宽里正好。
    #[must_use]
    pub fn with_fallback_font<F: Font>(mut self, font: F) -> Self {
        let probe = U8g2TextStyle::new(font, self.default_fg);
        let h = probe.line_height();
        assert!(
            h == self.cell_h,
            "fallback font line height {h} != primary {}",
            self.cell_h,
        );
        let off = measure_offset(&probe);
        // y_pad 是所有字体纵向补正的上界,新字体若过冲更大就抬高一格,位图 mask 才装得下。
        if off > self.y_pad {
            self.y_pad = off;
        }
        self.fallbacks.push((probe, off));
        self
    }

    /// 设置字形缓存容量(条目数)。终端不同字符天然有界,容量主要防「长时间运行、切换
    /// 不同语言/会话」时字符累积;给个够覆盖单次会话字符集的值即可(默认 256)。
    /// 内存紧张就调小,字符极多重复的场景可调大。
    #[must_use]
    pub fn with_glyph_cache_cap(mut self, cap: usize) -> Self {
        let cap = NonZeroUsize::new(cap.max(1)).expect("non-zero cache cap");
        self.glyph_cache = LruCache::new(cap);
        self
    }

    /// 配置一个字符替换:`from`(通常是所有字体都画不出的符号)→ `to`(可见字符)。
    /// 渲染时遇到 `from` 就画 `to`,让缺字至少能看到占位而不是空白。可链式多次调用。
    /// `to` 应选字体里确实有的字符(如 ASCII)。
    #[must_use]
    pub fn with_substitution(mut self, from: char, to: char) -> Self {
        self.substitutions.insert(from, to);
        self
    }

    /// 切换主题:覆盖默认前景/背景 + ANSI 16 色。字形缓存、字体配置不受影响。
    /// 用预设(如 [`Theme::SEQUOIA_MOONLIGHT`])或自定义 `Theme`。
    #[must_use]
    pub fn with_theme(mut self, theme: Theme) -> Self {
        self.default_fg = rgb565(
            theme.foreground.r(),
            theme.foreground.g(),
            theme.foreground.b(),
        );
        self.default_bg = rgb565(
            theme.background.r(),
            theme.background.g(),
            theme.background.b(),
        );
        self.ansi_16 = theme_palette(&theme);
        self
    }

    /// 可容纳的列数。
    #[must_use]
    pub fn cols(&self) -> u16 {
        self.cols
    }

    /// 可容纳的行数。
    #[must_use]
    pub fn rows(&self) -> u16 {
        self.rows
    }

    /// 单格像素尺寸 `(宽, 高)`。
    #[must_use]
    pub fn cell_size(&self) -> (u32, u32) {
        (self.cell_w, self.cell_h)
    }

    /// `vt100::Color` → `Rgb565`;`Color::Default` 用传入的默认色兜底。
    fn resolve(&self, color: vt100::Color, default: Rgb565) -> Rgb565 {
        match color {
            vt100::Color::Default => default,
            vt100::Color::Rgb(r, g, b) => rgb565(r, g, b),
            vt100::Color::Idx(i) => match i {
                0..=15 => self.ansi_16[i as usize],
                _ => {
                    let c = xterm256(i);
                    rgb565(c.r(), c.g(), c.b())
                }
            },
        }
    }

    /// `font_idx`(0=主字体,1..=fallback 序号)对应的纵向补正。
    fn offset_for(&self, font_idx: u8) -> i32 {
        if font_idx == 0 {
            self.style_offset
        } else {
            self.fallbacks[font_idx as usize - 1].1
        }
    }

    /// 把 `contents` 画到 `display` 的 `origin`(字模左上角),占 `width` 个格宽。
    ///
    /// **字形缓存**:单字符的解码形状按 `char` 缓存。命中 → 用当前 fg blit 位图 mask
    /// (跳过 u8g2 解码 + fallback 离屏检测);未命中 → [`Self::decode_glyph`] 定字体、
    /// 收形状,存进缓存后 blit。回放里字符高度重复,绝大多数走命中路径,几乎零解码开销。
    fn draw_glyph<D>(
        &mut self,
        contents: &str,
        fg: Rgb565,
        origin: Point,
        width: u32,
        display: &mut D,
    ) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        let single = contents
            .chars()
            .next()
            .filter(|_| contents.chars().count() == 1)
            .map(|ch| self.substitutions.get(&ch).copied().unwrap_or(ch));

        // 命中:取缓存里的位图 mask 直接 blit(零解码)。
        // 这里只读 self 的标量字段传给自由函数——`get` 是 &mut 借用,
        // 若再调 &self 方法会整体借用冲突,故 offset/cell 尺寸就地取标量。
        if let Some(ch) = single {
            if let Some(g) = self.glyph_cache.get(&ch) {
                #[cfg(feature = "timing")]
                let _t = Instant::now();
                let offset = if g.font_idx == 0 {
                    self.style_offset
                } else {
                    self.fallbacks[g.font_idx as usize - 1].1
                };
                fill_mask_into(
                    &mut self.scratch,
                    &g.bits,
                    g.width_cells,
                    self.cell_w,
                    self.cell_h,
                    self.y_pad,
                    offset,
                    fg,
                    origin,
                );
                let r = display.draw_iter(self.scratch.drain(..)).map(|_| ());
                #[cfg(feature = "timing")]
                {
                    self.t_hit += _t.elapsed().as_nanos() as u64;
                }
                return r;
            }
        }

        // 未命中:离屏解码 + fallback 检测。
        #[cfg(feature = "timing")]
        let _t = Instant::now();
        // 用替换后的字符(若配了 substitution)解码;多字符 contents 不走替换。
        let (font_idx, pixels) = match single {
            Some(ch) => {
                let mut buf = [0u8; 4];
                let s: &str = ch.encode_utf8(&mut buf);
                self.decode_glyph(s, fg, width)
            }
            None => self.decode_glyph(contents, fg, width),
        };

        // 存进缓存(仅单字符、且有形状)。
        if let Some(ch) = single {
            if let Some(mask) = pack_mask(&pixels, self.cell_w, width, self.cell_h, self.y_pad) {
                self.glyph_cache.put(
                    ch,
                    CachedGlyph {
                        font_idx,
                        width_cells: width as u8,
                        bits: mask,
                    },
                );
            }
        }

        // 把这一格画出来(存没存缓存都画)。
        let offset = self.offset_for(font_idx);
        let r = display.draw_iter(
            pixels
                .iter()
                .map(|&(px, py)| Pixel(Point::new(origin.x + px, origin.y + py + offset), fg)),
        );
        #[cfg(feature = "timing")]
        {
            self.t_miss += _t.elapsed().as_nanos() as u64;
        }
        r
    }

    /// 离屏解码 + fallback 检测:返回画出此字的字体序号(0=主)及其形状像素坐标
    /// (相对 InkBuf 原点,即字模左上角;**不含**纵向补正,blit 时再加)。
    fn decode_glyph(&self, contents: &str, fg: Rgb565, width: u32) -> (u8, Vec<(i32, i32)>) {
        let mut buf = InkBuf {
            size: Size::new(self.cell_w * width, self.cell_h),
            pixels: Vec::new(),
        };
        let mut font_idx = 0u8;
        let mut s = self.style.clone();
        s.text_color = Some(fg);
        let _ = Text::with_baseline(contents, Point::zero(), s, Baseline::Top).draw(&mut buf);
        if buf.pixels.is_empty() {
            for (i, (fb, _)) in self.fallbacks.iter().enumerate() {
                buf.pixels.clear();
                let mut s = fb.clone();
                s.text_color = Some(fg);
                // s.measure_string(text, position, baseline)
                let _ =
                    Text::with_baseline(contents, Point::zero(), s, Baseline::Top).draw(&mut buf);
                if !buf.pixels.is_empty() {
                    font_idx = i as u8 + 1;
                    break;
                }
            }
        }
        let pixels = buf.pixels.iter().map(|Pixel(p, _)| (p.x, p.y)).collect();
        (font_idx, pixels)
    }

    /// 把整屏 `screen` 渲染到 `display`。
    /// 渲染整屏 `screen` 到 `display`,等价于 [`Self::render_rows`]`(0, rows)`。
    pub fn render<D>(&mut self, screen: &Screen, display: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        self.render_rows(screen, display, 0, self.rows)
    }

    /// 渲染 `screen` 的指定行范围 `[row_start, row_end)` 到 `display`,其余同 [`Self::render`]。
    /// `row_start` 这行画在画布原点 (0,0):第 `row` 行落在 `(row - row_start) * cell_h`,
    /// 而不是画布的 `row * cell_h`。用于把某几行贴到画布顶部(局部刷新、分屏、viewport)。
    /// 光标仅当其所在行在范围内才画。参数钳到 `[0, rows]`;`row_end <= row_start` 时该范围本身不画。
    pub fn render_rows<D>(
        &mut self,
        screen: &Screen,
        display: &mut D,
        row_start: u16,
        row_end: u16,
    ) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        let row_start = row_start.min(self.rows);
        let row_end = row_end.min(self.rows).max(row_start);
        #[cfg(feature = "timing")]
        {
            self.t_bg = 0;
            self.t_hit = 0;
            self.t_miss = 0;
            self.t_cursor = 0;
        }
        #[cfg(feature = "timing")]
        let __frame = Instant::now();
        // ── Pass 1:背景。按行合并连续相同 bg 的 cell 成一段,一次 fill_solid ──
        // 减少 fill_solid 调用次数(整行同色时多次 → 1 次)。对小 cell 后端尤其重要:
        // 每次 fill 的逐行对齐/分支开销会被调用次数放大,合并后大矩形反而走得更顺。
        for row in row_start..row_end {
            let mut col: u16 = 0;
            while col < self.cols {
                let Some(cell) = screen.cell(row, col) else {
                    col += 1;
                    continue;
                };
                // 宽字符续格:属性是垃圾,跳过(背景已由前导格横跨两列覆盖)。
                if cell.is_wide_continuation() {
                    col += 1;
                    continue;
                }
                // 这个 cell 的格背景色。inverse 时 fg/bg 交换,所以格背景取原 fg 色;
                // dim 只压暗 fg,不动背景。
                let run_bg = if cell.inverse() {
                    self.resolve(cell.fgcolor(), self.default_fg)
                } else {
                    self.resolve(cell.bgcolor(), self.default_bg)
                };
                let run_start = col;
                let mut run_width: u32 = if cell.is_wide() { 2 } else { 1 };
                col += run_width as u16;
                // 向右合并同样 bg、且非续格的 cell
                while col < self.cols {
                    let Some(next) = screen.cell(row, col) else {
                        break;
                    };
                    if next.is_wide_continuation() {
                        break;
                    }
                    let next_bg = if next.inverse() {
                        self.resolve(next.fgcolor(), self.default_fg)
                    } else {
                        self.resolve(next.bgcolor(), self.default_bg)
                    };
                    if next_bg != run_bg {
                        break;
                    }
                    let w: u32 = if next.is_wide() { 2 } else { 1 };
                    run_width += w;
                    col += w as u16;
                }
                #[cfg(feature = "timing")]
                let _t = Instant::now();
                display.fill_solid(
                    &Rectangle::new(
                        Point::new(
                            run_start as i32 * self.cell_w as i32,
                            (row - row_start) as i32 * self.cell_h as i32,
                        ),
                        Size::new(self.cell_w * run_width, self.cell_h),
                    ),
                    run_bg,
                )?;
                #[cfg(feature = "timing")]
                {
                    self.t_bg += _t.elapsed().as_nanos() as u64;
                }
            }
        }

        // ── Pass 2:字形 + 下划线(逐格;背景已由 pass 1 填好)──
        for row in row_start..row_end {
            let mut col: u16 = 0;
            while col < self.cols {
                let Some(cell) = screen.cell(row, col) else {
                    col += 1;
                    continue;
                };
                if cell.is_wide_continuation() {
                    col += 1;
                    continue;
                }
                let width: u32 = if cell.is_wide() { 2 } else { 1 };

                // 字形色:inverse 时文字取原 bg 色,dim 时各通道 ÷2。背景不在这画。
                let mut fg = self.resolve(cell.fgcolor(), self.default_fg);
                if cell.inverse() {
                    fg = self.resolve(cell.bgcolor(), self.default_bg);
                }
                if cell.dim() {
                    // fg 是 Rgb565(原始 5/6/5 位);各通道 ÷2 再装回即可(值 ≤31/63,低位掩码无副作用)
                    fg = Rgb565::new(fg.r() / 2, fg.g() / 2, fg.b() / 2);
                }

                let x = col as i32 * self.cell_w as i32;
                let y = (row - row_start) as i32 * self.cell_h as i32;

                // 字形(有内容且非纯空格才画;空格只留背景)。
                // 注:u8g2 字体没有 bold 变体,`cell.bold()` 按常规字重渲染。
                if cell.has_contents() && cell.contents() != " " {
                    self.draw_glyph(cell.contents(), fg, Point::new(x, y), width, display)?;
                }
                // U8g2TextStyle 不支持下划线装饰 → 自己在格底画一条 1px 线保留该属性
                if cell.underline() {
                    #[cfg(feature = "timing")]
                    let _t = Instant::now();
                    display.fill_solid(
                        &Rectangle::new(
                            Point::new(x, y + self.cell_h as i32 - 1),
                            Size::new(self.cell_w * width, 1),
                        ),
                        fg,
                    )?;
                    #[cfg(feature = "timing")]
                    {
                        self.t_bg += _t.elapsed().as_nanos() as u64;
                    }
                }

                col += width as u16;
            }
        }

        #[cfg(feature = "timing")]
        let _t = Instant::now();
        self.draw_cursor(screen, display, row_start, row_end)?;
        #[cfg(feature = "timing")]
        {
            self.t_cursor += _t.elapsed().as_nanos() as u64;
            let total = __frame.elapsed().as_nanos() as u64;
            // 注:cursor 里的字形走 draw_glyph,会计入 t_hit/t_miss;故各段之和可能与 total
            // 略有出入(双重计了光标字形 + 未含循环/resolve 开销),看相对比例即可。
            let sum = self.t_bg + self.t_hit + self.t_miss + self.t_cursor;
            log::info!(
                "[render rows={}..{}] total={}μs | bg={}μs glyph(hit={}μs miss={}μs) cursor={}μs other={}μs",
                row_start,
                row_end,
                total / 1000,
                self.t_bg / 1000,
                self.t_hit / 1000,
                self.t_miss / 1000,
                self.t_cursor / 1000,
                total.saturating_sub(sum) / 1000,
            );
        }

        Ok(())
    }

    /// 画块光标:用该格自己的 fg 填块,有字就用 bg 反色重画。处理宽字符续格与行末越界。
    /// 增量渲染用的单格普通重画:背景(width 列)+ 字形 + 下划线,不含光标块。
    fn draw_cell<D>(
        &mut self,
        screen: &Screen,
        row: u16,
        col: u16,
        row_start: u16,
        display: &mut D,
    ) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        let Some(cell) = screen.cell(row, col) else {
            return Ok(());
        };
        if cell.is_wide_continuation() {
            return Ok(());
        }
        let width: u32 = if cell.is_wide() { 2 } else { 1 };
        let x = col as i32 * self.cell_w as i32;
        let y = (row as i32 - row_start as i32) * self.cell_h as i32;
        // 背景(inverse 时取 fg 色;dim 只压暗 fg,不动背景)
        let bg = if cell.inverse() {
            self.resolve(cell.fgcolor(), self.default_fg)
        } else {
            self.resolve(cell.bgcolor(), self.default_bg)
        };
        #[cfg(feature = "timing")]
        let _t = Instant::now();
        display.fill_solid(
            &Rectangle::new(
                Point::new(x, y),
                Size::new(self.cell_w * width, self.cell_h),
            ),
            bg,
        )?;
        #[cfg(feature = "timing")]
        {
            self.t_bg += _t.elapsed().as_nanos() as u64;
        }
        // 字形 + 下划线
        let mut fg = self.resolve(cell.fgcolor(), self.default_fg);
        if cell.inverse() {
            fg = self.resolve(cell.bgcolor(), self.default_bg);
        }
        if cell.dim() {
            fg = Rgb565::new(fg.r() / 2, fg.g() / 2, fg.b() / 2);
        }
        if cell.has_contents() && cell.contents() != " " {
            self.draw_glyph(cell.contents(), fg, Point::new(x, y), width, display)?;
        }
        if cell.underline() {
            #[cfg(feature = "timing")]
            let _t = Instant::now();
            display.fill_solid(
                &Rectangle::new(
                    Point::new(x, y + self.cell_h as i32 - 1),
                    Size::new(self.cell_w * width, 1),
                ),
                fg,
            )?;
            #[cfg(feature = "timing")]
            {
                self.t_bg += _t.elapsed().as_nanos() as u64;
            }
        }
        Ok(())
    }

    /// 在给定(前导)格画光标块:fg 填块,有字用 bg 反色重画。调用方保证 row/col 有效。
    fn draw_cursor_block<D>(
        &mut self,
        screen: &Screen,
        row: u16,
        col: u16,
        pos: Point,
        display: &mut D,
    ) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        let cell = screen.cell(row, col);
        let blk_fg = cell
            .map(|c| self.resolve(c.fgcolor(), self.default_fg))
            .unwrap_or(self.default_fg);
        let blk_bg = cell
            .map(|c| self.resolve(c.bgcolor(), self.default_bg))
            .unwrap_or(self.default_bg);
        display.fill_solid(
            &Rectangle::new(pos, Size::new(self.cell_w, self.cell_h)),
            blk_fg,
        )?;
        if let Some(c) = cell {
            if c.has_contents() && c.contents() != " " {
                let w = if c.is_wide() { 2 } else { 1 };
                self.draw_glyph(c.contents(), blk_bg, pos, w, display)?;
            }
        }
        Ok(())
    }

    /// 增量渲染:只重画自上次调用以来变化的格 + 光标旧/新位置,返回脏矩形包围盒。
    /// 调用方据此只 flush 那一块;返回 `None` 表示无变化,可整帧跳过 flush。
    ///
    /// 首次调用或屏幕尺寸变化时全屏重画,并把当前 screen 存为下一帧快照。
    /// 注意:scroll 等整屏变动会让所有格都「变」,脏矩形退化为全屏(正确,只是那种帧不省);
    /// append 少量字节、光标移动等常见场景才显著省 render + flush。
    /// 增量渲染整屏,等价于 [`Self::render_row_diff`]`(0, rows)`。
    pub fn render_diff<D>(
        &mut self,
        screen: &Screen,
        display: &mut D,
    ) -> Result<Option<Rectangle>, D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        self.render_row_diff(screen, display, 0, self.rows)
    }

    /// 增量渲染指定行范围 `[row_start, row_end)`:逐格对比上一帧快照,只重画变化的格,
    /// 返回脏矩形。`row_start` 这行落在画布原点 (0,0)——第 `row` 行的脏区域 y 为
    /// `(row - row_start) * cell_h`(不是 `row * cell_h`),和 [`Self::render_rows`] 一致。
    ///
    /// 光标仅当其所在行落在 `[row_start, row_end)` 时才处理(旧位置恢复 + 新位置画块);
    /// 范围外的光标变化不画,由调用方另行刷新那一带。首帧或尺寸变化时该范围全画。
    pub fn render_row_diff<D>(
        &mut self,
        screen: &Screen,
        display: &mut D,
        row_start: u16,
        row_end: u16,
    ) -> Result<Option<Rectangle>, D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        let row_start = row_start.min(self.rows);
        let row_end = row_end.min(self.rows).max(row_start);
        #[cfg(feature = "timing")]
        {
            self.t_bg = 0;
            self.t_hit = 0;
            self.t_miss = 0;
            self.t_cursor = 0;
        }
        #[cfg(feature = "timing")]
        let __frame = Instant::now();
        let rows = self.rows;
        let cols = self.cols;
        let n = rows as usize * cols as usize;
        let first = self.prev_grid.len() != n;
        if first {
            self.prev_grid.clear();
            self.prev_grid.resize(n, None);
        }
        let cell_w = self.cell_w;
        let cell_h = self.cell_h;
        let mut dirty: Option<Rectangle> = None;
        let mut mark = |row: u16, col: u16, width: u32| {
            let r = Rectangle::new(
                Point::new(
                    (col as u32 * cell_w) as i32,
                    ((row as u32 - row_start as u32) * cell_h) as i32,
                ),
                Size::new(cell_w * width, cell_h),
            );
            dirty = union_rect(dirty, r);
        };

        // Pass 1:逐格对比 prev 快照,变化的格普通重画(续格由前导覆盖,不单独画)。
        for row in row_start..row_end {
            for col in 0..cols {
                let idx = row as usize * cols as usize + col as usize;
                let cur = screen.cell(row, col).cloned();
                if first || self.prev_grid[idx] != cur {
                    self.prev_grid[idx] = cur.clone();
                    let is_cont = cur
                        .as_ref()
                        .map(|c| c.is_wide_continuation())
                        .unwrap_or(false);
                    if !is_cont {
                        self.draw_cell(screen, row, col, row_start, display)?;
                        let w = cur
                            .as_ref()
                            .map(|c| if c.is_wide() { 2 } else { 1 })
                            .unwrap_or(1);
                        mark(row, col, w);
                    }
                }
            }
        }

        // Pass 2:光标(仅当其行落在 [row_start, row_end) 才处理)。旧位置恢复,新位置画块。
        let cur_visible = !screen.hide_cursor();
        let (crow, ccol) = if cur_visible {
            let (r, mut c) = screen.cursor_position();
            if c > 0
                && screen
                    .cell(r, c)
                    .map(|x| x.is_wide_continuation())
                    .unwrap_or(false)
            {
                c -= 1;
            }
            (r, c)
        } else {
            (self.rows, self.cols) // 哨兵:越界 → 不画块、不误恢复
        };
        let (prow, pcol) = self.prev_cursor;
        if (prow, pcol) != (crow, ccol)
            && (row_start..row_end).contains(&prow)
            && prow < rows
            && pcol < cols
        {
            self.draw_cell(screen, prow, pcol, row_start, display)?;
            let w = screen
                .cell(prow, pcol)
                .map(|c| if c.is_wide() { 2 } else { 1 })
                .unwrap_or(1);
            mark(prow, pcol, w);
        }
        if cur_visible && (row_start..row_end).contains(&crow) && crow < rows && ccol < cols {
            let pos = Point::new(
                ccol as i32 * self.cell_w as i32,
                (crow as i32 - row_start as i32) * self.cell_h as i32,
            );
            self.draw_cursor_block(screen, crow, ccol, pos, display)?;
            mark(crow, ccol, 1);
        }
        self.prev_cursor = (crow, ccol);

        #[cfg(feature = "timing")]
        {
            let total = __frame.elapsed().as_nanos() as u64;
            let sum = self.t_bg + self.t_hit + self.t_miss + self.t_cursor;
            log::info!(
                "[render_row_diff rows={}..{}] total={}μs dirty={}x{} | bg={}μs glyph(hit={}μs miss={}μs) cursor={}μs other={}μs",
                row_start,
                row_end,
                total / 1000,
                dirty.as_ref().map(|r| r.size.width).unwrap_or(0),
                dirty.as_ref().map(|r| r.size.height).unwrap_or(0),
                self.t_bg / 1000,
                self.t_hit / 1000,
                self.t_miss / 1000,
                self.t_cursor / 1000,
                total.saturating_sub(sum) / 1000,
            );
        }

        Ok(dirty)
    }

    /// 作废增量快照:下次 [`Self::render_diff`] 会全屏重画并重建快照,但**保留字形缓存**
    /// (`glyph_cache`)和字体配置。用于「重建了 parser / 换了 screen / 想强制全刷」——
    /// 不必重建 renderer,字形缓存的命中收益继续生效。
    pub fn invalidate(&mut self) {
        self.prev_grid.clear();
        self.prev_cursor = (self.rows, self.cols); // 哨兵:下次首帧不误恢复旧光标位置
    }

    /// 字形缓存当前条目数(诊断用)。若每帧打印都接近 0,说明 renderer 被反复重建、
    /// 缓存没跨帧——这才是 miss 居高不下的根因,不是容量问题。
    pub fn cache_len(&self) -> usize {
        self.glyph_cache.len()
    }

    fn draw_cursor<D>(
        &mut self,
        screen: &Screen,
        display: &mut D,
        row_start: u16,
        row_end: u16,
    ) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        if screen.hide_cursor() {
            return Ok(());
        }
        let (crow, mut ccol) = screen.cursor_position();
        // 光标落在宽字符续格上 → 把块画到前导格(col 不可能为 0 且是续格)
        if ccol > 0
            && screen
                .cell(crow, ccol)
                .map(|c| c.is_wide_continuation())
                .unwrap_or(false)
        {
            ccol -= 1;
        }
        // 仅当光标行落在 [row_start, row_end) 且不越界时才画
        if !(row_start..row_end).contains(&crow) || crow >= self.rows || ccol >= self.cols {
            return Ok(());
        }
        let pos = Point::new(
            ccol as i32 * self.cell_w as i32,
            (crow - row_start) as i32 * self.cell_h as i32,
        );
        self.draw_cursor_block(screen, crow, ccol, pos, display)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;
    use embedded_graphics::Pixel;
    use embedded_graphics::geometry::OriginDimensions;
    use std::collections::HashMap;
    use u8g2_fonts::fonts::u8g2_font_unifont_t_gb2312;

    fn parser(renderer: &TerminalRenderer) -> vt100::Parser {
        vt100::Parser::new(
            renderer.rows().try_into().unwrap(),
            renderer.cols().try_into().unwrap(),
            0,
        )
    }

    #[test]
    fn output_newlines_and_split_utf8() {
        let mut parser = vt100::Parser::new(4.try_into().unwrap(), 20.try_into().unwrap(), 0);
        process_output(&mut parser, b"first\nsecond\r\n");
        process_output(&mut parser, &[0xe4, 0xb8]);
        process_output(&mut parser, &[0xad]);
        assert_eq!(parser.screen().contents(), "first\nsecond\n中");
    }

    /// 记录每个点最后被写入的颜色,用来在测试里断言像素(无需 SDL)。
    struct Canvas {
        size: Size,
        pixels: HashMap<(i32, i32), Rgb565>,
    }

    impl Canvas {
        fn new(size: Size) -> Self {
            Self {
                size,
                pixels: HashMap::new(),
            }
        }
        fn at(&self, x: i32, y: i32) -> Option<Rgb565> {
            self.pixels.get(&(x, y)).copied()
        }
    }

    #[derive(Debug)]
    struct VoidError;

    impl DrawTarget for Canvas {
        type Color = Rgb565;
        type Error = VoidError;

        fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
        where
            I: IntoIterator<Item = Pixel<Self::Color>>,
        {
            for Pixel(p, c) in pixels {
                self.pixels.insert((p.x, p.y), c);
            }
            Ok(())
        }
    }

    impl OriginDimensions for Canvas {
        fn size(&self) -> Size {
            self.size
        }
    }

    /// 喂字节给一个 2×2 cell 的小屏(unifont 8×16 → 16×32),渲染到 Canvas。
    fn render_bytes(bytes: &[u8]) -> Canvas {
        let display_size = Size::new(8 * 2, 16 * 2);
        let mut renderer = TerminalRenderer::new(
            display_size,
            u8g2_font_unifont_t_gb2312,
            Rgb565::WHITE,
            Rgb565::BLACK,
        );
        let mut parser = parser(&renderer);
        parser.process(bytes);
        let mut canvas = Canvas::new(display_size);
        renderer.render(parser.screen(), &mut canvas).unwrap();
        canvas
    }

    #[test]
    fn palette_math() {
        // xterm256 只管 16–255(0–15 由主题决定)
        assert_eq!(xterm256(196), Rgb888::new(255, 0, 0)); // 立方体 196 = 红
        assert_eq!(xterm256(232), Rgb888::new(8, 8, 8)); // 灰阶首
        assert_eq!(xterm256(255), Rgb888::new(238, 238, 238)); // 灰阶末
        // 主题 ANSI 16:Theme::DEFAULT 沿用旧 VGA(red 128,0,0;bright red 255,0,0;…)
        let p = theme_palette(&Theme::DEFAULT);
        assert_eq!(p[1], rgb565(128, 0, 0)); // red
        assert_eq!(p[9], rgb565(255, 0, 0)); // bright red
        assert_eq!(p[15], rgb565(255, 255, 255)); // bright white
    }

    #[test]
    fn theme_applies_to_ansi() {
        let r = TerminalRenderer::new(
            Size::new(8 * 4, 17 * 2),
            u8g2_font_unifont_t_gb2312,
            Rgb565::WHITE,
            Rgb565::BLACK,
        )
        .with_theme(Theme::SEQUOIA_MOONLIGHT);
        // Sequoia Moonlight ansi[1] (red) = #F58EE0;ansi[4] (blue) = #C58FFF
        assert_eq!(
            r.resolve(vt100::Color::Idx(1), Rgb565::BLACK),
            rgb565(245, 142, 224)
        );
        assert_eq!(
            r.resolve(vt100::Color::Idx(4), Rgb565::BLACK),
            rgb565(197, 143, 255)
        );
    }

    #[test]
    fn theme_red_bg_renders_sequoia() {
        // \x1b[41m = red 背景;Sequoia Moonlight red = #F58EE0 (245,142,224)
        let size = Size::new(8 * 2, 16 * 2);
        let mut renderer = TerminalRenderer::new(
            size,
            u8g2_font_unifont_t_gb2312,
            Rgb565::WHITE,
            Rgb565::BLACK,
        )
        .with_theme(Theme::SEQUOIA_MOONLIGHT);
        let mut parser = parser(&renderer);
        parser.process(b"\x1b[41m \x1b[m");
        let mut canvas = Canvas::new(size);
        renderer.render(parser.screen(), &mut canvas).unwrap();
        assert_eq!(canvas.at(4, 6), Some(rgb565(245, 142, 224)));
    }

    #[test]
    fn background_color_resolved() {
        // 绿底空格 → 整格背景应是 256 色 idx=2 对应的绿(Rgb888 0,128,0 → 565)
        let canvas = render_bytes(b"\x1b[42m \x1b[m");
        assert_eq!(canvas.at(4, 6), Some(rgb565(0, 128, 0)));
    }

    #[test]
    fn inverse_swaps_background() {
        // 反色空格,默认 fg=白 bg=黑 → 格背景应是白(fg 被换到背景)
        let canvas = render_bytes(b"\x1b[7m \x1b[m");
        assert_eq!(canvas.at(4, 6), Some(Rgb565::WHITE));
    }

    #[test]
    fn chinese_glyph_renders() {
        // 一个汉字(宽,占 2 格)→ 它覆盖的格区里应有非黑像素(字形真的画出来了)
        let canvas = render_bytes("中".as_bytes());
        let cell_w = 8; // unifont 列宽
        let mut ink = 0u64;
        for y in 0..16 {
            for x in 0..(cell_w * 2) {
                if let Some(c) = canvas.at(x as i32, y as i32) {
                    if c.r() > 0 || c.g() > 0 || c.b() > 0 {
                        ink += 1;
                    }
                }
            }
        }
        assert!(ink > 20, "expected Chinese glyph ink, got {ink}");
    }

    #[test]
    fn fallback_font_chain() {
        // ▐(U+2590) 在 t_symbols、不在 gb2312;❯(U+276F) 在 t_78_79、不在 gb2312/t_symbols。
        // 验证:主字体画不出(ink=0)→ fallback 检测 → 链走到能画的那一个。
        use u8g2_fonts::fonts::{u8g2_font_unifont_t_78_79, u8g2_font_unifont_t_symbols};
        let size = Size::new(8 * 2, 16 * 2);

        let ink = |r: &mut TerminalRenderer, ch: char| -> u64 {
            let mut p = parser(r);
            // 先隐光标,免得光标块的白像素混进 ink 计数
            let bytes = format!("\x1b[?25l{ch}");
            p.process(bytes.as_bytes());
            let mut c = Canvas::new(size);
            r.render(p.screen(), &mut c).unwrap();
            c.pixels
                .values()
                .filter(|&&col| col != Rgb565::BLACK)
                .count() as u64
        };

        let mut primary = TerminalRenderer::new(
            size,
            u8g2_font_unifont_t_gb2312,
            Rgb565::WHITE,
            Rgb565::BLACK,
        );
        let mut with_sym = TerminalRenderer::new(
            size,
            u8g2_font_unifont_t_gb2312,
            Rgb565::WHITE,
            Rgb565::BLACK,
        )
        .with_fallback_font(u8g2_font_unifont_t_symbols);
        let mut with_chain = TerminalRenderer::new(
            size,
            u8g2_font_unifont_t_gb2312,
            Rgb565::WHITE,
            Rgb565::BLACK,
        )
        .with_fallback_font(u8g2_font_unifont_t_symbols)
        .with_fallback_font(u8g2_font_unifont_t_78_79);

        // ▐: gb2312 无 → 0;加 t_symbols → 有
        assert_eq!(ink(&mut primary, '▐'), 0);
        assert!(ink(&mut with_sym, '▐') > 0);
        // ❯: gb2312 + t_symbols 都无 → 0;链加到 t_78_79 才有
        assert_eq!(ink(&mut with_sym, '❯'), 0);
        assert!(ink(&mut with_chain, '❯') > 0);
    }

    /// 收集 `canvas` 上某 cell 区域(`x0` 起,`cell_w`×`cell_h`)里所有白像素的相对坐标。
    fn glyph_set_at(
        canvas: &Canvas,
        x0: i32,
        cell_w: i32,
        cell_h: i32,
    ) -> std::collections::HashSet<(i32, i32)> {
        let mut s = std::collections::HashSet::new();
        for y in 0..cell_h {
            for x in 0..cell_w {
                if canvas.at(x0 + x, y) == Some(Rgb565::WHITE) {
                    s.insert((x, y));
                }
            }
        }
        s
    }

    #[test]
    fn cache_hit_matches_decode() {
        // 两个相同的 'A'(隐光标):cell0 走解码(miss)、cell1 走缓存命中。
        // 两处字形相对各自 cell 原点的像素集合必须逐点一致——缓存正确性的硬保证。
        let size = Size::new(8 * 4, 16 * 2);
        let mut renderer = TerminalRenderer::new(
            size,
            u8g2_font_unifont_t_gb2312,
            Rgb565::WHITE,
            Rgb565::BLACK,
        );
        let mut parser = parser(&renderer);
        parser.process(b"\x1b[?25lAA");
        let mut canvas = Canvas::new(size);
        renderer.render(parser.screen(), &mut canvas).unwrap();

        let g0 = glyph_set_at(&canvas, 0, 8, 16);
        let g1 = glyph_set_at(&canvas, 8, 8, 16);
        assert!(!g0.is_empty(), "decoded glyph should have ink");
        assert_eq!(
            g0, g1,
            "cached glyph must match decoded glyph pixel-for-pixel"
        );
    }

    #[test]
    fn cache_hit_matches_decode_fallback() {
        // ▐ 走 fallback(t_symbols):同样验证缓存命中与解码逐点一致。
        use u8g2_fonts::fonts::u8g2_font_unifont_t_symbols;
        let size = Size::new(8 * 4, 16 * 2);
        let mut renderer = TerminalRenderer::new(
            size,
            u8g2_font_unifont_t_gb2312,
            Rgb565::WHITE,
            Rgb565::BLACK,
        )
        .with_fallback_font(u8g2_font_unifont_t_symbols);
        let mut parser = parser(&renderer);
        parser.process("\x1b[?25l▐▐".as_bytes());
        let mut canvas = Canvas::new(size);
        renderer.render(parser.screen(), &mut canvas).unwrap();

        let g0 = glyph_set_at(&canvas, 0, 8, 16);
        let g1 = glyph_set_at(&canvas, 8, 8, 16);
        assert!(!g0.is_empty(), "fallback glyph should have ink");
        assert_eq!(g0, g1, "cached fallback glyph must match decoded");
    }

    #[test]
    fn render_diff_incremental() {
        // 4 列 × 2 行,隐光标免得光标块混进脏矩形
        let size = Size::new(8 * 4, 16 * 2);
        let mut renderer = TerminalRenderer::new(
            size,
            u8g2_font_unifont_t_gb2312,
            Rgb565::WHITE,
            Rgb565::BLACK,
        );
        let mut parser = parser(&renderer);
        parser.process(b"\x1b[?25lAB");
        let mut canvas = Canvas::new(size);

        // 首帧:全屏脏
        let r1 = renderer.render_diff(parser.screen(), &mut canvas).unwrap();
        assert!(r1.is_some(), "first frame should be fully dirty");

        // 无新字节:无变化 → None(调用方可整帧跳过)
        let r2 = renderer.render_diff(parser.screen(), &mut canvas).unwrap();
        assert!(r2.is_none(), "no change should yield no dirty rect");

        // append 'C':只有第 3 格(col=2)变化,脏矩形应只覆盖那一格
        parser.process(b"C");
        let r3 = renderer
            .render_diff(parser.screen(), &mut canvas)
            .unwrap()
            .expect("append should dirty something");
        assert!(
            r3.top_left.x >= 16,
            "dirty rect should start at col 2 (x=16), got x={}",
            r3.top_left.x
        );
        assert!(
            r3.size.width <= 16,
            "dirty rect should cover only col 2 (<=16px), got width={}",
            r3.size.width
        );
    }

    #[test]
    fn invalidate_forces_full_redraw() {
        let size = Size::new(8 * 4, 16 * 2);
        let mut renderer = TerminalRenderer::new(
            size,
            u8g2_font_unifont_t_gb2312,
            Rgb565::WHITE,
            Rgb565::BLACK,
        );
        let mut parser = parser(&renderer);
        parser.process(b"\x1b[?25lAB");
        let mut canvas = Canvas::new(size);

        let _ = renderer.render_diff(parser.screen(), &mut canvas).unwrap(); // 首帧
        let none = renderer.render_diff(parser.screen(), &mut canvas).unwrap();
        assert!(none.is_none(), "no change → None");

        // 模拟「重建了 screen」:作废快照(字形缓存不动),下次必须全屏重画。
        renderer.invalidate();
        let full = renderer.render_diff(parser.screen(), &mut canvas).unwrap();
        assert!(full.is_some(), "invalidate should force a full redraw");
        // 字形缓存应保留:invalidate 只清 prev_grid,没清 glyph_cache(逻辑保证,这里不重测 timing)。
    }

    #[test]
    fn substitution_replaces_missing_glyph() {
        // '😀'(emoji)所有 unifont 子集都不含 → 不配替换就是空白。
        // 配 substitution('😀' → '?') 后应画出 '?'(ASCII 可见)。
        let size = Size::new(8 * 4, 16 * 2);
        let mut renderer = TerminalRenderer::new(
            size,
            u8g2_font_unifont_t_gb2312,
            Rgb565::WHITE,
            Rgb565::BLACK,
        )
        .with_substitution('😀', '?');
        let mut parser = parser(&renderer);
        parser.process("\x1b[?25l😀".as_bytes());
        let mut canvas = Canvas::new(size);
        renderer.render(parser.screen(), &mut canvas).unwrap();

        let ink = canvas
            .pixels
            .values()
            .filter(|&&c| c != Rgb565::BLACK)
            .count();
        assert!(
            ink > 0,
            "substituted glyph should be visible, got {ink} ink px"
        );
    }

    #[test]
    fn render_row_diff_pastes_range_to_origin() {
        // cell_h = 17(unifont line_height),要 2 行得 height ≥ 34。
        // row0=AB,row1=CD。只增量渲染 row1,它应贴到画布 y=0,row0 不画。
        let size = Size::new(8 * 4, 17 * 2);
        let mut renderer = TerminalRenderer::new(
            size,
            u8g2_font_unifont_t_gb2312,
            Rgb565::WHITE,
            Rgb565::BLACK,
        );
        let (_cw, ch) = renderer.cell_size();
        let half = ch as i32;
        let mut parser = parser(&renderer);
        parser.process(b"\x1b[?25lAB\r\nCD");
        let mut canvas = Canvas::new(size);

        let r1 = renderer
            .render_row_diff(parser.screen(), &mut canvas, 1, 2)
            .unwrap();
        assert!(r1.is_some(), "first call should be dirty");

        let ink = |y0: i32, y1: i32| {
            (y0..y1)
                .flat_map(|y| (0..32).map(move |x| (x, y)))
                .filter(|&(x, y)| canvas.at(x, y).map(|c| c != Rgb565::BLACK).unwrap_or(false))
                .count()
        };
        // row1 (CD) 贴到画布上半部 y∈[0, half);row0 没画 → 下半部 y∈[half, 2*half) 无 ink。
        assert!(ink(0, half) > 0, "row1 should render to canvas top");
        assert_eq!(
            ink(half, half * 2),
            0,
            "row0 out of range → bottom half empty"
        );

        // row1 无变化 → None
        let r2 = renderer
            .render_row_diff(parser.screen(), &mut canvas, 1, 2)
            .unwrap();
        assert!(r2.is_none(), "no change in range → None");
    }
}
