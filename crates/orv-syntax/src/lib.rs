//! Orvane syntax crate: source map, spans, diagnostics, lexer, AST and parser.
//!
//! M0 provided the foundation types ([`Span`], [`SourceMap`], [`Diagnostic`])
//! plus their `ariadne` rendering; M1 adds the [lexer](lexer); M2 grows the
//! [AST](ast) and [parser](parser) incrementally.

pub mod ast;
pub mod ast_dump;
pub mod diagnostic;
pub mod diagnostics;
pub mod dump;
pub mod lexer;
pub mod parser;
pub mod render;
pub mod source;
pub mod span;

pub use ast::{
    Arg, AssignOp, BinaryOp, Block, DataDecl, EnumDecl, Expr, ExprKind, FieldDecl, FnDecl, Item,
    ItemKind, Literal, MatchArm, Param, Pattern, PatternKind, Program, Stmt, StmtKind, StrSegment,
    Type, TypeKind, UnaryOp, UseDecl, VariantDecl,
};
pub use diagnostic::{Diagnostic, Label, Severity};
pub use diagnostics::Diagnostics;
pub use lexer::{StrPart, Token, TokenKind, lex};
pub use parser::{ParseResult, Parser};
pub use render::{render, render_to_string};
pub use source::{FileId, SourceFile, SourceMap};
pub use span::Span;
