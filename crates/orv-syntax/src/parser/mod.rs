//! Recursive-descent parser (SPEC §5.2).
//!
//! The 0.1.0-alpha scope is recorded in ADR 0012: `intent`/`how`/`test`/`use py`
//! are **not** parsed.
//!
//! The parser consumes the token stream from the lexer, which may contain
//! best-effort tokens after a lexical error (ADR 0008 A). It reports its own
//! diagnostics into a [`Diagnostics`] aggregation seeded with the lexical ones,
//! so a parser diagnostic that intersects a lexical error is suppressed
//! (ADR 0008 amend, rules 2 and 3).
//!
//! Layout:
//! * [`expr`] — expressions (Pratt precedence from §5.2.1, postfix, primaries);
//! * [`stmt`] — statements and blocks;
//! * [`item`] — top-level items and the program root;
//! * [`types`] — type annotations.

mod expr;
mod item;
mod stmt;
mod types;

use crate::ast::Program;
use crate::diagnostic::Diagnostic;
use crate::diagnostics::Diagnostics;
use crate::lexer::{Keyword, Token, TokenKind};
use crate::span::Span;

/// The parser state while walking the token stream.
pub struct Parser<'a> {
    tokens: &'a [Token],
    /// Index of the next token to consume. Always points at a real token or at
    /// the final `Eof`, so it never runs off the end.
    cursor: usize,
    /// Aggregation seeded with the lexical diagnostics; the parser adds to it.
    diagnostics: Diagnostics,
    /// Recursion depth, to keep a pathological input from blowing the stack.
    depth: u32,
}

/// Deepest expression nesting accepted before the parser gives up.
///
/// A recursive-descent parser can be driven into stack overflow by deeply
/// nested input (`((((...`), and "never panic for arbitrary input" is a hard
/// requirement. The limit is generous for real programs and turns the
/// pathological case into a diagnostic (ADR 0013).
pub(crate) const MAX_DEPTH: u32 = 256;

/// The outcome of parsing a program.
#[derive(Debug)]
pub struct ParseResult {
    /// The program, when parsing started at all. `items` may be partial after
    /// error recovery.
    pub program: Option<Program>,
    /// Lexical and parser diagnostics, lexical first.
    pub diagnostics: Diagnostics,
}

impl ParseResult {
    /// Whether the pipeline must stop before sema/run (ADR 0008 amend, rule 5).
    pub fn has_errors(&self) -> bool {
        self.diagnostics.has_errors()
    }
}

impl<'a> Parser<'a> {
    /// Creates a parser over `tokens`, seeded with the lexical diagnostics.
    ///
    /// The file id is not stored: every span already carries its own
    /// [`FileId`](crate::source::FileId), and diagnostics are built from spans.
    pub fn new(tokens: &'a [Token], lexical: impl IntoIterator<Item = Diagnostic>) -> Self {
        Self {
            tokens,
            cursor: 0,
            diagnostics: Diagnostics::new(lexical),
            depth: 0,
        }
    }

    /// Creates a parser for a nested token stream (an interpolation's
    /// sub-parse) with no lexical diagnostics of its own.
    ///
    /// `depth` is the caller's current nesting depth: the sub-parse **shares**
    /// the recursion budget, so deeply nested interpolation (`"{ "{ ... }" }"`)
    /// hits the same `E0104` limit instead of overflowing the native stack
    /// (ADR 0013). The caller already shifted the tokens' spans into the outer
    /// file, so diagnostics from this parser point at the right place.
    pub(crate) fn new_nested(tokens: &'a [Token], depth: u32) -> Self {
        Self {
            tokens,
            cursor: 0,
            diagnostics: Diagnostics::new(std::iter::empty()),
            depth,
        }
    }

    /// The parser's current nesting depth (the caller of a sub-parse reads it
    /// back so the budget stays shared).
    pub(crate) fn depth(&self) -> u32 {
        self.depth
    }

    /// Raises this parser's depth to at least `depth`.
    ///
    /// Used after a sub-parse so a later interpolation in the same expression
    /// keeps the budget the sub-parse consumed (ADR 0013).
    pub(crate) fn absorb_depth(&mut self, depth: u32) {
        self.depth = self.depth.max(depth);
    }

    /// Consumes the parser, returning its diagnostics (for a nested parse whose
    /// diagnostics must be merged into the outer aggregation).
    pub(crate) fn into_diagnostics(self) -> Diagnostics {
        self.diagnostics
    }

    /// Parses a whole program (SPEC §5.2 `program`).
    pub fn parse_program(&mut self) -> ParseResult {
        let program = self.program();
        ParseResult {
            program,
            diagnostics: std::mem::take(&mut self.diagnostics),
        }
    }

    // --- Token access -------------------------------------------------------

    /// The token under the cursor.
    ///
    /// Falls back to the last token (`Eof`, per the lexer invariant) so this
    /// never panics, even on an empty slice.
    pub(crate) fn current(&self) -> &Token {
        match self.tokens.get(self.cursor) {
            Some(token) => token,
            None => match self.tokens.last() {
                Some(last) => last,
                // Only reachable with an empty token slice, which `lex` never
                // produces; an `Eof` span at offset 0 is the safe stand-in.
                None => &EMPTY_EOF,
            },
        }
    }

