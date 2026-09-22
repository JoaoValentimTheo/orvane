//! The type model (SPEC §5.3).
//!
//! Only what the alpha runtime can represent: primitives, `List`, `Map`,
//! tuples, optionals, `Result`, functions and user types (`data`/`enum`).
//! `Py` exists because the parser accepts the annotation, but the alpha has no
//! host, so a `Py` value can never be produced (ADR 0012).

use std::fmt;

use orv_syntax::{Span, Type as AstType, TypeKind};

/// A resolved type.
#[derive(Clone, PartialEq, Debug)]
pub enum Ty {
    /// `Int`
    Int,
    /// `Float`
    Float,
    /// `Bool`
    Bool,
    /// `Str`
    Str,
    /// `()` — Unit.
    Unit,
    /// `List<T>`
    List(Box<Ty>),
    /// `Map<K, V>`
    Map(Box<Ty>, Box<Ty>),
    /// `(A, B, …)` — a tuple of two or more.
    Tuple(Vec<Ty>),
    /// `T?` — an optional.
    Optional(Box<Ty>),
    /// `Result<T, Failure>`.
    Result(Box<Ty>),
    /// `fn(A) -> B`.
    Fn { params: Vec<Ty>, ret: Box<Ty> },
    /// A user type declared with `data`.
    Data(String),
    /// A user type declared with `enum`.
    Enum(String),
    /// `Py` — parsed but unimplemented in the alpha.
    Py,
    /// The type of an expression that already failed to check.
    ///
    /// Its only purpose is to stop a single mistake from producing a cascade:
    /// `Unknown` is compatible with everything.
    Unknown,
}

impl Ty {
    /// Whether this type is [`Ty::Unknown`].
    pub const fn is_unknown(&self) -> bool {
        matches!(self, Ty::Unknown)
    }

    /// Whether a value of this type may be used as `Bool` (§5.3: `if` requires
    /// `Bool`, there is no truthiness).
    pub const fn is_bool(&self) -> bool {
        matches!(self, Ty::Bool)
    }

    /// Whether this is a numeric type.
    pub const fn is_numeric(&self) -> bool {
        matches!(self, Ty::Int | Ty::Float)
    }

    /// A human-readable name, for diagnostics.
    pub fn name(&self) -> String {
        self.to_string()
    }
}

impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ty::Int => f.write_str("Int"),
            Ty::Float => f.write_str("Float"),
            Ty::Bool => f.write_str("Bool"),
            Ty::Str => f.write_str("Str"),
            Ty::Unit => f.write_str("()"),
            Ty::List(element) => write!(f, "List<{element}>"),
            Ty::Map(key, value) => write!(f, "Map<{key}, {value}>"),
            Ty::Tuple(elements) => {
                let inner: Vec<String> = elements.iter().map(Ty::name).collect();
                write!(f, "({})", inner.join(", "))
            }
            Ty::Optional(inner) => write!(f, "{inner}?"),
            Ty::Result(ok) => write!(f, "Result<{ok}, Failure>"),
            Ty::Fn { params, ret } => {
                let inner: Vec<String> = params.iter().map(Ty::name).collect();
                write!(f, "fn({}) -> {ret}", inner.join(", "))
            }
            Ty::Data(name) | Ty::Enum(name) => f.write_str(name),
            Ty::Py => f.write_str("Py"),
            Ty::Unknown => f.write_str("?"),
        }
    }
}

/// Converts a parsed type annotation into a [`Ty`].
///
/// A single type argument is required for `List`; `Map` takes two. Anything
/// else is reported by the caller through the returned error.
pub fn resolve_type(ast: &AstType) -> Result<Ty, TypeProblem> {
    match &ast.kind {
        TypeKind::Py => Ok(Ty::Py),
        TypeKind::Unit => Ok(Ty::Unit),
        TypeKind::Optional(inner) => Ok(Ty::Optional(Box::new(resolve_type(inner)?))),
        TypeKind::Tuple(elements) => {
            let resolved: Result<Vec<Ty>, TypeProblem> =
                elements.iter().map(resolve_type).collect();
            let resolved = resolved?;
            if resolved.len() < 2 {
                return Err(TypeProblem::TupleTooShort);
            }
            Ok(Ty::Tuple(resolved))
        }
        TypeKind::Fn { params, ret } => {
            let params: Result<Vec<Ty>, TypeProblem> = params.iter().map(resolve_type).collect();
            Ok(Ty::Fn {
                params: params?,
                ret: Box::new(resolve_type(ret)?),
            })
        }
        TypeKind::Named { name, args } => resolve_named(name, args, ast.span),
    }
}

