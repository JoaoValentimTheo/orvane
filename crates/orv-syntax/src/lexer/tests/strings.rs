use super::*;
// --- Strings ---------------------------------------------------------------

#[test]
fn lexes_plain_string_with_escapes() {
    // `\n`, `\t`, `\r`, `\\`, `\"`, `\{` and `\}` each resolve to one char.
    assert_eq!(
        lex_kinds(r#""\n\t\r\\\"\{\}""#),
        vec![str_parts(vec![lit("\n\t\r\\\"{}")])]
    );
}

#[test]
fn escaped_brace_does_not_open_interpolation() {
    assert_eq!(
        lex_kinds(r#""\{name\}""#),
        vec![str_parts(vec![lit("{name}")])]
    );
}

#[test]
fn lexes_empty_string() {
    assert_eq!(lex_kinds(r#""""#), vec![str_parts(vec![])]);
}

#[test]
fn lexes_interpolation() {
    // `á` is two bytes, so `name` starts at byte 7 (spans are byte offsets).
    assert_eq!(
        lex_kinds(r#""Olá {name}""#),
        vec![str_parts(vec![
            lit("Olá "),
            StrPart::Expr {
                src: "name".to_owned(),
                span: crate::Span::new(FileId(0), 7, 11),
            },
        ])]
    );
}

#[test]
fn interpolation_span_points_into_the_file() {
    let (tokens, _) = lex_src(r#""{a + b}""#);
    let Some(TokenKind::Str(parts)) = tokens.first().map(|t| &t.kind) else {
        panic!("expected a string token");
    };
    let Some(StrPart::Expr { src, span }) = parts.first() else {
        panic!("expected an expression part");
    };
    assert_eq!(src, "a + b");
    assert_eq!((span.start, span.end), (2, 7));
}

#[test]
fn literal_braces_via_doubling() {
    assert_eq!(lex_kinds(r#""{{}}""#), vec![str_parts(vec![lit("{}")])]);
    assert_eq!(
        lex_kinds(r#""a{{b}}c""#),
        vec![str_parts(vec![lit("a{b}c")])]
    );
}

#[test]
fn interpolation_allows_nested_strings_and_braces() {
    let kinds = lex_kinds(r#""{f("a")}""#);
    assert_eq!(
        kinds,
        vec![str_parts(vec![StrPart::Expr {
            src: r#"f("a")"#.to_owned(),
            span: crate::Span::new(FileId(0), 2, 8),
        }])]
    );
    // Nested braces inside the expression keep depth balanced, and the source
    // is captured verbatim (including surrounding spaces) for the parser.
    let kinds = lex_kinds(r#""{ {1: 2} }""#);
    assert_eq!(
        kinds,
        vec![str_parts(vec![StrPart::Expr {
            src: " {1: 2} ".to_owned(),
            span: crate::Span::new(FileId(0), 2, 10),
        }])]
    );
}

#[test]
fn backslash_inside_interpolation_is_e0006() {
    // A `\` in an interpolation is not an escape: nested strings are written
    // raw (ADR 0008). `\"a\"` has two backslashes but reports **one** `E0006`
    // per interpolation (ADR 0009), and the scanner still runs to the closing
    // `}` so the expression part is produced.
    let (tokens, diagnostics) = lex_src(r#""{f(\"a\")}""#);
    let e0006: Vec<_> = diagnostics.iter().filter(|d| d.code == "E0006").collect();
    assert_eq!(
        e0006.len(),
        1,
        "one diagnostic per interpolation: {diagnostics:?}"
    );
    assert_eq!(
        (e0006[0].primary.start, e0006[0].primary.end),
        (4, 5),
        "E0006 covers the first backslash"
    );
    let help = e0006[0].help.clone().unwrap_or_default();
    assert!(
        help.contains("without escaping"),
        "help should show the raw form, got: {help:?}"
    );
    let kinds: Vec<TokenKind> = tokens
        .into_iter()
        .filter(|t| !t.is_eof())
        .map(|t| t.kind)
        .collect();
    assert_eq!(
        kinds,
        vec![str_parts(vec![StrPart::Expr {
            src: r#"f(\"a\")"#.to_owned(),
            span: crate::Span::new(FileId(0), 2, 10),
        }])]
    );
}

#[test]
fn backslash_before_a_line_break_inside_interpolation_is_reported() {
    // A `\` followed by a line break must not swallow the break, whether the
    // break is LF or CRLF: both report the same diagnostics (ADR 0009).
    for source in ["\"{f(\"a\\\nb\")}\"", "\"{f(\"a\\\r\nb\")}\""] {
        let (_, diagnostics) = lex_src(source);
        let codes: Vec<&str> = diagnostics.iter().map(|d| d.code).collect();
        assert!(
            codes.contains(&"E0002"),
            "{source:?} should report E0002, got {codes:?}"
        );
        assert!(
            codes.contains(&"E0006"),
            "{source:?} should report E0006, got {codes:?}"
        );
    }
}

#[test]
fn line_break_after_backslash_inside_interpolation_is_not_swallowed() {
    // The break survives as trivia, so the next line is lexed normally instead
    // of being folded into the expression source.
    for source in ["\"{f(\"a\\\nb\")}\"", "\"{f(\"a\\\r\nb\")}\""] {
        let (tokens, _) = lex_src(source);
        let kinds: Vec<TokenKind> = tokens
            .into_iter()
            .filter(|t| !t.is_eof())
            .map(|t| t.kind)
            .collect();
        assert!(
            kinds.contains(&TokenKind::Newline),
            "{source:?} must emit a Newline, got {kinds:?}"
        );
        // The raw break never ends up inside an interpolation's source.
        for kind in &kinds {
            if let TokenKind::Str(parts) = kind {
                for part in parts {
                    if let StrPart::Expr { src, .. } = part {
                        assert!(
                            !src.contains('\n') && !src.contains('\r'),
                            "{source:?} leaked a line break into {src:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn lf_and_crlf_agree_inside_a_nested_string() {
    // The nested-string path must treat the break the same way.
    for source in ["\"{f(\"a\\\nb\")}\"", "\"{f(\"a\\\r\nb\")}\""] {
        let (_, diagnostics) = lex_src(source);
        assert!(
            diagnostics.iter().any(|d| d.code == "E0002"),
            "{source:?} should report E0002, got {diagnostics:?}"
        );
    }
}

#[test]
fn interpolation_with_a_raw_nested_string_is_valid() {
    // The documented spelling for a nested string: no backslashes.
    assert_eq!(
        lex_kinds(r#""{f("a")}""#),
        vec![str_parts(vec![StrPart::Expr {
            src: r#"f("a")"#.to_owned(),
            span: crate::Span::new(FileId(0), 2, 8),
        }])]
    );
}

#[test]
fn literal_braces_around_an_interpolation() {
    // `{{` ... `}}` are literal braces; `{x}` between them interpolates.
    assert_eq!(
        lex_kinds(r#""{{{x}}}""#),
        vec![str_parts(vec![
            lit("{"),
            StrPart::Expr {
                src: "x".to_owned(),
                span: crate::Span::new(FileId(0), 4, 5),
            },
            lit("}"),
        ])]
    );
}

#[test]
fn non_ascii_inside_strings_is_fine() {
    assert_eq!(
        lex_kinds(r#""日本語""#),
        vec![str_parts(vec![lit("日本語")])]
    );
}

#[test]
fn unterminated_string_reports_e0002() {
    assert_eq!(lex_errors(r#""abc"#), vec!["E0002"]);
}

#[test]
fn raw_newline_in_string_reports_e0002() {
    // The raw newline ends the string with `E0002`; scanning resumes at `def`,
    // which is itself unterminated, so a second `E0002` follows. The first
    // diagnostic is the one that points at the real mistake.
    let codes = lex_errors("\"abc\ndef\"");
    assert_eq!(codes.first(), Some(&"E0002"));
}

#[test]
fn unterminated_string_does_not_stop_the_lexer() {
    // Recovery: the broken string still yields a best-effort `Str` token, and
    // scanning resumes afterwards (ADR 0008).
    let (tokens, diagnostics) = lex_src("\"abc\nfn main");
    assert_eq!(diagnostics.len(), 1);
    let kinds: Vec<TokenKind> = tokens
        .into_iter()
        .filter(|t| !t.is_eof())
        .map(|t| t.kind)
        .collect();
    assert_eq!(
        kinds,
        vec![
            str_parts(vec![lit("abc")]),
            TokenKind::Newline,
            TokenKind::Kw(Keyword::Fn),
            ident("main")
        ]
    );
}

#[test]
fn unknown_escape_reports_e0004() {
    assert_eq!(lex_errors(r#""a\q""#), vec!["E0004"]);
}

#[test]
fn unknown_escape_message_uses_escape_debug() {
    // A bare control character inside a string is ordinary content, not an
    // error; only the escape after `\` is unknown.
    let (_, diagnostics) = lex_src("\"a\\\u{7}\"");
    let message = diagnostics
        .iter()
        .find(|d| d.code == "E0004")
        .map(|d| d.message.clone())
        .unwrap_or_default();
    assert!(
        message.contains("\\u{7}"),
        "the control char must be escaped in the message, got: {message:?}"
    );
    assert!(
        !message.contains('\n') && !message.contains('\r'),
        "message must be single-line: {message:?}"
    );
}

#[test]
fn backslash_before_a_line_break_reports_e0004_on_the_backslash_only() {
    // The `\` is the error; the break is left for the caller, so the string is
    // then reported as unterminated by the normal newline rule.
    let (_, diagnostics) = lex_src("\"a\\\nb\"");
    let e0004: Vec<_> = diagnostics.iter().filter(|d| d.code == "E0004").collect();
    assert_eq!(e0004.len(), 1, "got: {diagnostics:?}");
    let span = e0004[0].primary;
    assert_eq!(
        (span.start, span.end),
        (2, 3),
        "E0004 must cover only the backslash"
    );
    assert!(
        diagnostics.iter().any(|d| d.code == "E0002"),
        "the string is still unterminated: {diagnostics:?}"
    );
}

#[test]
fn backslash_before_a_line_break_does_not_swallow_the_newline() {
    // The `\n` survives as trivia; scanning resumes on the next line and `b`
    // is lexed as an identifier, which would be impossible if the break had
    // been consumed as part of an escape. The broken string still emits a
    // best-effort `Str` (ADR 0008).
    let (tokens, _) = lex_src("\"a\\\nb\"");
    let kinds: Vec<TokenKind> = tokens
        .into_iter()
        .filter(|t| !t.is_eof())
        .map(|t| t.kind)
        .collect();
    assert_eq!(
        kinds,
        vec![
            str_parts(vec![lit("a")]),
            TokenKind::Newline,
            ident("b"),
            str_parts(vec![])
        ],
        "the line break was consumed"
    );
}

#[test]
fn lone_closing_brace_reports_e0006() {
    assert_eq!(lex_errors(r#""a}b""#), vec!["E0006"]);
}

#[test]
fn empty_interpolation_reports_e0006() {
    assert_eq!(lex_errors(r#""{}""#), vec!["E0006"]);
    assert_eq!(lex_errors(r#""{  }""#), vec!["E0006"]);
}

#[test]
fn unclosed_interpolation_reports_e0002() {
    let codes = lex_errors(r#""{a""#);
    assert_eq!(codes.first(), Some(&"E0002"));
}

#[test]
fn unclosed_interpolation_reports_a_single_e0002() {
    // Decision D: one outermost diagnostic, not one per inner symptom.
    let (tokens, diagnostics) = lex_src("\"{a\n");
    let e0002: Vec<_> = diagnostics.iter().filter(|d| d.code == "E0002").collect();
    assert_eq!(e0002.len(), 1, "got: {diagnostics:?}");
    // The reported span starts at the `{` (byte 1).
    assert_eq!(e0002[0].primary.start, 1);
    let kinds: Vec<TokenKind> = tokens
        .into_iter()
        .filter(|t| !t.is_eof())
        .map(|t| t.kind)
        .collect();
    assert_eq!(kinds, vec![str_parts(vec![]), TokenKind::Newline]);
}

#[test]
fn error_tokens_are_best_effort() {
    // Decision A: a malformed literal still contributes a token.
    let (tokens, _) = lex_src("0x");
    let kinds: Vec<TokenKind> = tokens
        .into_iter()
        .filter(|t| !t.is_eof())
        .map(|t| t.kind)
        .collect();
    assert_eq!(kinds, vec![TokenKind::Int(0)]);

    // A malformed float keeps its float-ness in the placeholder.
    let (tokens, _) = lex_src("1.2e");
    let kinds: Vec<TokenKind> = tokens
        .into_iter()
        .filter(|t| !t.is_eof())
        .map(|t| t.kind)
        .collect();
    assert_eq!(kinds, vec![TokenKind::Float(0.0)]);

    // A broken string keeps the parts read so far.
    let (tokens, _) = lex_src("\"ab\\q\"");
    let kinds: Vec<TokenKind> = tokens
        .into_iter()
        .filter(|t| !t.is_eof())
        .map(|t| t.kind)
        .collect();
    assert_eq!(kinds, vec![str_parts(vec![lit("ab")])]);
}
