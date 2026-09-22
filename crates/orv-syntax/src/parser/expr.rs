//! Expression parsing: Pratt precedence (SPEC §5.2.1), postfix and primaries.

use crate::ast::{
    Arg, BinaryOp, Expr, ExprKind, Literal, MatchArm, Pattern, PatternKind, Type, TypeKind, UnaryOp,
};
use crate::lexer::{Keyword, StrPart, TokenKind};
use crate::span::Span;

use super::Parser;

/// Binding power of an infix operator, or `None` when the token is not one.
///
/// The levels are exactly §5.2.1 (menor → maior precedence): `??`, `or`, `and`,
/// comparisons, ranges, `+ -`, `* / %`. `as` (level 9) is handled separately
/// because its right-hand side is a type, not an expression.
fn infix_binding_power(kind: &TokenKind) -> Option<(BinaryOp, u8)> {
    let op = match kind {
        TokenKind::QuestionQuestion => (BinaryOp::Coalesce, 1),
        TokenKind::Kw(Keyword::Or) => (BinaryOp::Or, 2),
        TokenKind::Kw(Keyword::And) => (BinaryOp::And, 3),
        TokenKind::EqEq => (BinaryOp::Eq, 5),
        TokenKind::NotEq => (BinaryOp::NotEq, 5),
        TokenKind::Lt => (BinaryOp::Lt, 5),
        TokenKind::Le => (BinaryOp::Le, 5),
        TokenKind::Gt => (BinaryOp::Gt, 5),
        TokenKind::Ge => (BinaryOp::Ge, 5),
        TokenKind::DotDot => (BinaryOp::Range, 6),
        TokenKind::DotDotEq => (BinaryOp::RangeInclusive, 6),
        TokenKind::Plus => (BinaryOp::Add, 7),
        TokenKind::Minus => (BinaryOp::Sub, 7),
        TokenKind::Star => (BinaryOp::Mul, 8),
        TokenKind::Slash => (BinaryOp::Div, 8),
        TokenKind::Percent => (BinaryOp::Rem, 8),
        _ => return None,
    };
    Some(op)
}

