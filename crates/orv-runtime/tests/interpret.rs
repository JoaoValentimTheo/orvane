//! End-to-end interpreter tests: whole programs through lex → parse → check →
//! run, plus a proptest proving the runtime never panics on arbitrary input.

use orv_runtime::driver;
use orv_syntax::{Parser, Program, SourceMap, lex};

/// Lexes, parses and returns the program, asserting the front end was clean.
fn parse_ok(text: &str) -> Program {
    let mut sources = SourceMap::new();
    let id = sources.add("test.orv", text);
    let file = match sources.file(id).cloned() {
        Some(file) => file,
        None => orv_syntax::SourceFile::new(orv_syntax::FileId(0), "test.orv", text),
    };
    let (tokens, lexical) = lex(&file);
    let mut parser = Parser::new(&tokens, lexical);
    let result = parser.parse_program();
    assert!(
        !result.has_errors(),
        "front end errors for {text:?}: {:?}",
        result
            .diagnostics
            .as_slice()
            .iter()
            .map(|d| (d.code, d.message.as_str()))
            .collect::<Vec<_>>()
    );
    match result.program {
        Some(program) => program,
        None => panic!("no program for {text:?}"),
    }
}

/// Runs `text` and returns its stdout, asserting it succeeded.
fn run(text: &str) -> String {
    let program = parse_ok(text);
    let outcome = driver::run(&program);
    assert!(
        outcome.is_ok(),
        "unexpected failure: {:?}",
        outcome.failure.as_ref().map(|f| f.message.to_string())
    );
    outcome.output
}

/// Runs `text` and returns the failure message.
fn run_failure(text: &str) -> String {
    let program = parse_ok(text);
    let outcome = driver::run(&program);
    match outcome.failure {
        Some(failure) => failure.message.to_string(),
        None => panic!("expected a failure for {text:?}"),
    }
}

/// Wraps a body in `fn main`.
fn main(body: &str) -> String {
    format!("fn main() {{\n{body}\n}}\n")
}

// --- Basics -----------------------------------------------------------------

#[test]
fn runs_the_hello_example() {
    // §4.1
    assert_eq!(
        run("fn main() {\n    print(\"Olá, Orvane!\")\n}\n"),
        "Olá, Orvane!\n"
    );
}

#[test]
fn runs_arithmetic() {
    assert_eq!(run(&main("print(1 + 2 * 3)")), "7\n");
    assert_eq!(run(&main("print(10 - 3 - 2)")), "5\n");
    assert_eq!(run(&main("print(7 / 2)")), "3\n");
    assert_eq!(run(&main("print(7 % 2)")), "1\n");
    assert_eq!(run(&main("print(1.5 + 1.0)")), "2.5\n");
}

#[test]
fn runs_comparisons_and_logic() {
    assert_eq!(run(&main("print(1 < 2)")), "true\n");
    assert_eq!(run(&main("print(1 == 1)")), "true\n");
    assert_eq!(run(&main("print(1 != 2)")), "true\n");
    assert_eq!(run(&main("print(true and false)")), "false\n");
    assert_eq!(run(&main("print(true or false)")), "true\n");
    assert_eq!(run(&main("print(not true)")), "false\n");
}

