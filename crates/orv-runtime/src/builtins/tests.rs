//! Builtin tests (SPEC §5.6).

use crate::interpreter::Interpreter;
use crate::value::{MapKey, Value, display};

/// Calls a builtin directly.
fn call(name: &str, args: Vec<Value>) -> Result<Value, String> {
    let mut interpreter = Interpreter::new();
    let function = match interpreter.function(name) {
        Some(function) => function,
        None => return Err(format!("no builtin `{name}`")),
    };
    let span = orv_syntax::Span::point(orv_syntax::FileId(0), 0);
    interpreter
        .call_function(&function, args, span)
        .map_err(|failure| failure.message.to_string())
}

#[test]
fn print_writes_a_line() {
    let mut interpreter = Interpreter::new();
    let function = match interpreter.function("print") {
        Some(function) => function,
        None => panic!("print should exist"),
    };
    let span = orv_syntax::Span::point(orv_syntax::FileId(0), 0);
    let _ = interpreter.call_function(&function, vec![Value::str("hi")], span);
    assert_eq!(interpreter.output(), "hi\n");
}

#[test]
fn len_counts_chars_lists_and_maps() {
    assert_eq!(call("len", vec![Value::str("abc")]), Ok(Value::Int(3)));
    assert_eq!(call("len", vec![Value::str("áéí")]), Ok(Value::Int(3)));
    assert_eq!(
        call("len", vec![Value::list(vec![Value::Int(1), Value::Int(2)])]),
        Ok(Value::Int(2))
    );
}

#[test]
fn len_rejects_non_collections() {
    assert!(call("len", vec![Value::Int(1)]).is_err());
}

#[test]
fn range_is_exclusive() {
    assert_eq!(
        call("range", vec![Value::Int(1), Value::Int(4)]),
        Ok(Value::list(vec![
            Value::Int(1),
            Value::Int(2),
            Value::Int(3)
        ]))
    );
}

#[test]
fn range_of_an_empty_span_is_empty() {
    assert_eq!(
        call("range", vec![Value::Int(3), Value::Int(3)]),
        Ok(Value::list(vec![]))
    );
}

#[test]
fn str_renders_values() {
    assert_eq!(call("str", vec![Value::Int(42)]), Ok(Value::str("42")));
    assert_eq!(call("str", vec![Value::Float(1.0)]), Ok(Value::str("1.0")));
    assert_eq!(call("str", vec![Value::Bool(true)]), Ok(Value::str("true")));
}

#[test]
fn int_converts_and_parses() {
    assert_eq!(call("int", vec![Value::str("42")]), Ok(Value::Int(42)));
    assert_eq!(call("int", vec![Value::Float(1.9)]), Ok(Value::Int(1)));
    assert!(call("int", vec![Value::str("abc")]).is_err());
}

#[test]
fn float_converts_and_parses() {
    assert_eq!(call("float", vec![Value::Int(1)]), Ok(Value::Float(1.0)));
    assert_eq!(
        call("float", vec![Value::str("1.5")]),
        Ok(Value::Float(1.5))
    );
    assert!(call("float", vec![Value::str("abc")]).is_err());
}

#[test]
fn assert_passes_and_fails() {
    assert_eq!(call("assert", vec![Value::Bool(true)]), Ok(Value::Unit));
    let failure = call("assert", vec![Value::Bool(false)]);
    assert!(failure.is_err(), "assert(false) must fail");
}

#[test]
fn ok_and_err_build_results() {
    let ok = call("Ok", vec![Value::Int(1)]);
    assert!(matches!(
        ok,
        Ok(Value::Result { ok: true, ref value, .. }) if **value == Value::Int(1)
    ));
    let err = call("Err", vec![Value::str("boom")]);
    assert!(matches!(err, Ok(Value::Result { ok: false, .. })));
}

#[test]
fn all_prelude_names_exist() {
    let interpreter = Interpreter::new();
    for name in [
        "print", "len", "range", "str", "int", "float", "assert", "Ok", "Err",
    ] {
        assert!(
            interpreter.function(name).is_some(),
            "`{name}` should be in the prelude"
        );
    }
}

#[test]
fn failure_forms_display_stably() {
    // `Err` with a `data` Failure-like value reads its `message` field.
    let failure_like = Value::Data {
        name: "Failure".into(),
        fields: std::rc::Rc::new(vec![
            ("kind".into(), Value::str("Boom")),
            ("message".into(), Value::str("something broke")),
        ]),
    };
    let err = call("Err", vec![failure_like]);
    let rendered = match err {
        Ok(Value::Result {
            failure: Some(f), ..
        }) => crate::failure::render(&f),
        other => panic!("expected an Err result, got {other:?}"),
    };
    assert!(rendered.contains("something broke"), "got: {rendered}");
}

#[test]
fn map_keys_display_in_a_stable_way() {
    assert_eq!(display(&MapKey::Int(1).to_value()), "1");
    assert_eq!(display(&MapKey::Str("a".into()).to_value()), "a");
    assert_eq!(display(&MapKey::Bool(true).to_value()), "true");
}
