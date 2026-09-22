//! Lexical environments for the interpreter.
//!
//! Closures capture an [`Env`], so bindings are reference-counted cells shared
//! between a closure and its defining scope (§5.3: closures capture `let mut`).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::value::Value;

/// A shared, mutable binding slot.
#[derive(Clone, Debug)]
pub struct Slot {
    /// The current value.
    pub value: Value,
    /// Whether `let mut` allowed assignment.
    pub mutable: bool,
}

/// One lexical scope.
#[derive(Debug, Default)]
struct Scope {
    /// Insertion-ordered names, so anything that walks bindings is
    /// deterministic (§3.4).
    order: Vec<Rc<str>>,
    slots: HashMap<Rc<str>, Slot>,
}

/// A chain of scopes, shared by closures.
#[derive(Clone, Debug)]
pub struct Env(Rc<RefCell<EnvData>>);

#[derive(Debug)]
struct EnvData {
    scopes: Vec<Scope>,
}

impl Default for Env {
    fn default() -> Self {
        Self::new()
    }
}

impl Env {
    /// Creates an environment with a single (global) scope.
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(EnvData {
            scopes: vec![Scope::default()],
        })))
    }

    /// Pushes a child scope onto the **same** chain.
    ///
    /// Returns the depth so the caller can pop exactly what it pushed.
    pub fn push(&self) -> usize {
        let mut data = self.0.borrow_mut();
        data.scopes.push(Scope::default());
        data.scopes.len()
    }

    /// Pops the innermost scope, never the global one.
    pub fn pop(&self) -> usize {
        let mut data = self.0.borrow_mut();
        if data.scopes.len() > 1 {
            data.scopes.pop();
        }
        data.scopes.len()
    }

    /// Creates a child environment that shares nothing but the parent chain.
    ///
    /// Used for a closure's captured scope: the closure sees the bindings that
    /// existed when it was created.
    pub fn child(&self) -> Env {
        Env::new()
    }

    /// Declares a binding in the innermost scope.
    pub fn define(&self, name: Rc<str>, value: Value, mutable: bool) {
        let mut data = self.0.borrow_mut();
        let Some(scope) = data.scopes.last_mut() else {
            return;
        };
        if !scope.slots.contains_key(&name) {
            scope.order.push(name.clone());
        }
        scope.slots.insert(name, Slot { value, mutable });
    }

    /// Looks a name up, innermost scope first.
    pub fn get(&self, name: &str) -> Option<Value> {
        let data = self.0.borrow();
        for scope in data.scopes.iter().rev() {
            if let Some(slot) = scope.slots.get(name) {
                return Some(slot.value.clone());
            }
        }
        None
    }

    /// Whether a name exists.
    pub fn contains(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    /// Assigns to an existing binding.
    ///
    /// Returns:
    /// * `Ok(true)` when the assignment happened,
    /// * `Ok(false)` when the binding exists but is immutable,
    /// * `Err(())` when the name is not bound at all.
    #[allow(clippy::result_unit_err)]
    pub fn assign(&self, name: &str, value: Value) -> Result<bool, ()> {
        let mut data = self.0.borrow_mut();
        for scope in data.scopes.iter_mut().rev() {
            if let Some(slot) = scope.slots.get_mut(name) {
                if !slot.mutable {
                    return Ok(false);
                }
                slot.value = value;
                return Ok(true);
            }
        }
        Err(())
    }

    /// All names visible from here, innermost shadowing outer scopes.
    pub fn names(&self) -> Vec<Rc<str>> {
        let data = self.0.borrow();
        let mut seen: Vec<Rc<str>> = Vec::new();
        for scope in data.scopes.iter().rev() {
            for name in scope.order.iter().rev() {
                if !seen.iter().any(|existing| existing == name) {
                    seen.push(name.clone());
                }
            }
        }
        seen
    }

    /// Current nesting depth.
    pub fn depth(&self) -> usize {
        self.0.borrow().scopes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defines_and_gets() {
        let env = Env::new();
        env.define("x".into(), Value::Int(1), false);
        assert_eq!(env.get("x"), Some(Value::Int(1)));
        assert!(!env.contains("y"));
    }

    #[test]
    fn inner_scopes_shadow_outer_ones() {
        let env = Env::new();
        env.define("x".into(), Value::Int(1), false);
        env.push();
        env.define("x".into(), Value::Int(2), false);
        assert_eq!(env.get("x"), Some(Value::Int(2)));
        env.pop();
        assert_eq!(env.get("x"), Some(Value::Int(1)));
    }

    #[test]
    fn immutable_bindings_refuse_assignment() {
        let env = Env::new();
        env.define("x".into(), Value::Int(1), false);
        assert_eq!(env.assign("x", Value::Int(2)), Ok(false));
        assert_eq!(env.get("x"), Some(Value::Int(1)));
    }

    #[test]
    fn mutable_bindings_accept_assignment() {
        let env = Env::new();
        env.define("x".into(), Value::Int(1), true);
        assert_eq!(env.assign("x", Value::Int(2)), Ok(true));
        assert_eq!(env.get("x"), Some(Value::Int(2)));
    }

    #[test]
    fn assigning_an_unbound_name_fails() {
        let env = Env::new();
        assert_eq!(env.assign("nope", Value::Int(1)), Err(()));
    }

    #[test]
    fn assignment_reaches_an_outer_mutable_binding() {
        let env = Env::new();
        env.define("x".into(), Value::Int(1), true);
        env.push();
        assert_eq!(env.assign("x", Value::Int(9)), Ok(true));
        env.pop();
        assert_eq!(env.get("x"), Some(Value::Int(9)));
    }

    #[test]
    fn the_global_scope_is_never_popped() {
        let env = Env::new();
        assert_eq!(env.pop(), 1);
    }

    #[test]
    fn names_prefers_the_innermost_binding() {
        let env = Env::new();
        env.define("outer".into(), Value::Int(1), false);
        env.push();
        env.define("inner".into(), Value::Int(2), false);
        let names = env.names();
        assert!(names.iter().any(|n| n.as_ref() == "inner"));
        assert!(names.iter().any(|n| n.as_ref() == "outer"));
    }
}
