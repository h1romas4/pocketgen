#![no_std]

//! In-memory text editing and terminal input handling for PocketGen.
//!
//! [`Decoder`] turns a byte stream into [`Key`] events, [`Menu`] intercepts
//! menu commands, and [`Editor`] applies editing events to a UTF-8 document.
//! The crate uses `alloc` and does not depend on a display or operating system.
//!
//! ```
//! use pocketgen_editor::{Editor, Key};
//!
//! let mut editor = Editor::new("hello");
//! editor.apply(Key::End);
//! editor.apply(Key::Character('!'));
//! assert_eq!(editor.text(), "hello!");
//! editor.apply(Key::Undo);
//! assert_eq!(editor.text(), "hello");
//! ```

extern crate alloc;

/// Traits for reading and modifying text containers.
pub mod document;
mod gap_buffer;
/// Word-boundary navigation over readable documents.
pub mod navigation;
mod search;
/// Lightweight Python lexical highlighting.
pub mod syntax;

/// Heap-backed gap buffer used by the editor.
pub use gap_buffer::GapBuffer;
/// Search prompt state and forward document search.
pub use search::{Search, SearchStatus};

use alloc::string::String;
use alloc::vec::Vec;
use core::ops::Range;
use unicode_width::UnicodeWidthChar;

/// A decoded editing, navigation, menu, or execution command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// Insert a character; a tab is inserted as four spaces.
    Character(char),
    /// Insert a newline.
    Enter,
    /// Delete the selection or the preceding character.
    Backspace,
    /// Delete the selection or the following character.
    Delete,
    /// Move left by one Unicode scalar value.
    Left,
    /// Move right by one Unicode scalar value.
    Right,
    /// Move to the preceding line, retaining the preferred display column.
    Up,
    /// Move to the next line, retaining the preferred display column.
    Down,
    /// Move up by the editor's visible page height.
    PageUp,
    /// Move down by the editor's visible page height.
    PageDown,
    /// Move to the start of the current line.
    Home,
    /// Move to the end of the current line.
    End,
    /// Move to the start of the document.
    CtrlHome,
    /// Move to the end of the document.
    CtrlEnd,
    /// Extend the selection left.
    ShiftLeft,
    /// Extend the selection right.
    ShiftRight,
    /// Extend the selection up.
    ShiftUp,
    /// Extend the selection down.
    ShiftDown,
    /// Extend the selection up by one page.
    ShiftPageUp,
    /// Extend the selection down by one page.
    ShiftPageDown,
    /// Extend the selection to the start of the line.
    ShiftHome,
    /// Extend the selection to the end of the line.
    ShiftEnd,
    /// Extend the selection to the start of the document.
    CtrlShiftHome,
    /// Extend the selection to the end of the document.
    CtrlShiftEnd,
    /// Copy the selection to the editor's clipboard.
    Copy,
    /// Copy and remove the selection.
    Cut,
    /// Replace the selection with the editor's clipboard contents.
    Paste,
    /// Undo the most recent text change.
    Undo,
    /// Select the entire document.
    SelectAll,
    /// Open or close the menu.
    Menu,
    /// Open the search prompt.
    Find,
    /// Search for the previous match.
    FindPrevious,
    /// Search for the next match.
    FindNext,
    /// Close the menu or clear the editor's selection.
    Escape,
    /// Request execution by the application.
    Run,
    /// Toggle the application's output between text and graphics.
    ToggleOutput,
    /// Request loading the maze example.
    DemoMaze,
    /// Request loading the starfield example.
    DemoStarfield,
    /// Request loading the 3D cube example.
    DemoCube,
    /// Request loading the Mexican hat example.
    DemoMexicanHat,
    /// Request loading the sine graphics example.
    DemoSineGraph,
    /// Request loading the graphics Mexican hat example.
    DemoMexicanHatGraphics,
}

/// An invalid or incomplete terminal input sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputError {
    /// The input is not a valid, complete printable UTF-8 character.
    InvalidUtf8,
    /// The escape sequence is unsupported or incomplete.
    UnknownEscape,
}

/// Incrementally decodes UART or terminal bytes into [`Key`] events.
///
/// Escape sequences and multi-byte UTF-8 characters may span calls to
/// [`feed`](Self::feed). Call [`timeout`](Self::timeout) after an input pause
/// to resolve a standalone Escape or discard an incomplete sequence. Before
/// feeding each byte, call [`escape_before`](Self::escape_before) to resolve a
/// standalone Escape when the next byte cannot start a supported sequence.
#[derive(Default)]
pub struct Decoder {
    escape: [u8; 8],
    escape_len: usize,
    utf8: [u8; 4],
    utf8_len: usize,
    utf8_expected: usize,
    previous_cr: bool,
}

impl Decoder {
    /// Returns whether an escape sequence or UTF-8 character is incomplete.
    pub fn pending(&self) -> bool {
        self.escape_len != 0 || self.utf8_len != 0
    }

    /// Returns whether a standalone Escape is awaiting disambiguation.
    pub fn pending_escape(&self) -> bool {
        self.escape_len == 1
    }

    /// Resolves a pending Escape before a byte that cannot continue a sequence.
    ///
    /// The caller must still pass `byte` to [`feed`](Self::feed).
    pub fn escape_before(&mut self, byte: u8) -> Option<Key> {
        if self.pending_escape() && byte != b'[' && byte != b'O' {
            self.escape_len = 0;
            Some(Key::Escape)
        } else {
            None
        }
    }

