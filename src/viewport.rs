//! Rendering of the editor document, selection, cursor, and menu into VT100.

use alloc::{
    format,
    string::{String, ToString},
};
use pocketgen_editor::syntax::{Highlighter, Kind};
use pocketgen_editor::{width as text_width, Editor, Menu, Search, SearchStatus};
use pocketgen_terminal::vt100;

use crate::ui::blue_bar;

extern "C" {
    pub(crate) fn pocketgen_syntax_highlight_enabled() -> i32;
}

fn foreground(kind: Kind) -> &'static [u8] {
    match kind {
        Kind::Plain => b"\x1b[39m",
        Kind::Keyword => b"\x1b[96m",
        Kind::String => b"\x1b[93m",
        Kind::Comment => b"\x1b[92m",
        Kind::Number => b"\x1b[95m",
    }
}

/// Scroll offsets for the editor pane, adjusted to keep the cursor visible.
pub(crate) struct Viewport {
    /// Index of the first visible document line.
    pub(crate) top: usize,
    /// Horizontal offset in terminal display columns.
    pub(crate) left: usize,
}

impl Viewport {
    /// Displays the active search prompt in the editor header.
    pub(crate) fn render_search(&self, search: &Search, parser: &mut vt100::Parser, cols: u16) {
        if !search.active() {
            return;
        }
        let feedback = match search.status() {
            SearchStatus::Ready => "",
            SearchStatus::NotFound => " [Not found]",
            SearchStatus::QueryTooLong => " [Query too long]",
        };
        let prefix = " Find: ";
        let available = usize::from(cols).saturating_sub(prefix.len() + feedback.len());
        let query: String = search
            .query()
            .chars()
            .map(|ch| if ch.is_control() { ' ' } else { ch })
            .collect();
        let mut start = query.len();
        let mut used = 0;
        for (index, ch) in query.char_indices().rev() {
            let size = text_width(ch.encode_utf8(&mut [0; 4]));
            if used + size > available {
                break;
            }
            used += size;
            start = index;
        }
        parser.process(b"\x1b[1;1H\x1b[97;44m");
        let mut remaining = usize::from(cols);
        for ch in format!("{}{}{}", prefix, &query[start..], feedback).chars() {
            let mut buf = [0; 4];
            let encoded = ch.encode_utf8(&mut buf);
            let size = text_width(encoded);
            if size > remaining {
                break;
            }
            parser.process(encoded.as_bytes());
            remaining -= size;
        }
        parser.process(format!("{:width$}", "", width = remaining).as_bytes());
        parser.process(b"\x1b[0m\x1b[?25l");
    }

    pub(crate) fn scroll_page(&mut self, text: &str, old: usize, new: usize, rows: u16) {
        let old_line = text[..old].bytes().filter(|byte| *byte == b'\n').count();
        let new_line = text[..new].bytes().filter(|byte| *byte == b'\n').count();
        let total_lines = text.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let height = usize::from(rows).saturating_sub(1).max(1);
        self.top = self
            .top
            .saturating_add(new_line)
            .saturating_sub(old_line)
            .min(total_lines.saturating_sub(height));
    }

    /// Rebuilds the terminal screen from the editor and menu state.
    ///
    /// `rows` and `cols` describe the target terminal, including the header;
    /// callers must provide at least two rows and two columns. Line numbers
    /// occupy a separate gutter, and selected text uses inverse video.
    pub(crate) fn render(
        &mut self,
        editor: &Editor,
        menu: &Menu,
        parser: &mut vt100::Parser,
        rows: u16,
        cols: u16,
    ) {
        self.render_with_highlight(editor, menu, parser, rows, cols, unsafe {
            pocketgen_syntax_highlight_enabled() != 0
        });
    }