impl Parser<'_> {
    /// Parses an expression (SPEC §5.2 `expr`).
    pub(crate) fn expr(&mut self) -> Option<Expr> {
        if !self.enter() {
            return None;
        }
        let result = self.lambda();
        self.leave();
        result
    }

    /// Parses `lambda | binary`.
    ///
    /// A lambda is recognised by lookahead: `IDENT =>` or `( ... ) =>`
    /// (§5.2: the parentheses must be immediately followed by `=>`).
    fn lambda(&mut self) -> Option<Expr> {
        if let Some(expr) = self.try_lambda() {
            return Some(expr);
        }
        self.binary(1)
    }

    /// Attempts a lambda; returns `None` when the input is not one.
    fn try_lambda(&mut self) -> Option<Expr> {
        // `IDENT =>`
        if let (TokenKind::Ident(name), TokenKind::FatArrow) =
            (self.kind().clone(), self.peek(1).kind.clone())
        {
            let start = self.current().span;
            self.bump(); // the identifier
            self.bump(); // the `=>`
            let body = self.lambda_body()?;
            let span = start.to(body.span);
            return Some(Expr::new(
                ExprKind::Lambda {
                    params: vec![name],
                    body: Box::new(body),
                },
                span,
            ));
        }

        // `( IDENT { "," IDENT } ) =>`
        if self.kind() == &TokenKind::LParen {
            let params = self.lambda_param_list()?;
            let start = self.current().span;
            // `lambda_param_list` leaves the cursor on `=>`.
            self.bump(); // the `=>`
            let body = self.lambda_body()?;
            let span = start.to(body.span);
            return Some(Expr::new(
                ExprKind::Lambda {
                    params,
                    body: Box::new(body),
                },
                span,
            ));
        }

        None
    }

    /// Scans `( [IDENT {"," IDENT}] )` and returns the names **only** when the
    /// next token is `=>`; otherwise rewinds and reports "not a lambda".
    ///
    /// The scan is pure lookahead: it never emits a diagnostic of its own,
    /// because `(1 + 2)` is a legitimate expression, not a failed lambda.
    fn lambda_param_list(&mut self) -> Option<Vec<String>> {
        // The caller guarantees the cursor is on `(`.
        let mut offset = 1;
        let mut params: Vec<String> = Vec::new();

        loop {
            match self.peek(offset).kind.clone() {
                TokenKind::RParen => {
                    offset += 1;
                    break;
                }
                TokenKind::Ident(name) => {
                    params.push(name);
                    offset += 1;
                    match self.peek(offset).kind.clone() {
                        TokenKind::Comma => offset += 1,
                        TokenKind::RParen => {
                            offset += 1;
                            break;
                        }
                        // Not a parameter list, so not a lambda.
                        _ => return None,
                    }
                }
                _ => return None,
            }
        }

        if self.peek(offset).kind != TokenKind::FatArrow {
            return None;
        }
        // Commit: consume `(`, the parameters and `)`.
        for _ in 0..offset {
            self.bump();
        }
        Some(params)
    }

    /// Parses a lambda body: an expression or a block (§5.2 `lambda`).
    fn lambda_body(&mut self) -> Option<Expr> {
        if self.kind() == &TokenKind::LBrace {
            let block = self.block()?;
            let span = block.span;
            return Some(Expr::new(ExprKind::Block(block), span));
        }
        self.expr()
    }

    /// Pratt loop over infix operators, plus the `as` cast at level 9.
    fn binary(&mut self, min_bp: u8) -> Option<Expr> {
        let mut left = self.unary()?;

        loop {
            // `as` binds tighter than any arithmetic op but looser than unary
            // (level 9, left-associative) and is not a `TokenKind` operator, so
            // it gets its own arm.
            if self.kind() == &TokenKind::Kw(Keyword::As) && min_bp <= 9 {
                self.bump();
                let ty = self.type_annotation()?;
                let span = left.span.to(ty.span);
                left = Expr::new(
                    ExprKind::Cast {
                        expr: Box::new(left),
                        ty,
                    },
                    span,
                );
                continue;
            }

            let Some((op, bp)) = infix_binding_power(self.kind()) else {
                return Some(left);
            };
            if bp < min_bp {
                return Some(left);
            }

            self.bump();

            // §5.2.1: comparisons and ranges are non-associative, so the next
            // operator of the same class is rejected instead of chained.
            if op.is_comparison() || op.is_range() {
                let right = self.binary(bp + 1)?;
                if let Some((next_op, _)) = infix_binding_power(self.kind()) {
                    if next_op.is_comparison() || next_op.is_range() {
                        let found = self.current().clone();
                        self.report_expected("an operand", &found);
                        return None;
                    }
                }
                let span = left.span.to(right.span);
                return Some(Expr::new(
                    ExprKind::Binary {
                        op,
                        left: Box::new(left),
                        right: Box::new(right),
                    },
                    span,
                ));
            }

            // Left-associative: same level binds to the left, so the recursive
            // call uses `bp + 1`.
            let right = self.binary(bp + 1)?;
            let span = left.span.to(right.span);
            left = Expr::new(
                ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            );
        }
    }

    /// Parses a prefix operator followed by a unary expression (level 10).
    fn unary(&mut self) -> Option<Expr> {
        let op = match self.kind() {
            TokenKind::Minus => Some(UnaryOp::Neg),
            TokenKind::Kw(Keyword::Not) => Some(UnaryOp::Not),
            _ => None,
        };
        let Some(op) = op else {
            return self.postfix();
        };

        let start = self.current().span;
        self.bump();
        if !self.enter() {
            return None;
        }
        let operand = self.unary();
        self.leave();
        let operand = operand?;
        let span = start.to(operand.span);
        Some(Expr::new(
            ExprKind::Unary {
                op,
                operand: Box::new(operand),
            },
            span,
        ))
    }

    /// Parses a primary followed by any number of postfix forms (SPEC §5.2):
    ///
    /// ```ebnf
    /// postfix = primary { call | index | field | "?." IDENT } ;
    /// ```
    pub(crate) fn postfix(&mut self) -> Option<Expr> {
        let mut expr = self.primary()?;
        loop {
            match self.kind() {
                TokenKind::Dot | TokenKind::QuestionDot => {
                    let optional = self.kind() == &TokenKind::QuestionDot;
                    expr = self.field_access(expr, optional)?;
                }
                TokenKind::LParen => expr = self.call(expr)?,
                TokenKind::LBracket => expr = self.index(expr)?,
                _ => return Some(expr),
            }
        }
    }

    /// Parses `"." IDENT` or `"?." IDENT`, with `receiver` already parsed.
    ///
    /// A missing identifier is `E0102`; this is what rejects `1.2.3`, decided in
    /// ADR 0011 (`field = "." IDENT` cannot derive a numeric literal after `.`).
    fn field_access(&mut self, receiver: Expr, optional: bool) -> Option<Expr> {
        self.bump(); // the `.` or `?.`

        let token = self.current().clone();
        let TokenKind::Ident(name) = token.kind.clone() else {
            let expected = if optional {
                "identifier after `?.`"
            } else {
                "identifier after `.`"
            };
            self.report_expected(expected, &token);
            return None;
        };
        self.bump();

        let span = receiver.span.to(token.span);
        let kind = if optional {
            ExprKind::OptionalField {
                receiver: Box::new(receiver),
                name,
            }
        } else {
            ExprKind::Field {
                receiver: Box::new(receiver),
                name,
            }
        };
        Some(Expr::new(kind, span))
    }

    /// Parses `( [arg { "," arg } [","]] )`, with `callee` already parsed.
    fn call(&mut self, callee: Expr) -> Option<Expr> {
        self.bump(); // the `(`

        let mut args: Vec<Arg> = Vec::new();
        loop {
            let token = self.current().clone();
            match token.kind {
                TokenKind::RParen => {
                    self.bump();
                    let span = callee.span.to(token.span);
                    return Some(Expr::new(
                        ExprKind::Call {
                            callee: Box::new(callee),
                            args,
                        },
                        span,
                    ));
                }
                TokenKind::Eof => {
                    self.report_expected("`)`", &token);
                    return None;
                }
                TokenKind::Comma if args.is_empty() => {
                    self.report_expected("expression", &token);
                    return None;
                }
                _ => {
                    let arg = self.argument()?;
                    args.push(arg);
                    match self.kind() {
                        TokenKind::Comma => {
                            self.bump();
                        }
                        TokenKind::RParen => {
                            let close = self.bump();
                            let span = callee.span.to(close.span);
                            return Some(Expr::new(
                                ExprKind::Call {
                                    callee: Box::new(callee),
                                    args,
                                },
                                span,
                            ));
                        }
                        _ => {
                            let found = self.current().clone();
                            self.report_expected("`,` or `)`", &found);
                            return None;
                        }
                    }
                }
            }
        }
    }

    /// Parses one call argument, which may be named (`timeout: 3`, §5.2).
    fn argument(&mut self) -> Option<Arg> {
        let start = self.current().span;
        // A named argument is `IDENT ":"`; a bare identifier followed by
        // anything else is just an expression starting with that identifier.
        if let (TokenKind::Ident(name), TokenKind::Colon) =
            (self.kind().clone(), self.peek(1).kind.clone())
        {
            self.bump(); // the identifier
            self.bump(); // the `:`
            let value = self.expr()?;
            let span = start.to(value.span);
            return Some(Arg {
                name: Some(name),
                value,
                span,
            });
        }
        let value = self.expr()?;
        let span = start.to(value.span);
        Some(Arg {
            name: None,
            value,
            span,
        })
    }

    /// Parses `[ expr ]`, with `receiver` already parsed.
    fn index(&mut self, receiver: Expr) -> Option<Expr> {
        self.bump(); // the `[`

        let index = self.expr()?;
        let token = self.current().clone();
        if token.kind == TokenKind::RBracket {
            self.bump();
            let span = receiver.span.to(token.span);
            return Some(Expr::new(
                ExprKind::Index {
                    receiver: Box::new(receiver),
                    index: Box::new(index),
                },
                span,
            ));
        }
        self.report_expected("`]`", &token);
        None
    }

    /// Parses a primary expression (SPEC §5.2 `primary`).
    pub(crate) fn primary(&mut self) -> Option<Expr> {
        if !self.enter() {
            return None;
        }
        let result = self.primary_inner();
        self.leave();
        result
    }

    fn primary_inner(&mut self) -> Option<Expr> {
        let token = self.current().clone();
        match token.kind {
            TokenKind::Int(value) => {
                self.bump();
                Some(Expr::new(
                    ExprKind::Literal(Literal::Int(value)),
                    token.span,
                ))
            }
            TokenKind::Float(value) => {
                self.bump();
                Some(Expr::new(
                    ExprKind::Literal(Literal::Float(value)),
                    token.span,
                ))
            }
            TokenKind::Str(parts) => {
                self.bump();
                let parts = self.prepare_str_parts(parts, token.span);
                Some(Expr::new(
                    ExprKind::Literal(Literal::Str(parts)),
                    token.span,
                ))
            }
            TokenKind::Kw(Keyword::True) => {
                self.bump();
                Some(Expr::new(
                    ExprKind::Literal(Literal::Bool(true)),
                    token.span,
                ))
            }
            TokenKind::Kw(Keyword::False) => {
                self.bump();
                Some(Expr::new(
                    ExprKind::Literal(Literal::Bool(false)),
                    token.span,
                ))
            }
            TokenKind::Kw(Keyword::None) => {
                self.bump();
                Some(Expr::new(ExprKind::Literal(Literal::None), token.span))
            }
            TokenKind::Ident(name) => {
                self.bump();
                Some(Expr::new(ExprKind::Ident(name), token.span))
            }
            TokenKind::LParen => self.paren_or_tuple(),
            TokenKind::LBracket => self.list(),
            TokenKind::HashLBrace => self.map(),
            TokenKind::LBrace => {
                let block = self.block()?;
                let span = block.span;
                Some(Expr::new(ExprKind::Block(block), span))
            }
            TokenKind::Kw(Keyword::If) => self.if_expr(),
            TokenKind::Kw(Keyword::Match) => self.match_expr(),
            TokenKind::Kw(Keyword::Try) => {
                self.bump();
                let inner = self.expr()?;
                let span = token.span.to(inner.span);
                Some(Expr::new(ExprKind::Try(Box::new(inner)), span))
            }
            TokenKind::Kw(Keyword::Fail) => {
                self.bump();
                let inner = self.expr()?;
                let span = token.span.to(inner.span);
                Some(Expr::new(ExprKind::Fail(Box::new(inner)), span))
            }
            _ => {
                self.report_expected("expression", &token);
                None
            }
        }
    }

    /// Parses `( expr )` or a tuple `( expr, ... )`.
    fn paren_or_tuple(&mut self) -> Option<Expr> {
        let open = self.current().span;
        self.bump(); // the `(`

        // `()` is the Unit *type*, not an expression primary in §5.2.
        if self.kind() == &TokenKind::RParen {
            let close = self.bump();
            let span = open.to(close.span);
            self.report_expected("expression", &close);
            let _ = span;
            return None;
        }

        let first = self.expr()?;
        if self.kind() == &TokenKind::Comma {
            let mut elements = vec![first];
            while self.eat(&TokenKind::Comma).is_some() {
                if self.kind() == &TokenKind::RParen {
                    break;
                }
                elements.push(self.expr()?);
            }
            let close = self.current().clone();
            if close.kind != TokenKind::RParen {
                self.report_expected("`)`", &close);
                return None;
            }
            self.bump();
            let span = open.to(close.span);
            return Some(Expr::new(ExprKind::Tuple(elements), span));
        }

        let close = self.current().clone();
        if close.kind != TokenKind::RParen {
            self.report_expected("`)`", &close);
            return None;
        }
        self.bump();
        let span = open.to(close.span);
        Some(Expr::new(ExprKind::Paren(Box::new(first)), span))
    }

    /// Parses `[ expr { "," expr } [","] ]`.
    fn list(&mut self) -> Option<Expr> {
        let open = self.current().span;
        self.bump(); // the `[`

        let mut elements: Vec<Expr> = Vec::new();
        loop {
            if self.kind() == &TokenKind::RBracket {
                let close = self.bump();
                let span = open.to(close.span);
                return Some(Expr::new(ExprKind::List(elements), span));
            }
            if self.at_eof() {
                let found = self.current().clone();
                self.report_expected("`]`", &found);
                return None;
            }
            elements.push(self.expr()?);
            match self.kind() {
                TokenKind::Comma => {
                    self.bump();
                }
                TokenKind::RBracket => {}
                _ => {
                    let found = self.current().clone();
                    self.report_expected("`,` or `]`", &found);
                    return None;
                }
            }
        }
    }

    /// Parses `#{ key: value, ... }`.
    fn map(&mut self) -> Option<Expr> {
        let open = self.current().span;
        self.bump(); // the `#{`

        let mut entries: Vec<(Expr, Expr)> = Vec::new();
        loop {
            if self.kind() == &TokenKind::RBrace {
                let close = self.bump();
                let span = open.to(close.span);
                return Some(Expr::new(ExprKind::Map(entries), span));
            }
            if self.at_eof() {
                // An unclosed map literal ran to the end of input: `E0103`,
                // anchored at the `#{` that was never closed.
                let found = self.current().clone();
                self.report(
                    "E0103",
                    "unclosed map literal",
                    open.to(found.span),
                    Some("add `}` to close this map".to_owned()),
                );
                return None;
            }
            let key = self.expr()?;
            if self.kind() != &TokenKind::Colon {
                let found = self.current().clone();
                self.report_expected("`:`", &found);
                return None;
            }
            self.bump();
            let value = self.expr()?;
            entries.push((key, value));
            match self.kind() {
                TokenKind::Comma => {
                    self.bump();
                }
                TokenKind::RBrace => {}
                _ => {
                    let found = self.current().clone();
                    self.report_expected("`,` or `}`", &found);
                    return None;
                }
            }
        }
    }

    /// Parses `if cond block { else if .. } [else block]` as an expression.
    fn if_expr(&mut self) -> Option<Expr> {
        let start = self.current().span;
        self.bump(); // the `if`

        let condition = self.expr()?;
        let then_block = self.block()?;

        // §4.1 writes the `else` on the next line:
        //     let out = if i % 15 == 0 { "FizzBuzz" }
        //               else if i % 3 == 0 { "Fizz" }
        // so a newline before `else` belongs to the construct. The lookahead
        // must not *consume* it when there is no `else`: the newline also
        // terminates the statement, and eating it would make the next
        // statement look unexpected.
        let mut else_branch: Option<Box<Expr>> = None;
        self.skip_newlines_if_else_follows();
        if self.kind() == &TokenKind::Kw(Keyword::Else) {
            self.bump();
            if self.kind() == &TokenKind::Kw(Keyword::If) {
                let nested = self.if_expr()?;
                else_branch = Some(Box::new(nested));
            } else {
                let block = self.block()?;
                let span = block.span;
                else_branch = Some(Box::new(Expr::new(ExprKind::Block(block), span)));
            }
        }

        let end = match &else_branch {
            Some(branch) => branch.span,
            None => then_block.span,
        };
        let span = start.to(end);
        Some(Expr::new(
            ExprKind::If {
                condition: Box::new(condition),
                then_block,
                else_branch,
            },
            span,
        ))
    }

    /// Parses `match scrutinee { arm... }` as an expression.
    fn match_expr(&mut self) -> Option<Expr> {
        let start = self.current().span;
        self.bump(); // the `match`

        let scrutinee = self.expr()?;
        if self.kind() != &TokenKind::LBrace {
            let found = self.current().clone();
            self.report_expected("`{`", &found);
            return None;
        }
        self.bump(); // the `{`

        let mut arms: Vec<MatchArm> = Vec::new();
        loop {
            self.skip_newlines();
            if self.kind() == &TokenKind::RBrace {
                let close = self.bump();
                let span = start.to(close.span);
                return Some(Expr::new(
                    ExprKind::Match {
                        scrutinee: Box::new(scrutinee),
                        arms,
                    },
                    span,
                ));
            }
            if self.at_eof() {
                // An unclosed `match` body: `E0103`, anchored at the `match`.
                let found = self.current().clone();
                self.report(
                    "E0103",
                    "unclosed `match` body",
                    start.to(found.span),
                    Some("add `}` to close this `match`".to_owned()),
                );
                return None;
            }
            let arm_start = self.current().span;
            let pattern = self.pattern()?;
            let guard = if self.kind() == &TokenKind::Kw(Keyword::If) {
                self.bump();
                Some(self.expr()?)
            } else {
                None
            };
            if self.kind() != &TokenKind::FatArrow {
                let found = self.current().clone();
                self.report_expected("`=>`", &found);
                return None;
            }
            self.bump();
            let body = self.expr()?;
            let span = arm_start.to(body.span);
            arms.push(MatchArm {
                pattern,
                guard,
                body,
                span,
            });
            // Arms may be separated by a comma and/or newlines.
            self.skip_newlines();
            if self.kind() == &TokenKind::Comma {
                self.bump();
            }
        }
    }

    /// Parses a pattern (SPEC §5.2).
    pub(crate) fn pattern(&mut self) -> Option<Pattern> {
        let token = self.current().clone();
        match token.kind {
            TokenKind::Ident(name) if name == "_" => {
                self.bump();
                Some(Pattern::new(PatternKind::Wildcard, token.span))
            }
            TokenKind::Ident(name) => {
                self.bump();
                if self.kind() == &TokenKind::LParen {
                    self.bump();
                    let mut fields = Vec::new();
                    loop {
                        if self.kind() == &TokenKind::RParen {
                            break;
                        }
                        fields.push(self.pattern()?);
                        if self.kind() == &TokenKind::Comma {
                            self.bump();
                        } else {
                            break;
                        }
                    }
                    let close = self.current().clone();
                    if close.kind != TokenKind::RParen {
                        self.report_expected("`)`", &close);
                        return None;
                    }
                    self.bump();
                    let span = token.span.to(close.span);
                    return Some(Pattern::new(PatternKind::Variant { name, fields }, span));
                }
                Some(Pattern::new(PatternKind::Bind(name), token.span))
            }
            TokenKind::Int(value) => {
                self.bump();
                Some(Pattern::new(
                    PatternKind::Literal(Literal::Int(value)),
                    token.span,
                ))
            }
            TokenKind::Float(value) => {
                self.bump();
                Some(Pattern::new(
                    PatternKind::Literal(Literal::Float(value)),
                    token.span,
                ))
            }
            TokenKind::Str(parts) => {
                self.bump();
                let parts = self.prepare_str_parts(parts, token.span);
                Some(Pattern::new(
                    PatternKind::Literal(Literal::Str(parts)),
                    token.span,
                ))
            }
            TokenKind::Kw(Keyword::True) => {
                self.bump();
                Some(Pattern::new(
                    PatternKind::Literal(Literal::Bool(true)),
                    token.span,
                ))
            }
            TokenKind::Kw(Keyword::False) => {
                self.bump();
                Some(Pattern::new(
                    PatternKind::Literal(Literal::Bool(false)),
                    token.span,
                ))
            }
            TokenKind::Kw(Keyword::None) => {
                self.bump();
                Some(Pattern::new(
                    PatternKind::Literal(Literal::None),
                    token.span,
                ))
            }
            _ => {
                self.report_expected("pattern", &token);
                None
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
    /// The parts pass through unchanged, which *is* skipping the sub-parse:
    /// nothing here re-lexes `src`. The guard is consulted so the decision is
    /// observable, and it stays the switch a later step must use.
    pub(crate) fn prepare_str_parts(&self, parts: Vec<StrPart>, token_span: Span) -> Vec<StrPart> {
        let _subparse_allowed = self.subparse_allowed(token_span);
        parts
    }
}

/// A type annotation parsed where a value was expected gets its own message.
impl Parser<'_> {
    /// Parses `from`-style `as Type`, reporting `E0102` when no type follows.
    fn type_annotation(&mut self) -> Option<Type> {
        let token = self.current().clone();
        if let TokenKind::Ident(name) = token.kind.clone() {
            if name == "Py" {
                self.bump();
                return Some(Type::new(TypeKind::Py, token.span));
            }
        }
        self.parse_type()
    }
}
