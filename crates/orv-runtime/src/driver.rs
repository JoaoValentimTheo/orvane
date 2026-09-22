//! Running a whole program (SPEC §10 `orv run`).
//!
//! The driver registers the program's declarations (functions, `data` schemas,
//! enum variants) and then calls `main`, which is the only entry point §4 uses.

use std::rc::Rc;

use orv_syntax::{Item, ItemKind, Program};

use crate::failure::Failure;
use crate::interpreter::{EvalResult, Interpreter};
use crate::value::Value;

/// What running a program produced.
#[derive(Debug)]
pub struct RunOutcome {
    /// Everything `print` wrote, in order.
    pub output: String,
    /// The failure that escaped `main`, when there was one.
    pub failure: Option<Box<Failure>>,
}

impl RunOutcome {
    /// Whether the program completed without an escaping failure.
    pub fn is_ok(&self) -> bool {
        self.failure.is_none()
    }
}

/// Registers a program's declarations in a fresh interpreter.
pub fn prepare(program: &Program) -> Interpreter {
    let mut interpreter = Interpreter::new();
    register(&mut interpreter, program);
    interpreter
}

/// Registers an already-created interpreter's program declarations.
pub fn register(interpreter: &mut Interpreter, program: &Program) {
    // Functions first, so a later function can call an earlier one regardless
    // of declaration order (the checker already allows either order).
    for item in &program.items {
        if let ItemKind::Fn(decl) = &item.kind {
            let closure = Rc::new(crate::function::Closure::named(Rc::new(decl.clone())));
            interpreter.define_function(&decl.name, closure);
        }
    }
    for item in &program.items {
        register_types(interpreter, item);
    }
}

/// Registers the type schemas carried by one item.
fn register_types(interpreter: &mut Interpreter, item: &Item) {
    match &item.kind {
        ItemKind::Data(decl) => {
            let fields = decl.fields.iter().map(|field| field.name.clone()).collect();
            interpreter.define_data_schema(&decl.name, fields);
        }
        ItemKind::Enum(decl) => {
            for variant in &decl.variants {
                interpreter.define_variant_schema(&variant.name, &decl.name, variant.payload.len());
            }
        }
        ItemKind::Fn(_) | ItemKind::Use(_) => {}
    }
}

/// Runs `program`'s `main`.
///
/// A program without `main` is not an error here: `orv check` covers that case,
/// and `orv run` only needs to report the missing entry point.
pub fn run(program: &Program) -> RunOutcome {
    let mut interpreter = prepare(program);

    let Some(main) = interpreter.function("main") else {
        return RunOutcome {
            output: interpreter.take_output(),
            failure: Some(Box::new(Failure::new(
                crate::failure::FailureKind::Unsupported,
                "this program has no `main` function",
                orv_syntax::Span::point(orv_syntax::FileId(0), 0),
            ))),
        };
    };

    let span = orv_syntax::Span::point(orv_syntax::FileId(0), 0);
    let result = interpreter.call_function(&main, Vec::new(), span);
    let output = interpreter.take_output();

    match result {
        Ok(_) => RunOutcome {
            output,
            failure: None,
        },
        Err(failure) => RunOutcome {
            output,
            failure: Some(failure),
        },
    }
}

/// Runs `main` and returns its value, for tests that care about both.
pub fn run_main_value(program: &Program) -> EvalResult {
    let mut interpreter = prepare(program);
    match interpreter.function("main") {
        Some(main) => {
            let span = orv_syntax::Span::point(orv_syntax::FileId(0), 0);
            interpreter.call_function(&main, Vec::new(), span)
        }
        None => Err(Box::new(Failure::new(
            crate::failure::FailureKind::Unsupported,
            "this program has no `main` function",
            orv_syntax::Span::point(orv_syntax::FileId(0), 0),
        ))),
    }
}

/// Whether a value is the unit value.
pub fn is_unit(value: &Value) -> bool {
    matches!(value, Value::Unit)
}
