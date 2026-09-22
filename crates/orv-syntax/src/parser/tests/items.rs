//! Item tests: `fn`, `data`, `enum`, `use`, and the alpha's explicit refusals.

use super::{first_item, parse, parse_ok};
use crate::ast::{ItemKind, PatternKind, TypeKind};

#[test]
fn parses_a_function_declaration() {
    let item = first_item("fn add(a: Int, b: Int) -> Int {\n    a + b\n}\n");
    let ItemKind::Fn(decl) = item.kind else {
        panic!("expected a function");
    };
    assert_eq!(decl.name, "add");
    assert_eq!(decl.params.len(), 2);
    assert_eq!(decl.params[0].name, "a");
    assert!(decl.ret.is_some());
    assert!(!decl.public);
}

#[test]
fn a_function_without_a_return_type_is_unit() {
    let item = first_item("fn main() {\n}\n");
    let ItemKind::Fn(decl) = item.kind else {
        panic!("expected a function");
    };
    assert!(decl.ret.is_none(), "absent means `()` per §5.3");
}

#[test]
fn parses_parameter_defaults() {
    let item = first_item("fn f(a: Int, b: Str = \"x\") {\n}\n");
    let ItemKind::Fn(decl) = item.kind else {
        panic!("expected a function");
    };
    assert!(decl.params[0].default.is_none());
    assert!(decl.params[1].default.is_some());
}

#[test]
fn parses_pub_fn() {
    let item = first_item("pub fn helper() {\n}\n");
    let ItemKind::Fn(decl) = item.kind else {
        panic!("expected a function");
    };
    assert!(decl.public);
}

#[test]
fn parses_data_declaration() {
    let item =
        first_item("data User {\n    name: Str,\n    age: Int,\n    email: Str? = none,\n}\n");
    let ItemKind::Data(decl) = item.kind else {
        panic!("expected a data");
    };
    assert_eq!(decl.name, "User");
    assert_eq!(decl.fields.len(), 3);
    assert!(decl.fields[2].default.is_some());
    assert!(matches!(decl.fields[2].ty.kind, TypeKind::Optional(_)));
}

#[test]
fn parses_enum_declaration() {
    let item = first_item("enum Shape {\n    Circle(Float),\n    Rect(Float, Float),\n}\n");
    let ItemKind::Enum(decl) = item.kind else {
        panic!("expected an enum");
    };
    assert_eq!(decl.name, "Shape");
    assert_eq!(decl.variants.len(), 2);
    assert_eq!(decl.variants[0].payload.len(), 1);
    assert_eq!(decl.variants[1].payload.len(), 2);
}

#[test]
fn parses_unit_variant() {
    let item = first_item("enum Direction {\n    North,\n    South,\n}\n");
    let ItemKind::Enum(decl) = item.kind else {
        panic!("expected an enum");
    };
    assert!(decl.variants[0].payload.is_empty());
}

#[test]
fn parses_use_declaration() {
    let item = first_item("use util.math as m\n");
    let ItemKind::Use(decl) = item.kind else {
        panic!("expected a use");
    };
    assert_eq!(decl.path, vec!["util".to_owned(), "math".to_owned()]);
    assert_eq!(decl.alias.as_deref(), Some("m"));
}

#[test]
fn parses_use_without_alias() {
    let item = first_item("use util\n");
    let ItemKind::Use(decl) = item.kind else {
        panic!("expected a use");
    };
    assert_eq!(decl.path, vec!["util".to_owned()]);
    assert!(decl.alias.is_none());
}

#[test]
fn parses_several_items() {
    let program =
        parse_ok("fn a() {\n}\n\ndata D {\n    x: Int,\n}\n\nenum E {\n    V,\n}\n\nfn b() {\n}\n");
    assert_eq!(program.items.len(), 4);
}

// --- Types ------------------------------------------------------------------

#[test]
fn parses_generic_type_annotation() {
    let item = first_item("fn f(xs: List<Int>) -> Map<Str, Int> {\n}\n");
    let ItemKind::Fn(decl) = item.kind else {
        panic!("expected a function");
    };
    assert!(matches!(
        decl.params[0].ty.kind,
        TypeKind::Named { ref name, ref args } if name == "List" && args.len() == 1
    ));
    let Some(ret) = decl.ret else {
        panic!("expected a return type");
    };
    assert!(matches!(ret.kind, TypeKind::Named { ref name, .. } if name == "Map"));
}

#[test]
fn parses_tuple_and_fn_types() {
    let item = first_item("fn f(p: (Int, Str), g: fn(Int) -> Int) {\n}\n");
    let ItemKind::Fn(decl) = item.kind else {
        panic!("expected a function");
    };
    assert!(matches!(decl.params[0].ty.kind, TypeKind::Tuple(_)));
    assert!(matches!(decl.params[1].ty.kind, TypeKind::Fn { .. }));
}

#[test]
fn parses_unit_type() {
    let item = first_item("fn f() -> () {\n}\n");
    let ItemKind::Fn(decl) = item.kind else {
        panic!("expected a function");
    };
    let Some(ret) = decl.ret else {
        panic!("expected a return type");
    };
    assert!(matches!(ret.kind, TypeKind::Unit));
}

