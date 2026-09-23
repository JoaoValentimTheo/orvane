//! Runtime values (SPEC §5.3, §5.5).
//!
//! A `Value` is what an Orvane program manipulates at run time. Equality is
//! **structural** for lists, maps, tuples, `data` and `enum` (§5.3), which is
//! why [`Value`] implements [`PartialEq`] by hand rather than deriving it for
//! the aggregate cases.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::fmt::Write as _;
use std::rc::Rc;

use orv_sema::Ty;
use orv_syntax::Span;

use crate::failure::Failure;
use crate::function::Closure;

/// The fields of a `data` value: `(name, value)` in declaration order.
///
/// Shared and mutable so one field can be reassigned in place (ADR 0022).
pub type DataFields = Rc<RefCell<Vec<(Rc<str>, Value)>>>;

/// A runtime value.
///
/// `Clone` is implemented by hand: aggregates are shared (a list/map/tuple/`data`
/// clone shares the underlying `Rc`), **except** `data`, which has value
/// semantics (ADR 0022): cloning a `data` makes an independent copy of its
/// fields, so `let b = a; b.x = 1` does not change `a.x`.
#[derive(Debug)]
pub enum Value {
    /// A 64-bit integer.
    Int(i64),
    /// A double-precision float.
    Float(f64),
    /// A boolean.
    Bool(bool),
    /// A string.
    Str(Rc<str>),
    /// `()`
    Unit,
    /// A list, shared so closures can capture it by reference (SPEC §5.3
    /// mutable bindings).
    List(Rc<RefCell<Vec<Value>>>),
    /// A map with structural keys.
    Map(Rc<RefCell<MapValue>>),
    /// A tuple of two or more values.
    Tuple(Rc<Vec<Value>>),
    /// A `data` instance: the type name and its fields in declaration order.
    ///
    /// The fields sit behind a `RefCell` so a single field can be reassigned in
    /// place (`u.age = 2`, ADR 0022). `Value`'s manual `Clone` deep-copies them,
    /// which is what gives `data` value semantics.
    Data { name: Rc<str>, fields: DataFields },
    /// An `enum` value: the type name, the variant name and its payload.
    Variant {
        enum_name: Rc<str>,
        variant: Rc<str>,
        payload: Rc<Vec<Value>>,
    },
    /// `none` for an optional.
    None,
    /// A function value (a `fn` by name or a closure).
    Function(Rc<Closure>),
    /// An `Ok`/`Err` result.
    Result {
        ok: bool,
        value: Box<Value>,
        failure: Option<Box<Failure>>,
    },
}

impl Clone for Value {
    fn clone(&self) -> Self {
        match self {
            Value::Int(value) => Value::Int(*value),
            Value::Float(value) => Value::Float(*value),
            Value::Bool(value) => Value::Bool(*value),
            Value::Str(value) => Value::Str(value.clone()),
            Value::Unit => Value::Unit,
            Value::List(elements) => Value::List(elements.clone()),
            Value::Map(entries) => Value::Map(entries.clone()),
            Value::Tuple(elements) => Value::Tuple(elements.clone()),
            // Value semantics (ADR 0022): a `data` clone is an independent copy
            // of the fields, so mutating one binding does not touch another.
            Value::Data { name, fields } => Value::Data {
                name: name.clone(),
                fields: Rc::new(RefCell::new(fields.borrow().clone())),
            },
            Value::Variant {
                enum_name,
                variant,
                payload,
            } => Value::Variant {
                enum_name: enum_name.clone(),
                variant: variant.clone(),
                payload: payload.clone(),
            },
            Value::None => Value::None,
            Value::Function(function) => Value::Function(function.clone()),
            Value::Result { ok, value, failure } => Value::Result {
                ok: *ok,
                value: value.clone(),
                failure: failure.clone(),
            },
        }
    }
}

impl Value {
    /// Creates a string value.
    pub fn str(text: impl Into<Rc<str>>) -> Self {
        Value::Str(text.into())
    }

    /// Creates a list value.
    pub fn list(elements: Vec<Value>) -> Self {
        Value::List(Rc::new(RefCell::new(elements)))
    }

