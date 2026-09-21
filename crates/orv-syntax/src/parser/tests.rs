//! Unit tests for the primary-expression parser (SPEC §5.2).

use super::*;
use crate::ast::{Block, Literal};
use crate::lexer::lex;
use crate::source::SourceMap;

/// Lexes and parses `text`, returning the result plus the `SourceFile`.
fn parse(text: &str) -> (ParseResult, crate::source::SourceFile) {
    let mut sources = SourceMap::new();
    let id = sources.add("test.orv", text);
    let file = match sources.file(id).cloned() {
        Some(file) => file,
        None => crate::source::SourceFile::new(crate::source::FileId(0), "test.orv", text),
    };
    let (tokens, lexical) = lex(&file);
    let mut parser = Parser::new(&tokens, lexical);
    (parser.parse_expr(), file)
}

/// Parses `text` expecting no diagnostics, and returns the expression.
fn expr(text: &str) -> Expr {
    let (result, _file) = parse(text);
    assert!(
        result.diagnostics.is_empty(),
        "unexpected diagnostics for {text:?}: {:?}",
        result.diagnostics.as_slice()
    );
    match result.expr {
        Some(expr) => expr,
        None => panic!("expected an expression for {text:?}"),
    }
}

/// The diagnostic codes produced for `text`.
fn codes(text: &str) -> Vec<&'static str> {
    let (result, _file) = parse(text);
    result
        .diagnostics
        .as_slice()
        .iter()
        .map(|d| d.code)
        .collect()
}

// --- Literals and identifiers ----------------------------------------------

#[test]
fn parses_int_literal() {
    let e = expr("42");
    assert_eq!(e.kind, ExprKind::Literal(Literal::Int(42)));
    assert_eq!((e.span.start, e.span.end), (0, 2));
}

#[test]
fn parses_float_literal() {
    let e = expr("1.5");
    assert_eq!(e.kind, ExprKind::Literal(Literal::Float(1.5)));
    assert_eq!((e.span.start, e.span.end), (0, 3));
}

#[test]
fn parses_bool_literals() {
    assert_eq!(expr("true").kind, ExprKind::Literal(Literal::Bool(true)));
    assert_eq!(expr("false").kind, ExprKind::Literal(Literal::Bool(false)));
}

#[test]
fn parses_none_literal() {
    assert_eq!(expr("none").kind, ExprKind::Literal(Literal::None));
}

