//! The prelude and builtin functions (SPEC §5.6).
//!
//! The alpha provides exactly what §5.6 lists: `print`, `len`, `range`, `str`,
//! `int`, `float`, `assert`, `Ok`, `Err`, `Failure`. Anything else is out of
//! scope (ADR 0012).

use std::rc::Rc;

use orv_syntax::Span;

use crate::failure::{Failure, FailureKind};
use crate::interpreter::{EvalResult, Interpreter};
use crate::value::{Value, display, format_float};

/// A builtin function body.
pub type Builtin = fn(&mut Interpreter, &[Value], Span) -> EvalResult;

/// Looks a builtin up by name.
pub fn lookup(name: &str) -> Option<Builtin> {
    match name {
        "print" => Some(print),
        "len" => Some(len),
        "range" => Some(range),
        "str" => Some(to_str),
        "int" => Some(to_int),
        "float" => Some(to_float),
        "assert" => Some(assert),
        "Ok" => Some(ok),
        "Err" => Some(err),
        _ => None,
    }
}

/// Installs the builtins into the interpreter's global scope.
pub fn install(interpreter: &mut Interpreter) {
    for (name, arity) in [
        ("print", 1),
        ("len", 1),
        ("range", 2),
        ("str", 1),
        ("int", 1),
        ("float", 1),
        ("assert", 1),
        ("Ok", 1),
        ("Err", 1),
    ] {
        interpreter.define_builtin(name, arity);
    }
}

/// The field names of a `data` type, when `name` is one.
pub fn data_schema(interpreter: &Interpreter, name: &str) -> Option<Vec<String>> {
    interpreter.data_schema(name)
}

/// The enum name and arity of a variant, when `name` is one.
pub fn variant_schema(interpreter: &Interpreter, name: &str) -> Option<(String, usize)> {
    interpreter.variant_schema(name)
}

// --- Implementations --------------------------------------------------------

/// `print(value)` — writes one line.
fn print(interpreter: &mut Interpreter, args: &[Value], _span: Span) -> EvalResult {
    let Some(value) = args.first() else {
        return Err(arity("print", 1, 0));
    };
    interpreter.write_output(&display(value));
    interpreter.write_output("\n");
    Ok(Value::Unit)
}

/// `len(value)` — the length of a string, list or map.
fn len(_interpreter: &mut Interpreter, args: &[Value], span: Span) -> EvalResult {
    let Some(value) = args.first() else {
        return Err(arity("len", 1, 0));
    };
    let length = match value {
        Value::Str(text) => text.chars().count(),
        Value::List(elements) => elements.borrow().len(),
        Value::Map(entries) => entries.borrow().len(),
        other => {
            return Err(type_error("len", other.ty().to_string(), span));
        }
    };
    Ok(Value::Int(length as i64))
}

/// `range(start, end)` — the integers from `start` up to `end`, exclusive.
fn range(_interpreter: &mut Interpreter, args: &[Value], span: Span) -> EvalResult {
    if args.len() != 2 {
        return Err(arity("range", 2, args.len()));
    }
    let (Some(start), Some(end)) = (args[0].as_int(), args[1].as_int()) else {
        return Err(type_error("range", "Int, Int".to_owned(), span));
    };
    let mut values = Vec::new();
    let mut current = start;
    while current < end {
        values.push(Value::Int(current));
        match current.checked_add(1) {
            Some(next) => current = next,
            None => break,
        }
    }
    Ok(Value::list(values))
}

/// `str(value)` — the `Display` form of a value.
fn to_str(_interpreter: &mut Interpreter, args: &[Value], _span: Span) -> EvalResult {
    let Some(value) = args.first() else {
        return Err(arity("str", 1, 0));
    };
    Ok(Value::str(display(value)))
}