    /// Creates a `data` value from its type name and fields.
    pub fn data(name: impl Into<Rc<str>>, fields: Vec<(Rc<str>, Value)>) -> Self {
        Value::Data {
            name: name.into(),
            fields: Rc::new(RefCell::new(fields)),
        }
    }

    /// Creates a map value.
    pub fn map(entries: MapValue) -> Self {
        Value::Map(Rc::new(RefCell::new(entries)))
    }

    /// The `Ty` this value has, as far as it can be told at run time.
    ///
    /// Used by diagnostics; `none` reports `?` because an optional's element
    /// type is erased at run time.
    pub fn ty(&self) -> Ty {
        match self {
            Value::Int(_) => Ty::Int,
            Value::Float(_) => Ty::Float,
            Value::Bool(_) => Ty::Bool,
            Value::Str(_) => Ty::Str,
            Value::Unit => Ty::Unit,
            Value::List(elements) => Ty::List(Box::new(
                elements
                    .borrow()
                    .first()
                    .map(Value::ty)
                    .unwrap_or(Ty::Unknown),
            )),
            Value::Map(entries) => {
                let borrow = entries.borrow();
                let key = borrow.keys().first().map(MapKey::ty).unwrap_or(Ty::Unknown);
                let value = borrow.values().next().map(Value::ty).unwrap_or(Ty::Unknown);
                Ty::Map(Box::new(key), Box::new(value))
            }
            Value::Tuple(elements) => Ty::Tuple(elements.iter().map(Value::ty).collect()),
            Value::Data { name, .. } => Ty::Data(name.to_string()),
            Value::Variant { enum_name, .. } => Ty::Enum(enum_name.to_string()),
            Value::None => Ty::Unknown,
            Value::Function(_) => Ty::Unknown,
            Value::Result { .. } => Ty::Result(Box::new(Ty::Unknown)),
        }
    }

    /// Whether this is a numeric value.
    pub const fn is_number(&self) -> bool {
        matches!(self, Value::Int(_) | Value::Float(_))
    }

    /// The integer inside an `Int`, if that is what this is.
    pub const fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(value) => Some(*value),
            _ => None,
        }
    }

    /// The boolean inside a `Bool`, if that is what this is.
    pub const fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(value) => Some(*value),
            _ => None,
        }
    }

    /// Structural equality (SPEC §5.3).
    ///
    /// `Int` and `Float` compare numerically across the two types, matching the
    /// arithmetic promotion rule; two numbers of different kinds are equal when
    /// their mathematical values are.
    pub fn equals(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Int(a), Value::Float(b)) | (Value::Float(b), Value::Int(a)) => {
                (*a as f64) == *b
            }
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Unit, Value::Unit) => true,
            (Value::None, Value::None) => true,
            (Value::List(a), Value::List(b)) => {
                // Compare by value, not by identity of the `Rc`.
                if Rc::ptr_eq(a, b) {
                    return true;
                }
                let (a, b) = (a.borrow(), b.borrow());
                a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x.equals(y))
            }
            (Value::Map(a), Value::Map(b)) => {
                if Rc::ptr_eq(a, b) {
                    return true;
                }
                a.borrow().equals(&b.borrow())
            }
            (Value::Tuple(a), Value::Tuple(b)) => {
                a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x.equals(y))
            }
            (
                Value::Data {
                    name: an,
                    fields: af,
                },
                Value::Data {
                    name: bn,
                    fields: bf,
                },
            ) => {
                let (af, bf) = (af.borrow(), bf.borrow());
                an == bn
                    && af.len() == bf.len()
                    && af
                        .iter()
                        .zip(bf.iter())
                        .all(|((ak, av), (bk, bv))| ak == bk && av.equals(bv))
            }
            (
                Value::Variant {
                    enum_name: an,
                    variant: av,
                    payload: ap,
                },
                Value::Variant {
                    enum_name: bn,
                    variant: bv,
                    payload: bp,
                },
            ) => {
                an == bn
                    && av == bv
                    && ap.len() == bp.len()
                    && ap.iter().zip(bp.iter()).all(|(x, y)| x.equals(y))
            }
            // Functions are reference values.
            (Value::Function(a), Value::Function(b)) => Rc::ptr_eq(a, b),
            (
                Value::Result {
                    ok: aok,
                    value: av,
                    failure: af,
                },
                Value::Result {
                    ok: bok,
                    value: bv,
                    failure: bf,
                },
            ) => {
                aok == bok
                    && av.equals(bv)
                    && match (af, bf) {
                        (None, None) => true,
                        (Some(x), Some(y)) => x.equals(y),
                        _ => false,
                    }
            }
            _ => false,
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.equals(other)
    }
}

