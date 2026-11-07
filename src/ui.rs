//! Terminal construction and framebuffer composition for the application UI.

use alloc::format;
use core::num::NonZeroU16;
use embedded_graphics::geometry::Size;
use embedded_graphics::pixelcolor::{raw::RawU16, Rgb565, RgbColor};
use embedded_graphics::prelude::RawData;
use log::warn;
use pocketgen_terminal::{vt100, Rgb565Surface, TerminalRenderer};
use u8g2_fonts::fonts::u8g2_font_unifont_t_gb2312;

extern "C" {
    pub(crate) fn pocketgen_display_present(pixels: *const u16, width: u16, height: u16) -> i32;
}

/// Creates a renderer and cursor-hidden parser sized to the pixel region.
///
/// Returns `None` when the region cannot hold at least one row and column.
pub(crate) fn terminal(size: Size) -> Option<(TerminalRenderer, vt100::Parser)> {
    let renderer = TerminalRenderer::new(
        size,
        u8g2_font_unifont_t_gb2312,
        Rgb565::WHITE,
        Rgb565::BLACK,
    );
    let mut parser = vt100::Parser::new(
        NonZeroU16::new(renderer.rows())?,
        NonZeroU16::new(renderer.cols())?,
        0,
    );
    parser.process(b"\x1b[?25l");
    Some((renderer, parser))
}

/// Writes and pads an ASCII label with the blue bar style to `cols` bytes.
pub(crate) fn blue_bar(parser: &mut vt100::Parser, cols: u16, text: &str) {
    let len = text.len().min(cols as usize);
    parser.process(b"\x1b[97;44m");
    parser.process(&text.as_bytes()[..len]);
    parser.process(format!("{:width$}", "", width = cols as usize - len).as_bytes());
    parser.process(b"\x1b[0m");
}

/// A terminal-backed header or footer at a fixed framebuffer origin.
pub(crate) struct Bar {
    /// Renderer configured for the bar's pixel dimensions.
    pub(crate) renderer: TerminalRenderer,
    /// Screen contents and style state for the bar.
    pub(crate) parser: vt100::Parser,
    size: Size,
    x: u16,
    /// Vertical origin in the framebuffer, in pixels.
    pub(crate) y: u16,
}

impl Bar {
    /// Creates a bar at `(x, y)`, or returns `None` if it is too small.
    pub(crate) fn new(size: Size, x: u16, y: u16) -> Option<Self> {
        let (renderer, parser) = terminal(size)?;
        Some(Self {
            renderer,
            parser,
            size,
            x,
            y,
        })
    }

    pub(crate) fn height(&self) -> u16 {
        self.size.height as u16
    }

    /// Renders the bar into the corresponding region of the framebuffer.
    ///
    /// Panics if the region does not fit or rendering fails.
    pub(crate) fn render(&mut self, pixels: &mut [u16], width: u16) {
        self.renderer
            .render(
                self.parser.screen(),
                &mut Rgb565Surface::new(pixels, width, self.size, self.x, self.y).unwrap(),
            )
            .unwrap();
    }
}

/// Shows the interrupt shortcut while MicroPython is executing.
pub(crate) fn draw_running(bar: &mut Bar) {
    bar.parser.process(b"\x1b[2J\x1b[H\x1b[?25l");
    blue_bar(
        &mut bar.parser,
        bar.renderer.cols(),
        " Running...  Ctrl+C Stop",
    );
}

/// Rebuilds the footer shortcuts and the most recent Python heap status.
pub(crate) fn draw_shortcuts(bar: &mut Bar, free_bytes: Option<usize>) {
    let status = match free_bytes {
        Some(bytes) => format!(" MP free: {} B", bytes),
        None => " MP free: --".into(),
    };
    let cols = bar.renderer.cols() as usize;
    bar.parser.process(b"\x1b[2J\x1b[H\x1b[?25l");
    if status.len() > cols {
        blue_bar(&mut bar.parser, bar.renderer.cols(), " MP free unavailable");
        warn!("Display is too narrow for the MicroPython heap status");
        return;
    }
    let items = [
        (" F10 ", " Menu  "),
        (" ^E ", " Menu  "),
        (" ^R ", " Run  "),
        (" ^Z ", " Undo  "),
        (" ^X ", " Cut  "),
        (" ^C ", " Copy  "),
        (" ^V ", " Paste "),
    ];
    let mut remaining = cols - status.len();
    for (key, action) in items {
        for (style, text) in [
            (b"\x1b[30;47m".as_slice(), key),
            (b"\x1b[97;44m".as_slice(), action),
        ] {
            let len = text.len().min(remaining);
            bar.parser.process(style);
            bar.parser.process(&text.as_bytes()[..len]);
            remaining -= len;
        }
    }
    bar.parser.process(b"\x1b[97;44m");
    bar.parser
        .process(format!("{:width$}", "", width = remaining).as_bytes());
    bar.parser.process(status.as_bytes());
    bar.parser.process(b"\x1b[0m");
}

/// Composes both panes, the bars, and the divider, then presents the pixels.
///
/// Returns the C display backend's status. The dimensions and framebuffer
/// must accommodate the initialized pane and bar regions; invalid regions or
/// rendering failures panic.
pub(crate) fn render_display(
    left: &vt100::Parser,
    right: &vt100::Parser,
    left_renderer: &mut TerminalRenderer,
    right_renderer: &mut TerminalRenderer,
    right_header: &mut Bar,
    footer: &mut Bar,
    pixels: &mut [u16],
    width: u16,
    height: u16,
) -> i32 {
    let right_x = width / 2;
    let left_width = right_x - 1;
    let bar_height = footer.size.height as u16;
    left_renderer
        .render(
            left.screen(),
            &mut Rgb565Surface::new(
                pixels,
                width,
                Size::new(left_width.into(), (height - bar_height).into()),
                0,
                0,
            )
            .unwrap(),
        )
        .unwrap();
    right_renderer
        .render(
            right.screen(),
            &mut Rgb565Surface::new(
                pixels,
                width,
                Size::new((width - right_x).into(), (height - 2 * bar_height).into()),
                right_x,
                bar_height,
            )
            .unwrap(),
        )
        .unwrap();
    right_header.render(pixels, width);
    footer.render(pixels, width);
    let divider_color = RawU16::from(Rgb565::new(12, 24, 12)).into_inner();
    for y in bar_height..height - bar_height {
        pixels[y as usize * width as usize + left_width as usize] = divider_color;
    }
    unsafe { pocketgen_display_present(pixels.as_ptr(), width, height) }
}
