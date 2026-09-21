//! Orvane syntax crate: source map, spans, diagnostics, lexer, AST and parser.
//!
//! M0 provided the foundation types ([`Span`], [`SourceMap`], [`Diagnostic`])
//! plus their `ariadne` rendering; M1 adds the [lexer](lexer). The parser lands
//! in M2.

pub mod diagnostic;
pub mod diagnostics;
pub mod dump;
pub mod lexer;
pub mod render;
pub mod source;
pub mod span;

pub use diagnostic::{Diagnostic, Label, Severity};
pub use diagnostics::Diagnostics;
pub use lexer::{StrPart, Token, TokenKind, lex};
pub use render::{render, render_to_string};
pub use source::{FileId, SourceFile, SourceMap};
pub use span::Span;