/// A map key.
///
/// §5.3 restricts `Map` keys to `Int`, `Str` and `Bool`, which makes the key
/// order deterministic (declaration order) and hashing total.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum MapKey {
    /// An integer key.
    Int(i64),
    /// A string key.
    Str(Rc<str>),
    /// A boolean key.
    Bool(bool),
}

impl MapKey {
    /// The key for a value, when the value may be used as one.
    pub fn from_value(value: &Value) -> Option<MapKey> {
        match value {
            Value::Int(value) => Some(MapKey::Int(*value)),
            Value::Str(value) => Some(MapKey::Str(value.clone())),
            Value::Bool(value) => Some(MapKey::Bool(*value)),
            _ => None,
        }
    }

    /// The value form of this key.
    pub fn to_value(&self) -> Value {
        match self {
            MapKey::Int(value) => Value::Int(*value),
            MapKey::Str(value) => Value::Str(value.clone()),
            MapKey::Bool(value) => Value::Bool(*value),
        }
    }

    /// The type of this key.
    pub fn ty(&self) -> Ty {
        match self {
            MapKey::Int(_) => Ty::Int,
            MapKey::Str(_) => Ty::Str,
            MapKey::Bool(_) => Ty::Bool,
        }
    }
}

impl fmt::Display for MapKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MapKey::Int(value) => write!(f, "{value}"),
            MapKey::Str(value) => f.write_str(value),
            MapKey::Bool(value) => write!(f, "{value}"),
        }
    }
}

/// A map value that preserves insertion order.
///
/// Determinism is a hard requirement (§3.4): iterating a map must not depend on
/// hashing, so the order lives in `keys` and the lookup uses the `HashMap`.
#[derive(Clone, Debug, Default)]
pub struct MapValue {
    keys: Vec<MapKey>,
    entries: HashMap<MapKey, Value>,
}

impl MapValue {
    /// Creates an empty map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts `value` under `key`, replacing and keeping the original position.
    pub fn insert(&mut self, key: MapKey, value: Value) {
        if self.entries.insert(key.clone(), value).is_none() {
            self.keys.push(key);
        }
    }

    /// Looks a key up.
    pub fn get(&self, key: &MapKey) -> Option<&Value> {
        self.entries.get(key)
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Whether the map is empty.
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// The keys in insertion order.
    pub fn keys(&self) -> &[MapKey] {
        &self.keys
    }

    /// The values in insertion order.
    pub fn values(&self) -> impl Iterator<Item = &Value> {
        self.keys.iter().filter_map(|key| self.entries.get(key))
    }

    /// The entries in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&MapKey, &Value)> {
        self.keys
            .iter()
            .filter_map(|key| self.entries.get(key).map(|value| (key, value)))
    }

    /// Structural equality, order-insensitive (a map is a set of pairs).
    pub fn equals(&self, other: &MapValue) -> bool {
        if self.len() != other.len() {
            return false;
        }
        self.iter()
            .all(|(key, value)| other.get(key).is_some_and(|other| value.equals(other)))
    }
}

impl FromIterator<(MapKey, Value)> for MapValue {
    fn from_iter<T: IntoIterator<Item = (MapKey, Value)>>(iter: T) -> Self {
        let mut map = MapValue::new();
        for (key, value) in iter {
            map.insert(key, value);
        }
        map
    }
}

/// Formats a value the way `print` does (SPEC §5.3 `Display`).
///
/// Strings print without quotes at the top level; inside an aggregate they are
/// quoted, so `print(["a"])` is unambiguous.
pub fn display(value: &Value) -> String {
    let mut out = String::new();
    write_value(&mut out, value, false);
    out
}

/// Formats a value as a debug repr (strings quoted).
pub fn repr(value: &Value) -> String {
    let mut out = String::new();
    write_value(&mut out, value, true);
    out
}

