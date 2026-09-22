//! Lexical environments for the interpreter.
//!
//! Closures capture an [`Env`], so bindings are reference-counted cells shared
//! between a closure and its defining scope (§5.3: closures capture `let mut`).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::value::Value;

/// A shared, mutable binding slot.
///
/// The cell is reference-counted so a closure and its defining scope can share
/// the same binding: assigning to a captured `let mut` is visible to both
/// (§5.3).
#[derive(Clone, Debug)]
pub struct Slot(Rc<RefCell<SlotData>>);

#[derive(Debug)]
struct SlotData {
    /// The current value.
    value: Value,
    /// Whether `let mut` allowed assignment.
    mutable: bool,
}

impl Slot {
    /// Creates a slot holding `value`.
    fn new(value: Value, mutable: bool) -> Self {
        Slot(Rc::new(RefCell::new(SlotData { value, mutable })))
    }

    /// The current value.
    fn value(&self) -> Value {
        self.0.borrow().value.clone()
    }

    /// Whether the binding is assignable.
    fn is_mutable(&self) -> bool {
        self.0.borrow().mutable
    }

    /// Overwrites the value.
    fn set(&self, value: Value) {
        self.0.borrow_mut().value = value;
    }
}

/// One lexical scope.
///
/// A scope is shared (via `Rc`) between the environment that created it and any
/// closure that captured it, so its bindings outlive a `pop` on the defining
/// environment. Interior mutability is needed because `define` inserts into a
/// scope that may already be shared.
#[derive(Debug, Default)]
struct Scope {
    /// Insertion-ordered names, so anything that walks bindings is
    /// deterministic (§3.4).
    order: RefCell<Vec<Rc<str>>>,
    slots: RefCell<HashMap<Rc<str>, Slot>>,
}

/// A chain of scopes.
///
/// Each [`Env`] owns its own list of scope handles (an `Rc<RefCell<Vec<_>>>`),
/// so [`push`](Env::push)/[`pop`](Env::pop) on one environment never affects a
/// closure that captured a [`snapshot`](Env::snapshot) of it. The scopes and
/// their binding cells are shared, so captured mutable bindings stay live.
#[derive(Clone, Debug)]
pub struct Env(Rc<RefCell<Vec<Rc<Scope>>>>);

impl Default for Env {
    fn default() -> Self {
        Self::new()
    }
}

impl Env {
    /// Creates an environment with a single (global) scope.
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(vec![Rc::new(Scope::default())])))
    }

    /// Pushes a child scope onto **this** environment's chain.
    ///
    /// Returns the depth so the caller can pop exactly what it pushed.
    pub fn push(&self) -> usize {
        let mut scopes = self.0.borrow_mut();
        scopes.push(Rc::new(Scope::default()));
        scopes.len()
    }

    /// Pops the innermost scope, never the global one.
    pub fn pop(&self) -> usize {
        let mut scopes = self.0.borrow_mut();
        if scopes.len() > 1 {
            scopes.pop();
        }
        scopes.len()
    }

    /// A detached environment holding the same scopes as this one.
    ///
    /// Used for a closure's captured scope: the closure keeps seeing the
    /// bindings that existed when it was created, even after the defining
    /// environment pops its own frame. The scope handles (and therefore the
    /// binding cells) are shared, so a captured `let mut` stays assignable.
    pub fn snapshot(&self) -> Env {
        Env(Rc::new(RefCell::new(self.0.borrow().clone())))
    }

    /// The innermost scope, if any.
    fn innermost(&self) -> Option<Rc<Scope>> {
        self.0.borrow().last().cloned()
    }

    /// Declares a binding in the innermost scope.
    pub fn define(&self, name: Rc<str>, value: Value, mutable: bool) {
        let Some(scope) = self.innermost() else {
            return;
        };
        let mut slots = scope.slots.borrow_mut();
        if !slots.contains_key(&name) {
            scope.order.borrow_mut().push(name.clone());
        }
        slots.insert(name, Slot::new(value, mutable));
    }

    /// Looks a name up, innermost scope first.
    pub fn get(&self, name: &str) -> Option<Value> {
        let scopes = self.0.borrow().clone();
        for scope in scopes.iter().rev() {
            if let Some(slot) = scope.slots.borrow().get(name) {
                return Some(slot.value());
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
        let scopes = self.0.borrow().clone();
        for scope in scopes.iter().rev() {
            let slot = scope.slots.borrow().get(name).cloned();
            if let Some(slot) = slot {
                if !slot.is_mutable() {
                    return Ok(false);
                }
                slot.set(value);
                return Ok(true);
            }
        }
        Err(())
    }

    /// All names visible from here, innermost shadowing outer scopes.
    pub fn names(&self) -> Vec<Rc<str>> {
        let scopes = self.0.borrow().clone();
        let mut seen: Vec<Rc<str>> = Vec::new();
        for scope in scopes.iter().rev() {
            for name in scope.order.borrow().iter().rev() {
                if !seen.iter().any(|existing| existing == name) {
                    seen.push(name.clone());
                }
            }
        }
        seen
    }

    /// Current nesting depth.
    pub fn depth(&self) -> usize {
        self.0.borrow().len()
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
