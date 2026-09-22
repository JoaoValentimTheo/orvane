//! Statements and blocks (SPEC §5.2).
//!
//! Every node carries a [`Span`] (SPEC §3.4).

use super::{Expr, Pattern, Type};
use crate::span::Span;

/// A `{ ... }` block.
///
/// A block's value is that of its last expression, if the last statement is an
/// expression (§5.4.2: "Corpo sem `return` devolve o valor da última expressão").
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

    /// The trailing expression, when the block ends in one.
    pub fn tail_expr(&self) -> Option<&Expr> {
        match self.statements.last() {
            Some(Stmt {
                kind: StmtKind::Expr(expr),
                ..
            }) => Some(expr),
            _ => None,
        }
    }
}

/// A statement.
#[derive(Clone, PartialEq, Debug)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

impl Stmt {
    /// Creates a statement node.
    pub const fn new(kind: StmtKind, span: Span) -> Self {
        Self { kind, span }
    }
}

/// The statement forms.
#[derive(Clone, PartialEq, Debug)]
pub enum StmtKind {
    /// `let [mut] IDENT [: Type] = expr`.
    Let {
        pattern: Pattern,
        mutable: bool,
        ty: Option<Type>,
        value: Expr,
    },
    /// `lvalue (= | += | -= | *= | /=) expr`.
    Assign {
        target: Expr,
        op: AssignOp,
        value: Expr,
    },
    /// `while cond block`.
    While { condition: Expr, body: Block },
    /// `for IDENT in iterable block`.
    For {
        pattern: Pattern,
        iterable: Expr,
        body: Block,
    },
    /// `return [expr]`.
    Return(Option<Expr>),
    /// `break`.
    Break,
    /// `continue`.
    Continue,
    /// `fail expr` used as a statement.
    Fail(Expr),
    /// An expression evaluated for its value or effects.
    Expr(Expr),
}

/// The assignment operators (SPEC §5.2).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AssignOp {
    /// `=`
    Assign,
    /// `+=`
    Add,
    /// `-=`
    Sub,
    /// `*=`
    Mul,
    /// `/=`
    Div,
}

impl AssignOp {
    /// The source spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            AssignOp::Assign => "=",
            AssignOp::Add => "+=",
            AssignOp::Sub => "-=",
            AssignOp::Mul => "*=",
            AssignOp::Div => "/=",
        }
    }
}
