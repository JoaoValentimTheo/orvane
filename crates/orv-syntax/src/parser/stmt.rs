//! Statement and block parsing (SPEC §5.2).
//!
//! Error recovery: a failed statement reports a diagnostic and then skips to the
//! next `Newline` or `}` so the rest of the file is still parsed (M2 gate:
//! "múltiplos erros por arquivo").

use crate::ast::{AssignOp, Block, Pattern, PatternKind, Stmt, StmtKind, Type};
use crate::lexer::{Keyword, TokenKind};

use super::Parser;

impl Parser<'_> {
    /// Parses `{ { NEWLINE | stmt } }` (SPEC §5.2 `block`).
    ///
    /// A failed statement does not abort the block: the parser resynchronises
    /// and keeps going, so one mistake yields one diagnostic, not a cascade.
    pub(crate) fn block(&mut self) -> Option<Block> {
        if self.kind() != &TokenKind::LBrace {
            let found = self.current().clone();
            self.report_expected("`{`", &found);
            return None;
        }
        let open = self.bump();

        let mut statements: Vec<Stmt> = Vec::new();
        loop {
            self.skip_newlines();
            // An empty statement (`;`) is layout, not content.
            if self.kind() == &TokenKind::Semicolon {
                self.bump();
                continue;
            }
            match self.kind() {
                TokenKind::RBrace => {
                    let close = self.bump();
                    let mut block = Block::new(open.span.to(close.span));
                    block.statements = statements;
                    return Some(block);
                }
                TokenKind::Eof => {
                    let found = self.current().clone();
                    self.report_expected("`}`", &found);
                    let mut block = Block::new(open.span.to(found.span));
                    block.statements = statements;
                    return Some(block);
                }
                _ => {}
            }

            match self.statement() {
                Some(statement) => {
                    let terminated = self.at_statement_end();
                    statements.push(statement);
                    if !terminated {
                        let found = self.current().clone();
                        self.report_expected("end of statement", &found);
                        self.synchronize();
                    } else {
                        self.skip_newlines();
                    }
                }
                None => self.synchronize(),
            }
        }
    }

    /// Whether the cursor is at something that ends a statement.
    fn at_statement_end(&self) -> bool {
        matches!(
            self.kind(),
            TokenKind::Newline | TokenKind::Semicolon | TokenKind::RBrace | TokenKind::Eof
        )
    }

    /// Skips tokens until the next statement boundary, to recover.
    ///
    /// Stops *before* the boundary so the block loop can consume it, and at
    /// `Eof` so it always terminates.
    fn synchronize(&mut self) {
        // Always make progress, even if the cursor is already on a boundary
        // that the block loop will not consume (a stray `}` is left for the
        // caller; anything else is skipped).
        let start = self.cursor_offset();
        while !self.at_statement_end() {
            self.bump();
            if self.cursor_offset() == start {
                break;
            }
        }
        // Consume a statement terminator if present, so the next iteration
        // starts on a real statement.
        if matches!(self.kind(), TokenKind::Newline | TokenKind::Semicolon) {
            self.bump();
        }
    }

    /// The raw cursor index, used to guarantee `synchronize` makes progress.
    fn cursor_offset(&self) -> usize {
        self.cursor_index()
    }

    /// Parses one statement (SPEC §5.2 `stmt`).
    fn statement(&mut self) -> Option<Stmt> {
        if !self.enter() {
            return None;
        }
        let result = self.statement_inner();
        self.leave();
        result
    }

    fn statement_inner(&mut self) -> Option<Stmt> {
        let token = self.current().clone();
        match token.kind {
            TokenKind::Kw(Keyword::Let) => self.let_stmt(),
            TokenKind::Kw(Keyword::While) => self.while_stmt(),
            TokenKind::Kw(Keyword::For) => self.for_stmt(),
            TokenKind::Kw(Keyword::Return) => {
                self.bump();
                if self.at_statement_end() {
                    let span = token.span;
                    return Some(Stmt::new(StmtKind::Return(None), span));
                }
                let value = self.expr()?;
                let span = token.span.to(value.span);
                Some(Stmt::new(StmtKind::Return(Some(value)), span))
            }
            TokenKind::Kw(Keyword::Break) => {
                self.bump();
                Some(Stmt::new(StmtKind::Break, token.span))
            }
            TokenKind::Kw(Keyword::Continue) => {
                self.bump();
                Some(Stmt::new(StmtKind::Continue, token.span))
            }
            TokenKind::Kw(Keyword::Fail) => {
                self.bump();
                let value = self.expr()?;
                let span = token.span.to(value.span);
                Some(Stmt::new(StmtKind::Fail(value), span))
            }
            // `intent`, `how`, `test` and `use py` are out of the alpha scope
            // (ADR 0012); naming them gives a clear message instead of
            // "expected expression".
            TokenKind::Kw(Keyword::Intent | Keyword::How | Keyword::Test) => {
                self.report(
                    "E0105",
                    format!(
                        "`{}` is not supported in 0.1.0-alpha",
                        describe_keyword(&token.kind)
                    ),
                    token.span,
                    Some("intents land in a later milestone (ADR 0012)".to_owned()),
                );
                None
            }
            _ => {
                let expr = self.expr()?;
                // An assignment is an expression followed by an assignment
                // operator; §5.2 keeps `assign_stmt` separate from `expr`, so
                // the statement form is recognised here.
                if let Some(op) = self.assignment_op() {
                    self.bump(); // the operator
                    let value = self.expr()?;
                    let span = expr.span.to(value.span);
                    return Some(Stmt::new(
                        StmtKind::Assign {
                            target: expr,
                            op,
                            value,
                        },
                        span,
                    ));
                }
                let span = expr.span;
                Some(Stmt::new(StmtKind::Expr(expr), span))
            }
        }
    }

    /// The assignment operator under the cursor, if any.
    fn assignment_op(&self) -> Option<AssignOp> {
        match self.kind() {
            TokenKind::Eq => Some(AssignOp::Assign),
            TokenKind::PlusEq => Some(AssignOp::Add),
            TokenKind::MinusEq => Some(AssignOp::Sub),
            TokenKind::StarEq => Some(AssignOp::Mul),
            TokenKind::SlashEq => Some(AssignOp::Div),
            _ => None,
        }
    }

    /// Parses `let [mut] IDENT [: Type] = expr`.
    fn let_stmt(&mut self) -> Option<Stmt> {
        let start = self.bump().span; // the `let`

        let mutable = self.eat(&TokenKind::Kw(Keyword::Mut)).is_some();

        let name_token = self.current().clone();
        let TokenKind::Ident(name) = name_token.kind.clone() else {
            self.report_expected("identifier", &name_token);
            return None;
        };
        self.bump();
        let mut pattern = Pattern::new(PatternKind::Bind(name), name_token.span);

        let ty = if self.kind() == &TokenKind::Colon {
            self.bump();
            Some(self.type_annotation_stmt()?)
        } else {
            None
        };

        if self.kind() != &TokenKind::Eq {
            let found = self.current().clone();
            self.report_expected("`=`", &found);
            self.synchronize();
            return None;
        }
        self.bump();

        let value = self.expr()?;
        let span = start.to(value.span);
        pattern.span = name_token.span;
        Some(Stmt::new(
            StmtKind::Let {
                pattern,
                mutable,
                ty,
                value,
            },
            span,
        ))
    }

    /// Parses `while cond block`.
    fn while_stmt(&mut self) -> Option<Stmt> {
        let start = self.bump().span; // the `while`
        let condition = self.expr()?;
        let body = self.block()?;
        let span = start.to(body.span);
        Some(Stmt::new(StmtKind::While { condition, body }, span))
    }

    /// Parses `for IDENT in iterable block`.
    fn for_stmt(&mut self) -> Option<Stmt> {
        let start = self.bump().span; // the `for`

        let name_token = self.current().clone();
        let TokenKind::Ident(name) = name_token.kind.clone() else {
            self.report_expected("identifier", &name_token);
            return None;
        };
        self.bump();
        let pattern = Pattern::new(PatternKind::Bind(name), name_token.span);

        if self.kind() != &TokenKind::Kw(Keyword::In) {
            let found = self.current().clone();
            self.report_expected("`in`", &found);
            self.synchronize();
            return None;
        }
        self.bump();

        let iterable = self.expr()?;
        let body = self.block()?;
        let span = start.to(body.span);
        Some(Stmt::new(
            StmtKind::For {
                pattern,
                iterable,
                body,
            },
            span,
        ))
    }

    /// Parses a type annotation in statement position.
    fn type_annotation_stmt(&mut self) -> Option<Type> {
        self.annotation_type()
    }
}

/// The spelling of a keyword, for diagnostics.
fn describe_keyword(kind: &TokenKind) -> &'static str {
    match kind {
        TokenKind::Kw(keyword) => keyword.as_str(),
        _ => "?",
    }
}
