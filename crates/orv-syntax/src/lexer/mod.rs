//! Orvane lexer (SPEC §5.1, ADRs 0001/0006/0007).
//!
//! [`lex`] turns a [`SourceFile`] into a token stream plus diagnostics. It never
//! panics: every malformed input produces an `E00xx` diagnostic and scanning
//! resumes, and the stream always ends with [`TokenKind::Eof`].
//!
//! Invariants (checked by tests and by the structured proptest):
//!
//! * the last token is `Eof`;
//! * every span is inside the file, on `char` boundaries, non-overlapping and
//!   strictly increasing;
//! * no diagnostics are produced for a well-formed program.

mod cursor;
mod number;
mod string;
#[cfg(test)]
mod tests;
mod token;
mod trivia;

pub use token::{Keyword, StrPart, Token, TokenKind};

use crate::diagnostic::Diagnostic;
use crate::source::{FileId, SourceFile};
use crate::span::Span;

use cursor::Cursor;
use number::NumberOutcome;
use string::StringOutcome;

/// A delimiter tracked on the newline-suppression stack (§5.1 rule 1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Delimiter {
    /// `(` — suppresses `Newline`.
    Paren,
    /// `[` — suppresses `Newline`.
    Bracket,
    /// `#{` — suppresses `Newline`.
    HashBrace,
    /// `{` — does **not** suppress `Newline`, even inside a `(`.
    Brace,
}

impl Delimiter {
    /// Whether a `Newline` is suppressed while this delimiter is innermost.
    const fn suppresses_newline(self) -> bool {
        match self {
            Delimiter::Paren | Delimiter::Bracket | Delimiter::HashBrace => true,
            Delimiter::Brace => false,
        }
    }
}

/// Lexes `file` into tokens and diagnostics.
///
/// The token vector always ends with `Eof`. Diagnostics are returned in source
/// order and never abort scanning early.
pub fn lex(file: &SourceFile) -> (Vec<Token>, Vec<Diagnostic>) {
    Lexer::new(file.id, file.text()).run()
}

/// The lexer state machine.
///
/// Fields are `pub(super)` so the sibling modules (`trivia`, and the tests) can
/// work with them without turning the lexer's internals into crate API.
struct Lexer<'a> {
    pub(super) file: FileId,
    pub(super) cursor: Cursor<'a>,
    pub(super) tokens: Vec<Token>,
    pub(super) diagnostics: Vec<Diagnostic>,
    pub(super) delimiters: Vec<Delimiter>,
    /// Whether a `Newline` token may be emitted at the current position.
    /// Cleared at the start of the file and after each emitted `Newline`.
    pub(super) pending_newline: bool,
}

impl<'a> Lexer<'a> {
    fn new(file: FileId, text: &'a str) -> Self {
        Self {
            file,
            cursor: Cursor::new(text),
            tokens: Vec::new(),
            diagnostics: Vec::new(),
            delimiters: Vec::new(),
            pending_newline: false,
        }
    }

    fn run(mut self) -> (Vec<Token>, Vec<Diagnostic>) {
        while !self.cursor.is_eof() {
            self.next_token();
        }
        // A trailing newline leaves `pending_newline` set; flush it so the last
        // statement is still terminated.
        self.flush_newline();
        let span = Span::point(self.file, self.cursor.pos() as u32);
        self.tokens.push(Token::new(TokenKind::Eof, span));
        (self.tokens, self.diagnostics)
    }

    /// Scans one token (or consumes whitespace/comments and loops).
    fn next_token(&mut self) {
        // Whitespace and comments may reveal a line break, which we handle here
        // so a run of them collapses into a single `Newline`.
        loop {
            match self.cursor.peek() {
                Some(b' ' | b'\t' | 0x0b | 0x0c) => self.cursor.advance(1),
                Some(b'\r') => {
                    // `\r\n` is one break; a lone `\r` is whitespace.
                    if self.cursor.peek_byte(1) == Some(b'\n') {
                        self.cursor.advance(1);
                        self.note_newline();
                    } else {
                        self.cursor.advance(1);
                    }
                }
                Some(b'\n') => {
                    self.cursor.advance(1);
                    self.note_newline();
                }
                Some(b'/') if self.cursor.peek_byte(1) == Some(b'/') => self.line_comment(),
                Some(b'/') if self.cursor.peek_byte(1) == Some(b'*') => {
                    if !self.block_comment() {
                        return;
                    }
                }
                _ => break,
            }
        }

        if self.cursor.is_eof() {
            return;
        }

        if self.pending_newline {
            self.flush_newline();
            return;
        }

        self.scan_token();
    }

