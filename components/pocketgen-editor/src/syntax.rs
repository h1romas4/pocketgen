//! A small, allocation-free Python lexer for editor display.
//!
//! This is not a Python parser: f-string expressions are colored as strings,
//! and invalid or incomplete input is highlighted on a best-effort basis.

/// The display category of a byte range in a source line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Plain,
    Keyword,
    String,
    Comment,
    Number,
}

/// A half-open byte range relative to the current line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub kind: Kind,
}

#[derive(Clone, Copy, Default)]
enum State {
    #[default]
    Normal,
    Single(u8),
    Triple(u8),
}

/// Tracks strings that continue between lines.
#[derive(Default)]
pub struct Highlighter {
    state: State,
}

impl Highlighter {
    /// Begin a line. Consume the iterator fully before processing the next line.
    pub fn line<'a>(&'a mut self, line: &'a str) -> Line<'a> {
        if line.is_empty() && matches!(self.state, State::Single(_)) {
            self.state = State::Normal;
        }
        Line {
            state: &mut self.state,
            bytes: line.as_bytes(),
            offset: 0,
        }
    }
}

/// An iterator over contiguous spans covering a line.
pub struct Line<'a> {
    state: &'a mut State,
    bytes: &'a [u8],
    offset: usize,
}

fn identifier_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_' || c >= 0x80
}

fn identifier_continue(c: u8) -> bool {
    identifier_start(c) || c.is_ascii_digit()
}

fn string_prefix(word: &[u8]) -> bool {
    match word.len() {
        1 => matches!(word[0].to_ascii_lowercase(), b'r' | b'u' | b'b' | b'f'),
        2 => matches!(
            (word[0].to_ascii_lowercase(), word[1].to_ascii_lowercase()),
            (b'b', b'r') | (b'r', b'b') | (b'f', b'r') | (b'r', b'f')
        ),
        _ => false,
    }
}

fn keyword(word: &[u8]) -> bool {
    matches!(
        word,
        b"False"
            | b"None"
            | b"True"
            | b"and"
            | b"as"
            | b"assert"
            | b"async"
            | b"await"
            | b"break"
            | b"class"
            | b"continue"
            | b"def"
            | b"del"
            | b"elif"
            | b"else"
            | b"except"
            | b"finally"
            | b"for"
            | b"from"
            | b"global"
            | b"if"
            | b"import"
            | b"in"
            | b"is"
            | b"lambda"
            | b"nonlocal"
            | b"not"
            | b"or"
            | b"pass"
            | b"raise"
            | b"return"
            | b"try"
            | b"while"
            | b"with"
            | b"yield"
    )
}

impl Line<'_> {
    fn string(&mut self, quote: u8, triple: bool) {
        while self.offset < self.bytes.len() {
            if self.bytes[self.offset] == b'\\' {
                self.offset += 1;
                if self.offset < self.bytes.len() {
                    self.offset += 1;
                } else if !triple {
                    *self.state = State::Single(quote);
                    return;
                }
            } else if self.bytes[self.offset] == quote {
                if !triple {
                    self.offset += 1;
                    *self.state = State::Normal;
                    return;
                }
                if self.bytes[self.offset..].starts_with(&[quote, quote, quote]) {
                    self.offset += 3;
                    *self.state = State::Normal;
                    return;
                }
                self.offset += 1;
            } else {
                self.offset += 1;
            }
        }
        *self.state = if triple {
            State::Triple(quote)
        } else {
            State::Normal
        };
    }

    fn open_string(&mut self) {
        let quote = self.bytes[self.offset];
        let triple = self.bytes[self.offset..].starts_with(&[quote, quote, quote]);
        self.offset += if triple { 3 } else { 1 };
        self.string(quote, triple);
    }

    fn number(&mut self) {
        let start = self.offset;
        if self.bytes[start] == b'0' && start + 1 < self.bytes.len() {
            let radix = match self.bytes[start + 1].to_ascii_lowercase() {
                b'x' => 16,
                b'o' => 8,
                b'b' => 2,
                _ => 0,
            };
            if radix != 0 {
                self.offset += 2;
                while self.offset < self.bytes.len()
                    && (self.bytes[self.offset] == b'_'
                        || (self.bytes[self.offset] as char).is_digit(radix))
                {
                    self.offset += 1;
                }
                return;
            }
        }

        while self.offset < self.bytes.len()
            && (self.bytes[self.offset].is_ascii_digit() || self.bytes[self.offset] == b'_')
        {
            self.offset += 1;
        }
        if self.bytes.get(self.offset) == Some(&b'.') {
            self.offset += 1;
            while self.offset < self.bytes.len()
                && (self.bytes[self.offset].is_ascii_digit() || self.bytes[self.offset] == b'_')
            {
                self.offset += 1;
            }
        }
        if matches!(self.bytes.get(self.offset), Some(b'e' | b'E')) {
            self.offset += 1;
            if matches!(self.bytes.get(self.offset), Some(b'+' | b'-')) {
                self.offset += 1;
            }
            while self.offset < self.bytes.len()
                && (self.bytes[self.offset].is_ascii_digit() || self.bytes[self.offset] == b'_')
            {
                self.offset += 1;
            }
        }
        if matches!(self.bytes.get(self.offset), Some(b'j' | b'J')) {
            self.offset += 1;
        }
    }
}

impl Iterator for Line<'_> {
    type Item = Span;

    fn next(&mut self) -> Option<Span> {
        let start = self.offset;
        let c = *self.bytes.get(start)?;
        let kind = match *self.state {
            State::Single(quote) | State::Triple(quote) => {
                self.string(quote, matches!(*self.state, State::Triple(_)));
                Kind::String
            }
            State::Normal if c == b'#' => {
                self.offset = self.bytes.len();
                Kind::Comment
            }
            State::Normal if c == b'\'' || c == b'"' => {
                self.open_string();
                Kind::String
            }
            State::Normal if identifier_start(c) => {
                self.offset += 1;
                while self.offset < self.bytes.len() && identifier_continue(self.bytes[self.offset])
                {
                    self.offset += 1;
                }
                let word = &self.bytes[start..self.offset];
                if string_prefix(word) && matches!(self.bytes.get(self.offset), Some(b'\'' | b'"'))
                {
                    self.open_string();
                    Kind::String
                } else if keyword(word) {
                    Kind::Keyword
                } else {
                    Kind::Plain
                }
            }
            State::Normal
                if c.is_ascii_digit()
                    || (c == b'.' && self.bytes.get(start + 1).is_some_and(u8::is_ascii_digit)) =>
            {
                self.number();
                Kind::Number
            }
            State::Normal => {
                self.offset += 1;
                Kind::Plain
            }
        };
        Some(Span {
            start,
            end: self.offset,
            kind,
        })
    }
}
