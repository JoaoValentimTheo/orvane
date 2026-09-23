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

/// Asserts that a failure's span covers `needle` inside `source`, i.e. the
/// diagnostic points at the inner expression (here: inside `{...}`), not at
/// the string literal or the whole program (item (e)).
fn assert_inner_span(source: &str, span: orv_syntax::Span, needle: &str) {
    let start = source
        .find(needle)
        .unwrap_or_else(|| panic!("{needle:?} not found in {source:?}")) as u32;
    let end = start + needle.len() as u32;
    let text = source
        .get(span.start as usize..span.end as usize)
        .unwrap_or("<span out of source bounds>");
    assert_eq!(
        (span.start, span.end, text),
        (start, end, needle),
        "expected the span of the inner expression {needle:?}"
    );
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

// --- String interpolation (SPEC §5.1) ---------------------------------------

#[test]
fn interpolation_of_every_value_kind_matches_str() {
    // The acceptance invariant: `print("{x}")` == `print(str(x))` for every
    // value kind, because both go through `value::display`.
    let source = "\
enum Color { Red, Green(Int) }
data User { name: Str, age: Int, email: Str? = none }

fn main() {
    print(\"{1}|{str(1)}\")
    print(\"{1.5}|{str(1.5)}\")
    print(\"{true}|{str(true)}\")
    print(\"{none}|{str(none)}\")
    print(\"{Red}|{str(Red)}\")
    print(\"{Green(3)}|{str(Green(3))}\")
    let u = User(name: \"Mel\", age: 30)
    print(\"{u}|{str(u)}\")
    print(\"{[1, 2]}|{str([1, 2])}\")
    print(\"{#{\"a\": 1}}|{str(#{\"a\": 1})}\")
}
";
    assert_eq!(
        run(source),
        "1|1\n1.5|1.5\ntrue|true\nnone|none\nRed|Red\nGreen(3)|Green(3)\n\
         User(name: \"Mel\", age: 30, email: none)|User(name: \"Mel\", age: 30, email: none)\n\
         [1, 2]|[1, 2]\n#{\"a\": 1}|#{\"a\": 1}\n"
    );
}

#[test]
fn interpolation_concatenates_with_the_literal_text() {
    let source = "\
fn main() {
    let n = 3
    print(\"n = {n}, twice = {n * 2}!\")
}
";
    assert_eq!(run(source), "n = 3, twice = 6!\n");
}

#[test]
fn interpolation_can_nest() {
    // The lexer already accepted `"{f("{x}")}"`; it must now evaluate.
    let source = "\
fn id(s: Str) -> Str { s }
fn main() {
    let x = 7
    print(\"{id(\"{x}\")}\")
}
";
    assert_eq!(run(source), "7\n");
}

#[test]
fn an_interpolation_sees_the_scope_around_it() {
    let source = "\
fn main() {
    let outer = 1
    if true {
        let inner = 2
        print(\"{outer}-{inner}\")
    }
}
";
    assert_eq!(run(source), "1-2\n");
}

#[test]
fn a_runtime_error_inside_an_interpolation_has_the_inner_span() {
    let source = "fn main() {\n    print(\"v={1/0}\")\n}\n";
    let program = parse_ok(source);
    let outcome = driver::run(&program);
    let failure = outcome.failure.expect("a failure");
    assert_eq!(
        failure.kind,
        orv_runtime::FailureKind::DivisionByZero,
        "got {failure:?}"
    );
    assert!(
        failure.message.contains("division by zero"),
        "expected a division failure, got {failure:?}"
    );
    assert_inner_span(source, failure.span, "1/0");
}

#[test]
fn a_data_dependent_failure_inside_an_interpolation_keeps_its_code() {
    // Item (e): the failure code must be the same one the expression would
    // raise outside the string, not a generic interpolation error. Index out of
    // bounds is data-dependent (only fails on the third iteration here).
    let source = "\
fn main() {
    let xs = [10, 20]
    for i in 0..3 {
        print(\"v={xs[i]}\")
    }
}
";
    let program = parse_ok(source);
    let outcome = driver::run(&program);
    assert!(!outcome.is_ok(), "expected the out-of-bounds failure");
    assert_eq!(outcome.output, "v=10\nv=20\n");
    let failure = outcome.failure.expect("a failure");
    assert_eq!(
        failure.kind,
        orv_runtime::FailureKind::IndexOutOfBounds,
        "got {failure:?}"
    );
    assert!(failure.message.contains("out of bounds"), "got {failure:?}");
    assert_inner_span(source, failure.span, "xs[i]");
}

#[test]
fn a_failure_inside_a_match_inside_an_interpolation_keeps_its_code() {
    let source = "\
fn main() {
    let x = 0
    print(\"v={match x { 0 => 1/0, _ => x }}\")
}
";
    let program = parse_ok(source);
    let outcome = driver::run(&program);
    let failure = outcome.failure.expect("a failure");
    assert_eq!(
        failure.kind,
        orv_runtime::FailureKind::DivisionByZero,
        "got {failure:?}"
    );
    assert!(
        failure.message.contains("division by zero"),
        "expected a division failure, got {failure:?}"
    );
    assert_inner_span(source, failure.span, "1/0");
}

#[test]
fn a_missing_map_key_inside_an_interpolation_reports_r0003() {
    let source = "\
fn main() {
    let m = #{\"a\": 1}
    let k = \"b\"
    print(\"v={m[k]}\")
}
";
    let program = parse_ok(source);
    let outcome = driver::run(&program);
    let failure = outcome.failure.expect("a failure");
    assert_eq!(
        failure.kind,
        orv_runtime::FailureKind::MissingKey,
        "got {failure:?}"
    );
    assert_inner_span(source, failure.span, "m[k]");
}

#[test]
fn many_sequential_interpolations_scale_linearly() {
    // Item (b): N non-nested `{expr}` in one string must cost O(N), i.e. the
    // sub-parse of each part is independent and cheap. The bound is generous
    // (seconds, not milliseconds) so this is not flaky on a busy CI machine,
    // but it would catch an accidental O(N^2) (e.g. re-scanning the whole text
    // per part).
    use std::time::Instant;

    fn build(count: usize) -> String {
        let parts = "{x}".repeat(count);
        format!("fn main() {{\n    let x = 1\n    print(\"{parts}\")\n}}\n")
    }

    let small = 2_000;
    let large = 16_000; // 8x the parts

    let start = Instant::now();
    let output = run(&build(small));
    let small_time = start.elapsed();

    let start = Instant::now();
    let large_output = run(&build(large));
    let large_time = start.elapsed();

    assert_eq!(output.len(), small + 1); // each `{x}` prints one char, plus `\n`
    assert_eq!(large_output.len(), large + 1);

    // Linear would be ~8x; allow a wide margin (quadratic would be ~64x).
    let small_nanos = small_time.as_nanos().max(1);
    let ratio = large_time.as_nanos() / small_nanos;
    assert!(
        ratio < 30,
        "expected near-linear scaling, got {ratio}x for 8x the parts \
         ({small_time:?} -> {large_time:?})"
    );
}

// --- Mutable data fields (ADR 0022) -----------------------------------------

#[test]
fn mutating_a_data_field_is_visible_on_the_binding() {
    let source = "\
data User { name: Str, age: Int, email: Str? = none }

fn main() {
    let mut u = User(name: \"Mel\", age: 30)
    u.age = 99
    print(u.age)
    print(u)
}
";
    assert_eq!(
        run(source),
        "99\nUser(name: \"Mel\", age: 99, email: none)\n"
    );
}

#[test]
fn data_has_value_semantics_on_assignment() {
    // ADR 0022: `let b = a` copies. Mutating one binding must not touch the
    // other, so `b.age` keeps the old value after `a2.age = 99`.
    let source = "\
data User { name: Str, age: Int }

fn main() {
    let a = User(name: \"A\", age: 1)
    let b = a
    let mut a2 = a
    a2.age = 99
    print(b.age)
    print(a2.age)
}
";
    assert_eq!(run(source), "1\n99\n");
}

#[test]
fn interpolating_a_mutated_field_shows_the_new_value() {
    let source = "\
data User { name: Str, age: Int }

fn main() {
    let mut u = User(name: \"Mel\", age: 30)
    u.age = 31
    print(\"age = {u.age}, full = {u}\")
}
";
    assert_eq!(
        run(source),
        "age = 31, full = User(name: \"Mel\", age: 31)\n"
    );
}

#[test]
fn a_field_can_be_mutated_inside_a_loop() {
    let source = "\
data Counter { n: Int }

fn main() {
    let mut c = Counter(n: 0)
    for i in 0..5 {
        if i == 3 { break }
        c.n = c.n + 1
    }
    print(c.n)
}
";
    assert_eq!(run(source), "3\n");
}

#[test]
fn a_data_value_can_be_captured_and_mutated_by_a_closure_binding() {
    // The closure captures the binding by reference; mutating through the
    // closure and reading after must agree (value semantics, ADR 0022).
    let source = "\
data Counter { n: Int }

fn main() {
    let mut c = Counter(n: 0)
    let bump: fn() -> () = () => { c.n = c.n + 1 }
    bump()
    bump()
    print(c.n)
}
";
    assert_eq!(run(source), "2\n");
}

#[test]
fn equality_of_separately_built_data_is_structural_not_identity() {
    // Item (c): two `data` values built independently (not cloned from each
    // other) are `==` after the move to `Rc<RefCell<..>>`, and mutating one and
    // restoring the field keeps `==` true — the comparison is by content, not
    // by `Rc` identity.
    let source = "\
data U { x: Int, y: Str }

fn main() {
    let a = U(x: 1, y: \"s\")
    let b = U(x: 1, y: \"s\")
    print(a == b)

    let mut c = U(x: 1, y: \"s\")
    c.x = 99
    print(c == a)
    c.x = 1
    print(c == a)
    print(a == c)
}
";
    assert_eq!(run(source), "true\nfalse\ntrue\ntrue\n");
}

// --- Mutation across existing feature boundaries (items d) -------------------

#[test]
fn a_lambda_in_a_data_field_can_drive_a_field_mutation() {
    // Combines ADR 0017 (a closure as a value in a `data` field) with ADR 0022.
    let source = "\
enum Shape { Circle(Int), Dot }

data Holder { make: fn(Int) -> Shape, last: Shape }

fn main() {
    let mut h = Holder(make: r => Circle(r), last: Dot)
    let m = h.make
    h.last = m(3)
    print(h.last)
    print(\"{h.last}\")
}
";
    assert_eq!(run(source), "Circle(3)\nCircle(3)\n");
}

#[test]
fn mutation_and_interpolation_happen_inside_a_lambda() {
    // The lambda's own body mutates the captured binding and interpolates the
    // result; control flow stays inside the call (ADR 0021).
    let source = "\
data C { n: Int }

fn main() {
    let mut c = C(n: 0)
    let f: fn() -> Str = () => { c.n = c.n + 1; \"{c.n}\" }
    print(f())
    print(f())
    print(c.n)
}
";
    assert_eq!(run(source), "1\n2\n2\n");
}

#[test]
fn a_variant_constructor_lambda_can_mutate_a_data_field() {
    // The lambda's own body mutates the captured `data` field and evaluates to
    // a variant constructor application (ADR 0017) in the same expression
    // (ADR 0022), then the result is stored back through field assignment.
    let source = "\
enum Shape { Circle(Int), Dot }

data Box { n: Int, last: Shape }

fn main() {
    let mut b = Box(n: 0, last: Dot)
    let step: fn(Int) -> Shape = r => {
        b.n = b.n + 1
        Circle(r + b.n)
    }
    b.last = step(1)
    print(b.n)
    print(b.last)
    print(\"{b.last}\")
}
";
    assert_eq!(run(source), "1\nCircle(2)\nCircle(2)\n");
}

#[test]
fn a_data_field_holding_a_unit_enum_can_be_mutated_and_compared() {
    // Combines ADR 0020 (unit enum) with ADR 0022: the field is a unit variant,
    // and equality tracks the field value across mutation and restoration.
    let source = "\
enum Color { Red, Green }

data P { c: Color }

fn main() {
    let mut p = P(c: Red)
    let q = P(c: Red)
    print(p == q)
    p.c = Green
    print(p == q)
    p.c = Red
    print(p == q)
    print(\"{p.c}\")
}
";
    assert_eq!(run(source), "true\nfalse\ntrue\nRed\n");
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