    /// Discards any incomplete input and resets CRLF handling.
    pub fn reset(&mut self) {
        self.escape_len = 0;
        self.utf8_len = 0;
        self.utf8_expected = 0;
        self.previous_cr = false;
    }

    /// Resolves pending input after a pause and resets the decoder.
    ///
    /// A lone escape byte becomes [`Key::Escape`]; incomplete UTF-8 or escape
    /// sequences return an error. Returns `Ok(None)` when nothing is pending.
    pub fn timeout(&mut self) -> Result<Option<Key>, InputError> {
        let result = if self.escape_len == 1 && self.utf8_len == 0 {
            Ok(Some(Key::Escape))
        } else if self.utf8_len != 0 {
            Err(InputError::InvalidUtf8)
        } else if self.escape_len != 0 {
            Err(InputError::UnknownEscape)
        } else {
            Ok(None)
        };
        self.reset();
        result
    }

    /// Consumes one byte, returning a key once a complete sequence is available.
    ///
    /// Returns `Ok(None)` for incomplete sequences and ignored control bytes.
    /// A CRLF pair yields a single [`Key::Enter`]. Invalid input returns an
    /// [`InputError`]; buffered sequences are discarded on error.
    pub fn feed(&mut self, byte: u8) -> Result<Option<Key>, InputError> {
        if self.previous_cr && byte == b'\n' {
            self.previous_cr = false;
            return Ok(None);
        }
        self.previous_cr = false;
        if self.escape_len != 0 {
            if self.escape_len == self.escape.len() {
                self.reset();
                return Err(InputError::UnknownEscape);
            }
            self.escape[self.escape_len] = byte;
            self.escape_len += 1;
            let sequence = &self.escape[..self.escape_len];
            let key = match sequence {
                b"\x1b[A" | b"\x1bOA" => Some(Key::Up),
                b"\x1b[B" | b"\x1bOB" => Some(Key::Down),
                b"\x1b[C" | b"\x1bOC" => Some(Key::Right),
                b"\x1b[D" | b"\x1bOD" => Some(Key::Left),
                b"\x1b[H" | b"\x1bOH" | b"\x1b[1~" | b"\x1b[7~" => Some(Key::Home),
                b"\x1b[F" | b"\x1bOF" | b"\x1b[4~" | b"\x1b[8~" => Some(Key::End),
                b"\x1b[3~" => Some(Key::Delete),
                b"\x1b[5~" => Some(Key::PageUp),
                b"\x1b[6~" => Some(Key::PageDown),
                b"\x1b[5;2~" => Some(Key::ShiftPageUp),
                b"\x1b[6;2~" => Some(Key::ShiftPageDown),
                b"\x1b[1;2A" => Some(Key::ShiftUp),
                b"\x1b[1;2B" => Some(Key::ShiftDown),
                b"\x1b[1;2C" => Some(Key::ShiftRight),
                b"\x1b[1;2D" => Some(Key::ShiftLeft),
                b"\x1b[1;2H" | b"\x1b[7;2~" => Some(Key::ShiftHome),
                b"\x1b[1;2F" | b"\x1b[8;2~" => Some(Key::ShiftEnd),
                b"\x1b[1;5H" | b"\x1b[1;5~" | b"\x1b[7;5~" => Some(Key::CtrlHome),
                b"\x1b[1;5F" | b"\x1b[4;5~" | b"\x1b[8;5~" => Some(Key::CtrlEnd),
                b"\x1b[1;6H" | b"\x1b[1;6~" | b"\x1b[7;6~" => Some(Key::CtrlShiftHome),
                b"\x1b[1;6F" | b"\x1b[4;6~" | b"\x1b[8;6~" => Some(Key::CtrlShiftEnd),
                b"\x1b[21~" => Some(Key::Menu),
                b"\x1b[" | b"\x1bO" | b"\x1b[1" | b"\x1b[3" | b"\x1b[4" | b"\x1b[5" | b"\x1b[6"
                | b"\x1b[7" | b"\x1b[8" => return Ok(None),
                b"\x1b[2" | b"\x1b[21" | b"\x1b[1;" | b"\x1b[1;2" | b"\x1b[1;5" | b"\x1b[1;6"
                | b"\x1b[4;" | b"\x1b[4;5" | b"\x1b[4;6" | b"\x1b[5;" | b"\x1b[5;2"
                | b"\x1b[6;" | b"\x1b[6;2" | b"\x1b[7;" | b"\x1b[7;2" | b"\x1b[7;5"
                | b"\x1b[7;6" | b"\x1b[8;" | b"\x1b[8;2" | b"\x1b[8;5" | b"\x1b[8;6" => {
                    return Ok(None)
                }
                _ => {
                    self.reset();
                    return Err(InputError::UnknownEscape);
                }
            };
            self.escape_len = 0;
            return Ok(key);
        }
        if self.utf8_len != 0 {
            if !(0x80..=0xbf).contains(&byte) {
                self.reset();
                return Err(InputError::InvalidUtf8);
            }
            self.utf8[self.utf8_len] = byte;
            self.utf8_len += 1;
            if self.utf8_len != self.utf8_expected {
                return Ok(None);
            }
            let decoded = core::str::from_utf8(&self.utf8[..self.utf8_len])
                .ok()
                .and_then(|s| s.chars().next());
            self.utf8_len = 0;
            self.utf8_expected = 0;
            return decoded
                .filter(|c| !c.is_control())
                .map(|c| Some(Key::Character(c)))
                .ok_or(InputError::InvalidUtf8);
        }

        let key = match byte {
            b'\x1b' => {
                self.escape[0] = byte;
                self.escape_len = 1;
                return Ok(None);
            }
            b'\r' => {
                self.previous_cr = true;
                Some(Key::Enter)
            }
            b'\n' => Some(Key::Enter),
            0x7f | 0x08 => Some(Key::Backspace),
            0x01 => Some(Key::SelectAll),
            0x03 => Some(Key::Copy),
            0x05 => Some(Key::Menu),
            0x06 => Some(Key::Find),
            0x07 => Some(Key::ToggleOutput),
            0x0e => Some(Key::FindNext),
            0x10 => Some(Key::FindPrevious),
            0x12 => Some(Key::Run),
            0x16 => Some(Key::Paste),
            0x18 => Some(Key::Cut),
            0x1a => Some(Key::Undo),
            b'\t' => Some(Key::Character('\t')),
            0x20..=0x7e => Some(Key::Character(byte as char)),
            0xc2..=0xf4 => {
                self.utf8_expected = if byte < 0xe0 {
                    2
                } else if byte < 0xf0 {
                    3
                } else {
                    4
                };
                self.utf8[0] = byte;
                self.utf8_len = 1;
                None
            }
            0x80..=0xff => return Err(InputError::InvalidUtf8),
            _ => None,
        };
        Ok(key)
    }
}

