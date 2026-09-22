//! Statement tests: `let`, assignment, control flow, blocks.

use super::{main_statements, parse, parse_ok};
use crate::ast::{AssignOp, ItemKind, StmtKind};

#[test]
fn parses_let() {
    let statements = main_statements("let x = 1");
    let StmtKind::Let {
        pattern,
        mutable,
        ty,
        value,
    } = &statements[0].kind
    else {
        panic!("expected a let, got {:?}", statements[0]);
    };
    assert!(!mutable);
    assert!(ty.is_none());
    assert_eq!(pattern.kind, crate::ast::PatternKind::Bind("x".to_owned()));
    assert!(matches!(
        value.kind,
        crate::ast::ExprKind::Literal(crate::ast::Literal::Int(1))
    ));
}

#[test]
fn parses_let_mut_with_annotation() {
    let statements = main_statements("let mut total: Int = 0");
    let StmtKind::Let { mutable, ty, .. } = &statements[0].kind else {
        panic!("expected a let");
    };
    assert!(mutable);
    assert!(ty.is_some());
}

#[test]
fn parses_assignment_operators() {
    for (source, op) in [
        ("x = 1", AssignOp::Assign),
        ("x += 1", AssignOp::Add),
        ("x -= 1", AssignOp::Sub),
        ("x *= 1", AssignOp::Mul),
        ("x /= 1", AssignOp::Div),
    ] {
        let statements = main_statements(source);
        let StmtKind::Assign { op: found, .. } = &statements[0].kind else {
            panic!(
                "expected an assignment for {source:?}, got {:?}",
                statements[0]
            );
        };
        assert_eq!(*found, op, "for {source:?}");
    }
}

#[test]
fn assignment_to_a_field_is_still_an_assignment() {
    let statements = main_statements("u.name = \"x\"");
    assert!(matches!(
        statements[0].kind,
        StmtKind::Assign {
            op: AssignOp::Assign,
            ..
        }
    ));
}

#[test]
fn parses_return_variants() {
    let statements = main_statements("return\nreturn 1");
    assert!(matches!(statements[0].kind, StmtKind::Return(None)));
    assert!(matches!(statements[1].kind, StmtKind::Return(Some(_))));
}

#[test]
fn parses_break_and_continue() {
    let statements = main_statements("while true {\n    break\n}\nwhile true {\n    continue\n}\n");
    let StmtKind::While { body, .. } = &statements[0].kind else {
        panic!("expected a while");
    };
    assert!(matches!(body.statements[0].kind, StmtKind::Break));
    let StmtKind::While { body, .. } = &statements[1].kind else {
        panic!("expected a while");
    };
    assert!(matches!(body.statements[0].kind, StmtKind::Continue));
}

#[test]
fn parses_while_loop() {
    let statements = main_statements("while x < 10 {\n    x += 1\n}");
    let StmtKind::While { condition, body } = &statements[0].kind else {
        panic!("expected a while");
    };
    assert!(matches!(
        condition.kind,
        crate::ast::ExprKind::Binary { .. }
    ));
    assert_eq!(body.statements.len(), 1);
}

#[test]
fn parses_for_loop() {
    let statements = main_statements("for i in 1..=15 {\n    print(i)\n}");
    let StmtKind::For {
        pattern,
        iterable,
        body,
    } = &statements[0].kind
    else {
        panic!("expected a for");
    };
    assert_eq!(pattern.kind, crate::ast::PatternKind::Bind("i".to_owned()));
    assert!(matches!(iterable.kind, crate::ast::ExprKind::Binary { .. }));
    assert_eq!(body.statements.len(), 1);
}

#[test]
fn parses_nested_blocks() {
    let statements = main_statements("if true {\n    {\n        let x = 1\n    }\n}");
    let StmtKind::Expr(expr) = &statements[0].kind else {
        panic!("expected an if expression");
    };
    let crate::ast::ExprKind::If { then_block, .. } = &expr.kind else {
        panic!("expected an if");
    };
    assert!(matches!(then_block.statements[0].kind, StmtKind::Expr(_)));
}

#[test]
fn parses_empty_block() {
    let statements = main_statements("while false {\n}");
    let StmtKind::While { body, .. } = &statements[0].kind else {
        panic!("expected a while");
    };
    assert!(body.statements.is_empty());
}

#[test]
fn semicolons_terminate_statements() {
    let statements = main_statements("let a = 1; let b = 2");
    assert_eq!(statements.len(), 2);
}

#[test]
fn a_block_ending_in_an_expression_has_a_tail() {
    let source = "fn f() {\n    1 + 1\n}\n";
    let program = parse_ok(source);
    let ItemKind::Fn(decl) = &program.items[0].kind else {
        panic!("expected a function");
    };
    assert!(decl.body.tail_expr().is_some());
}

#[test]
fn a_block_ending_in_a_let_has_no_tail() {
    let source = "fn f() {\n    let x = 1\n}\n";
    let program = parse_ok(source);
    let ItemKind::Fn(decl) = &program.items[0].kind else {
        panic!("expected a function");
    };
    assert!(decl.body.tail_expr().is_none());
}

// --- Recovery ---------------------------------------------------------------

#[test]
fn a_broken_statement_does_not_hide_the_next_one() {
    // The recovery must resynchronise on the next statement boundary, so the
    // second `let` still parses (M2 gate: multiple errors per file).
    let parsed = parse("fn main() {\n    let = 1\n    let ok = 2\n}\n");
    assert!(
        !parsed.codes.is_empty(),
        "the first statement should be rejected"
    );
    let ItemKind::Fn(decl) = &parsed.program.items[0].kind else {
        panic!("expected a function");
    };
    assert_eq!(
        decl.body.statements.len(),
        1,
        "the valid `let ok = 2` should survive: {:?}",
        decl.body.statements
    );
}

#[test]
fn several_errors_in_one_file_are_all_reported() {
    let parsed = parse("fn main() {\n    let = 1\n    let = 2\n}\n");
    assert!(
        parsed.codes.len() >= 2,
        "expected one diagnostic per broken statement, got {:?}",
        parsed.codes
    );
}

#[test]
fn an_unclosed_block_reports_and_keeps_the_items_before_it() {
    let parsed = parse("fn a() {\n    1\n}\nfn b() {\n    2\n");
    assert!(
        parsed.codes.contains(&"E0102"),
        "expected a missing-brace error: {:?}",
        parsed.codes
    );
    assert_eq!(
        parsed.program.items.len(),
        2,
        "both functions should be present"
    );
}
