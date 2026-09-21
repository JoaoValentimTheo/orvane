//! Trivia handling: comments and `Newline` emission (SPEC §5.1).
//!
//! Kept apart from the main scanner so the token-dispatch match stays readable
//! (§3.4: modules around 400 lines, functions around 60).

use crate::diagnostic::Diagnostic;
use crate::span::Span;

use super::{Lexer, Token, TokenKind};

impl Lexer<'_> {
    /// `// ...` up to, but not including, the line break.
    pub(super) fn line_comment(&mut self) {
        while let Some(b) = self.cursor.peek() {
            if b == b'\n' || b == b'\r' {
                break;
            }
            self.cursor.advance(1);
        }
    }

    /// `/* ... */`, nestable. Returns `false` when the comment is unterminated
    /// (an `E0003` is reported and scanning stops).
    ///
    /// A block comment containing a line break counts as one `Newline`, like Go.
    pub(super) fn block_comment(&mut self) -> bool {
        let open = self.cursor.pos();
        self.cursor.advance(2);
        let mut depth = 1usize;
        let mut saw_newline = false;

        while depth > 0 {
            match self.cursor.peek() {
                None => {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "E0003",
                            "unterminated block comment",
                            Span::new(self.file, open as u32, self.cursor.pos() as u32),
                        )
                        .with_help("add a closing `*/`"),
                    );
                    return false;
                }
                Some(b'*') if self.cursor.peek_byte(1) == Some(b'/') => {
                    self.cursor.advance(2);
                    depth -= 1;
                }
                Some(b'/') if self.cursor.peek_byte(1) == Some(b'*') => {
                    self.cursor.advance(2);
                    depth += 1;
                }
                Some(b'\n') => {
                    self.cursor.advance(1);
                    saw_newline = true;
                }
                Some(b'\r') => {
                    if self.cursor.peek_byte(1) == Some(b'\n') {
                        self.cursor.advance(1);
                    }
                    self.cursor.advance(1);
                    saw_newline = true;
                }
                Some(_) => self.cursor.advance(1),
            }
        }

        if saw_newline {
            self.note_newline();
        }
        true
    }

    /// Records that a line break was seen. Emitted lazily so consecutive breaks
    /// collapse into one.
    pub(super) fn note_newline(&mut self) {
        self.pending_newline = true;
    }

    /// Emits the pending `Newline` unless the innermost delimiter suppresses it.
    ///
    /// A `Newline` is never emitted before the first real token, so leading
    /// blank lines produce no token at all (§5.1).
    pub(super) fn flush_newline(&mut self) {
        if !self.pending_newline {
            return;
        }
        self.pending_newline = false;
        if self.tokens.iter().all(|t| t.is_newline()) {
            return;
        }
        let suppressed = self
            .delimiters
            .last()
            .is_some_and(|d| d.suppresses_newline());
        if suppressed {
            return;
        }
        let pos = self.cursor.pos() as u32;
        self.tokens
            .push(Token::new(TokenKind::Newline, Span::point(self.file, pos)));
    }
}