fn write_value(out: &mut String, value: &Value, quoted: bool) {
    match value {
        Value::Int(value) => {
            let _ = write!(out, "{value}");
        }
        Value::Float(value) => out.push_str(&format_float(*value)),
        Value::Bool(value) => {
            let _ = write!(out, "{value}");
        }
        Value::Str(text) => {
            if quoted {
                let _ = write!(out, "{:?}", text.as_ref());
            } else {
                out.push_str(text);
            }
        }
        Value::Unit => out.push_str("()"),
        Value::None => out.push_str("none"),
        Value::List(elements) => {
            out.push('[');
            for (index, element) in elements.borrow().iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                write_value(out, element, true);
            }
            out.push(']');
        }
        Value::Map(entries) => {
            out.push_str("#{");
            for (index, (key, value)) in entries.borrow().iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                match key {
                    MapKey::Str(text) => {
                        let _ = write!(out, "{text:?}");
                    }
                    other => {
                        let _ = write!(out, "{other}");
                    }
                }
                out.push_str(": ");
                write_value(out, value, true);
            }
            out.push('}');
        }
        Value::Tuple(elements) => {
            out.push('(');
            for (index, element) in elements.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                write_value(out, element, true);
            }
            if elements.len() == 1 {
                out.push(',');
            }
            out.push(')');
        }
        Value::Data { name, fields } => {
            let _ = write!(out, "{name}(");
            for (index, (field, value)) in fields.borrow().iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                let _ = write!(out, "{field}: ");
                write_value(out, value, true);
            }
            out.push(')');
        }
        Value::Variant {
            variant, payload, ..
        } => {
            out.push_str(variant);
            if !payload.is_empty() {
                out.push('(');
                for (index, value) in payload.iter().enumerate() {
                    if index > 0 {
                        out.push_str(", ");
                    }
                    write_value(out, value, true);
                }
                out.push(')');
            }
        }
        Value::Function(function) => {
            let _ = write!(out, "<fn {}>", function.name);
        }
        Value::Result { ok, value, failure } => {
            if *ok {
                out.push_str("Ok(");
                write_value(out, value, true);
                out.push(')');
            } else {
                out.push_str("Err(");
                match failure {
                    Some(failure) => out.push_str(&crate::failure::render(failure)),
                    None => out.push_str("Failure"),
                }
                out.push(')');
            }
        }
    }
}

/// Renders a float so integral values keep a `.0` (matching the lexer's dump).
pub fn format_float(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_owned();
    }
    if value.is_infinite() {
        return if value.is_sign_negative() {
            "-inf".to_owned()
        } else {
            "inf".to_owned()
        };
    }
    let mut text = format!("{value}");
    if !text.contains(['.', 'e', 'E']) {
        text.push_str(".0");
    }
    text
}

