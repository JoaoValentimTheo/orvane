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

/// Builds a program that puts a tuple in a `data` field, in a list, and reads
/// it back through a `match`-free path.
fn tuple_program(arity: usize, in_data: bool, in_list: bool) -> Program {
    let arity = arity.max(2);
    let elements: Vec<String> = (0..arity).map(|i| i.to_string()).collect();
    let literal = format!("({})", elements.join(", "));

    let mut body = String::new();
    let mut decl = String::new();
    body.push_str(&format!("    let t = {literal}\n    print(t)\n"));
    if in_data {
        decl.push_str("data Holder {\n    t: (Int, Int),\n}\n\n");
        let pair = format!("({}, {})", 1, 2);
        body.push_str(&format!(
            "    let h = Holder(t: {pair})\n    print(h.t == {pair})\n"
        ));
    }
    if in_list {
        body.push_str("    let xs = [(1, 2), (3, 4)]\n    print(len(xs))\n");
    }

    Program {
        source: format!("{decl}fn main() {{\n{body}}}\n"),
        description: format!("tuple arity={arity} data={in_data} list={in_list}"),
    }
}

/// Builds a program with lambdas that capture a frame parameter or another
/// lambda's parameter, return a lambda, and nest one level.
fn lambda_program(captures_frame: bool, from_return: bool, nested: bool) -> Program {
    if captures_frame && from_return {
        // `adder`'s returned lambda closes over its parameter `n`.
        return Program {
            source: "\
fn adder(n: Int) -> fn(Int) -> Int {
    let f: fn(Int) -> Int = (x) => x + n
    return f
}

fn main() {
    print(adder(10)(5))
}
"
            .to_owned(),
            description: "lambda captures frame, returned".to_owned(),
        };
    }

    let mut body = String::new();
    if nested {
        body.push_str("    let f: fn(Int) -> fn(Int) -> Int = a => (b) => a + b\n");
        body.push_str("    print(f(1)(2))\n");
    } else {
        body.push_str("    let n = 10\n");
        body.push_str("    let f: fn(Int) -> Int = (x) => x + n\n");
        body.push_str("    print(f(5))\n");
    }

    Program {
        source: format!("fn main() {{\n{body}}}\n"),
        description: format!("lambda frame={captures_frame} return={from_return} nested={nested}"),
    }
}

/// Builds a program with an optional field, read via `?.` and `??`, and a
/// `data?` binding that may be `none`.
fn optional_program(field_present: bool, whole_optional: bool) -> Program {
    let mut decl = String::from("data Inner {\n    v: Int,\n}\n\n");
    let mut body = String::new();
    if whole_optional {
        decl.push_str("data Outer {\n    i: Inner?,\n}\n\n");
        let ctor = if field_present {
            "Outer(i: Inner(v: 9))".to_owned()
        } else {
            "Outer(i: none)".to_owned()
        };
        body.push_str(&format!("    let o = {ctor}\n"));
        body.push_str("    print(o.i?.v ?? 0)\n");
    } else {
        decl.push_str("data Outer {\n    v: Int?,\n}\n\n");
        let ctor = if field_present {
            "Outer(v: 9)".to_owned()
        } else {
            "Outer(v: none)".to_owned()
        };
        body.push_str(&format!("    let o = {ctor}\n"));
        body.push_str("    print(o.v ?? 0)\n");
    }

    Program {
        source: format!("{decl}fn main() {{\n{body}}}\n"),
        description: format!("optional field={field_present} whole={whole_optional}"),
    }
}

