//! Directed consistency test between sema and runtime.
//!
//! Random byte fuzzing cannot write the exact name of a variant declared
//! elsewhere in the file, which is why the "sema accepts, runtime does not know
//! what to do" class of bug survived hundreds of thousands of fuzz cases
//! (ADR 0017 was found by hand, not by the fuzzer).
//!
//! This test generates small but **consistent** programs from the same
//! structured fragments the language is built from — a `data` declaration and
//! its constructor call share the field names, an `enum` and its `match` share
//! the variant names — and asserts the invariant that matters:
//!
//! > if `check()` reports no error, then `run()` must not fail with one of the
//! > "the runtime does not know this construct" messages.
//!
//! Legitimate runtime failures (division by zero, overflow, index, depth) are
//! excluded by construction: the generator does not emit arithmetic on
//! unchecked values, indexing, or unbounded recursion.

use orv_runtime::driver;
use orv_sema::check as sema_check;
use orv_syntax::{Parser, SourceMap, lex};
use proptest::prelude::*;

/// The failure messages that mean "the runtime cannot evaluate something the
/// sema accepted" (all are `FailureKind::Unsupported`).
const DIVERGENCE_MARKERS: [&str; 4] = [
    "undefined name",
    "is not callable",
    "has no field",
    "has no `match` arm matched",
];

/// A generated program plus a short description for the failure message.
#[derive(Clone, Debug)]
struct Program {
    source: String,
    description: String,
}

/// Builds a program exercising a `data` type with named fields.
fn data_program(field_count: usize, named: bool, reversed: bool) -> Program {
    let fields: Vec<String> = (0..field_count).map(|i| format!("f{i}")).collect();
    let mut decl = String::new();
    decl.push_str("data D {\n");
    for field in &fields {
        decl.push_str(&format!("    {field}: Int,\n"));
    }
    decl.push_str("}\n");

    let values: Vec<String> = (0..field_count).map(|i| i.to_string()).collect();
    let mut args: Vec<String> = Vec::new();
    for (field, value) in fields.iter().zip(values.iter()) {
        if named {
            args.push(format!("{field}: {value}"));
        } else {
            args.push(value.clone());
        }
    }
    if reversed {
        args.reverse();
    }
    let call = format!("D({})", args.join(", "));

    // Reading the first field exercises field access on the value.
    let read = match fields.first() {
        Some(field) => format!("    print(v.{field})\n"),
        None => String::new(),
    };

    Program {
        source: format!("{decl}\nfn main() {{\n    let v = {call}\n{read}    print(v)\n}}\n"),
        description: format!("data fields={field_count} named={named} reversed={reversed}"),
    }
}

/// Builds a program with a unit-only `enum`, used bare, compared, as a field of
/// `data`, in a `match` and inside a list.
fn unit_enum_program(variant_count: usize) -> Program {
    let variants: Vec<String> = (0..variant_count).map(|i| format!("V{i}")).collect();
    let decl = format!("enum E {{ {} }}\n", variants.join(", "));

    let mut arms = String::new();
    for variant in &variants {
        arms.push_str(&format!("        {variant} => print(\"hit\"),\n"));
    }

    let first = variants.first().cloned().unwrap_or_else(|| "V0".to_owned());
    let list = variants.join(", ");

    Program {
        source: format!(
            "{decl}\ndata Holder {{\n    e: E,\n}}\n\n\
             fn main() {{\n\
                 let v = {first}\n\
                 print(v == {first})\n\
                 let h = Holder(e: v)\n\
                 print(h.e == {first})\n\
                 match v {{\n{arms}    }}\n\
                 let all = [{list}]\n\
                 print(len(all))\n\
                 for item in all {{\n\
                     match item {{\n{arms}        }}\n\
                 }}\n\
             }}\n"
        ),
        description: format!("unit enum variants={variant_count}"),
    }
}

/// Builds a program with a payload `enum`, constructing via the variant name
/// and via a first-class constructor value.
fn payload_enum_program(arity: usize) -> Program {
    let params: Vec<String> = (0..arity).map(|_| "Int".to_owned()).collect();
    let decl = format!("enum P {{ C({}) }}\n", params.join(", "));
    let args: Vec<String> = (0..arity).map(|i| i.to_string()).collect();
    let args = args.join(", ");
    let patterns: Vec<String> = (0..arity).map(|i| format!("a{i}")).collect();

    Program {
        source: format!(
            "{decl}\nfn main() {{\n\
                 let built = C({args})\n\
                 print(built)\n\
                 let ctor = C\n\
                 print(ctor({args}))\n\
                 match built {{\n\
                     C({}) => print(\"payload\"),\n\
                 }}\n\
             }}\n",
            patterns.join(", ")
        ),
        description: format!("payload enum arity={arity}"),
    }
}