// --- Patterns ---------------------------------------------------------------

#[test]
fn parses_wildcard_pattern() {
    let program = parse_ok("fn main() {\n    match x { _ => 1 }\n}\n");
    let ItemKind::Fn(decl) = &program.items[0].kind else {
        panic!("expected a function");
    };
    let crate::ast::StmtKind::Expr(expr) = &decl.body.statements[0].kind else {
        panic!("expected an expression statement");
    };
    let crate::ast::ExprKind::Match { arms, .. } = &expr.kind else {
        panic!("expected a match");
    };
    assert_eq!(arms[0].pattern.kind, PatternKind::Wildcard);
}

#[test]
fn parses_variant_pattern_with_fields() {
    let program = parse_ok("fn main() {\n    match s { Rect(w, h) => w * h, _ => 0.0 }\n}\n");
    let ItemKind::Fn(decl) = &program.items[0].kind else {
        panic!("expected a function");
    };
    let crate::ast::StmtKind::Expr(expr) = &decl.body.statements[0].kind else {
        panic!("expected an expression statement");
    };
    let crate::ast::ExprKind::Match { arms, .. } = &expr.kind else {
        panic!("expected a match");
    };
    let PatternKind::Variant { name, fields } = &arms[0].pattern.kind else {
        panic!("expected a variant pattern");
    };
    assert_eq!(name, "Rect");
    assert_eq!(fields.len(), 2);
}

#[test]
fn parses_literal_pattern() {
    let program = parse_ok("fn main() {\n    match n { 0 => 1, _ => 2 }\n}\n");
    let ItemKind::Fn(decl) = &program.items[0].kind else {
        panic!("expected a function");
    };
    let crate::ast::StmtKind::Expr(expr) = &decl.body.statements[0].kind else {
        panic!("expected an expression statement");
    };
    let crate::ast::ExprKind::Match { arms, .. } = &expr.kind else {
        panic!("expected a match");
    };
    assert!(matches!(arms[0].pattern.kind, PatternKind::Literal(_)));
}

// --- Out-of-alpha items report E0105 ----------------------------------------

#[test]
fn intent_is_refused_with_e0105() {
    let parsed = parse("intent f() -> Int {\n    1\n}\n");
    assert_eq!(parsed.codes, vec!["E0105"]);
    assert!(
        parsed.messages[0].contains("intent"),
        "got: {:?}",
        parsed.messages
    );
}

#[test]
fn how_is_refused_with_e0105() {
    let parsed = parse("how f() {\n}\n");
    assert_eq!(parsed.codes, vec!["E0105"]);
}

#[test]
fn test_block_is_refused_with_e0105() {
    let parsed = parse("test \"x\" {\n}\n");
    assert_eq!(parsed.codes, vec!["E0105"]);
}

#[test]
fn use_py_is_refused_with_e0105() {
    // `py` is contextual (§5.1), so this is a `use` path starting with `py`.
    let parsed = parse("use py math\n");
    assert_eq!(parsed.codes, vec!["E0105"]);
    assert!(
        parsed.messages[0].contains("use py"),
        "got: {:?}",
        parsed.messages
    );
}

#[test]
fn pub_data_is_refused_with_e0105() {
    let parsed = parse("pub data D {\n    x: Int,\n}\n");
    assert_eq!(parsed.codes, vec!["E0105"]);
}

// --- Item recovery ----------------------------------------------------------

#[test]
fn a_broken_item_does_not_hide_the_next_one() {
    let parsed = parse("fn ( {\n}\nfn ok() {\n}\n");
    assert!(!parsed.codes.is_empty());
    let functions: Vec<&str> = parsed
        .program
        .items
        .iter()
        .filter_map(|item| match &item.kind {
            ItemKind::Fn(decl) => Some(decl.name.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        functions.contains(&"ok"),
        "the valid function should survive: {functions:?}"
    );
}

#[test]
fn an_empty_program_has_no_items_and_no_errors() {
    let parsed = parse("");
    assert!(parsed.program.items.is_empty());
    assert!(parsed.codes.is_empty());
}

#[test]
fn a_comment_only_program_has_no_items() {
    let parsed = parse("// just a comment\n");
    assert!(parsed.program.items.is_empty());
    assert!(parsed.codes.is_empty());
}

#[test]
fn parses_the_appendix_a2_data_example() {
    // §4.2: `data` with a named-argument constructor call.
    parse_ok(
        r#"
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
"#,
    );
}

#[test]
fn parses_the_appendix_a2_enum_and_match_example() {
    // §A2: `enum` with payload and a `match` whose arms end in commas.
    parse_ok(
        r#"
enum Shape { Circle(Float), Rect(Float, Float) }

fn area(s: Shape) -> Float {
    match s {
        Circle(r)  => 3.14159 * r * r,
        Rect(w, h) => w * h,
    }
}

fn main() { print(area(Rect(2.0, 3.0))) }
"#,
    );
}
