//! Items and the program root (SPEC §5.2).
//!
//! `intent`, `how`, `test` and `use py` are deliberately absent: they are out of
//! the 0.1.0-alpha scope (ADR 0012) and are not parsed, so no semantics is
//! reserved that the runtime does not implement.

use super::{Block, Type};
use crate::span::Span;

/// A whole source file.
#[derive(Clone, PartialEq, Debug)]
pub struct Program {
    pub items: Vec<Item>,
    pub span: Span,
}

/// A top-level item.
#[derive(Clone, PartialEq, Debug)]
pub struct Item {
    pub kind: ItemKind,
    pub span: Span,
}

impl Item {
    /// Creates an item node.
    pub const fn new(kind: ItemKind, span: Span) -> Self {
        Self { kind, span }
    }
}

/// The item forms in the alpha.
#[derive(Clone, PartialEq, Debug)]
pub enum ItemKind {
    /// `[pub] fn NAME(params) [-> Type] block`.
    Fn(FnDecl),
    /// `[pub] data NAME { field, ... }`.
    Data(DataDecl),
    /// `[pub] enum NAME { Variant, ... }`.
    Enum(EnumDecl),
    /// `use path [as IDENT]` — a file module import (§5.6, minimal).
    Use(UseDecl),
}

/// A `fn` declaration.
#[derive(Clone, PartialEq, Debug)]
pub struct FnDecl {
    pub name: String,
    pub params: Vec<Param>,
    /// The annotated return type, if any. Absent means `()` (§5.3).
    pub ret: Option<Type>,
    pub body: Block,
    pub public: bool,
    /// Span of the signature, from `fn` through the return type.
    pub signature_span: Span,
}

/// A function parameter.
#[derive(Clone, PartialEq, Debug)]
pub struct Param {
    pub name: String,
    pub ty: Type,
    /// A default value, when present.
    pub default: Option<super::Expr>,
    pub span: Span,
}

/// A `data` declaration.
#[derive(Clone, PartialEq, Debug)]
pub struct DataDecl {
    pub name: String,
    pub fields: Vec<FieldDecl>,
    pub public: bool,
    pub span: Span,
}

/// A field of a `data` declaration.
#[derive(Clone, PartialEq, Debug)]
pub struct FieldDecl {
    pub name: String,
    pub ty: Type,
    /// A default value, when present.
    pub default: Option<super::Expr>,
    pub span: Span,
}

/// An `enum` declaration.
#[derive(Clone, PartialEq, Debug)]
pub struct EnumDecl {
    pub name: String,
    pub variants: Vec<VariantDecl>,
    pub public: bool,
    pub span: Span,
}

/// A variant of an `enum`.
#[derive(Clone, PartialEq, Debug)]
pub struct VariantDecl {
    pub name: String,
    /// Positional payload types, empty for a unit variant.
    pub payload: Vec<Type>,
    pub span: Span,
}

/// A minimal `use` declaration (file modules only; `use py` is out of scope).
#[derive(Clone, PartialEq, Debug)]
pub struct UseDecl {
    /// The dotted path, e.g. `["util", "math"]`.
    pub path: Vec<String>,
    /// The `as` alias, when present.
    pub alias: Option<String>,
    pub span: Span,
}
