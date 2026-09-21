//! Recursive-descent parser (SPEC §5.2).
//!
//! M2 builds this in steps. This step implements **primary expressions only**:
//! literals, identifiers, `( expr )` and a `{ }` block skeleton. Operators,
//! calls, indexing, field access, `if`/`match`, statements and error recovery
//! are deliberately absent.
//!
//! The parser consumes the token stream from the lexer, which may contain
//! best-effort tokens after a lexical error (ADR 0008 A). It reports its own
//! diagnostics into a [`Diagnostics`] aggregation seeded with the lexical ones,
//! so a parser diagnostic that intersects a lexical error is suppressed
//! (ADR 0008 amend, rules 2 and 3).

use crate::ast::{Block, Expr, ExprKind, Literal};
use crate::diagnostic::Diagnostic;
use crate::diagnostics::Diagnostics;
use crate::lexer::{Keyword, StrPart, Token, TokenKind};
use crate::span::Span;

/// The parser state while walking the token stream.
pub struct Parser<'a> {
    tokens: &'a [Token],
    /// Index of the next token to consume. Always points at a real token or at
    /// the final `Eof`, so it never runs off the end.
    cursor: usize,
    /// Aggregation seeded with the lexical diagnostics; the parser adds to it.
    diagnostics: Diagnostics,
}

