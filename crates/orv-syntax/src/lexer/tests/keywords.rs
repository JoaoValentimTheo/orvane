use super::*;
// --- Keywords and identifiers ---------------------------------------------

#[test]
fn lexes_every_reserved_keyword() {
    let source = "fn intent how given ensure when via priority data enum let mut if else \
                  match for in while return break continue use as try fail true false none \
                  and or not pub test";
    let expected: Vec<TokenKind> = [
        Keyword::Fn,
        Keyword::Intent,
        Keyword::How,
        Keyword::Given,
        Keyword::Ensure,
        Keyword::When,
        Keyword::Via,
        Keyword::Priority,
        Keyword::Data,
        Keyword::Enum,
        Keyword::Let,
        Keyword::Mut,
        Keyword::If,
        Keyword::Else,
        Keyword::Match,
        Keyword::For,
        Keyword::In,
        Keyword::While,
        Keyword::Return,
        Keyword::Break,
        Keyword::Continue,
        Keyword::Use,
        Keyword::As,
        Keyword::Try,
        Keyword::Fail,
        Keyword::True,
        Keyword::False,
        Keyword::None,
        Keyword::And,
        Keyword::Or,
        Keyword::Not,
        Keyword::Pub,
        Keyword::Test,
    ]
    .iter()
    .map(|k| TokenKind::Kw(*k))
    .collect();
    assert_eq!(lex_kinds(source), expected);
}

#[test]
fn py_is_lexed_as_an_identifier() {
    assert_eq!(lex_kinds("py"), vec![ident("py")]);
    assert_eq!(
        lex_kinds("py.eval"),
        vec![ident("py"), TokenKind::Dot, ident("eval")]
    );
}

#[test]
fn identifiers_allow_underscores_and_digits() {
    assert_eq!(
        lex_kinds("_x1 __ f2b"),
        vec![ident("_x1"), ident("__"), ident("f2b")]
    );
}

#[test]
fn keyword_prefix_is_not_a_keyword() {
    assert_eq!(lex_kinds("fnx"), vec![ident("fnx")]);
}
