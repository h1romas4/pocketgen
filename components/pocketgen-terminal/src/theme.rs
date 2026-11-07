//! 内置主题。
//!
//! 格式参考 wezterm / iTerm 的 color scheme:`foreground` / `background` + `ansi[8]`(0–7)
//! + `brights[8]`(8–15)。颜色用 `Rgb888`(`#hex` 友好),由 [`crate::TerminalRenderer`]
//! 内部转成 `Rgb565`。
//!
//! xterm 的 6×6×6 立方体(16–231)与灰阶(232–255)按标准固定,不进主题——这和
//! wezterm/tmux 的主题范围一致(只换 0–15 + 默认前后景)。

use embedded_graphics::pixelcolor::Rgb888;

/// 终端主题:默认前景/背景 + ANSI 16 色(normal 0–7 在 `ansi`,bright 8–15 在 `brights`)。
#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub foreground: Rgb888,
    pub background: Rgb888,
    pub ansi: [Rgb888; 8],
    pub brights: [Rgb888; 8],
}

impl Theme {
    /// 默认主题:VGA ANSI 16 + 黑底白字(沿用旧版硬编码的 `ANSI_16`)。
    pub const DEFAULT: Theme = Theme {
        foreground: Rgb888::new(255, 255, 255),
        background: Rgb888::new(0, 0, 0),
        ansi: [
            Rgb888::new(0, 0, 0),
            Rgb888::new(128, 0, 0),
            Rgb888::new(0, 128, 0),
            Rgb888::new(128, 128, 0),
            Rgb888::new(0, 0, 128),
            Rgb888::new(128, 0, 128),
            Rgb888::new(0, 128, 128),
            Rgb888::new(192, 192, 192),
        ],
        brights: [
            Rgb888::new(128, 128, 128),
            Rgb888::new(255, 0, 0),
            Rgb888::new(0, 255, 0),
            Rgb888::new(255, 255, 0),
            Rgb888::new(0, 0, 255),
            Rgb888::new(255, 0, 255),
            Rgb888::new(0, 255, 255),
            Rgb888::new(255, 255, 255),
        ],
    };

    /// Sequoia Moonlight(取自 Sequoia-Theme 的 iTerm 配色)。
    pub const SEQUOIA_MOONLIGHT: Theme = Theme {
        foreground: Rgb888::new(134, 134, 144),
        background: Rgb888::new(15, 16, 20),
        ansi: [
            Rgb888::new(19, 19, 23),
            Rgb888::new(245, 142, 224),
            Rgb888::new(142, 181, 245),
            Rgb888::new(152, 152, 166),
            Rgb888::new(197, 143, 255),
            Rgb888::new(253, 253, 254),
            Rgb888::new(255, 187, 136),
            Rgb888::new(134, 134, 144),
        ],
        brights: [
            Rgb888::new(87, 88, 97),
            Rgb888::new(245, 142, 224),
            Rgb888::new(142, 181, 245),
            Rgb888::new(152, 152, 166),
            Rgb888::new(197, 143, 255),
            Rgb888::new(253, 253, 254),
            Rgb888::new(255, 187, 136),
            Rgb888::new(134, 134, 144),
        ],
    };

    /// Solarized Dark。
    pub const SOLARIZED_DARK: Theme = Theme {
        foreground: Rgb888::new(131, 148, 150),
        background: Rgb888::new(0, 43, 54),
        ansi: [
            Rgb888::new(7, 54, 66),
            Rgb888::new(220, 50, 47),
            Rgb888::new(133, 153, 0),
            Rgb888::new(181, 137, 0),
            Rgb888::new(38, 139, 210),
            Rgb888::new(211, 54, 130),
            Rgb888::new(42, 161, 152),
            Rgb888::new(238, 232, 213),
        ],
        brights: [
            Rgb888::new(0, 43, 54),
            Rgb888::new(203, 75, 22),
            Rgb888::new(88, 110, 117),
            Rgb888::new(101, 123, 131),
            Rgb888::new(131, 148, 150),
            Rgb888::new(108, 113, 196),
            Rgb888::new(147, 161, 161),
            Rgb888::new(253, 246, 227),
        ],
    };

