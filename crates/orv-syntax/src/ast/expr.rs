//! Expressions (SPEC §5.2).
//!
//! Every node carries a [`Span`] (SPEC §3.4).

use super::{Block, Pattern, Type};
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

/// One piece of a string literal in the AST.
///
/// The lexer splits a string into [`StrPart`](crate::lexer::StrPart)s; the
/// parser sub-parses each interpolated `{expr}` into a real [`Expr`]
/// (SPEC §5.1). When the token carries a lexical error the raw text is kept as
/// [`StrSegment::Raw`] instead (ADR 0008 rule 4): nothing is re-lexed and the
/// part is never evaluated.
#[derive(Clone, PartialEq, Debug)]
pub enum StrSegment {
    /// Literal text with escapes already resolved.
    Lit(String),
    /// An interpolated `{expr}`, parsed; its span points inside the file.
    ///
    /// `src` is the original text between the braces, kept for the stable AST
    /// dump (ADR 0014) and for diagnostics.
    Expr { src: String, expr: Box<Expr> },
    /// An interpolated `{expr}` kept verbatim because the string token had a
    /// lexical error, so the text may be truncated or hold the bad byte.
    Raw(String),
}

impl StrSegment {
    /// The source text of an interpolation (parsed or raw). Returns `None` for
    /// a literal part.
    pub fn interpolation_src(&self) -> Option<&str> {
        match self {
            StrSegment::Expr { src, .. } | StrSegment::Raw(src) => Some(src),
            StrSegment::Lit(_) => None,
        }
    }
}

/// The expression forms.
#[derive(Clone, PartialEq, Debug)]
pub enum ExprKind {
    /// A literal.
    Literal(Literal),
    /// A bare identifier, including the contextual `py` (SPEC §5.1).
    Ident(String),
    /// `( expr )`.
    Paren(Box<Expr>),
    /// `{ ... }` — a block, whose value is its last expression (§5.4.2).
    Block(Block),
    /// `expr.IDENT` — field access.
    Field { receiver: Box<Expr>, name: String },
    /// `expr?.IDENT` — optional field access.
    OptionalField { receiver: Box<Expr>, name: String },
    /// `expr(arg, ...)` — a call.
    Call { callee: Box<Expr>, args: Vec<Arg> },
    /// `expr[index]` — indexing.
    Index {
        receiver: Box<Expr>,
        index: Box<Expr>,
    },
    /// A prefix operator: `-x` or `not x`.
    Unary { op: UnaryOp, operand: Box<Expr> },
    /// A binary operator application.
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    /// `expr as Type` — an explicit cast (SPEC §5.3).
    Cast { expr: Box<Expr>, ty: Type },
    /// `(a, b)` — a tuple of two or more elements.
    Tuple(Vec<Expr>),
    /// `[a, b, c]` — a list literal.
    List(Vec<Expr>),
    /// `#{k: v, ...}` — a map literal.
    Map(Vec<(Expr, Expr)>),
    /// `if cond { .. } else if .. { .. } else { .. }` — an expression (§5.2).
    If {
        condition: Box<Expr>,
        then_block: Block,
        else_branch: Option<Box<Expr>>,
    },
    /// `match scrutinee { arms }` — an expression (§5.2).
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<MatchArm>,
    },
    /// `try expr` — evaluates to a `Result<T, Failure>` and never propagates.
    Try(Box<Expr>),
    /// `fail expr` — raises a `Failure`.
    Fail(Box<Expr>),
    /// `IDENT => expr` or `(a, b) => expr` — a lambda; the body may be a block.
    Lambda {
        params: Vec<String>,
        body: Box<Expr>,
    },
}

impl ExprKind {
    /// The receiver of a postfix form, if this kind is one.
    ///
    /// Handy for tests and for passes that walk the left spine.
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

/// A call argument, optionally named (`timeout: 3`).
#[derive(Clone, PartialEq, Debug)]
pub struct Arg {
    /// The parameter name for a named argument.
    pub name: Option<String>,
    pub value: Expr,
    pub span: Span,
}

/// A prefix operator (SPEC §5.2.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UnaryOp {
    /// `-`
    Neg,
    /// `not`
    Not,
}

impl UnaryOp {
    /// The source spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            UnaryOp::Neg => "-",
            UnaryOp::Not => "not",
        }
    }
}

/// A binary operator (SPEC §5.2.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BinaryOp {
    /// `??`
    Coalesce,
    /// `or`
    Or,
    /// `and`
    And,
    /// `==`
    Eq,
    /// `!=`
    NotEq,
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
    /// `..`
    Range,
    /// `..=`
    RangeInclusive,
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`
    Div,
    /// `%`
    Rem,
}

impl BinaryOp {
    /// The source spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            BinaryOp::Coalesce => "??",
            BinaryOp::Or => "or",
            BinaryOp::And => "and",
            BinaryOp::Eq => "==",
            BinaryOp::NotEq => "!=",
            BinaryOp::Lt => "<",
            BinaryOp::Le => "<=",
            BinaryOp::Gt => ">",
            BinaryOp::Ge => ">=",
            BinaryOp::Range => "..",
            BinaryOp::RangeInclusive => "..=",
            BinaryOp::Add => "+",
            BinaryOp::Sub => "-",
            BinaryOp::Mul => "*",
            BinaryOp::Div => "/",
            BinaryOp::Rem => "%",
        }
    }

    /// Whether this is a comparison (non-associative in §5.2.1).
    pub const fn is_comparison(self) -> bool {
        matches!(
            self,
            BinaryOp::Eq
                | BinaryOp::NotEq
                | BinaryOp::Lt
                | BinaryOp::Le
                | BinaryOp::Gt
                | BinaryOp::Ge
        )
    }

    /// Whether this is a range operator (non-associative in §5.2.1).
    pub const fn is_range(self) -> bool {
        matches!(self, BinaryOp::Range | BinaryOp::RangeInclusive)
    }
}

/// One arm of a `match`.
#[derive(Clone, PartialEq, Debug)]
pub struct MatchArm {
    pub pattern: Pattern,
    /// The `if` guard, when present.
    pub guard: Option<Expr>,
    pub body: Expr,
    pub span: Span,
}

/// A literal value.
#[derive(Clone, PartialEq, Debug)]
pub enum Literal {
    /// An integer literal, already parsed to `i64` by the lexer.
    Int(i64),
    /// A float literal, already parsed to `f64` by the lexer.
    Float(f64),
    /// A string literal, split into literal and interpolated parts.
    ///
    /// Interpolated parts are parsed into real expressions unless the string
    /// token carried a lexical error (ADR 0008 rule 4), in which case they stay
    /// [`StrSegment::Raw`].
    Str(Vec<StrSegment>),
    /// `true` / `false`.
    Bool(bool),
    /// The `none` literal.
    None,
}