    /// Scans a single non-trivia token.
    fn scan_token(&mut self) {
        let Some(byte) = self.cursor.peek() else {
            return;
        };

        match byte {
            b'(' => self.opener(TokenKind::LParen, Delimiter::Paren, 1),
            b'[' => self.opener(TokenKind::LBracket, Delimiter::Bracket, 1),
            b'{' => self.opener(TokenKind::LBrace, Delimiter::Brace, 1),
            b')' => self.closer(TokenKind::RParen, Delimiter::Paren, 1),
            b']' => self.closer(TokenKind::RBracket, Delimiter::Bracket, 1),
            // `}` closes either a block `{` or a map `#{`; the opener on top of
            // the stack decides which.
            b'}' => self.closer_any(TokenKind::RBrace, &[Delimiter::Brace, Delimiter::HashBrace], 1),
            b'#' if self.cursor.peek_byte(1) == Some(b'{') => {
                self.opener(TokenKind::HashLBrace, Delimiter::HashBrace, 2);
            }
            b',' => self.punct(TokenKind::Comma, 1),
            b':' => self.punct(TokenKind::Colon, 1),
            b';' => self.punct(TokenKind::Semicolon, 1),
            b'"' => self.string(),
            b'0'..=b'9' => self.number(),
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => self.ident_or_keyword(),
            b'.' => self.dot(),
            b'-' => self.minus(),
            b'=' => self.eq(),
            b'!' => self.bang(),
            b'<' => self.lt(),
            b'>' => self.gt(),
            b'+' => self.plus(),
            b'*' => self.star(),
            b'/' => self.slash(),
            b'%' => self.punct(TokenKind::Percent, 1),
            b'?' => self.question(),
            _ => self.invalid_char(),
        }
    }

    // --- Identifiers and numbers -------------------------------------------

    fn ident_or_keyword(&mut self) {
        let start = self.cursor.pos();
        while let Some(b) = self.cursor.peek() {
            if b.is_ascii_alphanumeric() || b == b'_' {
                self.cursor.advance(1);
            } else {
                break;
            }
        }
        let end = self.cursor.pos();
        let text = self.cursor.text().get(start..end).unwrap_or("");
        let kind = match Keyword::lookup(text) {
            Some(kw) => TokenKind::Kw(kw),
            // `py` and every other identifier arrive here (SPEC §5.1).
            None => TokenKind::Ident(text.to_owned()),
        };
        self.push(kind, start, end);
    }

    fn number(&mut self) {
        let start = self.cursor.pos();
        match number::scan_number(&mut self.cursor, self.file, &mut self.diagnostics) {
            NumberOutcome::Int(value) => self.push(TokenKind::Int(value), start, self.cursor.pos()),
            NumberOutcome::Float(value) => {
                self.push(TokenKind::Float(value), start, self.cursor.pos());
            }
            // The diagnostic is already recorded; consume nothing more.
            NumberOutcome::Invalid => {}
        }
    }

    fn string(&mut self) {
        let start = self.cursor.pos();
        match string::scan_string(&mut self.cursor, self.file, &mut self.diagnostics) {
            StringOutcome::Parts(parts) => {
                self.push(TokenKind::Str(parts), start, self.cursor.pos());
            }
            StringOutcome::Invalid => {}
        }
    }

    // --- Operators and punctuation -----------------------------------------

    /// Emits a token that opens a delimiter and pushes it on the stack.
    fn opener(&mut self, kind: TokenKind, delim: Delimiter, len: usize) {
        let start = self.cursor.pos();
        self.delimiters.push(delim);
        self.cursor.advance(len);
        self.push(kind, start, self.cursor.pos());
    }

    /// Emits a closing token, popping its opener when one is on the stack.
    ///
    /// An unmatched closer is emitted normally; the parser reports it. It must
    /// never panic.
    fn closer(&mut self, kind: TokenKind, expected: Delimiter, len: usize) {
        self.closer_any(kind, &[expected], len);
    }

    /// Emits a closing token that may match more than one opener, popping the
    /// innermost when it matches. An unmatched closer is still emitted.
    fn closer_any(&mut self, kind: TokenKind, expected: &[Delimiter], len: usize) {
        let start = self.cursor.pos();
        if self.delimiters.last().is_some_and(|d| expected.contains(d)) {
            self.delimiters.pop();
        }
        self.cursor.advance(len);
        self.push(kind, start, self.cursor.pos());
    }

    /// Emits a token that neither opens nor closes a delimiter.
    fn punct(&mut self, kind: TokenKind, len: usize) {
        let start = self.cursor.pos();
        self.cursor.advance(len);
        self.push(kind, start, self.cursor.pos());
    }

    fn dot(&mut self) {
        let start = self.cursor.pos();
        if self.cursor.peek_byte(1) == Some(b'.') {
            let kind = if self.cursor.peek_byte(2) == Some(b'=') {
                self.cursor.advance(3);
                TokenKind::DotDotEq
            } else {
                self.cursor.advance(2);
                TokenKind::DotDot
            };
            self.push(kind, start, self.cursor.pos());
        } else {
            self.cursor.advance(1);
            self.push(TokenKind::Dot, start, self.cursor.pos());
        }
    }