#[test]
fn parses_string_literal() {
    let e = expr(r#""hi""#);
    let ExprKind::Literal(Literal::Str(parts)) = e.kind else {
        panic!("expected a string literal, got {e:?}");
    };
    assert_eq!(parts.len(), 1, "got: {parts:?}");
    assert_eq!((e.span.start, e.span.end), (0, 4));
}

#[test]
fn parses_interpolated_string_as_the_lexer_built_it() {
    // The parts pass through unchanged: the parser does not re-lex `Expr.src`
    // in this step (ADR 0008 amend, rule 4).
    let e = expr(r#""a{b}c""#);
    let ExprKind::Literal(Literal::Str(parts)) = e.kind else {
        panic!("expected a string literal, got {e:?}");
    };
    assert_eq!(parts.len(), 3, "got: {parts:?}");
}

#[test]
fn parses_identifier() {
    let e = expr("main");
    assert_eq!(e.kind, ExprKind::Ident("main".to_owned()));
    assert_eq!((e.span.start, e.span.end), (0, 4));
}

#[test]
fn parses_contextual_py_as_an_identifier() {
    // `py` is not a keyword (SPEC §5.1), so it arrives as an `Ident`.
    assert_eq!(expr("py").kind, ExprKind::Ident("py".to_owned()));
}

// --- Parenthesised ---------------------------------------------------------

#[test]
fn parses_parenthesised_int() {
    let e = expr("(42)");
    let ExprKind::Paren(inner) = e.kind else {
        panic!("expected parens, got {e:?}");
    };
    assert_eq!(inner.kind, ExprKind::Literal(Literal::Int(42)));
    // The paren node spans both parentheses.
    assert_eq!((e.span.start, e.span.end), (0, 4));
}

#[test]
fn parses_nested_parentheses() {
    let e = expr("((x))");
    let ExprKind::Paren(inner) = e.kind else {
        panic!("expected outer parens, got {e:?}");
    };
    let ExprKind::Paren(innermost) = inner.kind else {
        panic!("expected inner parens, got {inner:?}");
    };
    assert_eq!(innermost.kind, ExprKind::Ident("x".to_owned()));
}

#[test]
fn missing_closing_paren_reports_e0102() {
    assert_eq!(codes("(42"), vec!["E0102"]);
}

#[test]
fn missing_closing_paren_reports_on_the_offending_token() {
    let (result, _file) = parse("(42");
    let diagnostic = match result.diagnostics.as_slice().first() {
        Some(diagnostic) => diagnostic,
        None => panic!("expected a diagnostic"),
    };
    assert_eq!(diagnostic.code, "E0102");
    assert!(
        diagnostic.message.contains("`)`"),
        "message should name the expected token: {:?}",
        diagnostic.message
    );
    // The caret lands on the token that is not a `)` (the `Eof`), not on an
    // invented position.
    assert_eq!(diagnostic.primary.start, 3);
    assert!(result.expr.is_none(), "a missing `)` yields no expression");
}

#[test]
fn empty_parens_report_e0102() {
    // `()` is the Unit *type*, not an expression primary in §5.2.
    assert_eq!(codes("()"), vec!["E0102"]);
}

// --- Blocks ----------------------------------------------------------------

#[test]
fn parses_empty_block() {
    let e = expr("{}");
    let ExprKind::Block(block) = e.kind else {
        panic!("expected a block, got {e:?}");
    };
    assert!(block.statements.is_empty());
    assert_eq!((block.span.start, block.span.end), (0, 2));
    assert_eq!(e.span, block.span);
}

#[test]
fn parses_block_of_newlines() {
    let e = expr("{\n\n}");
    let ExprKind::Block(block) = e.kind else {
        panic!("expected a block, got {e:?}");
    };
    assert!(
        block.statements.is_empty(),
        "newlines are layout, not content"
    );
}

#[test]
fn unclosed_block_reports_e0102() {
    assert_eq!(codes("{"), vec!["E0102"]);
}

#[test]
fn a_block_with_content_is_rejected_for_now() {
    // Statements are out of scope in this step, so the parser refuses rather
    // than silently dropping them.
    assert_eq!(codes("{ 1 }"), vec!["E0101"]);
}

#[test]
fn block_empty_constructor_has_the_given_span() {
    let span = Span::new(crate::source::FileId(0), 3, 9);
    let block = Block::new(span);
    assert!(block.statements.is_empty());
    assert_eq!(block.span, span);
}

// --- Errors ----------------------------------------------------------------

#[test]
fn a_token_that_cannot_start_a_primary_reports_e0102() {
    for source in ["+", ")", "]", ",", "."] {
        let codes = codes(source);
        assert_eq!(codes, vec!["E0102"], "for {source:?}");
    }
}

#[test]
fn expected_message_names_what_was_found() {
    let (result, _file) = parse("+");
    let diagnostic = match result.diagnostics.as_slice().first() {
        Some(diagnostic) => diagnostic,
        None => panic!("expected a diagnostic"),
    };
    assert!(
        diagnostic.message.contains("expected expression"),
        "got: {:?}",
        diagnostic.message
    );
}

#[test]
fn empty_input_reports_e0102_about_end_of_file() {
    let (result, _file) = parse("");
    assert_eq!(result.diagnostics.len(), 1);
    let diagnostic = &result.diagnostics.as_slice()[0];
    assert_eq!(diagnostic.code, "E0102");
    assert!(
        diagnostic.message.contains("end of file"),
        "got: {:?}",
        diagnostic.message
    );
    assert!(result.expr.is_none());
}

// --- Diagnostic pipeline integration ---------------------------------------

#[test]
fn lexical_diagnostics_come_first_and_suppress_the_parser() {
    // `0x` is `E0005`, and the best-effort token is `Int(0)`. The lexical error
    // is emitted first; a parser diagnostic on the same span would be
    // suppressed, so the only code that survives is the lexical one.
    let (result, _file) = parse("0x");
    let codes: Vec<&str> = result
        .diagnostics
        .as_slice()
        .iter()
        .map(|d| d.code)
        .collect();
    assert_eq!(codes, vec!["E0005"], "lexical first, parser suppressed");
    assert!(result.has_errors());
}

#[test]
fn no_errors_means_the_sema_gate_is_open() {
    let (result, _file) = parse("42");
    assert!(!result.has_errors());
}

#[test]
fn a_string_with_a_lexical_error_blocks_the_subparse_and_adds_nothing() {
    // ADR 0008 amend, rule 4: `"{f(\"a\")}"` carries `E0006`, so the `Str` is
    // best effort. The guard must report "no sub-parse", and the parser must not
    // invent a diagnostic from `Expr.src`.
    let (result, _file) = parse(r#""{f(\"a\")}""#);
    let codes: Vec<&str> = result
        .diagnostics
        .as_slice()
        .iter()
        .map(|d| d.code)
        .collect();
    assert_eq!(
        codes,
        vec!["E0006"],
        "only the lexical error survives: {:?}",
        result.diagnostics.as_slice()
    );

    // The expression is still produced (the `Str` is best effort, not absent).
    let Some(expr) = result.expr else {
        panic!("the best-effort string should still yield an expression");
    };
    let ExprKind::Literal(Literal::Str(_)) = expr.kind else {
        panic!("expected a string literal, got {expr:?}");
    };
}

#[test]
fn the_subparse_guard_is_consulted_for_strings() {
    // A clean string has the sub-parse path open; a broken one does not. This
    // pins the guard's value at the parser boundary, using the same
    // `Diagnostics` state the parser holds.
    let (clean, _file) = parse(r#""a{b}c""#);
    let clean_span = clean.expr.as_ref().map(|e| e.span);
    let clean_span = match clean_span {
        Some(span) => span,
        None => panic!("expected an expression"),
    };
    assert!(clean.diagnostics.should_subparse_expr(clean_span));

    let (broken, _file) = parse(r#""{f(\"a\")}""#);
    let broken_span = broken.expr.as_ref().map(|e| e.span);
    let broken_span = match broken_span {
        Some(span) => span,
        None => panic!("expected an expression"),
    };
    assert!(!broken.diagnostics.should_subparse_expr(broken_span));
}

#[test]
fn parser_diagnostics_are_reported_through_the_aggregation() {
    // Sanity: `E0102` reaches `as_slice`, so suppression did not eat a
    // legitimate error far from any lexical problem.
    let (result, _file) = parse("(42");
    assert_eq!(result.diagnostics.as_slice().len(), 1);
    assert!(result.has_errors());
}

#[test]
fn a_parser_error_inside_a_broken_literal_is_suppressed() {
    // `(0x`: the lexer reports `E0005` and emits a best-effort `Int(0)`; the
    // parser then finds `Eof` where it wants `)`, producing an `E0102` at the
    // end of the broken literal's span. That intersection is exactly what
    // ADR 0008 amend rule 3 suppresses — the author should see the lexical
    // cause, not a cascade.
    let (result, _file) = parse("(0x");
    let codes: Vec<&str> = result
        .diagnostics
        .as_slice()
        .iter()
        .map(|d| d.code)
        .collect();
    assert_eq!(
        codes,
        vec!["E0005"],
        "the parser error must be suppressed: {codes:?}"
    );
    assert!(result.has_errors());
}

#[test]
fn without_suppression_that_parser_error_would_exist() {
    // Guards the test above: the `E0102` is real, it is only filtered because it
    // intersects the lexical error. `(42` has no lexical error, so the same
    // shape produces `E0102` and it survives.
    assert_eq!(codes("(42"), vec!["E0102"]);
}