const UNDO_LIMIT: usize = 10;

struct UndoEntry {
    start: usize,
    inserted_len: usize,
    removed: String,
    cursor: usize,
    anchor: Option<usize>,
}

/// A UTF-8 document with cursor, selection, clipboard, and bounded undo history.
///
/// Cursor and selection positions are byte offsets on UTF-8 character
/// boundaries. The clipboard and the last ten text changes belong to this
/// editor instance and are kept in memory.
pub struct Editor {
    document: GapBuffer,
    cursor: usize,
    preferred_column: Option<usize>,
    anchor: Option<usize>,
    clipboard: String,
    undo: Vec<UndoEntry>,
}

impl Editor {
    /// Creates an editor containing `initial`, with the cursor at byte zero.
    pub fn new(initial: &str) -> Self {
        let mut document = GapBuffer::new();
        document.replace(0..0, initial.as_bytes());
        Self {
            document,
            cursor: 0,
            preferred_column: None,
            anchor: None,
            clipboard: String::new(),
            undo: Vec::new(),
        }
    }

    /// Returns an owned copy of the complete document.
    pub fn text(&self) -> String {
        let mut bytes = Vec::new();
        self.document
            .extract_raw(0..self.document.len(), &mut bytes, 0);
        String::from_utf8(bytes).expect("editor maintains valid UTF-8")
    }

    /// Returns the cursor's UTF-8 byte offset into [`text`](Self::text).
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Returns the selected UTF-8 byte range, or `None` for an empty selection.
    pub fn selection(&self) -> Option<Range<usize>> {
        self.anchor.and_then(|anchor| {
            (anchor != self.cursor).then_some(anchor.min(self.cursor)..anchor.max(self.cursor))
        })
    }

    /// Replaces the document as one undoable edit and moves to its start.
    pub fn replace_all(&mut self, replacement: &str) {
        let text = self.text();
        self.replace_range(&text, 0..text.len(), replacement);
        self.cursor = 0;
        self.preferred_column = None;
    }

    fn replace_range(&mut self, text: &str, range: Range<usize>, replacement: &str) {
        if &text[range.clone()] != replacement {
            if self.undo.len() == UNDO_LIMIT {
                self.undo.remove(0);
            }
            self.undo.push(UndoEntry {
                start: range.start,
                inserted_len: replacement.len(),
                removed: String::from(&text[range.clone()]),
                cursor: self.cursor,
                anchor: self.anchor,
            });
        }
        self.cursor = range.start + replacement.len();
        self.document.replace(range, replacement.as_bytes());
        self.anchor = None;
    }

    fn replace_selection(&mut self, text: &str, replacement: &str) {
        let range = self.selection().unwrap_or(self.cursor..self.cursor);
        self.replace_range(text, range, replacement);
    }

    /// Applies an editing or navigation key to the document.
    ///
    /// Shift-navigation extends the selection. Vertical movement uses display
    /// columns, while horizontal movement follows Unicode scalar boundaries.
    /// Page keys move one line; use [`apply_with_page_height`](Self::apply_with_page_height)
    /// to supply the visible page height.
    /// [`Key::Run`] and [`Key::Menu`] are handled by the application instead.
    pub fn apply(&mut self, key: Key) {
        self.apply_with_page_height(key, 1);
    }

