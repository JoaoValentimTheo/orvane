//! Abstract syntax tree (SPEC §5.2).
//!
//! Grown incrementally; the 0.1.0-alpha scope is recorded in ADR 0012. Every node
//! carries a [`Span`] (SPEC §3.4: "`Span` em **todo** nó de AST").
//!
//! Layout:
//! * [`expr`] — expressions;
//! * [`stmt`] — statements and blocks;
//! * [`item`] — top-level items and the program root;
//! * [`types`] — type annotations and patterns.

mod expr;
mod item;
mod stmt;
mod types;

pub use expr::{Arg, BinaryOp, Expr, ExprKind, Literal, MatchArm, StrSegment, UnaryOp};
pub use item::{
    DataDecl, EnumDecl, FieldDecl, FnDecl, Item, ItemKind, Param, Program, UseDecl, VariantDecl,
};
pub use stmt::{AssignOp, Block, Stmt, StmtKind};
pub use types::{Pattern, PatternKind, Type, TypeKind};
