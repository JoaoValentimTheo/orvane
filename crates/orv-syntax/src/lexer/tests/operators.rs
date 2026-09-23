use super::*;
// --- Operators and punctuation ---------------------------------------------

#[test]
fn lexes_all_operators() {
    let source =
        "( ) [ ] { } #{ , : ; . .. ..= -> => ? ?. ?? = == != < <= > >= + - * / % += -= *= /=";
    let expected = vec![
        TokenKind::LParen,
        TokenKind::RParen,
        TokenKind::LBracket,
        TokenKind::RBracket,
        TokenKind::LBrace,
        TokenKind::RBrace,
        TokenKind::HashLBrace,
        TokenKind::Comma,
        TokenKind::Colon,
        TokenKind::Semicolon,
        TokenKind::Dot,
        TokenKind::DotDot,
        TokenKind::DotDotEq,
        TokenKind::Arrow,
        TokenKind::FatArrow,
        TokenKind::Question,
        TokenKind::QuestionDot,
        TokenKind::QuestionQuestion,
        TokenKind::Eq,
        TokenKind::EqEq,
        TokenKind::NotEq,
        TokenKind::Lt,
        TokenKind::Le,
        TokenKind::Gt,
        TokenKind::Ge,
        TokenKind::Plus,
        TokenKind::Minus,
        TokenKind::Star,
        TokenKind::Slash,
        TokenKind::Percent,
        TokenKind::PlusEq,
        TokenKind::MinusEq,
        TokenKind::StarEq,
        TokenKind::SlashEq,
    ];
    assert_eq!(lex_kinds(source), expected);
}

#[test]
fn maximal_munch_prefers_longer_operators() {
    assert_eq!(lex_kinds("..="), vec![TokenKind::DotDotEq]);
    assert_eq!(lex_kinds("?."), vec![TokenKind::QuestionDot]);
    assert_eq!(lex_kinds("??"), vec![TokenKind::QuestionQuestion]);
    assert_eq!(
        lex_kinds("a ?? b"),
        vec![ident("a"), TokenKind::QuestionQuestion, ident("b")]
    );
    assert_eq!(lex_kinds("="), vec![TokenKind::Eq]);
    assert_eq!(lex_kinds("=>"), vec![TokenKind::FatArrow]);
    assert_eq!(lex_kinds("=="), vec![TokenKind::EqEq]);
    assert_eq!(lex_kinds("->"), vec![TokenKind::Arrow]);
}

#[test]
fn triple_dash_is_not_a_single_operator() {
    // `-->` is not in §5.1: it is `-` followed by `->`.
    assert_eq!(
        lex_kinds("a-->b"),
        vec![ident("a"), TokenKind::Minus, TokenKind::Arrow, ident("b")]
    );
    // Sanity: the intermediate `--` is still two `-`, never a token.
    assert_eq!(
        lex_kinds("a--b"),
        vec![ident("a"), TokenKind::Minus, TokenKind::Minus, ident("b")]
    );
}

#[test]
fn hash_without_brace_is_an_error() {
    let codes = lex_errors("#");
    assert_eq!(codes, vec!["E0001"]);
}

#[test]
fn invalid_characters_report_e0001_and_continue() {
    for source in ["!", "&", "|", "@", "$", "\\", "`"] {
        let codes = lex_errors(source);
        assert_eq!(codes, vec!["E0001"], "for {source:?}");
    }
    // Scanning resumes after the bad character.
    let (tokens, diagnostics) = lex_src("a @ b");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0001");
    let kinds: Vec<TokenKind> = tokens
        .into_iter()
        .filter(|t| !t.is_eof())
        .map(|t| t.kind)
        .collect();
    assert_eq!(kinds, vec![ident("a"), ident("b")]);
}

#[test]
fn non_ascii_outside_strings_reports_e0001_with_ascii_help() {
    let (_, diagnostics) = lex_src("let á = 1");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0001");
    let help = diagnostics[0].help.clone().unwrap_or_default();
    assert!(help.contains("ASCII-only"), "got help: {help}");
}
