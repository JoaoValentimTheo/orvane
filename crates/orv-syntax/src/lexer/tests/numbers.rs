use super::*;
// --- Integers and floats ---------------------------------------------------

#[test]
fn lexes_simple_int() {
    assert_eq!(lex_kinds("123"), vec![TokenKind::Int(123)]);
}

#[test]
fn lexes_int_with_separators() {
    assert_eq!(lex_kinds("1_000"), vec![TokenKind::Int(1000)]);
}

#[test]
fn lexes_hex_and_binary() {
    assert_eq!(lex_kinds("0xFF"), vec![TokenKind::Int(255)]);
    assert_eq!(lex_kinds("0b1010"), vec![TokenKind::Int(10)]);
    assert_eq!(lex_kinds("0x1_0"), vec![TokenKind::Int(16)]);
}

#[test]
fn radix_prefixes_and_exponents_accept_uppercase() {
    // `0X`/`0B` and `E` are accepted alongside their lowercase forms (ADR 0007).
    assert_eq!(lex_kinds("0XFF"), vec![TokenKind::Int(255)]);
    assert_eq!(lex_kinds("0B1010"), vec![TokenKind::Int(10)]);
    assert_eq!(lex_kinds("1E10"), vec![TokenKind::Float(1e10)]);
    assert_eq!(lex_kinds("1E-3"), vec![TokenKind::Float(1e-3)]);
}

#[test]
fn i64_min_cannot_be_written_as_a_literal() {
    // `9223372036854775808` does not fit in `i64`, so `-9223372036854775808`
    // lexes as `Minus` plus an `E0005`. ADR 0007 §6.2 chose option 2: there is
    // no literal for `i64::MIN`, and the program writes `(-9223372036854775807) - 1`.
    let (tokens, diagnostics) = lex_src("-9223372036854775808");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0005");
    let kinds: Vec<TokenKind> = tokens
        .into_iter()
        .filter(|t| !t.is_eof())
        .map(|t| t.kind)
        .collect();
    assert_eq!(
        kinds,
        vec![TokenKind::Minus, TokenKind::Int(0)],
        "best-effort tokens: minus plus the Int placeholder"
    );
}

#[test]
fn the_overflowing_magnitude_is_not_recoverable_from_tokens() {
    // This is *why* ADR 0007 §6.2 rejected folding `Minus` into the literal:
    // the reject path keeps no trace of `9223372036854775808`, so no later
    // phase could reconstruct `i64::MIN`. Pinning it prevents someone from
    // "fixing" the missing literal by reading the source back.
    let (tokens, _) = lex_src("-9223372036854775808");
    let rendered: Vec<String> = tokens
        .iter()
        .map(|t| crate::dump::kind_text(&t.kind))
        .collect();
    assert_eq!(rendered, vec!["Minus", "Int(0)", "Eof"]);
    assert!(
        !rendered.iter().any(|k| k.contains("9223372036854775808")),
        "the magnitude must not survive anywhere in the token stream"
    );
}

#[test]
fn i64_max_magnitude_is_fine() {
    // The lexer never applies the sign: `-x` is `Minus` then the literal, so
    // the literal carries the magnitude.
    assert_eq!(
        lex_kinds("-9223372036854775807"),
        vec![TokenKind::Minus, TokenKind::Int(i64::MAX)]
    );
}

#[test]
fn lexes_float_forms() {
    assert_eq!(lex_kinds("1.5"), vec![TokenKind::Float(1.5)]);
    assert_eq!(lex_kinds("2e10"), vec![TokenKind::Float(2e10)]);
    assert_eq!(lex_kinds("1e-3"), vec![TokenKind::Float(1e-3)]);
    assert_eq!(lex_kinds("1.5e+3"), vec![TokenKind::Float(1500.0)]);
    assert_eq!(lex_kinds("1_0.5"), vec![TokenKind::Float(10.5)]);
}

#[test]
fn dot_only_starts_a_fraction_before_a_digit() {
    // `1..5` is Int(1) DotDot Int(5), not a float followed by `.5`.
    assert_eq!(
        lex_kinds("1..5"),
        vec![TokenKind::Int(1), TokenKind::DotDot, TokenKind::Int(5)]
    );
    // `1.foo` is Int(1) Dot Ident(foo).
    assert_eq!(
        lex_kinds("1.foo"),
        vec![TokenKind::Int(1), TokenKind::Dot, ident("foo")]
    );
}

