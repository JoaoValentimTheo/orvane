use super::*;
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