#[test]
fn runs_string_concatenation() {
    assert_eq!(run(&main(r#"print("a" + "b")"#)), "ab\n");
}

#[test]
fn runs_interpolated_string_literal_parts() {
    // The alpha does not evaluate `{...}` parts (ADR 0012); the literal parts
    // are kept, which is enough to see output.
    assert_eq!(run(&main(r#"print("hello world")"#)), "hello world\n");
}

// --- Functions and closures -------------------------------------------------

#[test]
fn runs_a_function_call() {
    assert_eq!(
        run(
            "fn add(a: Int, b: Int) -> Int {\n    a + b\n}\n\nfn main() {\n    print(add(1, 2))\n}\n"
        ),
        "3\n"
    );
}

#[test]
fn runs_recursion() {
    let source = "\
fn fib(n: Int) -> Int {
    if n < 2 {
        n
    } else {
        fib(n - 1) + fib(n - 2)
    }
}

fn main() {
    print(fib(10))
}
";
    assert_eq!(run(source), "55\n");
}

#[test]
fn runs_a_closure_capturing_a_mutable_binding() {
    let source = "\
fn main() {
    let mut count = 0
    let bump = x => count + x
    count = 5
    print(bump(1))
}
";
    // The closure captures the binding, so it sees the updated value.
    assert_eq!(run(source), "6\n");
}

#[test]
fn a_returned_lambda_keeps_its_captured_parameters() {
    // The `adder` frame (holding `n`) is popped before the returned lambda is
    // called, so the closure must keep the scope it captured. Regression for
    // `R0010: undefined name n` (sema accepted, runtime did not).
    let source = "\
fn adder(n: Int) -> fn(Int) -> Int {
    let f: fn(Int) -> Int = (x) => x + n
    return f
}

fn main() {
    print(adder(10)(5))
}
";
    assert_eq!(run(source), "15\n");
}

#[test]
fn a_nested_lambda_closes_over_the_outer_lambda_parameter() {
    // The inner lambda captures `a`, a parameter of the outer lambda frame.
    let source = "\
fn main() {
    let f: fn(Int) -> fn(Int) -> Int = a => (b) => a + b
    print(f(1)(2))
}
";
    assert_eq!(run(source), "3\n");
}

#[test]
fn a_lambda_returned_from_a_function_still_sees_later_outer_mutation() {
    // Capture is by reference for `let mut`, even across a returning frame.
    let source = "\
fn make() -> fn() -> Int {
    let mut n = 1
    let read: fn() -> Int = () => n
    n = 7
    return read
}

fn main() {
    print(make()())
}
";
    assert_eq!(run(source), "7\n");
}

#[test]
fn runs_a_lambda_passed_to_a_function() {
    let source = "\
fn apply(f: fn(Int) -> Int, x: Int) -> Int {
    f(x)
}

fn main() {
    print(apply(y => y * 2, 21))
}
";
    assert_eq!(run(source), "42\n");
}

#[test]
fn a_break_inside_a_lambda_cannot_leave_the_call() {
    // The sema rejects this (`E0303`), so the test bypasses the checker to
    // exercise the runtime invariant directly: control flow is confined to a
    // call (ADR 0021). Before the fix, the caller's `for` silently ended after
    // the first iteration, printing `before` / `end` with no error.
    let source = "\
fn main() {
    for i in 0..3 {
        print(\"before\")
        let f: fn() -> () = () => { break }
        f()
        print(\"after\")
    }
    print(\"end\")
}
";
    let message = run_failure(source);
    assert!(
        message.contains("cannot leave the function"),
        "expected a confinement failure, got {message:?}"
    );
}

#[test]
fn a_loop_inside_a_lambda_still_works() {
    // The legitimate case: `break` belongs to the lambda's own loop and is
    // consumed there, so it never reaches the call boundary.
    let source = "\
fn main() {
    let stop: fn() -> () = () => {
        for i in 0..10 {
            if i == 2 { break }
            print(i)
        }
    }
    stop()
    print(\"done\")
}
";
    assert_eq!(run(source), "0\n1\ndone\n");
}

// --- Control flow -----------------------------------------------------------

#[test]
fn runs_while_with_mutation() {
    let source = "\
fn main() {
    let mut i = 0
    let mut total = 0
    while i < 5 {
        total += i
        i += 1
    }
    print(total)
}
";
    assert_eq!(run(source), "10\n");
}

#[test]
fn runs_break_and_continue() {
    let source = "\
fn main() {
    let mut total = 0
    for i in range(0, 10) {
        if i == 5 {
            break
        }
        if i % 2 == 0 {
            continue
        }
        total += i
    }
    print(total)
}
";
    // Odd numbers under 5: 1 + 3 = 4.
    assert_eq!(run(source), "4\n");
}

#[test]
fn runs_for_over_a_range() {
    let source = "\
fn main() {
    for i in 1..=3 {
        print(i)
    }
}
";
    assert_eq!(run(source), "1\n2\n3\n");
}

#[test]
fn runs_the_fizzbuzz_reference_program() {
    // Appendix A1, byte for byte.
    let source = r#"
fn main() {
    for i in 1..=15 {
        let out = if i % 15 == 0 { "FizzBuzz" }
                  else if i % 3 == 0 { "Fizz" }
                  else if i % 5 == 0 { "Buzz" }
                  else { str(i) }
        print(out)
    }
}
"#;
    assert_eq!(
        run(source),
        "1\n2\nFizz\n4\nBuzz\nFizz\n7\n8\nFizz\nBuzz\n11\nFizz\n13\n14\nFizzBuzz\n"
    );
}

#[test]
fn runs_if_as_an_expression() {
    assert_eq!(
        run(&main(
            r#"let x = if true { "a" } else { "b" }
print(x)"#
        )),
        "a\n"
    );
}

// --- Collections ------------------------------------------------------------

#[test]
fn runs_list_operations() {
    let source = "\
fn main() {
    let xs = [1, 2, 3]
    print(len(xs))
    print(xs[0])
    print(xs[2])
}
";
    assert_eq!(run(source), "3\n1\n3\n");
}

