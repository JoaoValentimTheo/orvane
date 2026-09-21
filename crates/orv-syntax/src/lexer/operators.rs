//! Operator and punctuation scanning (SPEC §5.1).
//!
//! Split from the main scanner so the token-dispatch match in `mod.rs` stays
//! readable (§3.4). Every helper here consumes exactly the bytes it emits and
//! never panics: an unmatched closer is simply a token the parser rejects.

use super::{Delimiter, Lexer, TokenKind};

impl Lexer<'_> {
    /// Emits a token that opens a delimiter and pushes it on the stack.
    pub(super) fn opener(&mut self, kind: TokenKind, delim: Delimiter, len: usize) {
        let start = self.cursor.pos();
        self.delimiters.push(delim);
        self.cursor.advance(len);
        self.push(kind, start, self.cursor.pos());
    }

    /// Emits a closing token, popping its opener when one is on the stack.
    ///
    /// An unmatched closer is emitted normally; the parser reports it. It must
    /// never panic.
    pub(super) fn closer(&mut self, kind: TokenKind, expected: Delimiter, len: usize) {
        self.closer_any(kind, &[expected], len);
    }

    /// Emits a closing token that may match more than one opener, popping the
    /// innermost when it matches. An unmatched closer is still emitted.
    pub(super) fn closer_any(&mut self, kind: TokenKind, expected: &[Delimiter], len: usize) {
        let start = self.cursor.pos();
        if self.delimiters.last().is_some_and(|d| expected.contains(d)) {
            self.delimiters.pop();
        }
        self.cursor.advance(len);
        self.push(kind, start, self.cursor.pos());
    }

    /// Emits a token that neither opens nor closes a delimiter.
    pub(super) fn punct(&mut self, kind: TokenKind, len: usize) {
        let start = self.cursor.pos();
        self.cursor.advance(len);
        self.push(kind, start, self.cursor.pos());
    }

    pub(super) fn dot(&mut self) {
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

    pub(super) fn minus(&mut self) {
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

    pub(super) fn eq(&mut self) {
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
    pub(super) fn bang(&mut self) {
        let start = self.cursor.pos();
        if self.cursor.peek_byte(1) == Some(b'=') {
            self.cursor.advance(2);
            self.push(TokenKind::NotEq, start, self.cursor.pos());
        } else {
            // `invalid_char` consumes the `!` itself.
            self.invalid_char();
        }
    }

    pub(super) fn lt(&mut self) {
        let start = self.cursor.pos();
        if self.cursor.peek_byte(1) == Some(b'=') {
            self.cursor.advance(2);
            self.push(TokenKind::Le, start, self.cursor.pos());
        } else {
            self.cursor.advance(1);
            self.push(TokenKind::Lt, start, self.cursor.pos());
        }
    }

    pub(super) fn gt(&mut self) {
        let start = self.cursor.pos();
        if self.cursor.peek_byte(1) == Some(b'=') {
            self.cursor.advance(2);
            self.push(TokenKind::Ge, start, self.cursor.pos());
        } else {
            self.cursor.advance(1);
            self.push(TokenKind::Gt, start, self.cursor.pos());
        }
    }

    pub(super) fn plus(&mut self) {
        let start = self.cursor.pos();
        if self.cursor.peek_byte(1) == Some(b'=') {
            self.cursor.advance(2);
            self.push(TokenKind::PlusEq, start, self.cursor.pos());
        } else {
            self.cursor.advance(1);
            self.push(TokenKind::Plus, start, self.cursor.pos());
        }
    }

    pub(super) fn star(&mut self) {
        let start = self.cursor.pos();
        if self.cursor.peek_byte(1) == Some(b'=') {
            self.cursor.advance(2);
            self.push(TokenKind::StarEq, start, self.cursor.pos());
        } else {
            self.cursor.advance(1);
            self.push(TokenKind::Star, start, self.cursor.pos());
        }
    }

    pub(super) fn slash(&mut self) {
        let start = self.cursor.pos();
        if self.cursor.peek_byte(1) == Some(b'=') {
            self.cursor.advance(2);
            self.push(TokenKind::SlashEq, start, self.cursor.pos());
        } else {
            self.cursor.advance(1);
            self.push(TokenKind::Slash, start, self.cursor.pos());
        }
    }

    pub(super) fn question(&mut self) {
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
}