    fn minus(&mut self) {
        let start = self.cursor.pos();
        match self.cursor.peek_byte(1) {
            Some(b'-') if self.cursor.peek_byte(2) == Some(b'>') => {
                // Not in the operator table, but a clear maximal-munch win.
                self.cursor.advance(3);
                self.push(TokenKind::Arrow, start, self.cursor.pos());
            }
            Some(b'>') => {
                self.cursor.advance(2);
                self.push(TokenKind::Arrow, start, self.cursor.pos());
            }
            Some(b'=') => {
                self.cursor.advance(2);
                self.push(TokenKind::MinusEq, start, self.cursor.pos());
            }
            _ => {
                self.cursor.advance(1);
                self.push(TokenKind::Minus, start, self.cursor.pos());
            }
        }
    }

    fn eq(&mut self) {
        let start = self.cursor.pos();
        match self.cursor.peek_byte(1) {
            Some(b'=') => {
                self.cursor.advance(2);
                self.push(TokenKind::EqEq, start, self.cursor.pos());
            }
            Some(b'>') => {
                self.cursor.advance(2);
                self.push(TokenKind::FatArrow, start, self.cursor.pos());
            }
            _ => {
                self.cursor.advance(1);
                self.push(TokenKind::Eq, start, self.cursor.pos());
            }
        }
    }

    /// `!` is only valid as `!=`; a lone `!` is `E0001`.
    fn bang(&mut self) {
        let start = self.cursor.pos();
        if self.cursor.peek_byte(1) == Some(b'=') {
            self.cursor.advance(2);
            self.push(TokenKind::NotEq, start, self.cursor.pos());
        } else {
            // `invalid_char` consumes the `!` itself.
            self.invalid_char();
        }
    }

    fn lt(&mut self) {
        let start = self.cursor.pos();
        if self.cursor.peek_byte(1) == Some(b'=') {
            self.cursor.advance(2);
            self.push(TokenKind::Le, start, self.cursor.pos());
        } else {
            self.cursor.advance(1);
            self.push(TokenKind::Lt, start, self.cursor.pos());
        }
    }

    fn gt(&mut self) {
        let start = self.cursor.pos();
        if self.cursor.peek_byte(1) == Some(b'=') {
            self.cursor.advance(2);
            self.push(TokenKind::Ge, start, self.cursor.pos());
        } else {
            self.cursor.advance(1);
            self.push(TokenKind::Gt, start, self.cursor.pos());
        }
    }

    fn plus(&mut self) {
        let start = self.cursor.pos();
        if self.cursor.peek_byte(1) == Some(b'=') {
            self.cursor.advance(2);
            self.push(TokenKind::PlusEq, start, self.cursor.pos());
        } else {
            self.cursor.advance(1);
            self.push(TokenKind::Plus, start, self.cursor.pos());
        }
    }

    fn star(&mut self) {
        let start = self.cursor.pos();
        if self.cursor.peek_byte(1) == Some(b'=') {
            self.cursor.advance(2);
            self.push(TokenKind::StarEq, start, self.cursor.pos());
        } else {
            self.cursor.advance(1);
            self.push(TokenKind::Star, start, self.cursor.pos());
        }
    }

    fn slash(&mut self) {
        let start = self.cursor.pos();
        if self.cursor.peek_byte(1) == Some(b'=') {
            self.cursor.advance(2);
            self.push(TokenKind::SlashEq, start, self.cursor.pos());
        } else {
            self.cursor.advance(1);
            self.push(TokenKind::Slash, start, self.cursor.pos());
        }
    }

    fn question(&mut self) {
        let start = self.cursor.pos();
        match self.cursor.peek_byte(1) {
            Some(b'.') => {
                self.cursor.advance(2);
                self.push(TokenKind::QuestionDot, start, self.cursor.pos());
            }
            Some(b'?') => {
                self.cursor.advance(2);
                self.push(TokenKind::QuestionQuestion, start, self.cursor.pos());
            }
            _ => {
                self.cursor.advance(1);
                self.push(TokenKind::Question, start, self.cursor.pos());
            }
        }
    }

    // --- Errors and emission -------------------------------------------------

    /// Reports `E0001` for an unrecognised character and skips it.
    ///
    /// Consumes exactly one `char`, so scanning always progresses even for a
    /// byte sequence the lexer cannot classify.
    fn invalid_char(&mut self) {
        let start = self.cursor.pos();
        let non_ascii = self.cursor.peek_char().is_some_and(|c| !c.is_ascii());
        self.cursor.bump_char();
        let end = self.cursor.pos();

        let span = Span::new(self.file, start as u32, end as u32);
        let help = if non_ascii {
            // Reached only outside a string or comment.
            "identifiers are ASCII-only in v0.1; non-ASCII text must be inside a string or comment"
        } else {
            "`!` is only valid as `!=`; `&`, `|`, `@`, `$`, `\\` and backticks are not operators"
        };
        self.diagnostics.push(
            Diagnostic::error("E0001", "invalid character", span).with_help(help),
        );
    }

    /// Pushes a token covering `[start, end)`.
    fn push(&mut self, kind: TokenKind, start: usize, end: usize) {
        self.tokens
            .push(Token::new(kind, Span::new(self.file, start as u32, end as u32)));
    }
}