#[test]
fn runs_map_operations() {
    let source = "\
fn main() {
    let m = #{\"a\": 1, \"b\": 2}
    print(len(m))
    print(m[\"b\"])
}
";
    assert_eq!(run(source), "2\n2\n");
}

#[test]
fn runs_list_equality() {
    assert_eq!(run(&main("print([1, 2] == [1, 2])")), "true\n");
    assert_eq!(run(&main("print([1, 2] == [1, 3])")), "false\n");
}

#[test]
fn runs_for_over_a_list() {
    let source = "\
fn main() {
    let mut total = 0
    for x in [10, 20, 30] {
        total += x
    }
    print(total)
}
";
    assert_eq!(run(source), "60\n");
}

// --- data and enum ----------------------------------------------------------

#[test]
fn runs_data_construction_and_display() {
    // §4.2
    let source = r#"
data User {
    name: Str,
    age: Int,
    email: Str? = none,
}

fn main() {
    let u = User(name: "Mel", age: 30)
    print(u)
    print(u == User("Mel", 30))
}
"#;
    assert_eq!(
        run(source),
        "User(name: \"Mel\", age: 30, email: none)\ntrue\n"
    );
}

#[test]
fn runs_field_access() {
    let source = "\
data Point {
    x: Int,
    y: Int,
}

fn main() {
    let p = Point(x: 3, y: 4)
    print(p.x * p.x + p.y * p.y)
}
";
    assert_eq!(run(source), "25\n");
}

#[test]
fn runs_a_unit_only_enum() {
    // Regression: the checker resolves `Active` (a unit variant) to the enum
    // type, so the runtime must be able to build it too. Before the fix this
    // failed with R0010 "undefined name `Active`" even though `orv check`
    // returned 0.
    let source = "\
enum Status { Active, Done }

fn main() {
    let s = Active
    match s {
        Active => print(\"a\"),
        Done => print(\"d\"),
    }
}
";
    assert_eq!(run(source), "a\n");
}

#[test]
fn runs_unit_variants_as_values_and_in_collections() {
    let source = "\
enum Status { Active, Done }

fn label(s: Status) -> Str {
    match s {
        Active => \"active\",
        Done => \"done\",
    }
}

fn main() {
    let all = [Active, Done]
    for s in all {
        print(label(s))
    }
    print(label(Active))
    print(Active == Active)
    print(Active == Done)
}
";
    assert_eq!(run(source), "active\ndone\nactive\ntrue\nfalse\n");
}

#[test]
fn a_payload_variant_is_a_first_class_constructor() {
    // ADR 0017: the checker types a bare `Circle` as `fn(Float) -> Shape`, so
    // the runtime provides the matching value — a callable constructor.
    let source = "\
enum Shape { Circle(Float) }

fn main() {
    let ctor = Circle
    print(ctor(2.0))
    print(ctor(1.5))
}
";
    assert_eq!(run(source), "Circle(2.0)\nCircle(1.5)\n");
}

#[test]
fn a_constructor_can_be_passed_to_a_higher_order_function() {
    let source = "\
enum Shape { Circle(Float) }

fn apply(f: fn(Float) -> Shape) -> Shape {
    f(3.0)
}

fn main() {
    print(apply(Circle))
}
";
    assert_eq!(run(source), "Circle(3.0)\n");
}

#[test]
fn a_constructor_equality_and_display_match_the_direct_form() {
    let source = "\
enum Shape { Circle(Float) }

fn main() {
    let ctor = Circle
    print(ctor(1.0) == Circle(1.0))
    print(ctor(1.0) == Circle(2.0))
}
";
    assert_eq!(run(source), "true\nfalse\n");
}

#[test]
fn runs_the_enum_and_match_example() {
    // Appendix A2
    let source = r#"
enum Shape { Circle(Float), Rect(Float, Float) }

fn area(s: Shape) -> Float {
    match s {
        Circle(r) => 3.14159 * r * r,
        Rect(w, h) => w * h,
    }
}

fn main() { print(area(Rect(2.0, 3.0))) }
"#;
    assert_eq!(run(source), "6.0\n");
}

#[test]
fn runs_match_on_integers() {
    let source = "\
fn label(n: Int) -> Str {
    match n {
        0 => \"zero\",
        1 => \"one\",
        _ => \"many\",
    }
}

fn main() {
    print(label(0))
    print(label(1))
    print(label(9))
}
";
    assert_eq!(run(source), "zero\none\nmany\n");
}

// --- Runtime failures -------------------------------------------------------

#[test]
fn division_by_zero_is_a_failure() {
    assert!(run_failure(&main("print(1 / 0)")).contains("division by zero"));
}

#[test]
fn modulo_by_zero_is_a_failure() {
    assert!(run_failure(&main("print(1 % 0)")).contains("division by zero"));
}

#[test]
fn integer_overflow_is_a_failure() {
    let message = run_failure(&main("print(9223372036854775807 + 1)"));
    assert!(message.contains("overflow"), "got: {message}");
}

#[test]
fn index_out_of_bounds_is_a_failure() {
    let message = run_failure(&main("let xs = [1]\nprint(xs[5])"));
    assert!(message.contains("out of bounds"), "got: {message}");
}

#[test]
fn missing_map_key_is_a_failure() {
    let message = run_failure(&main(
        r#"let m = #{"a": 1}
print(m["z"])"#,
    ));
    assert!(message.contains("not found"), "got: {message}");
}

/// A program whose `f(n)` recurses exactly `n` times, plus the `main` frame.
fn recursion_program(depth: i64) -> String {
    format!(
        "fn f(n: Int) -> Int {{\n    if n == 0 {{\n        0\n    }} else {{\n        f(n - 1)\n    }}\n}}\n\nfn main() {{\n    print(f({depth}))\n}}\n"
    )
}

#[test]
fn the_call_depth_boundary_is_exact() {
    // ADR 0016: `MAX_CALL_DEPTH` counts the `main` frame, so the deepest chain
    // that fits is `main` + 47 `f` frames = 48. This pins the boundary so a
    // future change to the counter cannot shift it silently.
    assert_eq!(run(&recursion_program(46)), "0\n");

    let message = run_failure(&recursion_program(47));
    assert!(
        message.contains("call depth exceeded"),
        "expected the depth limit at 47, got: {message}"
    );
}

#[test]
fn infinite_recursion_is_a_failure_not_a_crash() {
    let source = "\
fn boom(n: Int) -> Int {
    boom(n + 1)
}

fn main() {
    print(boom(0))
}
";
    let message = run_failure(source);
    assert!(
        message.contains("depth") || message.contains("recursion"),
        "got: {message}"
    );
}

