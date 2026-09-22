//! Scoped symbol tables (SPEC §5.3: "Inferência: local e bidirecional").

use std::collections::HashMap;

use super::ty::Ty;

/// A binding visible in a scope.
#[derive(Clone, PartialEq, Debug)]
pub struct Symbol {
    pub name: String,
    pub ty: Ty,
    /// Whether `let mut` allowed reassignment of this binding.
    pub mutable: bool,
}

/// What a name lookup found.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SymbolKind {
    /// A local variable or parameter.
    Local,
    /// A top-level function.
    Function,
}

/// A stack of scopes.
///
/// Determinism matters for diagnostics, so each scope keeps insertion order and
/// lookups walk the stack; no iteration over a `HashMap` affects output.
#[derive(Clone, Debug, Default)]
pub struct Scopes {
    /// One entry per lexical scope, innermost last. Each is an ordered list.
    scopes: Vec<Vec<Symbol>>,
    /// Index by name for fast lookup; never iterated for output.
    index: HashMap<String, Vec<usize>>,
}

impl Scopes {
    /// Starts with the global scope pushed.
    pub fn new() -> Self {
        Self {
            scopes: vec![Vec::new()],
            index: HashMap::new(),
        }
    }

    /// Pushes a new scope.
    pub fn push(&mut self) {
        self.scopes.push(Vec::new());
    }

    /// Pops the innermost scope.
    ///
    /// The global scope is never popped; the count is returned so the caller
    /// can assert balance in tests.
    pub fn pop(&mut self) -> usize {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
        self.scopes.len()
    }

    /// Declares `symbol` in the innermost scope.
    ///
    /// Returns `false` when the name is already declared **in the same scope**
    /// (shadowing an outer scope is allowed, §5.3).
    pub fn declare(&mut self, symbol: Symbol) -> bool {
        let Some(scope) = self.scopes.last() else {
            return false;
        };
        if scope.iter().any(|existing| existing.name == symbol.name) {
            return false;
        }
        let scope_index = self.scopes.len() - 1;
        let position = scope.len();
        if let Some(scope) = self.scopes.last_mut() {
            scope.push(symbol);
        }
        self.index
            .entry(
                self.scopes
                    .get(scope_index)
                    .and_then(|s| s.get(position))
                    .map(|s| s.name.clone())
                    .unwrap_or_default(),
            )
            .or_default()
            .push(scope_index);
        true
    }

    /// Looks a name up, innermost scope first.
    pub fn lookup(&self, name: &str) -> Option<&Symbol> {
        for scope in self.scopes.iter().rev() {
            if let Some(found) = scope.iter().rev().find(|s| s.name == name) {
                return Some(found);
            }
        }
        None
    }

    /// Whether `name` is declared in the innermost scope.
    pub fn declared_here(&self, name: &str) -> bool {
        self.scopes
            .last()
            .is_some_and(|scope| scope.iter().any(|s| s.name == name))
    }

    /// Current nesting depth, for tests and debugging.
    pub fn depth(&self) -> usize {
        self.scopes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn symbol(name: &str, mutable: bool) -> Symbol {
        Symbol {
            name: name.to_owned(),
            ty: Ty::Int,
            mutable,
        }
    }

    #[test]
    fn declares_and_looks_up() {
        let mut scopes = Scopes::new();
        assert!(scopes.declare(symbol("x", false)));
        assert_eq!(scopes.lookup("x").map(|s| s.ty.clone()), Some(Ty::Int));
        assert!(scopes.lookup("y").is_none());
    }

    #[test]
    fn rejects_a_duplicate_in_the_same_scope() {
        let mut scopes = Scopes::new();
        assert!(scopes.declare(symbol("x", false)));
        assert!(!scopes.declare(symbol("x", true)));
        assert!(scopes.declared_here("x"));
    }

    #[test]
    fn shadowing_an_outer_scope_is_allowed() {
        let mut scopes = Scopes::new();
        assert!(scopes.declare(symbol("x", false)));
        scopes.push();
        assert!(scopes.declare(symbol("x", true)));
        assert_eq!(scopes.lookup("x").map(|s| s.mutable), Some(true));
    }

    #[test]
    fn popping_restores_the_outer_binding() {
        let mut scopes = Scopes::new();
        scopes.declare(symbol("x", false));
        scopes.push();
        scopes.declare(symbol("x", true));
        scopes.pop();
        assert_eq!(scopes.lookup("x").map(|s| s.mutable), Some(false));
        assert_eq!(scopes.depth(), 1);
    }

    #[test]
    fn the_global_scope_is_never_popped() {
        let mut scopes = Scopes::new();
        assert_eq!(scopes.pop(), 1);
    }

    #[test]
    fn nested_scopes_lookup_through_the_chain() {
        let mut scopes = Scopes::new();
        scopes.declare(symbol("global", false));
        scopes.push();
        scopes.declare(symbol("outer", false));
        scopes.push();
        assert!(scopes.lookup("global").is_some());
        assert!(scopes.lookup("outer").is_some());
        assert!(!scopes.declared_here("outer"));
        assert!(scopes.declared_here("x").eq(&false));
    }
}