    /// Applies a key, moving by `visible_lines` for page navigation.
    ///
    /// A zero height is treated as one line.
    pub fn apply_with_page_height(&mut self, key: Key, visible_lines: usize) {
        let steps = if matches!(
            key,
            Key::PageUp | Key::PageDown | Key::ShiftPageUp | Key::ShiftPageDown
        ) {
            visible_lines.max(1)
        } else {
            1
        };
        let text = self.text();
        let shift = matches!(
            key,
            Key::ShiftLeft
                | Key::ShiftRight
                | Key::ShiftUp
                | Key::ShiftDown
                | Key::ShiftPageUp
                | Key::ShiftPageDown
                | Key::ShiftHome
                | Key::ShiftEnd
                | Key::CtrlShiftHome
                | Key::CtrlShiftEnd
        );
        let key = match key {
            Key::ShiftLeft => Key::Left,
            Key::ShiftRight => Key::Right,
            Key::ShiftUp => Key::Up,
            Key::ShiftDown => Key::Down,
            Key::PageUp | Key::ShiftPageUp => Key::Up,
            Key::PageDown | Key::ShiftPageDown => Key::Down,
            Key::ShiftHome => Key::Home,
            Key::ShiftEnd => Key::End,
            Key::CtrlShiftHome => Key::CtrlHome,
            Key::CtrlShiftEnd => Key::CtrlEnd,
            other => other,
        };
        if shift {
            self.anchor.get_or_insert(self.cursor);
        } else if matches!(key, Key::Left | Key::Right) {
            if let Some(range) = self.selection() {
                self.cursor = if key == Key::Left {
                    range.start
                } else {
                    range.end
                };
                self.anchor = None;
                self.preferred_column = None;
                return;
            }
            self.anchor = None;
        } else if matches!(
            key,
            Key::Up | Key::Down | Key::Home | Key::End | Key::CtrlHome | Key::CtrlEnd | Key::Escape
        ) {
            self.anchor = None;
        }
        let previous = text[..self.cursor]
            .char_indices()
            .next_back()
            .map(|(i, _)| i);
        let next = text[self.cursor..]
            .chars()
            .next()
            .map(|c| self.cursor + c.len_utf8());
        let start = text[..self.cursor].rfind('\n').map_or(0, |i| i + 1);
        let end = text[self.cursor..]
            .find('\n')
            .map_or(text.len(), |i| self.cursor + i);
        match key {
            Key::Character(c) => {
                if c.is_control() && c != '\t' {
                    return;
                }
                let mut encoded = [0; 4];
                let inserted = if c == '\t' {
                    "    "
                } else {
                    c.encode_utf8(&mut encoded)
                };
                self.replace_selection(&text, inserted);
            }
            Key::Enter => {
                self.replace_selection(&text, "\n");
            }
            Key::Backspace => {
                if self.selection().is_some() {
                    self.replace_selection(&text, "");
                } else if let Some(pos) = previous {
                    self.replace_range(&text, pos..self.cursor, "");
                }
            }
            Key::Delete => {
                if self.selection().is_some() {
                    self.replace_selection(&text, "");
                } else if let Some(pos) = next {
                    self.replace_range(&text, self.cursor..pos, "");
                }
            }
            Key::Left => self.cursor = previous.unwrap_or(self.cursor),
            Key::Right => self.cursor = next.unwrap_or(self.cursor),
            Key::Home => self.cursor = start,
            Key::End => self.cursor = end,
            Key::CtrlHome => self.cursor = 0,
            Key::CtrlEnd => self.cursor = text.len(),
            Key::Up | Key::Down => {
                let column = self
                    .preferred_column
                    .unwrap_or_else(|| width(&text[start..self.cursor]));
                self.preferred_column = Some(column);
                for _ in 0..steps {
                    let start = text[..self.cursor].rfind('\n').map_or(0, |i| i + 1);
                    let end = text[self.cursor..]
                        .find('\n')
                        .map_or(text.len(), |i| self.cursor + i);
                    let target = if key == Key::Up && start > 0 {
                        let target_end = start - 1;
                        let target_start = text[..target_end].rfind('\n').map_or(0, |i| i + 1);
                        Some((target_start, target_end))
                    } else if key == Key::Down && end < text.len() {
                        let target_start = end + 1;
                        let target_end = text[target_start..]
                            .find('\n')
                            .map_or(text.len(), |i| target_start + i);
                        Some((target_start, target_end))
                    } else {
                        None
                    };
                    let Some((target_start, target_end)) = target else {
                        break;
                    };
                    let mut pos = target_start;
                    let mut current = 0;
                    for c in text[target_start..target_end].chars() {
                        let char_width = UnicodeWidthChar::width(c).unwrap_or(0);
                        if current + char_width > column {
                            break;
                        }
                        current += char_width;
                        pos += c.len_utf8();
                    }
                    self.cursor = pos;
                }
                return;
            }
            Key::Copy | Key::Cut => {
                if let Some(range) = self.selection() {
                    self.clipboard = String::from(&text[range]);
                    if key == Key::Cut {
                        self.replace_selection(&text, "");
                    }
                }
            }
            Key::Paste => {
                if !self.clipboard.is_empty() {
                    let clipboard = self.clipboard.clone();
                    self.replace_selection(&text, &clipboard);
                }
            }
            Key::Undo => {
                if let Some(entry) = self.undo.pop() {
                    self.document.replace(
                        entry.start..entry.start + entry.inserted_len,
                        entry.removed.as_bytes(),
                    );
                    self.cursor = entry.cursor;
                    self.anchor = entry.anchor;
                }
            }
            Key::SelectAll => {
                self.anchor = Some(0);
                self.cursor = text.len();
            }
            Key::Run
            | Key::ToggleOutput
            | Key::Menu
            | Key::Find
            | Key::FindPrevious
            | Key::FindNext
            | Key::Escape
            | Key::DemoMaze
            | Key::DemoStarfield
            | Key::DemoCube
            | Key::DemoMexicanHat
            | Key::DemoSineGraph => return,
            Key::DemoMexicanHatGraphics => return,
            Key::ShiftLeft
            | Key::ShiftRight
            | Key::ShiftUp
            | Key::ShiftDown
            | Key::PageUp
            | Key::PageDown
            | Key::ShiftPageUp
            | Key::ShiftPageDown
            | Key::ShiftHome
            | Key::ShiftEnd
            | Key::CtrlShiftHome
            | Key::CtrlShiftEnd => unreachable!(),
        }
        self.preferred_column = None;
    }
}