/// Resolves a named type, checking its generic arity.
fn resolve_named(name: &str, args: &[AstType], span: Span) -> Result<Ty, TypeProblem> {
    match (name, args.len()) {
        ("Int", 0) => Ok(Ty::Int),
        ("Float", 0) => Ok(Ty::Float),
        ("Bool", 0) => Ok(Ty::Bool),
        ("Str", 0) => Ok(Ty::Str),
        ("Py", 0) => Ok(Ty::Py),
        ("Failure", 0) => Ok(Ty::Data("Failure".to_owned())),
        ("List", 1) => Ok(Ty::List(Box::new(resolve_type(&args[0])?))),
        ("Map", 2) => Ok(Ty::Map(
            Box::new(resolve_type(&args[0])?),
            Box::new(resolve_type(&args[1])?),
        )),
        ("Result", 1) => Ok(Ty::Result(Box::new(resolve_type(&args[0])?))),
        // A builtin used with the wrong number of arguments.
        ("Int" | "Float" | "Bool" | "Str" | "Py" | "Failure", n) if n > 0 => {
            Err(TypeProblem::WrongArity {
                name: name.to_owned(),
                expected: 0,
                found: n,
                span,
            })
        }
        ("List", n) => Err(TypeProblem::WrongArity {
            name: name.to_owned(),
            expected: 1,
            found: n,
            span,
        }),
        ("Map", n) => Err(TypeProblem::WrongArity {
            name: name.to_owned(),
            expected: 2,
            found: n,
            span,
        }),
        ("Result", n) => Err(TypeProblem::WrongArity {
            name: name.to_owned(),
            expected: 1,
            found: n,
            span,
        }),
        // A user type: resolved against the declarations by the caller, which
        // is why the name is carried through unresolved here.
        _ => {
            if args.is_empty() {
                Ok(Ty::Data(name.to_owned()))
            } else {
                Err(TypeProblem::GenericsUnsupported { span })
            }
        }
    }
}

/// A problem found while resolving a type annotation.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TypeProblem {
    /// A builtin was given the wrong number of type arguments.
    WrongArity {
        name: String,
        expected: usize,
        found: usize,
        span: Span,
    },
    /// A tuple with fewer than two elements.
    TupleTooShort,
    /// Generic user types are out of the alpha (ADR 0012).
    GenericsUnsupported { span: Span },
}

#[cfg(test)]
mod tests {
    use super::*;
    use orv_syntax::{FileId, Type, TypeKind};

    fn named(name: &str, args: Vec<Type>) -> Type {
        Type::new(
            TypeKind::Named {
                name: name.to_owned(),
                args,
            },
            Span::point(FileId(0), 0),
        )
    }

    #[test]
    fn resolves_primitives() {
        for (name, expected) in [
            ("Int", Ty::Int),
            ("Float", Ty::Float),
            ("Bool", Ty::Bool),
            ("Str", Ty::Str),
            ("Py", Ty::Py),
        ] {
            assert_eq!(resolve_type(&named(name, vec![])), Ok(expected.clone()));
        }
    }

    #[test]
    fn resolves_unit_and_tuple() {
        let unit = Type::new(TypeKind::Unit, Span::point(FileId(0), 0));
        assert_eq!(resolve_type(&unit), Ok(Ty::Unit));

        let tuple = Type::new(
            TypeKind::Tuple(vec![named("Int", vec![]), named("Str", vec![])]),
            Span::point(FileId(0), 0),
        );
        assert_eq!(resolve_type(&tuple), Ok(Ty::Tuple(vec![Ty::Int, Ty::Str])));
    }

    #[test]
    fn resolves_containers() {
        assert_eq!(
            resolve_type(&named("List", vec![named("Int", vec![])])),
            Ok(Ty::List(Box::new(Ty::Int)))
        );
        assert_eq!(
            resolve_type(&named(
                "Map",
                vec![named("Str", vec![]), named("Int", vec![])]
            )),
            Ok(Ty::Map(Box::new(Ty::Str), Box::new(Ty::Int)))
        );
    }

    #[test]
    fn resolves_optional() {
        let optional = Type::new(
            TypeKind::Optional(Box::new(named("Int", vec![]))),
            Span::point(FileId(0), 0),
        );
        assert_eq!(resolve_type(&optional), Ok(Ty::Optional(Box::new(Ty::Int))));
    }

    #[test]
    fn resolves_fn_type() {
        let fn_ty = Type::new(
            TypeKind::Fn {
                params: vec![named("Int", vec![])],
                ret: Box::new(named("Str", vec![])),
            },
            Span::point(FileId(0), 0),
        );
        assert_eq!(
            resolve_type(&fn_ty),
            Ok(Ty::Fn {
                params: vec![Ty::Int],
                ret: Box::new(Ty::Str),
            })
        );
    }

    #[test]
    fn rejects_wrong_generic_arity() {
        let problem = resolve_type(&named("List", vec![]));
        assert!(matches!(
            problem,
            Err(TypeProblem::WrongArity { ref name, .. }) if name == "List"
        ));
    }

    #[test]
    fn rejects_generics_on_user_types() {
        let problem = resolve_type(&named("Box", vec![named("Int", vec![])]));
        assert!(matches!(
            problem,
            Err(TypeProblem::GenericsUnsupported { .. })
        ));
    }

    #[test]
    fn unknown_is_compatible_and_named() {
        assert!(Ty::Unknown.is_unknown());
        assert_eq!(Ty::Unknown.name(), "?");
        assert!(Ty::Bool.is_bool());
        assert!(Ty::Int.is_numeric());
        assert!(!Ty::Str.is_numeric());
    }

    #[test]
    fn display_is_stable() {
        assert_eq!(Ty::List(Box::new(Ty::Int)).to_string(), "List<Int>");
        assert_eq!(
            Ty::Map(Box::new(Ty::Str), Box::new(Ty::Bool)).to_string(),
            "Map<Str, Bool>"
        );
        assert_eq!(Ty::Optional(Box::new(Ty::Int)).to_string(), "Int?");
        assert_eq!(
            Ty::Fn {
                params: vec![Ty::Int],
                ret: Box::new(Ty::Str)
            }
            .to_string(),
            "fn(Int) -> Str"
        );
    }
}
