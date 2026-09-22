use super::*;
// --- Newlines and the delimiter stack --------------------------------------

#[test]
fn no_newline_at_start_of_file() {
    assert_eq!(lex_kinds("\n\n\nfn"), vec![TokenKind::Kw(Keyword::Fn)]);
}

#[test]
fn consecutive_newlines_collapse() {
    assert_eq!(
        lex_kinds("a\n\n\n b"),
        vec![ident("a"), TokenKind::Newline, ident("b")]
    );
}

#[test]
fn crlf_is_one_line_break() {
    assert_eq!(
        lex_kinds("a\r\nb"),
        vec![ident("a"), TokenKind::Newline, ident("b")]
    );
}

#[test]
fn lone_carriage_return_is_whitespace() {
    assert_eq!(lex_kinds("a\rb"), vec![ident("a"), ident("b")]);
}

#[test]
fn lone_carriage_return_does_not_end_a_line_comment() {
    // `\r` is whitespace, so it stays inside the comment; only `\n` ends it.
    assert_eq!(
        lex_kinds("a // c\rd\nb"),
        vec![ident("a"), TokenKind::Newline, ident("b")]
    );
}

#[test]
fn carriage_return_newline_does_end_a_line_comment() {
    assert_eq!(
        lex_kinds("a // c\r\nb"),
        vec![ident("a"), TokenKind::Newline, ident("b")]
    );
}

#[test]
fn lone_carriage_return_inside_a_block_comment_is_not_a_newline() {
    // Only `\n` and `\r\n` count as a line break inside `/* */`.
    assert_eq!(lex_kinds("a /* x\ry */ b"), vec![ident("a"), ident("b")]);
}

#[test]
fn crlf_inside_a_block_comment_is_one_newline() {
    assert_eq!(
        lex_kinds("a /* x\r\ny */ b"),
        vec![ident("a"), TokenKind::Newline, ident("b")]
    );
}

#[test]
fn bom_is_ignored() {
    // `SourceMap::add` strips the leading BOM, so the lexer never sees it.
    assert_eq!(
        lex_kinds("\u{feff}fn main"),
        vec![TokenKind::Kw(Keyword::Fn), ident("main")]
    );
}

#[test]
fn second_bom_is_an_invalid_character() {
    // Only the first BOM is stripped (by `SourceMap::add`); a second one is
    // ordinary content the lexer must reject, not silently eat.
    let (tokens, diagnostics) = lex_src("\u{feff}\u{feff}x");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0001");
    let kinds: Vec<TokenKind> = tokens
        .into_iter()
        .filter(|t| !t.is_eof())
        .map(|t| t.kind)
        .collect();
    assert_eq!(kinds, vec![ident("x")]);
}

#[test]
fn parens_suppress_newlines() {
    assert_eq!(
        lex_kinds("f(\na,\nb\n)"),
        vec![
            ident("f"),
            TokenKind::LParen,
            ident("a"),
            TokenKind::Comma,
            ident("b"),
            TokenKind::RParen
        ]
    );
}

#[test]
fn brackets_suppress_newlines() {
    assert_eq!(
        lex_kinds("[\n1,\n2\n]"),
        vec![
            TokenKind::LBracket,
            TokenKind::Int(1),
            TokenKind::Comma,
            TokenKind::Int(2),
            TokenKind::RBracket
        ]
    );
}

#[test]
fn hash_brace_suppresses_newlines() {
    // The mandatory M1 test: a multiline `#{ }` map literal.
    assert_eq!(
        lex_kinds("a #{\n\"a\": 1,\n\"b\": 2,\n}\nb"),
        vec![
            ident("a"),
            TokenKind::HashLBrace,
            str_parts(vec![lit("a")]),
            TokenKind::Colon,
            TokenKind::Int(1),
            TokenKind::Comma,
            str_parts(vec![lit("b")]),
            TokenKind::Colon,
            TokenKind::Int(2),
            TokenKind::Comma,
            TokenKind::RBrace,
            TokenKind::Newline,
            ident("b"),
        ]
    );
}

#[test]
fn block_brace_keeps_newlines_significant_inside_parens() {
    // The mandatory M1 test: a lambda with a block inside a call.
    let kinds = lex_kinds("xs.map(x => {\nlet y = 1\ny\n})");
    assert_eq!(
        kinds,
        vec![
            ident("xs"),
            TokenKind::Dot,
            ident("map"),
            TokenKind::LParen,
            ident("x"),
            TokenKind::FatArrow,
            TokenKind::LBrace,
            // The `{` makes newlines significant even though a `(` is open.
            TokenKind::Newline,
            TokenKind::Kw(Keyword::Let),
            ident("y"),
            TokenKind::Eq,
            TokenKind::Int(1),
            TokenKind::Newline,
            ident("y"),
            TokenKind::Newline,
            TokenKind::RBrace,
            TokenKind::RParen,
        ]
    );
}

#[test]
fn newline_after_closing_hash_brace_is_emitted() {
    let kinds = lex_kinds("x #{}\ny");
    assert!(
        kinds.contains(&TokenKind::Newline),
        "expected a Newline after the map's closing brace, got {kinds:?}"
    );
}

#[test]
fn unmatched_closers_do_not_panic() {
    // Unbalanced input is the parser's problem; the lexer only emits tokens.
    assert_eq!(
        lex_kinds(") ] }"),
        vec![TokenKind::RParen, TokenKind::RBracket, TokenKind::RBrace]
    );
    // A closer with no opener must not corrupt the suppression state.
    assert_eq!(
        lex_kinds(")\na"),
        vec![TokenKind::RParen, TokenKind::Newline, ident("a")]
    );
}

#[test]
fn a_closer_unwinds_mismatched_openers_above_its_match() {
    // `f(1 }` closes the call from inside the block: the `}` matches the `{`
    // that is *below* the `(` on the stack, so the `(` is wound down with it.
    // The following Newline must therefore be emitted (ADR 0008).
    assert_eq!(
        lex_kinds("fn g() { f(1 }\nlet b = 2"),
        vec![
            TokenKind::Kw(Keyword::Fn),
            ident("g"),
            TokenKind::LParen,
            TokenKind::RParen,
            TokenKind::LBrace,
            ident("f"),
            TokenKind::LParen,
            TokenKind::Int(1),
            TokenKind::RBrace,
            TokenKind::Newline,
            TokenKind::Kw(Keyword::Let),
            ident("b"),
            TokenKind::Eq,
            TokenKind::Int(2),
        ]
    );
}

#[test]
fn a_closer_with_no_match_leaves_the_stack_alone() {
    // `)` with no `(` on the stack must not pop the enclosing `{`, which keeps
    // Newlines significant afterwards.
    assert_eq!(
        lex_kinds("{\n)\na\n}"),
        vec![
            TokenKind::LBrace,
            TokenKind::Newline,
            TokenKind::RParen,
            TokenKind::Newline,
            ident("a"),
            TokenKind::Newline,
            TokenKind::RBrace,
        ]
    );
}

#[test]
fn nested_delimiters_use_the_innermost_context() {
    // Inside `(` then `{`: the block brace wins, so the newline survives.
    assert_eq!(
        lex_kinds("({\na\n})"),
        vec![
            TokenKind::LParen,
            TokenKind::LBrace,
            TokenKind::Newline,
            ident("a"),
            TokenKind::Newline,
            TokenKind::RBrace,
            TokenKind::RParen,
        ]
    );
}