/// Returns the sum of Unicode character widths in terminal display columns.
///
/// Characters without a defined width contribute zero; this does not expand
/// tabs or account for grapheme-cluster rendering.
pub fn width(text: &str) -> usize {
    text.chars()
        .map(|c| UnicodeWidthChar::width(c).unwrap_or(0))
        .sum()
}

/// Tracks the open menu and maps menu navigation to [`Key`] commands.
#[derive(Default)]
pub struct Menu {
    open: Option<(usize, usize)>,
}

impl Menu {
    /// Returns the open `(group, item)` indices, or `None` when closed.
    ///
    /// Groups 0, 1, and 2 contain demos, editing commands, and run commands.
    pub fn state(&self) -> Option<(usize, usize)> {
        self.open
    }

    /// Handles menu input, forwarding unconsumed keys or the selected command.
    ///
    /// Returns `None` when the menu consumes the key, including when a menu is
    /// opened, navigated, or dismissed.
    pub fn handle(&mut self, key: Key) -> Option<Key> {
        let Some((group, item)) = self.open else {
            if key == Key::Menu {
                self.open = Some((1, 0));
                return None;
            }
            return Some(key);
        };
        match key {
            Key::Menu | Key::Escape => self.open = None,
            Key::Left => self.open = Some(((group + 2) % 3, 0)),
            Key::Right => self.open = Some(((group + 1) % 3, 0)),
            Key::Up => {
                let count = [6, 5, 1][group];
                self.open = Some((group, (item + count - 1) % count));
            }
            Key::Down => {
                let count = [6, 5, 1][group];
                self.open = Some((group, (item + 1) % count));
            }
            Key::Enter => {
                self.open = None;
                return Some(match (group, item) {
                    (0, 0) => Key::DemoMaze,
                    (0, 1) => Key::DemoStarfield,
                    (0, 2) => Key::DemoCube,
                    (0, 3) => Key::DemoMexicanHat,
                    (0, 4) => Key::DemoSineGraph,
                    (0, 5) => Key::DemoMexicanHatGraphics,
                    (1, 0) => Key::Undo,
                    (1, 1) => Key::Cut,
                    (1, 2) => Key::Copy,
                    (1, 3) => Key::Paste,
                    (1, 4) => Key::SelectAll,
                    (2, 0) => Key::Run,
                    _ => unreachable!(),
                });
            }
            _ => {}
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoder_handles_fragmented_keys_unicode_and_errors() {
        let mut d = Decoder::default();
        let bytes = b"\x1b[A\x1b[3~\xe6\x97\xa5\r\n\x12";
        let keys: Vec<_> = bytes.iter().filter_map(|b| d.feed(*b).unwrap()).collect();
        assert_eq!(
            keys,
            [
                Key::Up,
                Key::Delete,
                Key::Character('日'),
                Key::Enter,
                Key::Run
            ]
        );
        assert_eq!(d.feed(0xff), Err(InputError::InvalidUtf8));
        assert_eq!(d.feed(b'x'), Ok(Some(Key::Character('x'))));
        assert_eq!(d.feed(0x1b), Ok(None));
        assert_eq!(d.feed(b'x'), Err(InputError::UnknownEscape));
        assert_eq!(d.feed(0xe0), Ok(None));
        assert_eq!(d.feed(0x80), Ok(None));
        assert_eq!(d.feed(0x80), Err(InputError::InvalidUtf8));
        assert_eq!(d.feed(0x1b), Ok(None));
        assert!(d.pending());
        d.reset();
        assert!(!d.pending());
        let keys: Vec<_> = b"\r\xe6\x97\xa5\n"
            .iter()
            .filter_map(|b| d.feed(*b).unwrap())
            .collect();
        assert_eq!(keys, [Key::Enter, Key::Character('日'), Key::Enter]);
    }

    #[test]
    fn edits_at_unicode_boundaries_and_navigates_lines() {
        let mut e = Editor::new("a日c\nxy\n123456");
        e.apply(Key::Right);
        e.apply(Key::Right);
        assert_eq!(e.cursor(), 4);
        e.apply(Key::Down);
        assert_eq!(e.cursor(), "a日c\nxy".len());
        e.apply(Key::Down);
        assert_eq!(e.cursor(), "a日c\nxy\n123".len());
        e.apply(Key::Up);
        e.apply(Key::Up);
        assert_eq!(e.cursor(), 4);
        e.apply(Key::Backspace);
        assert_eq!(e.text(), "ac\nxy\n123456");
        e.apply(Key::Delete);
        e.apply(Key::Character('界'));
        e.apply(Key::Enter);
        assert_eq!(e.text(), "a界\n\nxy\n123456");
        e.apply(Key::Character('\x1b'));
        assert_eq!(e.text(), "a界\n\nxy\n123456");
    }

    #[test]
    fn decodes_shift_commands_menu_and_clipboard_shortcuts() {
        let mut d = Decoder::default();
        let bytes =
            b"\x1b[1;2D\x1b[1;2C\x1b[1;2A\x1b[1;2B\x1b[1;2H\x1b[1;2F\x1b[21~\x03\x18\x16\x1a\x01\x05";
        let keys: Vec<_> = bytes.iter().filter_map(|b| d.feed(*b).unwrap()).collect();
        assert_eq!(
            keys,
            [
                Key::ShiftLeft,
                Key::ShiftRight,
                Key::ShiftUp,
                Key::ShiftDown,
                Key::ShiftHome,
                Key::ShiftEnd,
                Key::Menu,
                Key::Copy,
                Key::Cut,
                Key::Paste,
                Key::Undo,
                Key::SelectAll,
                Key::Menu,
            ]
        );
        assert_eq!(d.feed(0x1b), Ok(None));
        assert_eq!(d.timeout(), Ok(Some(Key::Escape)));
        assert_eq!(d.feed(0x1b), Ok(None));
        assert_eq!(d.feed(b'['), Ok(None));
        assert_eq!(d.timeout(), Err(InputError::UnknownEscape));
    }

    #[test]
    fn resolves_escape_before_following_keys_without_losing_input() {
        let cases: &[(&[u8], &[Key])] = &[
            (b"\x1bx", &[Key::Escape, Key::Character('x')]),
            (b"\x1b\xe6\x97\xa5", &[Key::Escape, Key::Character('日')]),
            (b"\x1b\x12", &[Key::Escape, Key::Run]),
            (b"\x1b\x1b[A", &[Key::Escape, Key::Up]),
            (b"\x1b[A", &[Key::Up]),
            (b"\x1bOA", &[Key::Up]),
        ];
        for &(bytes, expected) in cases {
            let mut decoder = Decoder::default();
            let mut keys = Vec::new();
            for &byte in bytes {
                if let Some(key) = decoder.escape_before(byte) {
                    keys.push(key);
                }
                if let Some(key) = decoder.feed(byte).unwrap() {
                    keys.push(key);
                }
            }
            assert_eq!(keys, expected, "sequence: {bytes:?}");
            assert!(!decoder.pending());
        }
        let mut decoder = Decoder::default();
        decoder.feed(0x1b).unwrap();
        assert!(decoder.pending_escape());
        assert_eq!(decoder.timeout(), Ok(Some(Key::Escape)));
        assert!(!decoder.pending_escape());
        assert_eq!(decoder.feed(b'['), Ok(Some(Key::Character('['))));
    }

    #[test]
    fn decodes_document_navigation_shortcuts() {
        assert_eq!(Decoder::default().feed(0x02), Ok(None));
        let cases: &[(&[u8], Key)] = &[
            (b"\x07", Key::ToggleOutput),
            (b"\x10", Key::FindPrevious),
            (b"\x0e", Key::FindNext),
            (b"\x1b[1;5H", Key::CtrlHome),
            (b"\x1b[1;5~", Key::CtrlHome),
            (b"\x1b[7;5~", Key::CtrlHome),
            (b"\x1b[1;5F", Key::CtrlEnd),
            (b"\x1b[4;5~", Key::CtrlEnd),
            (b"\x1b[8;5~", Key::CtrlEnd),
            (b"\x1b[1;6H", Key::CtrlShiftHome),
            (b"\x1b[1;6~", Key::CtrlShiftHome),
            (b"\x1b[7;6~", Key::CtrlShiftHome),
            (b"\x1b[1;6F", Key::CtrlShiftEnd),
            (b"\x1b[4;6~", Key::CtrlShiftEnd),
            (b"\x1b[8;6~", Key::CtrlShiftEnd),
            (b"\x1b[5~", Key::PageUp),
            (b"\x1b[6~", Key::PageDown),
            (b"\x1b[5;2~", Key::ShiftPageUp),
            (b"\x1b[6;2~", Key::ShiftPageDown),
            (b"\x01", Key::SelectAll),
        ];
        for &(sequence, expected) in cases {
            let mut decoder = Decoder::default();
            let keys: Vec<_> = sequence
                .iter()
                .filter_map(|&byte| decoder.feed(byte).unwrap())
                .collect();
            assert_eq!(keys, [expected], "sequence: {sequence:?}");
            assert!(!decoder.pending());
        }
    }

    #[test]
    fn page_navigation_retains_column_and_handles_selection_and_boundaries() {
        let text = "a日z\nq\n1234\n界x\nlast\n\nend";
        let mut editor = Editor::new(text);
        editor.apply(Key::Right);
        editor.apply(Key::Right);
        editor.apply_with_page_height(Key::PageDown, 2);
        assert_eq!(editor.cursor(), "a日z\nq\n123".len());
        editor.apply_with_page_height(Key::PageDown, 2);
        assert_eq!(editor.cursor(), "a日z\nq\n1234\n界x\nlas".len());
        editor.apply_with_page_height(Key::ShiftPageUp, 2);
        assert_eq!(
            editor.selection(),
            Some("a日z\nq\n123".len().."a日z\nq\n1234\n界x\nlas".len())
        );
        editor.apply_with_page_height(Key::ShiftPageUp, 20);
        assert_eq!(
            editor.selection(),
            Some("a日".len().."a日z\nq\n1234\n界x\nlas".len())
        );
        editor.apply_with_page_height(Key::PageDown, 100);
        assert_eq!(editor.selection(), None);
        assert_eq!(editor.cursor(), text.len());
        editor.apply_with_page_height(Key::PageUp, 0);
        assert_eq!(editor.cursor(), "a日z\nq\n1234\n界x\nlast\n".len());

        let mut empty = Editor::new("");
        empty.apply_with_page_height(Key::ShiftPageDown, 30);
        assert_eq!(empty.cursor(), 0);
        assert_eq!(empty.selection(), None);
    }

    #[test]
    fn document_navigation_and_select_all_preserve_utf8_boundaries() {
        let text = "a日\n界z\n";
        let mut editor = Editor::new(text);
        editor.apply(Key::Down);
        editor.apply(Key::Right);
        editor.apply(Key::CtrlShiftEnd);
        assert_eq!(editor.selection(), Some("a日\n界".len()..text.len()));
        editor.apply(Key::CtrlShiftHome);
        assert_eq!(editor.selection(), Some(0.."a日\n界".len()));
        editor.apply(Key::CtrlEnd);
        assert_eq!(editor.cursor(), text.len());
        assert_eq!(editor.selection(), None);
        editor.apply(Key::SelectAll);
        assert_eq!(editor.selection(), Some(0..text.len()));
        editor.apply(Key::CtrlHome);
        assert_eq!(editor.cursor(), 0);
        assert_eq!(editor.selection(), None);
        editor.apply(Key::CtrlHome);
        editor.apply(Key::CtrlEnd);
        assert_eq!(editor.cursor(), text.len());

        let mut empty = Editor::new("");
        for key in [
            Key::SelectAll,
            Key::CtrlHome,
            Key::CtrlEnd,
            Key::CtrlShiftHome,
        ] {
            empty.apply(key);
            assert_eq!(empty.cursor(), 0);
            assert_eq!(empty.selection(), None);
        }
    }

    #[test]
    fn selection_clipboard_and_replacement_across_utf8_lines() {
        let mut e = Editor::new("a日\n界z");
        e.apply(Key::Right);
        e.apply(Key::ShiftRight);
        e.apply(Key::ShiftRight);
        e.apply(Key::ShiftRight);
        assert_eq!(e.selection(), Some(1..8));
        e.apply(Key::Copy);
        assert_eq!(e.selection(), Some(1..8));
        e.apply(Key::Cut);
        assert_eq!(e.text(), "az");
        assert_eq!(e.cursor(), 1);
        e.apply(Key::Paste);
        assert_eq!(e.text(), "a日\n界z");
        e.apply(Key::ShiftLeft);
        e.apply(Key::ShiftLeft);
        e.apply(Key::Character('!'));
        assert_eq!(e.text(), "a日!z");
        e.apply(Key::Paste);
        assert_eq!(e.text(), "a日!日\n界z");
        e.apply(Key::SelectAll);
        e.apply(Key::Backspace);
        assert_eq!(e.text(), "");
        e.apply(Key::Paste);
        assert_eq!(e.text(), "日\n界");
    }

    #[test]
    fn reverse_selection_navigation_and_delete() {
        let mut e = Editor::new("abc\nxy\n12345");
        e.apply(Key::End);
        e.apply(Key::ShiftLeft);
        e.apply(Key::ShiftLeft);
        assert_eq!(e.selection(), Some(1..3));
        e.apply(Key::Right);
        assert_eq!(e.cursor(), 3);
        assert_eq!(e.selection(), None);
        e.apply(Key::ShiftDown);
        assert_eq!(e.selection(), Some(3..6));
        e.apply(Key::ShiftDown);
        assert_eq!(e.cursor(), 10);
        e.apply(Key::ShiftUp);
        assert_eq!(e.cursor(), 6);
        e.apply(Key::Delete);
        assert_eq!(e.text(), "abc\n12345");
        e.apply(Key::SelectAll);
        e.apply(Key::Enter);
        assert_eq!(e.text(), "\n");
    }

    #[test]
    fn undo_typing_deletion_and_unicode_restores_cursor() {
        let mut e = Editor::new("a日\nb");
        e.apply(Key::Right);
        e.apply(Key::Delete);
        assert_eq!(e.text(), "a\nb");
        e.apply(Key::Undo);
        assert_eq!((e.text(), e.cursor()), (String::from("a日\nb"), 1));
        e.apply(Key::Right);
        e.apply(Key::Backspace);
        e.apply(Key::Undo);
        assert_eq!((e.text(), e.cursor()), (String::from("a日\nb"), 4));
        e.apply(Key::Enter);
        e.apply(Key::Character('界'));
        e.apply(Key::Undo);
        assert_eq!((e.text(), e.cursor()), (String::from("a日\n\nb"), 5));
        e.apply(Key::Undo);
        assert_eq!((e.text(), e.cursor()), (String::from("a日\nb"), 4));
        e.apply(Key::Undo);
        assert_eq!((e.text(), e.cursor()), (String::from("a日\nb"), 4));
    }

    #[test]
    fn undo_selection_replacements_restores_selection_and_clipboard() {
        let mut e = Editor::new("a日\n界z");
        e.apply(Key::End);
        e.apply(Key::ShiftLeft);
        e.apply(Key::ShiftLeft);
        assert_eq!(e.selection(), Some(0..4));
        e.apply(Key::Cut);
        assert_eq!(e.text(), "\n界z");
        e.apply(Key::Undo);
        assert_eq!(e.text(), "a日\n界z");
        assert_eq!(e.cursor(), 0);
        assert_eq!(e.selection(), Some(0..4));
        e.apply(Key::Character('!'));
        e.apply(Key::Undo);
        assert_eq!(e.text(), "a日\n界z");
        assert_eq!(e.selection(), Some(0..4));
        e.apply(Key::Right);
        e.apply(Key::Paste);
        assert_eq!(e.text(), "a日a日\n界z");
        e.apply(Key::Undo);
        assert_eq!(e.text(), "a日\n界z");
        assert_eq!(e.cursor(), 4);
        assert_eq!(e.selection(), None);
        e.apply(Key::Paste);
        assert_eq!(e.text(), "a日a日\n界z");
    }

    #[test]
    fn undo_history_is_limited_and_ignores_non_edits() {
        let mut e = Editor::new("");
        for ch in "abcdefghijkl".chars() {
            e.apply(Key::Character(ch));
        }
        e.apply(Key::Copy);
        e.apply(Key::Delete);
        e.apply(Key::Character('\x1b'));
        for _ in 0..10 {
            e.apply(Key::Undo);
        }
        assert_eq!((e.text(), e.cursor()), (String::from("ab"), 2));
        e.apply(Key::Undo);
        assert_eq!(e.text(), "ab");
        e.apply(Key::Character('x'));
        e.apply(Key::Undo);
        assert_eq!((e.text(), e.cursor()), (String::from("ab"), 2));
    }

    #[test]
    fn undo_preserves_history_for_noop_selection_replacement() {
        let mut e = Editor::new("");
        e.apply(Key::Character('x'));
        e.apply(Key::ShiftLeft);
        e.apply(Key::Copy);
        e.apply(Key::Paste);
        assert_eq!(e.text(), "x");
        e.apply(Key::Undo);
        assert_eq!(e.text(), "");
        assert_eq!(e.cursor(), 0);
    }

    #[test]
    fn menu_navigation_and_cancellation() {
        let mut menu = Menu::default();
        assert_eq!(menu.handle(Key::Menu), None);
        assert_eq!(menu.state(), Some((1, 0)));
        assert_eq!(menu.handle(Key::Enter), Some(Key::Undo));
        assert_eq!(menu.handle(Key::Menu), None);
        assert_eq!(menu.handle(Key::Down), None);
        assert_eq!(menu.handle(Key::Down), None);
        assert_eq!(menu.handle(Key::Enter), Some(Key::Copy));
        assert_eq!(menu.state(), None);
        menu.handle(Key::Menu);
        menu.handle(Key::Right);
        assert_eq!(menu.handle(Key::Enter), Some(Key::Run));
        menu.handle(Key::Menu);
        menu.handle(Key::Left);
        assert_eq!(menu.state(), Some((0, 0)));
        menu.handle(Key::Down);
        assert_eq!(menu.handle(Key::Enter), Some(Key::DemoStarfield));
        menu.handle(Key::Menu);
        menu.handle(Key::Left);
        assert_eq!(menu.handle(Key::Enter), Some(Key::DemoMaze));
        menu.handle(Key::Menu);
        menu.handle(Key::Left);
        menu.handle(Key::Up);
        assert_eq!(menu.handle(Key::Enter), Some(Key::DemoMexicanHatGraphics));
        menu.handle(Key::Menu);
        menu.handle(Key::Left);
        menu.handle(Key::Up);
        menu.handle(Key::Up);
        assert_eq!(menu.handle(Key::Enter), Some(Key::DemoSineGraph));
        menu.handle(Key::Menu);
        menu.handle(Key::Left);
        menu.handle(Key::Up);
        menu.handle(Key::Up);
        menu.handle(Key::Up);
        assert_eq!(menu.handle(Key::Enter), Some(Key::DemoMexicanHat));
        menu.handle(Key::Menu);
        menu.handle(Key::Left);
        menu.handle(Key::Down);
        menu.handle(Key::Down);
        assert_eq!(menu.handle(Key::Enter), Some(Key::DemoCube));
        menu.handle(Key::Menu);
        menu.handle(Key::Left);
        menu.handle(Key::Left);
        assert_eq!(menu.handle(Key::Enter), Some(Key::Run));
        menu.handle(Key::Menu);
        assert_eq!(menu.handle(Key::Escape), None);
        assert_eq!(menu.state(), None);
        assert_eq!(menu.handle(Key::Character('x')), Some(Key::Character('x')));
    }

    #[test]
    fn replacing_document_is_undoable_without_changing_clipboard() {
        let mut editor = Editor::new("a日\noriginal");
        editor.apply(Key::ShiftRight);
        editor.apply(Key::ShiftRight);
        editor.apply(Key::Copy);
        editor.replace_all("new\n文");
        assert_eq!(editor.text(), "new\n文");
        assert_eq!(editor.cursor(), 0);
        assert_eq!(editor.selection(), None);
        editor.apply(Key::Undo);
        assert_eq!(editor.text(), "a日\noriginal");
        assert_eq!(editor.selection(), Some(0.."a日".len()));
        editor.apply(Key::CtrlEnd);
        editor.apply(Key::Paste);
        assert_eq!(editor.text(), "a日\noriginala日");
    }
}
