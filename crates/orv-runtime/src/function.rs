//! Function values: named `fn`s and lambdas.

use std::fmt;
use std::rc::Rc;

use orv_syntax::{Block, Expr, FnDecl};

use crate::env::Env;

/// A callable value.
///
/// A named function and a lambda differ only in where their body comes from and
/// whether they capture an environment, so they share one type.
#[derive(Debug)]
pub enum Callable {
    /// A top-level `fn`.
    Named(Rc<FnDecl>),
    /// A builtin implemented in Rust (the prelude, SPEC §5.6).
    Builtin,
    /// A lambda: parameters plus a body expression (which may be a block).
    Lambda {
        params: Rc<Vec<String>>,
        body: Rc<Expr>,
        captured: Env,
    },
}

/// A function value with its name (for diagnostics) and arity.
#[derive(Debug)]
pub struct Closure {
    /// The name shown in `<fn name>` and in stack frames.
    pub name: Rc<str>,
    /// How many parameters the callable takes.
    pub arity: usize,
    /// The body.
    pub callable: Callable,
}

impl Closure {
    /// Creates a builtin closure; its body lives in
    /// [`crate::builtins::lookup`].
    pub fn builtin(name: &str, arity: usize) -> Self {
        Self {
            name: Rc::from(name),
            arity,
            callable: Callable::Builtin,
        }
    }

    /// Creates a named-function closure.
    pub fn named(decl: Rc<FnDecl>) -> Self {
        Self {
            name: Rc::from(decl.name.as_str()),
            arity: decl.params.len(),
            callable: Callable::Named(decl),
        }
    }

    /// Creates a lambda closure from its AST node.
    pub fn lambda(
        params: Vec<String>,
        body: Rc<Expr>,
        captured: Env,
        span: orv_syntax::Span,
    ) -> Self {
        let _ = span;
        Self {
            name: Rc::from("<lambda>"),
            arity: params.len(),
            callable: Callable::Lambda {
                params: Rc::new(params),
                body,
                captured,
            },
        }
    }

    /// The AST block of a named function, when this is one.
    pub fn block(&self) -> Option<&Block> {
        match &self.callable {
            Callable::Named(decl) => Some(&decl.body),
            Callable::Lambda { .. } | Callable::Builtin => None,
        }
    }

    /// Whether this is a builtin.
    pub const fn is_builtin(&self) -> bool {
        matches!(self.callable, Callable::Builtin)
    }

    /// The body expression of a lambda, when this is one.
    pub fn lambda_body(&self) -> Option<&Expr> {
        match &self.callable {
            Callable::Lambda { body, .. } => Some(body),
            Callable::Named(_) | Callable::Builtin => None,
        }
    }

    /// The parameter names, for binding arguments.
    pub fn param_names(&self) -> Vec<String> {
        match &self.callable {
            Callable::Named(decl) => decl.params.iter().map(|p| p.name.clone()).collect(),
            Callable::Lambda { params, .. } => params.as_ref().clone(),
            // A builtin receives its arguments positionally; the names are
            // never used for binding.
            Callable::Builtin => Vec::new(),
        }
    }
}

impl fmt::Display for Closure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<fn {}>", self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orv_syntax::FileId;

    #[test]
    fn a_lambda_reports_its_arity() {
        let body = Expr::new(
            orv_syntax::ExprKind::Literal(orv_syntax::Literal::Int(1)),
            orv_syntax::Span::point(FileId(0), 0),
        );
        let closure = Closure::lambda(
            vec!["a".to_owned(), "b".to_owned()],
            Rc::new(body),
            Env::new(),
            orv_syntax::Span::point(FileId(0), 0),
        );
        assert_eq!(closure.arity, 2);
        assert_eq!(closure.name.as_ref(), "<lambda>");
        assert_eq!(closure.param_names(), vec!["a", "b"]);
        assert!(closure.lambda_body().is_some());
        assert!(closure.block().is_none());
    }

    #[test]
    fn a_lambda_displays_as_a_function() {
        let body = Expr::new(
            orv_syntax::ExprKind::Literal(orv_syntax::Literal::None),
            orv_syntax::Span::point(FileId(0), 0),
        );
        let closure = Closure::lambda(
            vec![],
            Rc::new(body),
            Env::new(),
            orv_syntax::Span::point(FileId(0), 0),
        );
        assert_eq!(closure.to_string(), "<fn <lambda>>");
    }
}
