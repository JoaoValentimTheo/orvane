//! Types and patterns (SPEC §5.2, §5.3).

use crate::span::Span;

/// A type annotation (SPEC §5.2 `type`, §5.3).
#[derive(Clone, PartialEq, Debug)]
pub struct Type {
    pub kind: TypeKind,
    pub span: Span,
}

impl Type {
    /// Creates a type node.
    pub const fn new(kind: TypeKind, span: Span) -> Self {
        Self { kind, span }
    }
}

/// The type forms of v0.1.
#[derive(Clone, PartialEq, Debug)]
pub enum TypeKind {
    /// A named type, possibly generic: `Int`, `Str`, `List<Int>`, `User`.
    Named { name: String, args: Vec<Type> },
    /// `T?` — an optional.
    Optional(Box<Type>),
    /// `()` — Unit.
    Unit,
    /// `(A, B)` — a tuple of two or more.
    Tuple(Vec<Type>),
    /// `fn(A) -> B`.
    Fn { params: Vec<Type>, ret: Box<Type> },
    /// `Py` — the host dynamic type (parsed, not usable in the alpha; §6).
    Py,
}

/// A pattern (SPEC §5.2).
#[derive(Clone, PartialEq, Debug)]
pub struct Pattern {
    pub kind: PatternKind,
    pub span: Span,
}

impl Pattern {
    /// Creates a pattern node.
    pub const fn new(kind: PatternKind, span: Span) -> Self {
        Self { kind, span }
    }
}

/// The pattern forms of v0.1.
#[derive(Clone, PartialEq, Debug)]
pub enum PatternKind {
    /// `_` — matches anything, binds nothing.
    Wildcard,
    /// A literal pattern, compared structurally.
    Literal(super::Literal),
    /// A bare identifier: binds the whole value.
    Bind(String),
    /// `Name(p1, p2)` — a variant or `data` constructor pattern.
    Variant { name: String, fields: Vec<Pattern> },
}