/// Builds a program mixing functions, closures, collections and control flow.
fn mixed_program(iterations: usize, use_closure: bool, use_map: bool) -> Program {
    let mut body = String::new();
    body.push_str("    let mut total = 0\n");
    if use_closure {
        body.push_str("    let add: fn(Int) -> Int = n => total + n\n");
        body.push_str("    total = total + 1\n");
        body.push_str("    print(add(2))\n");
    }
    body.push_str(&format!(
        "    for i in 0..{iterations} {{\n        total += i\n    }}\n"
    ));
    body.push_str("    print(total)\n");
    if use_map {
        body.push_str("    let m = #{\"a\": 1, \"b\": 2}\n");
        body.push_str("    print(len(m))\n");
        body.push_str("    for key in m {\n        print(key)\n    }\n");
    }
    body.push_str("    let xs = [1, 2, 3]\n    print(len(xs))\n");

    Program {
        source: format!("fn helper(n: Int) -> Int {{\n    n * 2\n}}\n\nfn main() {{\n{body}}}\n"),
        description: format!("mixed iters={iterations} closure={use_closure} map={use_map}"),
    }
}

/// Runs one generated program and asserts the sema/runtime agreement invariant.
///
/// Returns `Ok(())` when the program is consistent (either the sema rejects it,
/// or it runs, or it fails for a legitimate reason).
fn assert_agreement(program: &Program) -> Result<(), TestCaseError> {
    let mut sources = SourceMap::new();
    let id = sources.add("generated.orv", program.source.as_str());
    let file = match sources.file(id).cloned() {
        Some(file) => file,
        None => {
            return Err(TestCaseError::fail(format!(
                "source map failure for {}",
                program.description
            )));
        }
    };

    let (tokens, lexical) = lex(&file);
    let mut parser = Parser::new(&tokens, lexical);
    let parsed = parser.parse_program();

    let Some(ast) = parsed.program else {
        // The generator produces valid syntax; a parse failure is a generator
        // bug worth surfacing rather than silently skipping.
        return Err(TestCaseError::fail(format!(
            "generated program did not parse: {}\n{}",
            program.description, program.source
        )));
    };

    // A program the parser rejected has front-end diagnostics; only programs
    // that get as far as sema are relevant to this invariant.
    if parsed.diagnostics.has_errors() {
        return Ok(());
    }

    let checked = sema_check(&ast);
    if checked.has_errors() {
        // The sema rejected it, so the runtime is never asked to evaluate it.
        return Ok(());
    }

    let outcome = driver::run(&ast);
    match outcome.failure {
        None => Ok(()),
        Some(failure) => {
            let message = failure.message.to_string();
            if DIVERGENCE_MARKERS
                .iter()
                .any(|marker| message.contains(marker))
            {
                Err(TestCaseError::fail(format!(
                    "sema accepted a program the runtime cannot evaluate \
                     ({}): {message}\n--- source ---\n{}",
                    program.description, program.source
                )))
            } else {
                // A legitimate runtime failure (R0001-R0004) is acceptable.
                Ok(())
            }
        }
    }
}

proptest! {
    /// `data` construction and field access stay in sync between the layers.
    #[test]
    fn data_construction_agrees(
        field_count in 0usize..5,
        named in any::<bool>(),
        reversed in any::<bool>(),
    ) {
        assert_agreement(&data_program(field_count, named, reversed))?;
    }

    /// Unit variants are constructible both as values and in collections.
    #[test]
    fn unit_enum_agrees(variant_count in 1usize..5) {
        assert_agreement(&unit_enum_program(variant_count))?;
    }

    /// Payload variants work both called directly and as constructor values.
    #[test]
    fn payload_enum_agrees(arity in 1usize..4) {
        assert_agreement(&payload_enum_program(arity))?;
    }

    /// Functions, closures, collections and loops stay in sync.
    #[test]
    fn mixed_program_agrees(
        iterations in 0usize..6,
        use_closure in any::<bool>(),
        use_map in any::<bool>(),
    ) {
        assert_agreement(&mixed_program(iterations, use_closure, use_map))?;
    }
}
