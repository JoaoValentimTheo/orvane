//! Sema tests: name resolution, type checking, and a proptest proving the
//! checker never panics on arbitrary AST.

use orv_syntax::{Parser, SourceMap, lex};

use super::{CheckResult, check};
use crate::ty::Ty;

/// Lexes, parses and checks `text`.
fn analyse(text: &str) -> CheckResult {
    let mut sources = SourceMap::new();
    let id = sources.add("test.orv", text);
    let file = match sources.file(id).cloned() {
        Some(file) => file,
        None => orv_syntax::SourceFile::new(orv_syntax::FileId(0), "test.orv", text),
    };
    let (tokens, lexical) = lex(&file);
    let mut parser = Parser::new(&tokens, lexical);
    let result = parser.parse_program();
    match result.program {
        Some(program) => check(&program),
        None => CheckResult {
            diagnostics: Vec::new(),
            functions: std::collections::HashMap::new(),
        },
    }
}

/// The diagnostic codes produced for `text`.
fn codes(text: &str) -> Vec<&'static str> {
    analyse(text).diagnostics.iter().map(|d| d.code).collect()
}

/// Asserts that `text` checks cleanly.
fn ok(text: &str) {
    let result = analyse(text);
    assert!(
        !result.has_errors(),
        "expected a clean check for {text:?}, got {:?}",
        result
            .diagnostics
            .iter()
            .map(|d| (d.code, d.message.as_str()))
            .collect::<Vec<_>>()
    );
}

/// Wraps statements in `fn main` and checks them.
fn ok_main(body: &str) {
    ok(&format!("fn main() {{\n{body}\n}}\n"));
}

// --- Valid programs ---------------------------------------------------------

#[test]
fn accepts_arithmetic_with_inference() {
    ok_main("let x = 1 + 2\nlet y = x * 3\nprint(y)");
}

#[test]
fn accepts_int_float_promotion() {
    ok_main("let x = 1 + 1.5");
}