    /// The token `offset` positions ahead, clamped to the last token.
    pub(crate) fn peek(&self, offset: usize) -> &Token {
        let index = self.cursor.saturating_add(offset);
        match self.tokens.get(index) {
            Some(token) => token,
            None => match self.tokens.last() {
                Some(last) => last,
                None => &EMPTY_EOF,
            },
        }
    }

    /// The kind under the cursor.
    pub(crate) fn kind(&self) -> &TokenKind {
        &self.current().kind
    }

    /// Whether the cursor is on `Eof`.
    pub(crate) fn at_eof(&self) -> bool {
        matches!(self.kind(), TokenKind::Eof)
    }

    /// Consumes the current token and returns it.
    pub(crate) fn bump(&mut self) -> Token {
        let token = self.current().clone();
        if self.cursor + 1 < self.tokens.len() {
            self.cursor += 1;
        }
        token
    }

    /// Consumes the current token when it is `kind`.
    pub(crate) fn eat(&mut self, kind: &TokenKind) -> Option<Token> {
        if self.kind() == kind {
            Some(self.bump())
        } else {
            None
        }
    }

    /// Skips any run of `Newline` tokens (layout inside a block or file).
    pub(crate) fn skip_newlines(&mut self) {
        while self.kind() == &TokenKind::Newline {
            self.bump();
        }
    }

    /// Skips newlines only when the next real token is `else`.
    ///
    /// Used by `if`: a newline before `else` is part of the construct, but the
    /// same newline may also terminate the statement, so it must not be eaten
    /// when no `else` follows.
    pub(crate) fn skip_newlines_if_else_follows(&mut self) {
        let mut offset = 0;
        while self.peek(offset).kind == TokenKind::Newline {
            offset += 1;
        }
        if self.peek(offset).kind == TokenKind::Kw(Keyword::Else) {
            for _ in 0..offset {
                self.bump();
            }
        }
    }

    /// Enters a nested parse, refusing past [`MAX_DEPTH`].
    ///
    /// Returns `false` (after reporting `E0104`) when the limit is reached, so
    /// the caller can stop instead of overflowing the stack.
    pub(crate) fn enter(&mut self) -> bool {
        if self.depth >= MAX_DEPTH {
            let span = self.current().span;
            self.report(
                "E0104",
                "expression nesting is too deep",
                span,
                Some(format!("the limit is {MAX_DEPTH} levels")),
            );
            return false;
        }
        self.depth += 1;
        true
    }

    /// Leaves a nested parse started with [`Self::enter`].
    pub(crate) fn leave(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    // --- Diagnostics --------------------------------------------------------

    /// Reports a diagnostic, subject to lexical suppression.
    pub(crate) fn report(
        &mut self,
        code: &'static str,
        message: impl Into<String>,
        span: Span,
        help: Option<String>,
    ) {
        let mut diagnostic = Diagnostic::error(code, message, span);
        if let Some(help) = help {
            diagnostic = diagnostic.with_help(help);
        }
        self.diagnostics.extend_suppressed([diagnostic]);
    }

    /// Reports `E0102` ("expected X, found Y") for the current token.
    pub(crate) fn report_expected(&mut self, expected: &str, found: &Token) {
        let message = format!("expected {expected}, found {}", describe(&found.kind));
        self.report("E0102", message, found.span, None);
    }

    /// Whether the string token at `span` may have its `Expr` parts sub-parsed.
    ///
    /// A `Str` is *best effort* when the lexer already failed on it (ADR 0008 A),
    /// so its interpolation text must not be re-lexed (rule 4).
    pub(crate) fn subparse_allowed(&self, span: Span) -> bool {
        self.diagnostics.should_subparse_expr(span)
    }

    /// The raw cursor index, used to guarantee progress during recovery.
    pub(crate) fn cursor_index(&self) -> usize {
        self.cursor
    }
}

/// A stand-in `Eof` used only if the token slice is somehow empty.
static EMPTY_EOF: Token = Token {
    kind: TokenKind::Eof,
    span: Span::new(crate::source::FileId(0), 0, 0),
};

/// A short human-readable description of a token, for `E0102` messages.
///
/// Kept separate from the dump format on purpose: diagnostics are prose, the
/// dump is a machine-readable contract (ADR 0006).
pub(crate) fn describe(kind: &TokenKind) -> String {
    match kind {
        TokenKind::Int(value) => format!("`{value}`"),
        TokenKind::Float(value) => format!("`{value}`"),
        TokenKind::Str(_) => "a string literal".to_owned(),
        TokenKind::Ident(name) => format!("`{name}`"),
        TokenKind::Kw(keyword) => format!("`{}`", keyword.as_str()),
        TokenKind::Newline => "a line break".to_owned(),
        TokenKind::Eof => "end of file".to_owned(),
        other => format!("`{}`", crate::dump::kind_text(other)),
    }
}

#[cfg(test)]
mod tests;