/// Builds a program with a list of `data`, a map of `data`, and an enum list.
fn user_type_collections_program(in_list: bool, in_map: bool, enum_list: bool) -> Program {
    let mut decl = String::from("data P {\n    x: Int,\n}\n\n");
    let mut body = String::new();

    if in_list {
        body.push_str("    let ps = [P(x: 1), P(x: 2)]\n");
        body.push_str("    print(len(ps))\n");
        body.push_str("    for p in ps {\n        print(p.x)\n    }\n");
    }
    if in_map {
        body.push_str("    let m = #{\"a\": P(x: 1)}\n");
        body.push_str("    print(m[\"a\"].x)\n");
    }
    if enum_list {
        decl.push_str("enum E {\n    A,\n    B,\n}\n\n");
        body.push_str("    let es = [A, B]\n");
        body.push_str("    print(len(es))\n");
        body.push_str("    for e in es {\n        match e {\n            A => print(\"a\"),\n            B => print(\"b\"),\n        }\n    }\n");
    }

    Program {
        source: format!("{decl}fn main() {{\n{body}}}\n"),
        description: format!("user collections list={in_list} map={in_map} enum_list={enum_list}"),
    }
}

/// Builds a program that interpolates every value kind a generator can make.
///
/// `"{expr}"` must produce the same text as `str(expr)` (ADR 0022 / 0.1.2);
/// the invariant here is only that the sema and runtime agree, which the
/// generator checks by keeping every interpolation well-typed and in scope.
fn interpolation_program(use_data: bool, use_enum: bool, use_collection: bool) -> Program {
    let mut decl = String::new();
    let mut body = String::new();
    body.push_str("    let n = 7\n");
    body.push_str("    print(\"n={n} and {n + 1}\")\n");
    body.push_str("    print(\"{1.5} {true} {none}\")\n");
    if use_data {
        decl.push_str("data P {\n    x: Int,\n}\n\n");
        body.push_str("    let p = P(x: 3)\n");
        body.push_str("    print(\"p={p} x={p.x}\")\n");
    }
    if use_enum {
        decl.push_str("enum E {\n    A,\n    B(Int),\n}\n\n");
        body.push_str("    let e = B(2)\n");
        body.push_str("    print(\"e={e}\")\n");
    }
    if use_collection {
        body.push_str("    let xs = [1, 2]\n");
        body.push_str("    print(\"xs={xs} len={len(xs)}\")\n");
    }

    Program {
        source: format!("{decl}fn main() {{\n{body}}}\n"),
        description: format!("interpolation data={use_data} enum={use_enum} coll={use_collection}"),
    }
}

/// Builds a program that mutates a `data` field and reads it back.
///
/// The generator keeps the binding `let mut` when it mutates, so the sema
/// accepts; the invariant is that the runtime then runs without a
/// "does not know this construct" failure.
fn field_mutation_program(mutable: bool, read_back: bool, in_loop: bool) -> Program {
    let binding = if mutable { "let mut u" } else { "let u" };
    let mut body = format!("    {binding} = P(x: 1)\n");
    if mutable {
        body.push_str("    u.x = 2\n");
    }
    if read_back {
        body.push_str("    print(u.x)\n");
    }
    if in_loop {
        body.push_str("    for i in 0..3 {\n        print(i)\n    }\n");
    }

    Program {
        source: format!("data P {{\n    x: Int,\n}}\n\nfn main() {{\n{body}}}\n"),
        description: format!("field mutation mutable={mutable} read={read_back} loop={in_loop}"),
    }
}

/// Builds a program with a `data` whose field is a unit enum, mutated in
/// place, crossing ADR 0020 (unit enum) with ADR 0022 (field mutation).
fn enum_field_mutation_program(mutate: bool, compare: bool, interpolate: bool) -> Program {
    let binding = if mutate { "let mut p" } else { "let p" };
    let mut body = format!("    {binding} = P(c: Red)\n    let q = P(c: Red)\n");
    if mutate {
        body.push_str("    p.c = Green\n");
    }
    if compare {
        body.push_str("    print(p == q)\n");
    }
    if interpolate {
        body.push_str("    print(\"{p.c}\")\n");
    }

    Program {
        source: format!(
            "enum Color {{\n    Red,\n    Green,\n}}\n\ndata P {{\n    c: Color,\n}}\n\nfn main() {{\n{body}}}\n"
        ),
        description: format!("enum field mutation={mutate} compare={compare} interp={interpolate}"),
    }
}

