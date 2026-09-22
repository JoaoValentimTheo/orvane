//! Orvane semantic analysis: name resolution and type checking (SPEC §5.3).
//!
//! The 0.1.0-alpha scope is recorded in ADR 0012: primitives, `List`, `Map`,
//! tuples, optionals, functions, closures, `data`/`enum` and `as`. Generics on
//! user types, modules, `intent` checking and Python interop are out.
//!
//! The checker never panics: a failed sub-expression becomes
//! [`ty::Ty::Unknown`], which is compatible with everything, so one mistake
//! yields one diagnostic instead of a cascade.

#![forbid(unsafe_code)]

pub mod check;
pub mod scopes;
pub mod ty;

pub use check::{CheckResult, Checker, EnumVariant, UserType, check};
pub use scopes::{Scopes, Symbol, SymbolKind};
pub use ty::{Ty, TypeProblem, resolve_type};

pub use orv_syntax::Diagnostic;