/// The outcome of a parse: the expression plus every retained diagnostic.
///
/// `diagnostics` is the merged set (lexical first, parser additions filtered),
/// so callers can decide with [`Diagnostics::has_errors`] whether to continue to
/// sema/run. As there is no CLI for this yet, the gate is documented, not wired.
#[derive(Debug)]
pub struct ParseResult {
    /// The parsed expression, when the input starts with a valid primary.
    pub expr: Option<Expr>,
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
        }
    }

    /// Parses a single expression from the start of the stream.
    ///
    /// Trailing tokens are **not** consumed: this step has no statement
    /// grammar, so the caller decides what to do with the remainder.
    pub fn parse_expr(&mut self) -> ParseResult {
        let expr = self.parse_primary();
        ParseResult {
            expr,
            diagnostics: std::mem::take(&mut self.diagnostics),
        }
    }

    /// Parses a primary expression.
    ///
    /// Returns `None` after reporting a diagnostic when the current token cannot
    /// start a primary.
    pub fn parse_primary(&mut self) -> Option<Expr> {
        let token = self.current().clone();
        match token.kind {
            TokenKind::Int(value) => {
                self.advance();
                Some(Expr::new(
                    ExprKind::Literal(Literal::Int(value)),
                    token.span,
                ))
            }
            TokenKind::Float(value) => {
                self.advance();
                Some(Expr::new(
                    ExprKind::Literal(Literal::Float(value)),
                    token.span,
                ))
            }
            TokenKind::Str(parts) => {
                self.advance();
                let parts = self.prepare_str_parts(parts, token.span);
                Some(Expr::new(
                    ExprKind::Literal(Literal::Str(parts)),
                    token.span,
                ))
            }
            TokenKind::Kw(Keyword::True) => {
                self.advance();
                Some(Expr::new(
                    ExprKind::Literal(Literal::Bool(true)),
                    token.span,
                ))
            }
            TokenKind::Kw(Keyword::False) => {
                self.advance();
                Some(Expr::new(
                    ExprKind::Literal(Literal::Bool(false)),
                    token.span,
                ))
            }
            TokenKind::Kw(Keyword::None) => {
                self.advance();
                Some(Expr::new(ExprKind::Literal(Literal::None), token.span))
            }
            TokenKind::Ident(name) => {
                self.advance();
                Some(Expr::new(ExprKind::Ident(name), token.span))
            }
            TokenKind::LParen => self.parse_paren(),
            TokenKind::LBrace => self
                .parse_block()
                .map(|block| Expr::new(ExprKind::Block(block.clone()), block.span)),
            _ => {
                self.report_expected("expression", &token);
                None
            }
        }
    }

    /// Parses `( expr )`, reporting `E0102` when the `)` is missing.
    fn parse_paren(&mut self) -> Option<Expr> {
        let open = self.current().span;
        self.advance(); // the `(`

        let inner = self.parse_primary()?;

        let close = self.current().clone();
        if close.kind == TokenKind::RParen {
            self.advance();
            let span = open.to(close.span);
            return Some(Expr::new(ExprKind::Paren(Box::new(inner)), span));
        }

        // Missing `)`. The span covers what was read, so the caret lands on the
        // offending token rather than on an invented position.
        self.report_expected("`)`", &close);
        None
    }

    /// Parses a `{ ... }` block skeleton.
    ///
    /// The statement grammar is out of scope, so only the empty block (and a
    /// block of newlines) is accepted; anything else reports `E0101`.
    fn parse_block(&mut self) -> Option<Block> {
        let open = self.current().span;
        self.advance(); // the `{`

        loop {
            let token = self.current().clone();
            match token.kind {
                TokenKind::RBrace => {
                    self.advance();
                    return Some(Block::new(open.to(token.span)));
                }
                // Newlines are layout, not content, in this skeleton.
                TokenKind::Newline => self.advance(),
                TokenKind::Eof => {
                    // No closing brace: the span runs to the end of input.
                    self.report_expected("`}`", &token);
                    return None;
                }
                _ => {
                    // Statements are not implemented yet, so a block with
                    // content is rejected rather than silently dropped.
                    self.report_unexpected_statement(&token);
                    return None;
                }
            }
        }
    }

    /// Prepares the parts of a string literal for the AST.
    ///
    /// When the `Str` token carries a lexical error it is *best effort*: its
    /// [`StrPart::Expr`] text may be truncated or still contain the offending
    /// byte, so it must not be re-lexed, and no parser diagnostic may come from
    /// it (ADR 0008 amend, rule 4). The guard is the span of the whole token,
    /// because the lexical diagnostic can point at any byte of the literal.
    ///
    /// The lexer already produced the parts, so returning them unchanged *is*
    /// skipping the sub-parse: nothing here re-lexes [`StrPart::Expr::src`].
    /// That field is what a later step will feed to the parser, and this guard
    /// is the switch that later step must consult.
    ///
    /// [`StrPart::Expr::src`]: crate::lexer::StrPart::Expr
    fn prepare_str_parts(&self, parts: Vec<StrPart>, token_span: Span) -> Vec<StrPart> {
        // The decision is recorded even though nothing acts on it yet, so the
        // guard is exercised (and its value observable in tests) from the step
        // that introduces string parsing.
        let _subparse_allowed = self.diagnostics.should_subparse_expr(token_span);
        parts
    }

    // --- Token access -------------------------------------------------------

    /// The token under the cursor.
    ///
    /// Falls back to the last token (`Eof`, per the lexer invariant) so this
    /// never panics, even on an empty slice.
    fn current(&self) -> &Token {
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

    /// Advances past the current token, stopping at the last one.
    fn advance(&mut self) {
        if self.cursor + 1 < self.tokens.len() {
            self.cursor += 1;
        }
    }

    // --- Diagnostics --------------------------------------------------------

    /// Reports `E0102` ("expected X, found Y") for `found`.
    ///
    /// Goes through [`Diagnostics::extend_suppressed`], so a lexical error on
    /// the same region suppresses it (ADR 0008 amend, rule 3).
    fn report_expected(&mut self, expected: &str, found: &Token) {
        let message = format!("expected {expected}, found {}", describe(&found.kind));
        self.diagnostics
            .extend_suppressed([Diagnostic::error("E0102", message, found.span)]);
    }

    /// Reports `E0101` for a token that is unexpected here.
    fn report_unexpected_statement(&mut self, found: &Token) {
        self.diagnostics.extend_suppressed([Diagnostic::error(
            "E0101",
            format!(
                "unexpected {} (statements are not implemented yet)",
                describe(&found.kind)
            ),
            found.span,
        )]);
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
fn describe(kind: &TokenKind) -> String {
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