    pub(crate) fn render_with_highlight(
        &mut self,
        editor: &Editor,
        menu: &Menu,
        parser: &mut vt100::Parser,
        rows: u16,
        cols: u16,
        highlight: bool,
    ) {
        let text = editor.text();
        let selection = editor.selection();
        let cursor = editor.cursor();
        let line = text[..cursor].bytes().filter(|b| *b == b'\n').count();
        let start = text[..cursor].rfind('\n').map_or(0, |i| i + 1);
        let column = text_width(&text[start..cursor]);
        let height = usize::from(rows).saturating_sub(1).max(1);
        let digits = (text.bytes().filter(|b| *b == b'\n').count() + 1)
            .to_string()
            .len()
            .max(2);
        let gutter = (digits + 1).min(usize::from(cols) - 1);
        let width = usize::from(cols) - gutter;
        if line < self.top {
            self.top = line;
        }
        if line >= self.top + height {
            self.top = line + 1 - height;
        }
        if column < self.left {
            self.left = column;
        }
        if column >= self.left + width {
            self.left = column + 1 - width;
        }

        parser.process(b"\x1b[2J\x1b[H\x1b[?25l");
        blue_bar(parser, cols, " Demo  Edit  Run");
        let mut line_start = 0;
        let mut highlighter = Highlighter::default();
        for line_text in text.split('\n').take(self.top) {
            if highlight {
                for _ in highlighter.line(line_text) {}
            }
            line_start += line_text.len() + 1;
        }
        for (row, line_text) in text.split('\n').skip(self.top).take(height).enumerate() {
            parser.process(format!("\x1b[{};1H", row + 2).as_bytes());
            let number = format!("{}", self.top + row + 1);
            let number_width = if gutter == 1 { 1 } else { gutter - 1 };
            let visible = &number[number.len().saturating_sub(number_width)..];
            parser.process(b"\x1b[90m");
            parser.process(format!("{:>width$}", visible, width = number_width).as_bytes());
            if gutter > 1 {
                parser.process(b" ");
            }
            parser.process(b"\x1b[0m");
            let mut cell = 0;
            let mut selected = false;
            let mut foreground_kind = Kind::Plain;
            let mut spans = highlight.then(|| highlighter.line(line_text));
            let mut span = spans.as_mut().and_then(Iterator::next);
            for (offset, ch) in line_text.char_indices() {
                let mut buf = [0; 4];
                let encoded = ch.encode_utf8(&mut buf);
                let size = text_width(encoded);
                let in_selection = selection.as_ref().is_some_and(|range| {
                    line_start + offset >= range.start && line_start + offset < range.end
                });
                if in_selection != selected {
                    parser.process(if in_selection {
                        b"\x1b[7m"
                    } else {
                        b"\x1b[27m"
                    });
                    selected = in_selection;
                }
                let visible = cell >= self.left && cell + size <= self.left + width;
                let clipped_left = cell < self.left && cell + size > self.left;
                if visible || clipped_left {
                    while span.is_some_and(|s| offset >= s.end) {
                        span = spans.as_mut().and_then(Iterator::next);
                    }
                    let kind = span.map_or(Kind::Plain, |s| s.kind);
                    if kind != foreground_kind {
                        parser.process(foreground(kind));
                        foreground_kind = kind;
                    }
                    parser.process(if visible { encoded.as_bytes() } else { b" " });
                }
                cell += size;
                if cell >= self.left + width {
                    break;
                }
            }
            if let Some(spans) = spans {
                for _ in spans {}
            }
            if line_start + line_text.len() < text.len()
                && selection
                    .as_ref()
                    .is_some_and(|range| range.contains(&(line_start + line_text.len())))
                && cell >= self.left
                && cell < self.left + width
            {
                parser.process(b"\x1b[39;7m ");
            }
            parser.process(b"\x1b[0m");
            line_start += line_text.len() + 1;
        }
        if let Some((group, item)) = menu.state() {
            let (col, labels): (usize, &[&str]) = match group {
                0 => (
                    1,
                    &[
                        " Maze              ",
                        " Starfield         ",
                        " 3D Cube           ",
                        " Mexican Hat       ",
                        " Sine Graph        ",
                        " Hat (Graphics)    ",
                        " Plot Trig         ",
                        " Plot Projectile   ",
                        " Plot Growth       ",
                        " Chart Scatter     ",
                        " Chart Histogram   ",
                        " Chart Bar         ",
                        " Chart Pie         ",
                        " Numeric RC        ",
                        " Snake             ",
                    ],
                ),
                1 => (
                    7,
                    &[
                        " Undo       Ctrl+Z ",
                        " Cut        Ctrl+X ",
                        " Copy       Ctrl+C ",
                        " Paste      Ctrl+V ",
                        " Select all        ",
                    ],
                ),
                _ => (13, &[" Run        Ctrl+R "]),
            };
            let first = item.saturating_sub(height.saturating_sub(1));
            for (row, (i, label)) in labels.iter().enumerate().skip(first).take(height).enumerate() {
                parser.process(format!("\x1b[{};{}H", row + 2, col).as_bytes());
                parser.process(if i == item {
                    b"\x1b[37;44m"
                } else {
                    b"\x1b[30;47m"
                });
                parser.process(label.as_bytes());
                parser.process(b"\x1b[0m");
            }
            return;
        }
        parser.process(
            format!(
                "\x1b[{};{}H\x1b[?25h",
                line - self.top + 2,
                gutter + column - self.left + 1
            )
            .as_bytes(),
        );
    }
}
