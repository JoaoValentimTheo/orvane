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
mod operators;
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
            b'}' => self.closer_any(
                TokenKind::RBrace,
                &[Delimiter::Brace, Delimiter::HashBrace],
                1,
            ),
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

    /// Scans a number, emitting a best-effort token even when it is malformed
    /// (ADR 0008): `Int(0)` or `Float(0.0)`, so the stream still covers the
    /// source and `orv tokens` shows the line.
    fn number(&mut self) {
        let start = self.cursor.pos();
        match number::scan_number(&mut self.cursor, self.file, &mut self.diagnostics) {
            NumberOutcome::Int(value) => self.push(TokenKind::Int(value), start, self.cursor.pos()),
            NumberOutcome::Float(value) => {
                self.push(TokenKind::Float(value), start, self.cursor.pos());
            }
            NumberOutcome::Invalid { looked_like_float } => {
                let placeholder = if looked_like_float {
                    TokenKind::Float(0.0)
                } else {
                    TokenKind::Int(0)
                };
                self.push(placeholder, start, self.cursor.pos());
            }
        }
    }

    /// Scans a string, emitting a best-effort `Str` token with the parts read
    /// before the error (ADR 0008).
    fn string(&mut self) {
        let start = self.cursor.pos();
        match string::scan_string(&mut self.cursor, self.file, &mut self.diagnostics) {
            StringOutcome::Parts(parts) | StringOutcome::Invalid(parts) => {
                self.push(TokenKind::Str(parts), start, self.cursor.pos());
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
        self.diagnostics
            .push(Diagnostic::error("E0001", "invalid character", span).with_help(help));
    }

    /// Pushes a token covering `[start, end)`.
    fn push(&mut self, kind: TokenKind, start: usize, end: usize) {
        self.tokens.push(Token::new(
            kind,
            Span::new(self.file, start as u32, end as u32),
        ));
    }
}
