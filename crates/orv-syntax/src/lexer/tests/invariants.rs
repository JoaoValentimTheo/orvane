use super::*;
use crate::diagnostics::Diagnostics;
// --- Token stream invariants -----------------------------------------------

#[test]
fn always_ends_with_eof() {
    for source in ["", " ", "fn main() {}", "\n\n", "/* unterminated"] {
        let (tokens, _) = lex_src(source);
        match tokens.last() {
            Some(last) => assert!(last.is_eof(), "for {source:?}"),
            None => panic!("token stream for {source:?} must not be empty"),
        }
    }
}

#[test]
fn spans_are_ordered_and_within_the_file() {
    let source = "fn main() {\n    let x = 1_0\n}\n";
    let (tokens, _) = lex_src(source);
    assert_spans_are_valid(&tokens, source.len());
}

#[test]
fn token_spans_match_the_source_text() {
    let source = "fn main";
    let (tokens, _) = lex_src(source);
    // `fn` covers bytes 0..2 and `main` covers 3..7.
    assert_eq!((tokens[0].span.start, tokens[0].span.end), (0, 2));
    assert_eq!((tokens[1].span.start, tokens[1].span.end), (3, 7));
}

#[test]
fn empty_input_has_only_eof() {
    let (tokens, diagnostics) = lex_src("");
    assert!(diagnostics.is_empty());
    assert_eq!(tokens.len(), 1);
    assert!(tokens[0].is_eof());
}

#[test]
fn keyword_helper_reflects_the_kind() {
    let (tokens, _) = lex_src("fn x");
    assert_eq!(tokens[0].keyword(), Some(Keyword::Fn));
    assert_eq!(tokens[1].keyword(), None);
}

/// Asserts the span invariants: in bounds, on char boundaries, ordered, disjoint.
fn assert_spans_are_valid(tokens: &[Token], file_len: usize) {
    let mut previous_end = 0u32;
    for token in tokens {
        let (start, end) = (token.span.start, token.span.end);
        assert!(
            (start as usize) <= file_len && (end as usize) <= file_len,
            "span {start}..{end} out of bounds for a {file_len}-byte file"
        );
        assert!(start <= end, "span {start}..{end} is backwards");
        assert!(
            start >= previous_end,
            "span {start}..{end} overlaps the previous token ending at {previous_end}"
        );
        previous_end = end;
    }
}

#[test]
fn lexical_diagnostics_cover_their_best_effort_token() {
    // ADR 0008 (emenda, M2): a parser diagnostic whose primary span intersects
    // a lexical one must be suppressed. This pins the raw material the parser
    // needs: for every lexical error, some token span intersects it, so the
    // parser can find the offending region without a new `Token` flag.
    for source in ["0x", "\"abc", "\"a\\q\"", "1e999", "\"a}b\""] {
        let (tokens, diagnostics) = lex_src(source);
        assert!(!diagnostics.is_empty(), "for {source:?}");
        for diagnostic in &diagnostics {
            assert!(
                tokens.iter().any(|t| t.span.intersects(diagnostic.primary)),
                "no token intersects {} for {source:?}: {tokens:?}",
                diagnostic.code
            );
        }
    }
}

#[test]
fn a_newline_after_an_erroneous_token_does_not_overlap_it() {
    // The `Newline` point sits after the broken literal (byte 11 vs span 8..10),
    // so the parser still sees a statement terminator and can resynchronise.
    // A point span only intersects when it lands inside the error, which is what
    // keeps suppression from swallowing unrelated diagnostics.
    let (tokens, diagnostics) = lex_src("let x = 0x\nlet y = 1\n");
    let e0005 = diagnostics
        .iter()
        .find(|d| d.code == "E0005")
        .map(|d| d.primary);
    let e0005 = match e0005 {
        Some(span) => span,
        None => panic!("expected E0005, got {diagnostics:?}"),
    };
    assert!(
        tokens.iter().any(|t| t.span.intersects(e0005)),
        "something must intersect the error: {tokens:?}"
    );
    let newline_after_error = tokens
        .iter()
        .find(|t| t.is_newline() && t.span.start > e0005.end)
        .is_some();
    assert!(
        newline_after_error,
        "the statement terminator must survive so the parser can resync: {tokens:?}"
    );
}

#[test]
fn the_aggregator_suppresses_parser_noise_but_keeps_the_terminator() {
    // (b) with real lexer output: a parser diagnostic anchored on a best-effort
    // token disappears, and the `Newline` that ends the broken statement stays
    // in the stream so the parser can still resynchronise (ADR 0008 amend).
    let (tokens, lexical) = lex_src("let x = 0x\nlet y = 1\n");
    let e0005 = match lexical.iter().find(|d| d.code == "E0005") {
        Some(diagnostic) => diagnostic.primary,
        None => panic!("expected E0005, got {lexical:?}"),
    };
    let terminator = match tokens
        .iter()
        .find(|t| t.is_newline() && t.span.start > e0005.end)
    {
        Some(token) => token,
        None => panic!("expected a Newline after the broken statement: {tokens:?}"),
    };

    // A parser diagnostic about the placeholder token is noise and must go.
    let on_placeholder = Diagnostic::error("E0101", "unexpected token", e0005);
    // The terminator sits *after* the error, so it does not intersect it: a
    // parser diagnostic about the newline is about a token the author really
    // wrote, and it survives. Suppression is not allowed to cascade.
    let on_terminator = Diagnostic::error("E0101", "unexpected newline", terminator.span);

    let mut all = Diagnostics::new(lexical);
    all.extend_suppressed([on_placeholder, on_terminator.clone()]);

    assert_eq!(
        all.as_slice().iter().map(|d| d.code).collect::<Vec<_>>(),
        vec!["E0005", "E0101"],
        "the lexical error and the terminator diagnostic survive: {all:?}"
    );
    assert_eq!(
        all.as_slice().last(),
        Some(&on_terminator),
        "the surviving parser diagnostic is the one about the terminator"
    );
    assert!(
        all.has_errors(),
        "a lexical error must block sema/run (ADR 0008 amend, rule 5)"
    );
}