/// `int(value)` — parses or truncates to an `Int`.
fn to_int(_interpreter: &mut Interpreter, args: &[Value], span: Span) -> EvalResult {
    let Some(value) = args.first() else {
        return Err(arity("int", 1, 0));
    };
    match value {
        Value::Int(value) => Ok(Value::Int(*value)),
        Value::Float(value) => Ok(Value::Int(*value as i64)),
        Value::Str(text) => text.trim().parse::<i64>().map(Value::Int).map_err(|_| {
            Box::new(Failure::new(
                FailureKind::Unsupported,
                format!("`int` cannot parse {text:?}"),
                span,
            ))
        }),
        Value::Bool(value) => Ok(Value::Int(i64::from(*value))),
        other => Err(type_error("int", other.ty().to_string(), span)),
    }
}

/// `float(value)` — converts to a `Float`.
fn to_float(_interpreter: &mut Interpreter, args: &[Value], span: Span) -> EvalResult {
    let Some(value) = args.first() else {
        return Err(arity("float", 1, 0));
    };
    match value {
        Value::Int(value) => Ok(Value::Float(*value as f64)),
        Value::Float(value) => Ok(Value::Float(*value)),
        Value::Str(text) => text.trim().parse::<f64>().map(Value::Float).map_err(|_| {
            Box::new(Failure::new(
                FailureKind::Unsupported,
                format!("`float` cannot parse {text:?}"),
                span,
            ))
        }),
        other => Err(type_error("float", other.ty().to_string(), span)),
    }
}

/// `assert(condition)` — fails when the condition is false (§5.7).
fn assert(_interpreter: &mut Interpreter, args: &[Value], span: Span) -> EvalResult {
    let Some(value) = args.first() else {
        return Err(arity("assert", 1, 0));
    };
    match value.as_bool() {
        Some(true) => Ok(Value::Unit),
        Some(false) => Err(Box::new(Failure::new(
            FailureKind::AssertionFailed,
            "assertion failed",
            span,
        ))),
        None => Err(type_error("assert", "Bool".to_owned(), span)),
    }
}

/// `Ok(value)` — a successful result.
fn ok(_interpreter: &mut Interpreter, args: &[Value], _span: Span) -> EvalResult {
    let Some(value) = args.first() else {
        return Err(arity("Ok", 1, 0));
    };
    Ok(Value::Result {
        ok: true,
        value: Box::new(value.clone()),
        failure: None,
    })
}

/// `Err(failure)` — a failed result.
fn err(_interpreter: &mut Interpreter, args: &[Value], span: Span) -> EvalResult {
    let Some(value) = args.first() else {
        return Err(arity("Err", 1, 0));
    };
    let failure = match value {
        Value::Str(message) => Failure::new(FailureKind::Unsupported, message.clone(), span),
        Value::Data { fields, .. } => {
            let message = fields
                .borrow()
                .iter()
                .find(|(name, _)| name.as_ref() == "message")
                .map(|(_, value)| display(value))
                .unwrap_or_else(|| "failure".to_owned());
            Failure::new(FailureKind::Unsupported, message, span)
        }
        other => {
            return Err(type_error("Err", other.ty().to_string(), span));
        }
    };
    Ok(Value::Result {
        ok: false,
        value: Box::new(Value::Unit),
        failure: Some(Box::new(failure)),
    })
}

/// A wrong-arity failure.
fn arity(name: &str, expected: usize, found: usize) -> Box<Failure> {
    Box::new(Failure::new(
        FailureKind::Unsupported,
        format!("`{name}` takes {expected} argument(s) but {found} were given"),
        Span::new(orv_syntax::FileId(0), 0, 0),
    ))
}

/// A wrong-type failure.
fn type_error(name: &str, expected: String, span: Span) -> Box<Failure> {
    Box::new(Failure::new(
        FailureKind::Unsupported,
        format!("`{name}` expects {expected}"),
        span,
    ))
}

/// Formats a float for the `str` builtin.
pub fn float_text(value: f64) -> String {
    format_float(value)
}

/// A display helper re-exported for the interpreter.
pub fn show(value: &Value) -> String {
    display(value)
}

/// An `Rc<str>` helper for builtin names.
pub fn name(text: &str) -> Rc<str> {
    Rc::from(text)
}

#[cfg(test)]
mod tests;