    /// Solarized Light。
    pub const SOLARIZED_LIGHT: Theme = Theme {
        foreground: Rgb888::new(101, 123, 131),
        background: Rgb888::new(253, 246, 227),
        ansi: [
            Rgb888::new(7, 54, 66),
            Rgb888::new(220, 50, 47),
            Rgb888::new(133, 153, 0),
            Rgb888::new(181, 137, 0),
            Rgb888::new(38, 139, 210),
            Rgb888::new(211, 54, 130),
            Rgb888::new(42, 161, 152),
            Rgb888::new(238, 232, 213),
        ],
        brights: [
            Rgb888::new(0, 43, 54),
            Rgb888::new(203, 75, 22),
            Rgb888::new(88, 110, 117),
            Rgb888::new(101, 123, 131),
            Rgb888::new(131, 148, 150),
            Rgb888::new(108, 113, 196),
            Rgb888::new(147, 161, 161),
            Rgb888::new(253, 246, 227),
        ],
    };

    /// Dracula。
    pub const DRACULA: Theme = Theme {
        foreground: Rgb888::new(248, 248, 242),
        background: Rgb888::new(40, 42, 54),
        ansi: [
            Rgb888::new(0, 0, 0),
            Rgb888::new(255, 85, 85),
            Rgb888::new(80, 250, 123),
            Rgb888::new(241, 250, 140),
            Rgb888::new(189, 147, 249),
            Rgb888::new(255, 121, 198),
            Rgb888::new(139, 233, 253),
            Rgb888::new(191, 191, 191),
        ],
        brights: [
            Rgb888::new(77, 77, 77),
            Rgb888::new(255, 110, 103),
            Rgb888::new(90, 247, 142),
            Rgb888::new(244, 249, 157),
            Rgb888::new(202, 169, 250),
            Rgb888::new(255, 146, 208),
            Rgb888::new(154, 237, 254),
            Rgb888::new(230, 230, 230),
        ],
    };

    /// GitHub Dark。
    pub const GITHUB_DARK: Theme = Theme {
        foreground: Rgb888::new(230, 237, 243),
        background: Rgb888::new(13, 17, 23),
        ansi: [
            Rgb888::new(72, 79, 88),
            Rgb888::new(255, 123, 114),
            Rgb888::new(63, 185, 80),
            Rgb888::new(210, 153, 34),
            Rgb888::new(88, 166, 255),
            Rgb888::new(188, 140, 255),
            Rgb888::new(57, 197, 207),
            Rgb888::new(177, 186, 196),
        ],
        brights: [
            Rgb888::new(110, 118, 129),
            Rgb888::new(255, 161, 152),
            Rgb888::new(86, 211, 100),
            Rgb888::new(227, 179, 65),
            Rgb888::new(121, 192, 255),
            Rgb888::new(210, 168, 255),
            Rgb888::new(86, 212, 221),
            Rgb888::new(240, 246, 252),
        ],
    };

    /// Monokai。
    pub const MONOKAI: Theme = Theme {
        foreground: Rgb888::new(248, 248, 242),
        background: Rgb888::new(39, 40, 34),
        ansi: [
            Rgb888::new(39, 40, 34),
            Rgb888::new(249, 38, 114),
            Rgb888::new(166, 226, 46),
            Rgb888::new(244, 191, 117),
            Rgb888::new(102, 217, 239),
            Rgb888::new(174, 129, 255),
            Rgb888::new(161, 239, 228),
            Rgb888::new(248, 248, 242),
        ],
        brights: [
            Rgb888::new(117, 113, 94),
            Rgb888::new(249, 38, 114),
            Rgb888::new(166, 226, 46),
            Rgb888::new(244, 191, 117),
            Rgb888::new(102, 217, 239),
            Rgb888::new(174, 129, 255),
            Rgb888::new(161, 239, 228),
            Rgb888::new(249, 248, 245),
        ],
    };

    /// Aura(Gogh,取自 iTerm2-Color-Schemes)。
    pub const AURA: Theme = Theme {
        foreground: Rgb888::new(237, 236, 238),
        background: Rgb888::new(21, 20, 27),
        ansi: [
            Rgb888::new(17, 15, 24),
            Rgb888::new(255, 103, 103),
            Rgb888::new(97, 255, 202),
            Rgb888::new(255, 202, 133),
            Rgb888::new(162, 119, 255),
            Rgb888::new(162, 119, 255),
            Rgb888::new(97, 255, 202),
            Rgb888::new(237, 236, 238),
        ],
        brights: [
            Rgb888::new(77, 77, 77),
            Rgb888::new(255, 202, 133),
            Rgb888::new(162, 119, 255),
            Rgb888::new(255, 202, 133),
            Rgb888::new(162, 119, 255),
            Rgb888::new(162, 119, 255),
            Rgb888::new(97, 255, 202),
            Rgb888::new(237, 236, 238),
        ],
    };
}
