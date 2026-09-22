//! Expression tests: literals, operators, precedence, lambdas, collections.

use super::{expr_of, main_statements, parse, parse_ok};
use crate::ast::{BinaryOp, ExprKind, Literal, UnaryOp};

fn literal(text: &str) -> ExprKind {
    expr_of(text).kind
}

#[test]
fn parses_literals() {
    assert_eq!(literal("42"), ExprKind::Literal(Literal::Int(42)));
    assert_eq!(literal("1.5"), ExprKind::Literal(Literal::Float(1.5)));
    assert_eq!(literal("true"), ExprKind::Literal(Literal::Bool(true)));
    assert_eq!(literal("false"), ExprKind::Literal(Literal::Bool(false)));
    assert_eq!(literal("none"), ExprKind::Literal(Literal::None));
    assert!(matches!(
        literal(r#""hi""#),
        ExprKind::Literal(Literal::Str(_))
    ));
}

#[test]
fn parses_identifier_and_py() {
    assert_eq!(literal("main"), ExprKind::Ident("main".to_owned()));
    assert_eq!(literal("py"), ExprKind::Ident("py".to_owned()));
}

#[test]
fn parses_parenthesised_expression() {
    let ExprKind::Paren(inner) = literal("(42)") else {
        panic!("expected parens");
    };
    assert_eq!(inner.kind, ExprKind::Literal(Literal::Int(42)));
}

#[test]
fn parses_tuple() {
    let ExprKind::Tuple(elements) = literal("(1, 2, 3)") else {
        panic!("expected a tuple");
    };
    assert_eq!(elements.len(), 3);
}

// --- Operators --------------------------------------------------------------

#[test]
fn parses_arithmetic_with_precedence() {
    // `1 + 2 * 3` binds as `1 + (2 * 3)` (§5.2.1 level 8 over 7).
    let ExprKind::Binary {
        op: BinaryOp::Add,
        left,
        right,
    } = literal("1 + 2 * 3")
    else {
        panic!("expected + at the root, got {:?}", literal("1 + 2 * 3"));
    };
    assert_eq!(left.kind, ExprKind::Literal(Literal::Int(1)));
    let ExprKind::Binary {
        op: BinaryOp::Mul, ..
    } = right.kind
    else {
        panic!("expected * on the right, got {right:?}");
    };
}

#[test]
fn subtraction_is_left_associative() {
    // `10 - 3 - 2` is `(10 - 3) - 2`.
    let ExprKind::Binary {
        op: BinaryOp::Sub,
        left,
        right,
    } = literal("10 - 3 - 2")
    else {
        panic!("expected - at the root");
    };
    assert_eq!(right.kind, ExprKind::Literal(Literal::Int(2)));
    let ExprKind::Binary {
        op: BinaryOp::Sub, ..
    } = left.kind
    else {
        panic!("expected a nested -, got {left:?}");
    };
}

#[test]
fn coalesce_binds_looser_than_or() {
    // `a ?? b or c` is `a ?? (b or c)` (§5.2.1: `??` is level 1, `or` level 2).
    let ExprKind::Binary {
        op: BinaryOp::Coalesce,
        right,
        ..
    } = literal("a ?? b or c")
    else {
        panic!("expected ?? at the root");
    };
    let ExprKind::Binary {
        op: BinaryOp::Or, ..
    } = right.kind
    else {
        panic!("expected or under ??, got {right:?}");
    };
}

#[test]
fn and_binds_tighter_than_or() {
    let ExprKind::Binary {
        op: BinaryOp::Or,
        right,
        ..
    } = literal("a or b and c")
    else {
        panic!("expected or at the root");
    };
    let ExprKind::Binary {
        op: BinaryOp::And, ..
    } = right.kind
    else {
        panic!("expected and under or, got {right:?}");
    };
}

#[test]
fn comparison_binds_looser_than_addition() {
    let ExprKind::Binary {
        op: BinaryOp::Lt,
        left,
        ..
    } = literal("1 + 2 < 4")
    else {
        panic!("expected < at the root");
    };
    let ExprKind::Binary {
        op: BinaryOp::Add, ..
    } = left.kind
    else {
        panic!("expected + under <, got {left:?}");
    };
}

#[test]
fn parses_all_comparison_operators() {
    for (source, op) in [
        ("a == b", BinaryOp::Eq),
        ("a != b", BinaryOp::NotEq),
        ("a < b", BinaryOp::Lt),
        ("a <= b", BinaryOp::Le),
        ("a > b", BinaryOp::Gt),
        ("a >= b", BinaryOp::Ge),
        ("a and b", BinaryOp::And),
        ("a or b", BinaryOp::Or),
        ("a ?? b", BinaryOp::Coalesce),
        ("a + b", BinaryOp::Add),
        ("a - b", BinaryOp::Sub),
        ("a * b", BinaryOp::Mul),
        ("a / b", BinaryOp::Div),
        ("a % b", BinaryOp::Rem),
    ] {
        let ExprKind::Binary { op: found, .. } = literal(source) else {
            panic!("expected a binary expression for {source:?}");
        };
        assert_eq!(found, op, "for {source:?}");
    }
}

#[test]
fn parses_ranges() {
    for (source, op) in [
        ("1..5", BinaryOp::Range),
        ("1..=5", BinaryOp::RangeInclusive),
    ] {
        let ExprKind::Binary { op: found, .. } = literal(source) else {
            panic!("expected a range for {source:?}");
        };
        assert_eq!(found, op);
    }
}

#[test]
fn parses_unary_operators() {
    let ExprKind::Unary {
        op: UnaryOp::Neg,
        operand,
    } = literal("-1")
    else {
        panic!("expected unary minus");
    };
    assert_eq!(operand.kind, ExprKind::Literal(Literal::Int(1)));

    let ExprKind::Unary {
        op: UnaryOp::Not,
        operand,
    } = literal("not ok")
    else {
        panic!("expected unary not");
    };
    assert_eq!(operand.kind, ExprKind::Ident("ok".to_owned()));
}

#[test]
fn unary_binds_tighter_than_multiplication() {
    // `-1 * 2` is `(-1) * 2` (§5.2.1: unary is level 10, `*` level 8).
    let ExprKind::Binary {
        op: BinaryOp::Mul,
        left,
        ..
    } = literal("-1 * 2")
    else {
        panic!("expected * at the root");
    };
    assert!(matches!(left.kind, ExprKind::Unary { .. }));
}

#[test]
fn parses_cast() {
    // `1 as Float` is a cast; the type is parsed after `as`.
    let ExprKind::Cast { expr, ty } = literal("1 as Float") else {
        panic!("expected a cast, got {:?}", literal("1 as Float"));
    };
    assert_eq!(expr.kind, ExprKind::Literal(Literal::Int(1)));
    assert!(matches!(ty.kind, crate::ast::TypeKind::Named { .. }));
}

#[test]
fn precedence_of_cast_relative_to_arithmetic() {
    // §5.2.1: `as` (9) binds tighter than `+` (7).
    let ExprKind::Binary {
        op: BinaryOp::Add,
        right,
        ..
    } = literal("1 + 2 as Float")
    else {
        panic!("expected + at the root");
    };
    assert!(matches!(right.kind, ExprKind::Cast { .. }));
}

// --- Postfix ----------------------------------------------------------------

#[test]
fn parses_postfix_chain() {
    let ExprKind::OptionalField { receiver, name } = literal("a.b(c)[d]?.e") else {
        panic!("expected the outer optional field");
    };
    assert_eq!(name, "e");
    assert!(matches!(receiver.kind, ExprKind::Index { .. }));
}

#[test]
fn parses_call_with_named_argument() {
    let ExprKind::Call { args, .. } = literal("f(timeout: 3, retries: 2)") else {
        panic!("expected a call");
    };
    assert_eq!(args.len(), 2);
    assert_eq!(args[0].name.as_deref(), Some("timeout"));
    assert_eq!(args[1].name.as_deref(), Some("retries"));
}

#[test]
fn float_dot_int_is_rejected() {
    let parsed = parse("fn main() {\n    1.2.3\n}\n");
    assert_eq!(parsed.codes, vec!["E0102"], "ADR 0011");
}

// --- Collections ------------------------------------------------------------

#[test]
fn parses_list_literal() {
    let ExprKind::List(elements) = literal("[1, 2, 3]") else {
        panic!("expected a list");
    };
    assert_eq!(elements.len(), 3);
}

#[test]
fn parses_empty_list() {
    let ExprKind::List(elements) = literal("[]") else {
        panic!("expected a list");
    };
    assert!(elements.is_empty());
}

#[test]
fn parses_list_with_trailing_comma() {
    let ExprKind::List(elements) = literal("[1, 2,]") else {
        panic!("expected a list");
    };
    assert_eq!(elements.len(), 2);
}

#[test]
fn parses_map_literal() {
    let ExprKind::Map(entries) = literal(r#"#{"a": 1, "b": 2}"#) else {
        panic!("expected a map");
    };
    assert_eq!(entries.len(), 2);
}

#[test]
fn parses_empty_map() {
    let ExprKind::Map(entries) = literal("#{}") else {
        panic!("expected an empty map");
    };
    assert!(entries.is_empty());
}

// --- Control flow -----------------------------------------------------------

#[test]
fn parses_if_expression() {
    let ExprKind::If {
        else_branch,
        then_block,
        ..
    } = literal("if x { 1 } else { 2 }")
    else {
        panic!("expected an if");
    };
    assert_eq!(then_block.statements.len(), 1);
    assert!(else_branch.is_some());
}

#[test]
fn allows_a_newline_before_else() {
    // §4.1's FizzBuzz puts `else` on the next line; the newline is part of the
    // `if`, not a statement boundary.
    let source = "fn main() {\n    let out = if a { 1 }\n              else if b { 2 }\n              else { 3 }\n}\n";
    let program = parse_ok(source);
    let crate::ast::ItemKind::Fn(decl) = &program.items[0].kind else {
        panic!("expected a function");
    };
    let crate::ast::StmtKind::Let { value, .. } = &decl.body.statements[0].kind else {
        panic!("expected a let");
    };
    assert!(matches!(value.kind, ExprKind::If { .. }));
}

#[test]
fn parses_the_appendix_a1_fizzbuzz() {
    // §12 M2 gate: the reference programs must parse.
    parse_ok(
        r#"
fn main() {
    for i in 1..=15 {
        let out = if i % 15 == 0 { "FizzBuzz" }
                  else if i % 3 == 0 { "Fizz" }
                  else if i % 5 == 0 { "Buzz" }
                  else { str(i) }
        print(out)
    }
}
"#,
    );
}

#[test]
fn parses_else_if_chain() {
    let ExprKind::If { else_branch, .. } = literal("if a { 1 } else if b { 2 } else { 3 }") else {
        panic!("expected an if");
    };
    let Some(else_branch) = else_branch else {
        panic!("expected an else branch");
    };
    assert!(matches!(else_branch.kind, ExprKind::If { .. }));
}

#[test]
fn parses_match_expression() {
    let ExprKind::Match { arms, .. } = literal("match x { 1 => \"a\", _ => \"b\" }") else {
        panic!("expected a match");
    };
    assert_eq!(arms.len(), 2);
    assert!(arms[1].guard.is_none());
}

#[test]
fn parses_match_with_guard() {
    let ExprKind::Match { arms, .. } = literal("match x { n if n > 0 => 1, _ => 0 }") else {
        panic!("expected a match");
    };
    assert!(arms[0].guard.is_some());
}

#[test]
fn parses_try_expression() {
    let ExprKind::Try(inner) = literal("try risky()") else {
        panic!("expected a try");
    };
    assert!(matches!(inner.kind, ExprKind::Call { .. }));
}

#[test]
fn parses_fail_as_a_statement() {
    // §5.2 lists `fail expr` under `stmt`, so it becomes a statement, not an
    // expression statement.
    let statements = main_statements("fail \"boom\"");
    let crate::ast::StmtKind::Fail(_) = &statements[0].kind else {
        panic!("expected a fail statement, got {:?}", statements[0]);
    };
}

// --- Lambdas ----------------------------------------------------------------

#[test]
fn parses_single_parameter_lambda() {
    let ExprKind::Lambda { params, body } = literal("x => x + 1") else {
        panic!("expected a lambda");
    };
    assert_eq!(params, vec!["x".to_owned()]);
    assert!(matches!(body.kind, ExprKind::Binary { .. }));
}

#[test]
fn parses_multi_parameter_lambda() {
    let ExprKind::Lambda { params, .. } = literal("(a, b) => a + b") else {
        panic!("expected a lambda");
    };
    assert_eq!(params, vec!["a".to_owned(), "b".to_owned()]);
}

#[test]
fn parses_lambda_with_block_body() {
    // The M1 mandatory shape: a lambda with a block inside a call.
    let ExprKind::Call { args, .. } = literal("xs.map(x => {\n    let y = 1\n    y\n})") else {
        panic!("expected a call");
    };
    assert_eq!(args.len(), 1);
    assert!(matches!(args[0].value.kind, ExprKind::Lambda { .. }));
}

#[test]
fn parenthesised_expression_is_not_a_lambda() {
    // `(1 + 2)` has no `=>` after the `)`, so it is a plain parenthesised expr.
    assert!(matches!(literal("(1 + 2)"), ExprKind::Paren(_)));
}

#[test]
fn empty_parens_are_rejected() {
    let parsed = parse("fn main() {\n    ()\n}\n");
    assert!(
        parsed.codes.contains(&"E0102"),
        "`()` is the Unit type, not a primary: {:?}",
        parsed.codes
    );
}

// --- Interpolated strings ---------------------------------------------------

#[test]
fn interpolated_string_parts_are_preserved() {
    let ExprKind::Literal(Literal::Str(parts)) = literal(r#""a{x}b""#) else {
        panic!("expected a string");
    };
    assert_eq!(parts.len(), 3);
}

#[test]
fn a_broken_interpolated_string_yields_only_the_lexical_error() {
    // ADR 0008 amend rule 4: the guard is consulted and no parser diagnostic
    // comes from the best-effort `Expr.src`.
    let parsed = parse("fn main() {\n    \"{f(\\\"a\\\")}\"\n}\n");
    assert_eq!(parsed.codes, vec!["E0006"], "got {:?}", parsed.codes);
}

#[test]
fn parses_a_large_program_without_diagnostics() {
    // A smoke test that the whole expression grammar composes.
    let program = parse_ok(
        r#"
fn main() {
    let xs = [1, 2, 3]
    let m = #{"a": 1}
    let f = x => x * 2
    let v = if xs[0] < 2 { f(1) } else { 0 }
    print("v = {v}")
}
"#,
    );
    assert_eq!(program.items.len(), 1);
}
