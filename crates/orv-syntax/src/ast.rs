//! Abstract syntax tree (SPEC §5.2).
//!
//! M2 grows this incrementally. This step covers only what a *primary*
//! expression needs: literals, identifiers, a parenthesised expression and a
//! block skeleton. Calls, indexing, field access, operators, `if`/`match` and
//! the full statement grammar are deliberately absent (they arrive in later
//! steps, per the M2 roteiro).
//!
//! Every node carries a [`Span`] (SPEC §3.4: "`Span` em **todo** nó de AST").

use crate::lexer::StrPart;
use crate::span::Span;

/// An expression.
#[derive(Clone, PartialEq, Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

impl Expr {
    /// Creates an expression node.
    pub const fn new(kind: ExprKind, span: Span) -> Self {
        Self { kind, span }
    }
}

/// The expression forms implemented so far.
#[derive(Clone, PartialEq, Debug)]
pub enum ExprKind {
    /// A literal.
    Literal(Literal),
    /// A bare identifier, including the contextual `py` (SPEC §5.1).
    Ident(String),
    /// `( expr )`.
    Paren(Box<Expr>),
    /// `{ ... }` — a block *skeleton*.
    Block(Block),
    /// `expr.IDENT` — field access.
    Field { receiver: Box<Expr>, name: String },
    /// `expr?.IDENT` — optional field access.
    OptionalField { receiver: Box<Expr>, name: String },
    /// `expr(arg, ...)` — a call.
    ///
    /// Arguments are parsed as expressions, which in this step means primaries
    /// (and their postfix chains); operators arrive with the Pratt step.
    Call { callee: Box<Expr>, args: Vec<Expr> },
    /// `expr[index]` — indexing.
    Index {
        receiver: Box<Expr>,
        index: Box<Expr>,
    },
}

impl ExprKind {
    /// The receiver of a postfix form, if this kind is one.
    ///
    /// Handy for tests and for later passes that walk the left spine.
    pub fn receiver(&self) -> Option<&Expr> {
        match self {
            ExprKind::Field { receiver, .. }
            | ExprKind::OptionalField { receiver, .. }
            | ExprKind::Call {
                callee: receiver, ..
            }
            | ExprKind::Index { receiver, .. } => Some(receiver),
            _ => None,
        }
    }
}

/// A literal value.
#[derive(Clone, PartialEq, Debug)]
pub enum Literal {
    /// An integer literal, already parsed to `i64` by the lexer.
    Int(i64),
    /// A float literal, already parsed to `f64` by the lexer.
    Float(f64),
    /// A string literal, with its parts as the lexer produced them.
    ///
    /// A part of kind [`StrPart::Expr`] is sub-parsed only when the token has no
    /// lexical error (ADR 0008 amend, rule 4); the parser guards that with
    /// [`Diagnostics::should_subparse_expr`](crate::Diagnostics::should_subparse_expr).
    Str(Vec<StrPart>),
    /// `true` / `false`.
    Bool(bool),
    /// The `none` literal.
    None,
}

/// A `{ ... }` block.
///
/// A skeleton for now: the statement grammar (`let`, `return`, assignment, …) is
/// out of scope for this step, so the body carries whatever statements a later
/// step can represent, which today is nothing.
#[derive(Clone, PartialEq, Debug)]
pub struct Block {
    /// Statements in source order.
    pub statements: Vec<Stmt>,
    /// Span of the whole block, including both braces.
    pub span: Span,
}

impl Block {
    /// Creates an empty block with the given span.
    pub const fn new(span: Span) -> Self {
        Self {
            statements: Vec::new(),
            span,
        }
    }
}

/// A statement placeholder.
///
/// M2's statement grammar is not implemented yet. The type exists so [`Block`]
/// has a concrete element type and a later step can fill it in without changing
/// the shape of the AST.
#[derive(Clone, PartialEq, Debug)]
pub struct Stmt {
    pub span: Span,
}
