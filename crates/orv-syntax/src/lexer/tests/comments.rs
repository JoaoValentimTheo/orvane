use super::*;
// --- Comments --------------------------------------------------------------

#[test]
fn line_comment_does_not_consume_the_newline() {
    assert_eq!(
        lex_kinds("a // hi\nb"),
        vec![ident("a"), TokenKind::Newline, ident("b")]
    );
}

#[test]
fn line_comment_at_end_of_file_is_fine() {
    assert_eq!(lex_kinds("a // hi"), vec![ident("a")]);
}

#[test]
fn block_comments_are_nestable() {
    assert_eq!(
        lex_kinds("a /* one /* two */ three */ b"),
        vec![ident("a"), ident("b")]
    );
}

#[test]
fn block_comment_with_newline_counts_as_one_newline() {
    assert_eq!(
        lex_kinds("a /* x\ny\nz */ b"),
        vec![ident("a"), TokenKind::Newline, ident("b")]
    );
}

#[test]
fn block_comment_without_newline_emits_no_newline() {
    assert_eq!(lex_kinds("a /* x */ b"), vec![ident("a"), ident("b")]);
}

#[test]
fn unterminated_block_comment_reports_e0003() {
    assert_eq!(lex_errors("a /* x"), vec!["E0003"]);
}
