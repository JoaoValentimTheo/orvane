//! Orvane runtime: values, environments, the tree-walking interpreter and the
//! program driver (SPEC §5.3, §5.5).
//!
//! The 0.1.0-alpha scope is recorded in ADR 0012: functions, closures, control
//! flow, collections, arithmetic/comparison, string interpolation of literal
//! parts, `print` and the prelude. `intent`/`how`, Python interop, modules and
//! `data`-mutation of nested fields are out.
//!
//! Nothing in this crate panics for user input: arithmetic uses checked
//! operations, indexing is bounds-checked and recursion is depth-limited
//! (ADR 0016), so every problem surfaces as a [`failure::Failure`].

#![forbid(unsafe_code)]

pub mod builtins;
pub mod driver;
pub mod env;
pub mod failure;
pub mod function;
pub mod interpreter;
pub mod value;

pub use driver::{RunOutcome, prepare, register, run};
pub use env::Env;
pub use failure::{Failure, FailureKind, Frame};
pub use function::{Callable, Closure};
pub use interpreter::{Control, EvalResult, Interpreter, MAX_CALL_DEPTH};
pub use value::{MapKey, MapValue, Value, display, repr};