#[test]
fn lexes_i64_boundaries() {
    assert_eq!(
        lex_kinds("9223372036854775807"),
        vec![TokenKind::Int(i64::MAX)]
    );
    // One past `i64::MAX` is `E0005`, with a best-effort `Int(0)` token so the
    // stream still covers the literal (ADR 0008).
    let (tokens, diagnostics) = lex_src("9223372036854775808");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0005");
    let kinds: Vec<TokenKind> = tokens
        .into_iter()
        .filter(|t| !t.is_eof())
        .map(|t| t.kind)
        .collect();
    assert_eq!(kinds, vec![TokenKind::Int(0)]);
}

#[test]
fn invalid_numbers_report_e0005() {
    for source in ["0x", "0b", "1__0", "1_", "0x_1", "1e", "1e+"] {
        let codes = lex_errors(source);
        assert!(
            codes.contains(&"E0005"),
            "{source:?} should report E0005, got {codes:?}"
        );
    }
}

#[test]
fn float_out_of_range_reports_e0005_with_a_float_placeholder() {
    // `1e999` is not representable as a finite `f64`; it must not become
    // `Float(inf)` (ADR 0010).
    for source in ["1e999", "1E999", "1.0e999", "1e309"] {
        let (tokens, diagnostics) = lex_src(source);
        assert_eq!(diagnostics.len(), 1, "for {source:?}");
        assert_eq!(diagnostics[0].code, "E0005", "for {source:?}");
        assert_eq!(
            diagnostics[0].help.as_deref(),
            Some("float literal out of range"),
            "for {source:?}"
        );
        let kinds: Vec<TokenKind> = tokens
            .into_iter()
            .filter(|t| !t.is_eof())
            .map(|t| t.kind)
            .collect();
        assert_eq!(kinds, vec![TokenKind::Float(0.0)], "for {source:?}");
    }
}

#[test]
fn largest_finite_float_is_still_accepted() {
    // `1e308` is finite, so it must not be rejected by the range check.
    let (tokens, diagnostics) = lex_src("1e308");
    assert!(diagnostics.is_empty(), "got: {diagnostics:?}");
    assert!(matches!(tokens.first().map(|t| &t.kind), Some(TokenKind::Float(v)) if v.is_finite()));
}

#[test]
fn chained_dots_are_not_a_number_error() {
    // `1.2.3` is `Float(1.2)` `.` `Int(3)`: the first `.` starts a fraction,
    // the second follows a value and is a field access.
    assert_eq!(
        lex_kinds("1.2.3"),
        vec![TokenKind::Float(1.2), TokenKind::Dot, TokenKind::Int(3)]
    );
}

#[test]
fn float_dot_int_is_a_syntax_error_by_grammar() {
    // ADR 0011: `Float Dot <not-IDENT>` is a *syntax* error, not a lexical one.
    // §5.2 defines `field = "." IDENT`, so §5.2 cannot derive `1.2.3`; the
    // parser rejects it with `E0102`. The lexer must keep producing this
    // sequence, so the decision is not "fixed" in the wrong layer.
    let kinds = lex_kinds("1.2.3");
    assert_eq!(
        kinds,
        vec![TokenKind::Float(1.2), TokenKind::Dot, TokenKind::Int(3)]
    );
    assert!(
        matches!(kinds.get(2), Some(TokenKind::Int(_))),
        "the token after `.` is not an IDENT, which is what the parser rejects"
    );

    // The neighbouring forms *are* derivable and must not be caught by mistake:
    // `1.foo` is Int `Dot` Ident (a `field`), and `1..5` is Int `DotDot` Int.
    assert_eq!(
        lex_kinds("1.foo"),
        vec![TokenKind::Int(1), TokenKind::Dot, ident("foo")]
    );
    assert_eq!(
        lex_kinds("1..5"),
        vec![TokenKind::Int(1), TokenKind::DotDot, TokenKind::Int(5)]
    );
}