/// A value plus where it came from, used for runtime diagnostics (SPEC §5.5:
/// "pilha de chamadas").
#[derive(Clone, Debug)]
pub struct Traced {
    pub value: Value,
    pub span: Span,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: Vec<(MapKey, Value)>) -> Value {
        Value::map(pairs.into_iter().collect())
    }

    #[test]
    fn ints_and_floats_compare_numerically() {
        assert!(Value::Int(1).equals(&Value::Float(1.0)));
        assert!(Value::Float(1.0).equals(&Value::Int(1)));
        assert!(!Value::Int(1).equals(&Value::Float(1.5)));
    }

    #[test]
    fn lists_compare_structurally_not_by_identity() {
        let a = Value::list(vec![Value::Int(1), Value::Int(2)]);
        let b = Value::list(vec![Value::Int(1), Value::Int(2)]);
        assert!(!Rc::ptr_eq(
            match &a {
                Value::List(list) => list,
                _ => panic!(),
            },
            match &b {
                Value::List(list) => list,
                _ => panic!(),
            }
        ));
        assert_eq!(a, b);
    }

    #[test]
    fn maps_compare_regardless_of_insertion_order() {
        let a = map(vec![
            (MapKey::Str("a".into()), Value::Int(1)),
            (MapKey::Str("b".into()), Value::Int(2)),
        ]);
        let b = map(vec![
            (MapKey::Str("b".into()), Value::Int(2)),
            (MapKey::Str("a".into()), Value::Int(1)),
        ]);
        assert_eq!(a, b);
    }

    #[test]
    fn maps_preserve_insertion_order_for_display() {
        let m = map(vec![
            (MapKey::Str("z".into()), Value::Int(1)),
            (MapKey::Str("a".into()), Value::Int(2)),
        ]);
        assert_eq!(display(&m), "#{\"z\": 1, \"a\": 2}");
    }

    #[test]
    fn map_insert_keeps_the_original_position() {
        let mut map = MapValue::new();
        map.insert(MapKey::Str("a".into()), Value::Int(1));
        map.insert(MapKey::Str("b".into()), Value::Int(2));
        map.insert(MapKey::Str("a".into()), Value::Int(3));
        assert_eq!(map.len(), 2);
        assert_eq!(map.keys()[0], MapKey::Str("a".into()));
        assert_eq!(map.get(&MapKey::Str("a".into())), Some(&Value::Int(3)));
    }

    #[test]
    fn tuples_and_variants_compare_structurally() {
        let a = Value::Tuple(Rc::new(vec![Value::Int(1), Value::str("x")]));
        let b = Value::Tuple(Rc::new(vec![Value::Int(1), Value::str("x")]));
        assert_eq!(a, b);

        let c = Value::Variant {
            enum_name: "E".into(),
            variant: "A".into(),
            payload: Rc::new(vec![Value::Int(1)]),
        };
        let d = Value::Variant {
            enum_name: "E".into(),
            variant: "A".into(),
            payload: Rc::new(vec![Value::Int(1)]),
        };
        assert_eq!(c, d);
    }

    #[test]
    fn data_equality_uses_field_names_and_values() {
        let a = Value::data("U", vec![("name".into(), Value::str("x"))]);
        let b = Value::data("U", vec![("name".into(), Value::str("x"))]);
        assert_eq!(a, b);
    }

    #[test]
    fn display_quotes_strings_inside_aggregates() {
        let list = Value::list(vec![Value::str("a"), Value::Int(1)]);
        assert_eq!(display(&list), "[\"a\", 1]");
        assert_eq!(display(&Value::str("a")), "a");
        assert_eq!(repr(&Value::str("a")), "\"a\"");
    }

    #[test]
    fn display_of_a_data_matches_the_spec_example() {
        // §4.2: `User(name: "Mel", age: 30, email: none)`
        let user = Value::data(
            "User",
            vec![
                ("name".into(), Value::str("Mel")),
                ("age".into(), Value::Int(30)),
                ("email".into(), Value::None),
            ],
        );
        assert_eq!(display(&user), "User(name: \"Mel\", age: 30, email: none)");
    }

    #[test]
    fn cloning_a_data_makes_an_independent_copy() {
        // Value semantics (ADR 0022): a `data` clone does not share fields.
        let a = Value::data("U", vec![("age".into(), Value::Int(1))]);
        let b = a.clone();
        if let Value::Data { fields, .. } = &a {
            fields.borrow_mut()[0].1 = Value::Int(99);
        }
        assert_eq!(display(&a), "U(age: 99)");
        assert_eq!(display(&b), "U(age: 1)");
    }

    #[test]
    fn floats_display_with_a_decimal_point() {
        assert_eq!(display(&Value::Float(1.0)), "1.0");
        assert_eq!(display(&Value::Float(1.5)), "1.5");
        assert_eq!(format_float(f64::INFINITY), "inf");
        assert_eq!(format_float(f64::NAN), "NaN");
    }

    #[test]
    fn map_keys_reject_unsupported_values() {
        assert!(MapKey::from_value(&Value::Int(1)).is_some());
        assert!(MapKey::from_value(&Value::str("a")).is_some());
        assert!(MapKey::from_value(&Value::Bool(true)).is_some());
        assert!(MapKey::from_value(&Value::list(vec![])).is_none());
    }

    #[test]
    fn key_round_trips_through_value() {
        for key in [MapKey::Int(1), MapKey::Str("a".into()), MapKey::Bool(false)] {
            assert_eq!(MapKey::from_value(&key.to_value()), Some(key));
        }
    }
}