#[test]
fn accepts_string_concatenation() {
    ok_main(r#"let s = "a" + "b""#);
}

#[test]
fn accepts_annotated_let() {
    ok_main("let x: Int = 42");
}

#[test]
fn accepts_lists_and_indexing() {
    ok_main("let xs = [1, 2, 3]\nlet first = xs[0]");
}

#[test]
fn accepts_maps() {
    ok_main(
        r#"let m = #{"a": 1}
let v = m["a"]"#,
    );
}

#[test]
fn accepts_functions_and_calls() {
    ok("fn add(a: Int, b: Int) -> Int {\n    a + b\n}\n\nfn main() {\n    print(add(1, 2))\n}\n");
}

#[test]
fn accepts_mut_reassignment() {
    ok_main("let mut x = 1\nx = 2\nx += 1");
}

#[test]
fn accepts_while_and_for() {
    ok_main("let mut i = 0\nwhile i < 10 {\n    i += 1\n}\nfor j in 1..=3 {\n    print(j)\n}");
}

#[test]
fn accepts_data_construction() {
    ok(
        "data User {\n    name: Str,\n    age: Int,\n    email: Str? = none,\n}\n\nfn main() {\n    let u = User(name: \"Mel\", age: 30)\n    print(u.name)\n}\n",
    );
}

#[test]
fn accepts_enum_and_match() {
    ok(
        "enum Shape { Circle(Float), Rect(Float, Float) }\n\nfn area(s: Shape) -> Float {\n    match s {\n        Circle(r) => 3.14159 * r * r,\n        Rect(w, h) => w * h,\n    }\n}\n\nfn main() {\n    print(area(Circle(1.0)))\n}\n",
    );
}

#[test]
fn accepts_if_expression_in_let() {
    ok_main(r#"let label = if true { "yes" } else { "no" }"#);
}

#[test]
fn accepts_optional_coalesce() {
    ok_main("let maybe: Int? = none\nlet value = maybe ?? 0");
}

#[test]
fn accepts_cast_int_to_float() {
    ok_main("let x = 1 as Float");
}

#[test]
fn accepts_nested_blocks_and_shadowing() {
    ok_main("let x = 1\nif true {\n    let x = \"shadow\"\n    print(x)\n}");
}

#[test]
fn accepts_function_returning_unit_implicitly() {
    ok("fn f() {\n    print(1)\n}\n");
}

#[test]
fn accepts_tail_expression_matching_return_type() {
    ok("fn f() -> Int {\n    1 + 1\n}\n");
}

#[test]
fn accepts_explicit_return_matching_declared_type() {
    ok("fn f() -> Int {\n    return 42\n}\n");
}

// --- Name resolution --------------------------------------------------------

#[test]
fn undefined_name_reports_e0201() {
    assert_eq!(codes("fn main() {\n    print(nope)\n}\n"), vec!["E0201"]);
}

#[test]
fn undefined_type_reports_e0201() {
    assert_eq!(
        codes("fn f(x: Missing) {\n}\n"),
        vec!["E0201"],
        "`Missing` is not a declared type"
    );
}

#[test]
fn duplicate_definition_reports_e0202() {
    assert_eq!(codes("fn f() {\n}\nfn f() {\n}\n"), vec!["E0202"]);
}

#[test]
fn duplicate_field_reports_e0202() {
    assert_eq!(
        codes("data D {\n    x: Int,\n    x: Str,\n}\n"),
        vec!["E0202"]
    );
}

#[test]
fn duplicate_parameter_reports_e0202() {
    assert_eq!(codes("fn f(a: Int, a: Int) {\n}\n"), vec!["E0202"]);
}

#[test]
fn shadowing_in_an_inner_scope_is_allowed() {
    ok_main("let x = 1\nwhile false {\n    let x = \"ok\"\n    print(x)\n}");
}

#[test]
fn assigning_to_an_immutable_reports_e0230() {
    assert_eq!(
        codes("fn main() {\n    let x = 1\n    x = 2\n}\n"),
        vec!["E0230"]
    );
}

#[test]
fn compound_assignment_to_an_immutable_reports_e0230() {
    assert_eq!(
        codes("fn main() {\n    let x = 1\n    x += 2\n}\n"),
        vec!["E0230"]
    );
}

#[test]
fn assigning_to_a_field_of_an_immutable_is_allowed() {
    ok(
        "data User {\n    age: Int,\n}\n\nfn main() {\n    let u = User(age: 1)\n    u.age = 2\n}\n",
    );
}

// --- Type errors ------------------------------------------------------------

#[test]
fn annotated_let_mismatch_reports_e0301() {
    assert_eq!(
        codes("fn main() {\n    let x: Int = \"s\"\n}\n"),
        vec!["E0301"]
    );
}

#[test]
fn arithmetic_on_a_string_reports_e0301() {
    assert_eq!(
        codes("fn main() {\n    let x = \"a\" - \"b\"\n}\n"),
        vec!["E0301"]
    );
}

#[test]
fn adding_a_string_and_an_int_reports_e0301() {
    assert_eq!(
        codes("fn main() {\n    let x = \"a\" + 1\n}\n"),
        vec!["E0301"]
    );
}

#[test]
fn wrong_argument_count_reports_e0302() {
    assert_eq!(
        codes("fn f(a: Int) {\n}\n\nfn main() {\n    f(1, 2)\n}\n"),
        vec!["E0302"]
    );
}

#[test]
fn wrong_argument_type_reports_e0301() {
    assert_eq!(
        codes("fn f(a: Int) {\n}\n\nfn main() {\n    f(\"s\")\n}\n"),
        vec!["E0301"]
    );
}

#[test]
fn calling_a_non_function_reports_e0303() {
    assert_eq!(
        codes("fn main() {\n    let x = 1\n    x()\n}\n"),
        vec!["E0303"]
    );
}

#[test]
fn unknown_field_reports_e0304() {
    assert_eq!(
        codes(
            "data User {\n    name: Str,\n}\n\nfn main() {\n    let u = User(name: \"x\")\n    print(u.age)\n}\n"
        ),
        vec!["E0304"]
    );
}

#[test]
fn condition_must_be_bool_reports_e0312() {
    // §5.3: no truthiness.
    assert_eq!(codes("fn main() {\n    if 1 {\n    }\n}\n"), vec!["E0312"]);
}

#[test]
fn while_condition_must_be_bool_reports_e0312() {
    assert_eq!(
        codes("fn main() {\n    while \"x\" {\n    }\n}\n"),
        vec!["E0312"]
    );
}

#[test]
fn indexing_a_non_collection_reports_e0303() {
    assert_eq!(
        codes("fn main() {\n    let x = 1\n    x[0]\n}\n"),
        vec!["E0303"]
    );
}

#[test]
fn indexing_a_list_with_a_string_reports_e0301() {
    assert_eq!(
        codes("fn main() {\n    let xs = [1]\n    xs[\"a\"]\n}\n"),
        vec!["E0301"]
    );
}

#[test]
fn mixed_list_elements_report_e0301() {
    assert_eq!(
        codes("fn main() {\n    let xs = [1, \"a\"]\n}\n"),
        vec!["E0301"]
    );
}

#[test]
fn return_type_mismatch_reports_e0301() {
    assert_eq!(
        codes("fn f() -> Int {\n    return \"s\"\n}\n"),
        vec!["E0301"]
    );
}

#[test]
fn tail_expression_mismatch_reports_e0301() {
    assert_eq!(codes("fn f() -> Int {\n    \"s\"\n}\n"), vec!["E0301"]);
}

#[test]
fn negating_a_string_reports_e0301() {
    assert_eq!(codes("fn main() {\n    let x = -\"a\"\n}\n"), vec!["E0301"]);
}

#[test]
fn not_on_a_non_bool_reports_e0301() {
    assert_eq!(codes("fn main() {\n    let x = not 1\n}\n"), vec!["E0301"]);
}

#[test]
fn a_lambda_infers_from_a_let_annotation() {
    // §5.3: "Lambda infere parâmetros do tipo esperado."
    ok_main("let f: fn(Int) -> Int = x => x + 1");
}

#[test]
fn a_lambda_infers_from_a_call_argument_type() {
    ok(
        "fn apply(f: fn(Int) -> Int, x: Int) -> Int {\n    f(x)\n}\n\nfn main() {\n    print(apply(y => y * 2, 21))\n}\n",
    );
}

#[test]
fn a_concrete_value_widens_into_an_optional() {
    // `let e: Str? = "x"` is a widening, not a mismatch.
    ok_main(r#"let e: Str? = "x""#);
    ok_main("let e: Str? = none");
}

#[test]
fn an_optional_field_accepts_a_concrete_value() {
    ok(
        "data User {\n    email: Str? = none,\n}\n\nfn main() {\n    let u = User(email: \"a@b\")\n    print(u)\n}\n",
    );
}

#[test]
fn lambda_without_context_reports_e0310() {
    // §5.3: a lambda needs an expected type for its parameters.
    assert_eq!(codes("fn main() {\n    let f = x => x\n}\n"), vec!["E0310"]);
}

#[test]
fn cast_to_an_undefined_type_reports_e0201() {
    // The target name does not exist, so the precise error is the undefined
    // type (`E0201`), not a generic `E0311`.
    assert_eq!(
        codes("fn main() {\n    let x = 1 as NotAType\n}\n"),
        vec!["E0201"]
    );
}

#[test]
fn casting_a_string_to_int_reports_e0311() {
    // Both types exist, but §5.3 only allows `Int as Float`.
    assert_eq!(
        codes("fn main() {\n    let x = \"s\" as Int\n}\n"),
        vec!["E0311"]
    );
}

#[test]
fn casting_int_to_float_is_allowed() {
    assert!(
        analyse("fn main() {\n    let x = 1 as Float\n}\n")
            .diagnostics
            .is_empty()
    );
}

#[test]
fn variant_pattern_field_count_reports_e0302() {
    assert_eq!(
        codes(
            "enum E { A(Int, Int) }\n\nfn f(e: E) -> Int {\n    match e {\n        A(x) => x,\n    }\n}\n"
        ),
        vec!["E0302"]
    );
}

#[test]
fn variant_pattern_against_a_non_enum_reports_e0301() {
    // A name that *is* a variant, used on a non-enum scrutinee.
    assert_eq!(
        codes("enum E { A }\n\nfn f(n: Int) -> Int {\n    match n {\n        A => 1,\n    }\n}\n"),
        vec!["E0301"]
    );
}

#[test]
fn an_unknown_variant_name_binds_instead() {
    // §5.2's grammar makes `B` indistinguishable from a binding, and the spec
    // is silent on the tie-break (ADR 0015): an unmatched bare name binds. A
    // misspelled variant is therefore caught by exhaustiveness (`E0320`, M5),
    // not here.
    let result =
        analyse("enum E { A }\n\nfn f(e: E) -> Int {\n    match e {\n        B => 1,\n    }\n}\n");
    assert!(
        result.diagnostics.is_empty(),
        "`B` binds: {:?}",
        result
            .diagnostics
            .iter()
            .map(|d| d.code)
            .collect::<Vec<_>>()
    );
}

#[test]
fn a_unit_variant_pattern_is_accepted() {
    ok(
        "enum E { A, B }\n\nfn f(e: E) -> Int {\n    match e {\n        A => 1,\n        B => 2,\n    }\n}\n",
    );
}

#[test]
fn a_bare_name_that_is_not_a_variant_binds() {
    // `x` is not a variant, so it binds and the arm type still checks.
    ok("fn f(n: Int) -> Int {\n    match n {\n        x => x + 1,\n    }\n}\n");
}

#[test]
fn failing_branch_types_report_e0301() {
    assert_eq!(
        codes("fn main() {\n    let x = if true { 1 } else { \"s\" }\n}\n"),
        vec!["E0301"]
    );
}

#[test]
fn match_arm_type_mismatch_reports_e0301() {
    assert_eq!(
        codes(
            "fn f(n: Int) -> Int {\n    match n {\n        0 => 1,\n        _ => \"s\",\n    }\n}\n"
        ),
        vec!["E0301"]
    );
}

// --- No cascades ------------------------------------------------------------

#[test]
fn one_undefined_name_yields_one_diagnostic() {
    // `nope + 1 + 2` must not produce one error per parent expression.
    let codes = codes("fn main() {\n    let x = nope + 1 + 2\n}\n");
    assert_eq!(codes, vec!["E0201"], "got {codes:?}");
}

#[test]
fn an_undefined_callee_still_checks_its_arguments() {
    // The callee error is reported once; a broken argument reports too.
    let codes = codes("fn main() {\n    nope(other)\n}\n");
    assert_eq!(codes, vec!["E0201", "E0201"], "got {codes:?}");
}

#[test]
fn a_mistake_in_a_condition_does_not_cascade_into_the_body() {
    let codes = codes("fn main() {\n    if nope {\n        let x = 1\n    }\n}\n");
    assert_eq!(codes, vec!["E0201"], "got {codes:?}");
}

// --- Type resolution --------------------------------------------------------

#[test]
fn records_function_signatures() {
    let result = analyse("fn add(a: Int, b: Int) -> Int {\n    a + b\n}\n");
    assert_eq!(
        result.functions.get("add"),
        Some(&Ty::Fn {
            params: vec![Ty::Int, Ty::Int],
            ret: Box::new(Ty::Int),
        })
    );
}

#[test]
fn a_function_without_annotation_returns_unit() {
    let result = analyse("fn f() {\n}\n");
    assert_eq!(
        result.functions.get("f"),
        Some(&Ty::Fn {
            params: vec![],
            ret: Box::new(Ty::Unit),
        })
    );
}

#[test]
fn calls_can_refer_to_later_functions() {
    // Two passes: signatures are collected before bodies are checked.
    ok("fn main() {\n    print(later())\n}\n\nfn later() -> Int {\n    1\n}\n");
}

#[test]
fn data_types_can_be_used_before_declaration_order() {
    ok("fn make() -> User {\n    return User(name: \"x\")\n}\n\ndata User {\n    name: Str,\n}\n");
}

#[test]
fn an_empty_program_checks_cleanly() {
    ok("");
}

// --- Property test ----------------------------------------------------------

mod prop {
    use super::analyse;
    use orv_syntax::{Parser, lex};
    use proptest::prelude::*;

    /// Runs the checker over text built from fragments that stress the type
    /// rules (optionals, collections, arithmetic, calls, patterns).
    fn check_text(text: &str) {
        let _ = analyse(text);
    }

    /// Parses an arbitrary token kind sequence, if it happens to parse, and
    /// checks the result. Any AST the parser can build must not crash sema.
    fn check_parsed(text: &str) {
        let mut sources = orv_syntax::SourceMap::new();
        let id = sources.add("fuzz.orv", text);
        let file = match sources.file(id).cloned() {
            Some(file) => file,
            None => orv_syntax::SourceFile::new(orv_syntax::FileId(0), "fuzz.orv", text),
        };
        let (tokens, lexical) = lex(&file);
        let mut parser = Parser::new(&tokens, lexical);
        if let Some(program) = parser.parse_program().program {
            let _ = super::super::check(&program);
        }
    }

    proptest! {
        /// Sema never panics for arbitrary input (SPEC §11 item 3).
        #[test]
        fn sema_never_panics(text in any::<String>()) {
            check_text(&text);
        }

        /// Structured fragments produce ASTs far more often than random bytes,
        /// exercising the checker's real paths without ever panicking.
        #[test]
        fn sema_never_panics_on_fragments(
            fragments in prop::collection::vec(
                prop::sample::select(vec![
                    "fn", "main", "(", ")", "{", "}", "[", "]", "#{", ",", ":", ".", "=>",
                    "->", "?", "?", "?.", "??", "=", "==", "+", "-", "*", "/", "%", "as",
                    "let", "mut", "if", "else", "match", "for", "in", "while", "return",
                    "break", "continue", "data", "enum", "true", "false", "none", "and", "or",
                    "not", "Int", "Float", "Bool", "Str", "List", "Map", "1", "1.5", "\"s\"",
                    "ident", "_", "\n", " ", "x => x", "Ok", "Err",
                ]),
                0..30,
            )
        ) {
            let text: String = fragments.concat();
            check_parsed(&text);
        }
    }
}
