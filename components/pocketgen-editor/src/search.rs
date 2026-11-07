use alloc::string::String;
use core::ops::Range;

use crate::{Editor, Key};

const MAX_QUERY_BYTES: usize = 256;

/// Feedback from the search prompt.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SearchStatus {
    /// No search has been performed for the current query.
    #[default]
    Ready,
    /// The query was not found.
    NotFound,
    /// The query reached its maximum size in bytes.
    QueryTooLong,
}

/// A modal, case-sensitive search prompt.
#[derive(Default)]
pub struct Search {
    active: bool,
    query: String,
    last_match: Option<Range<usize>>,
    status: SearchStatus,
}

impl Search {
    /// Whether the prompt is open.
    pub fn active(&self) -> bool {
        self.active
    }

    /// The current query.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// The current feedback.
    pub fn status(&self) -> SearchStatus {
        self.status
    }

    /// Opens the prompt or consumes input while it is open.
    ///
    /// Enter and Ctrl+N select the next match; Ctrl+P selects the previous.
    /// Searches wrap around the document.
    /// A single-line selection of at most 256 bytes prefills the query.
    /// Escape closes the prompt and leaves the selected match in place.
    /// Search does not modify the document or its undo history.
    pub fn handle(&mut self, key: Key, editor: &mut Editor) -> bool {
        if !self.active {
            if key != Key::Find {
                return false;
            }
            self.active = true;
            self.query.clear();
            self.last_match = None;
            if let Some(range) = editor.selection() {
                if range.len() <= MAX_QUERY_BYTES {
                    let text = editor.text();
                    let selected = &text[range.clone()];
                    if !selected.contains(['\n', '\r']) {
                        self.query.push_str(selected);
                        self.last_match = Some(range);
                    }
                }
            }
            self.status = SearchStatus::Ready;
            return true;
        }
        match key {
            Key::Escape => self.active = false,
            Key::Character(c) if !c.is_control() => {
                if self.query.len() + c.len_utf8() <= MAX_QUERY_BYTES {
                    self.query.push(c);
                    self.last_match = None;
                    self.status = SearchStatus::Ready;
                } else {
                    self.status = SearchStatus::QueryTooLong;
                }
            }
            Key::Backspace => {
                self.query.pop();
                self.last_match = None;
                self.status = SearchStatus::Ready;
            }
            Key::Enter | Key::FindNext | Key::FindPrevious if !self.query.is_empty() => {
                let text = editor.text();
                let previous = key == Key::FindPrevious;
                let current = self.last_match.as_ref().filter(|range| {
                    editor.selection().as_ref() == Some(*range)
                        && range.end <= text.len()
                        && text.is_char_boundary(range.end)
                });
                let start = current.map_or(editor.cursor, |range| {
                    if previous {
                        range.start
                    } else {
                        range.end
                    }
                });
                let found = if previous {
                    text[..start]
                        .char_indices()
                        .rev()
                        .find_map(|(offset, _)| {
                            text[offset..].starts_with(&self.query).then_some(offset)
                        })
                        .or_else(|| text.rfind(&self.query))
                } else {
                    text[start..]
                        .find(&self.query)
                        .map(|offset| start + offset)
                        .or_else(|| text.find(&self.query).filter(|&offset| offset < start))
                };
                if let Some(start) = found {
                    let range = start..start + self.query.len();
                    editor.cursor = range.start;
                    editor.anchor = Some(range.end);
                    editor.preferred_column = None;
                    self.last_match = Some(range);
                    self.status = SearchStatus::Ready;
                } else {
                    self.last_match = None;
                    self.status = SearchStatus::NotFound;
                }
            }
            _ => {}
        }
        true
    }
}