/// Builds a program where a `data` binding is mutated and interpolated inside a
/// lambda body, crossing ADR 0021 (call-confined control flow) with the 0.1.2
/// features.
fn lambda_mutation_program(with_loop: bool) -> Program {
    let loop_body = if with_loop {
        "        for i in 0..2 {\n            if i == 1 { break }\n            c.n = c.n + 1\n        }\n"
    } else {
        "        c.n = c.n + 1\n"
    };
    let source = format!(
        "data C {{\n    n: Int,\n}}\n\nfn main() {{\n    let mut c = C(n: 0)\n    let f: fn() -> Str = () => {{\n{loop_body}        \"{{c.n}}\"\n    }}\n    print(f())\n    print(c.n)\n}}\n"
    );
    Program {
        source,
        description: format!("lambda mutation loop={with_loop}"),
    }
}

/// Runs one generated program and asserts the sema/runtime agreement invariant.
///
/// Returns `Ok(())` when the program is consistent (either the sema rejects it,
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

    /// Tuples agree, including a tuple as a `data` field and inside a list.
    #[test]
    fn tuple_agrees(
        arity in 2usize..5,
        in_data in any::<bool>(),
        in_list in any::<bool>(),
    ) {
        assert_agreement(&tuple_program(arity, in_data, in_list))?;
    }

    /// Lambdas agree whether they capture a frame, are returned, or nest.
    #[test]
    fn lambda_agrees(
        captures_frame in any::<bool>(),
        from_return in any::<bool>(),
        nested in any::<bool>(),
    ) {
        assert_agreement(&lambda_program(captures_frame, from_return, nested))?;
    }

    /// Optional fields agree for present/absent and `data?`/`Int?`.
    #[test]
    fn optional_agrees(field_present in any::<bool>(), whole_optional in any::<bool>()) {
        assert_agreement(&optional_program(field_present, whole_optional))?;
    }

    /// Lists/maps of user types and enum lists agree.
    #[test]
    fn user_type_collections_agree(
        in_list in any::<bool>(),
        in_map in any::<bool>(),
        enum_list in any::<bool>(),
    ) {
        assert_agreement(&user_type_collections_program(in_list, in_map, enum_list))?;
    }

    /// Interpolation of the generated value kinds agrees between the layers.
    #[test]
    fn interpolation_agrees(
        use_data in any::<bool>(),
        use_enum in any::<bool>(),
        use_collection in any::<bool>(),
    ) {
        assert_agreement(&interpolation_program(use_data, use_enum, use_collection))?;
    }

    /// Field mutation on a `let mut` binding agrees; an immutable one is
    /// rejected by the sema, which is also consistent.
    #[test]
    fn field_mutation_agrees(
        mutable in any::<bool>(),
        read_back in any::<bool>(),
        in_loop in any::<bool>(),
    ) {
        assert_agreement(&field_mutation_program(mutable, read_back, in_loop))?;
    }

    /// A `data` field holding a unit enum agrees across mutation and compare.
    #[test]
    fn enum_field_mutation_agrees(
        mutate in any::<bool>(),
        compare in any::<bool>(),
        interpolate in any::<bool>(),
    ) {
        assert_agreement(&enum_field_mutation_program(mutate, compare, interpolate))?;
    }

    /// Mutation + interpolation inside a lambda body agrees, with and without
    /// a bounded loop.
    #[test]
    fn lambda_mutation_agrees(with_loop in any::<bool>()) {
        assert_agreement(&lambda_mutation_program(with_loop))?;
    }

    /// Deeply nested interpolation agrees: the whole pipeline either accepts it
    /// or rejects it with a diagnostic, never crashing. The sub-parse shares
    /// the parser's depth budget (ADR 0013), so this also guards the P0 fixed
    /// in `fix-0.1.3`.
    #[test]
    fn nested_interpolation_agrees(levels in 0usize..400) {
        let mut inner = "x".to_owned();
        for _ in 0..levels {
            inner = format!("\"{{{inner}}}\"");
        }
        let program = Program {
            source: format!("fn main() {{\n    let x = 1\n    print({inner})\n}}\n"),
            description: format!("nested interpolation levels={levels}"),
        };
        assert_agreement(&program)?;
    }
}