#[test]
fn a_missing_main_is_a_failure() {
    let program = parse_ok("fn other() {\n}\n");
    let outcome = driver::run(&program);
    assert!(outcome.failure.is_some());
}

#[test]
fn try_returns_an_err_result_instead_of_propagating() {
    let source = "\
fn risky() -> Int {
    fail \"boom\"
}

fn main() {
    let result = try risky()
    print(result)
}
";
    // `fail` as a statement is out of the alpha, so the failure is `Unsupported`
    // in practice; either way `try` must not propagate it.
    let output = run(source);
    assert!(output.starts_with("Err("), "got: {output}");
}

// --- Property tests ---------------------------------------------------------

mod prop {
    use orv_runtime::driver;
    use orv_syntax::{Parser, lex};
    use proptest::prelude::*;

    /// Runs whatever the front end can build; the runtime must never panic.
    fn run_text(text: &str) {
        let mut sources = orv_syntax::SourceMap::new();
        let id = sources.add("fuzz.orv", text);
        let file = match sources.file(id).cloned() {
            Some(file) => file,
            None => orv_syntax::SourceFile::new(orv_syntax::FileId(0), "fuzz.orv", text),
        };
        let (tokens, lexical) = lex(&file);
        let mut parser = Parser::new(&tokens, lexical);
        if let Some(program) = parser.parse_program().program {
            let _ = driver::run(&program);
        }
    }

    proptest! {
        /// The runtime never panics for arbitrary input (SPEC §11 item 3).
        #[test]
        fn runtime_never_panics(text in any::<String>()) {
            run_text(&text);
        }

        /// Structured fragments reach the interpreter far more often than
        /// random bytes, exercising arithmetic, indexing and recursion.
        #[test]
        fn runtime_never_panics_on_fragments(
            fragments in prop::collection::vec(
                prop::sample::select(vec![
                    "fn", "main", "(", ")", "{", "}", "[", "]", "#{", ",", ":", ".", "=>",
                    "->", "?", "?.", "??", "=", "==", "+", "-", "*", "/", "%", "as",
                    "let", "mut", "if", "else", "match", "for", "in", "while", "return",
                    "break", "continue", "fail", "try", "data", "enum", "true", "false",
                    "none", "and", "or", "not", "Int", "Str", "List", "Map",
                    "1", "0", "-1", "1.5", "\"s\"", "ident", "_", "\n", " ", "x => x",
                    "print", "len", "range", "str", "assert",
                ]),
                0..30,
            )
        ) {
            let text: String = fragments.concat();
            run_text(&text);
        }
    }
}
