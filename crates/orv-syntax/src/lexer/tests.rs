//! Unit tests for the lexer (SPEC §5.1, §11 item 1).
//!
//! These cover one case per token kind, every error code (`E0001`–`E0006`) and
//! each rule from the milestone: CRLF, BOM, `1..5`, `a ?? b`, `#{`, block
//! comments containing a newline, and nested interpolation.

use proptest::prelude::*;

use crate::diagnostic::Diagnostic;
use crate::lexer::{Keyword, StrPart, Token, TokenKind, lex};
use crate::source::{FileId, SourceFile, SourceMap};

/// Lexes `text` and returns the tokens without the trailing `Eof`.
fn lex_kinds(text: &str) -> Vec<TokenKind> {
    let (tokens, diagnostics) = lex_src(text);
    assert!(
        diagnostics.is_empty(),
        "unexpected diagnostics for {text:?}: {diagnostics:?}"
    );
    tokens
        .into_iter()
        .filter(|t| !t.is_eof())
        .map(|t| t.kind)
        .collect()
}

/// Lexes `text`, returning tokens (including `Eof`) and diagnostics.
fn lex_src(text: &str) -> (Vec<Token>, Vec<Diagnostic>) {
    let (_sources, _file, tokens, diagnostics) = lex_with_sources(text);
    (tokens, diagnostics)
}

/// Lexes `text`, also returning the source map and file so callers can render.
fn lex_with_sources(text: &str) -> (SourceMap, SourceFile, Vec<Token>, Vec<Diagnostic>) {
    let mut sources = SourceMap::new();
    let id = sources.add("test.orv", text);
    let file = match sources.file(id).cloned() {
        Some(file) => file,
        None => SourceFile {
            id: FileId(0),
            name: "test.orv".to_owned(),
            path: None,
            text: std::rc::Rc::from(text),
        },
    };
    let (tokens, diagnostics) = lex(&file);
    (sources, file, tokens, diagnostics)
}

/// Lexes `text`, asserting exactly `n` diagnostics and returning their codes.
fn lex_errors(text: &str) -> Vec<&'static str> {
    let (_, diagnostics) = lex_src(text);
    assert!(
        !diagnostics.is_empty(),
        "expected at least one diagnostic for {text:?}"
    );
    diagnostics.iter().map(|d| d.code).collect()
}

fn ident(name: &str) -> TokenKind {
    TokenKind::Ident(name.to_owned())
}

fn str_parts(parts: Vec<StrPart>) -> TokenKind {
    TokenKind::Str(parts)
}

fn lit(text: &str) -> StrPart {
    StrPart::Lit(text.to_owned())
}

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
    // One past `i64::MAX` is `E0005`, not a wrapped or truncated value.
    let (tokens, diagnostics) = lex_src("9223372036854775808");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0005");
    assert!(
        tokens.iter().all(|t| t.is_eof()),
        "an invalid literal produces no token: {tokens:?}"
    );
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
fn chained_dots_are_not_a_number_error() {
    // `1.2.3` is `Float(1.2)` `.` `Int(3)`: the first `.` starts a fraction,
    // the second follows a value and is a field access.
    assert_eq!(
        lex_kinds("1.2.3"),
        vec![TokenKind::Float(1.2), TokenKind::Dot, TokenKind::Int(3)]
    );
}

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
fn escaped_quotes_inside_interpolation_do_not_close_the_string() {
    // `\"` is a literal quote, so the interpolation runs to the real `}`.
    let kinds = lex_kinds(r#""{f(\"a\")}""#);
    assert_eq!(
        kinds,
        vec![str_parts(vec![StrPart::Expr {
            src: r#"f(\"a\")"#.to_owned(),
            span: crate::Span::new(FileId(0), 2, 10),
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
    // Recovery: tokens after the broken string are still produced.
    let (tokens, diagnostics) = lex_src("\"abc\nfn main");
    assert_eq!(diagnostics.len(), 1);
    let kinds: Vec<TokenKind> = tokens
        .into_iter()
        .filter(|t| !t.is_eof())
        .map(|t| t.kind)
        .collect();
    assert_eq!(kinds, vec![TokenKind::Kw(Keyword::Fn), ident("main")]);
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
    // been consumed as part of an escape.
    let (tokens, _) = lex_src("\"a\\\nb\"");
    let kinds: Vec<TokenKind> = tokens
        .into_iter()
        .filter(|t| !t.is_eof())
        .map(|t| t.kind)
        .collect();
    assert_eq!(kinds, vec![ident("b")], "the line break was consumed");
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

// --- Property tests --------------------------------------------------------

proptest! {
    /// `lex` never panics, for any input at all (SPEC §11 item 3).
    #[test]
    fn lex_never_panics(text in any::<String>()) {
        let (_tokens, _diagnostics) = lex_src(&text);
    }

    /// Structured fragments exercise the interaction of strings, comments,
    /// braces and numbers, checking the invariants on the result — including
    /// on the spans attached to diagnostics, and that the downstream renderers
    /// never panic either.
    #[test]
    fn lex_invariants_hold(
        fragments in prop::collection::vec(
            prop::sample::select(vec![
                "fn", "main", "{", "}", "(", ")", "[", "]", "#{", "\"", "{{", "}}", "\\",
                "\\n", "/*", "*/", "//", "1", "1.", "1..5", "0x", "0xFF", "_", "??", "?.", "..=",
                "=>", "->", "\n", "\r\n", " ", "a", "é", "!", "x => {\n", "\"a{b}c\"",
                "#{\n1: 2\n}", ";", ", ",
                // Added in M1.1: BOM, bare CR, astral char, the rejected `-->`
                // pseudo-operator, and a backslash followed by a real newline.
                "\u{feff}", "\r", "😀", "-->", "\\\n",
            ]),
            0..24,
        )
    ) {
        let text: String = fragments.concat();
        let (sources, file, tokens, diagnostics) = lex_with_sources(&text);
        // Spans are offsets into `SourceFile::text()`, which is the input with
        // the leading BOM already stripped by `SourceMap::add`.
        let file_text = file.text();

        prop_assert!(tokens.last().is_some_and(|t| t.is_eof()), "must end with Eof");

        let mut previous_end = 0u32;
        for token in &tokens {
            let (start, end) = (token.span.start, token.span.end);
            prop_assert!(start <= end);
            prop_assert!((end as usize) <= file_text.len());
            prop_assert!(start >= previous_end, "overlapping spans");
            prop_assert!(file_text.is_char_boundary(start as usize));
            prop_assert!(file_text.is_char_boundary(end as usize));
            // ADR 0007: a `Newline` is a zero-width point, so it cannot cover
            // bytes that belong to another token.
            if token.is_newline() {
                prop_assert_eq!(start, end, "Newline must be zero-width");
            }
            previous_end = end;
        }

        // Diagnostic spans obey the same bounds and char-boundary rules, and
        // must not start after they end.
        for diagnostic in &diagnostics {
            let (start, end) = (diagnostic.primary.start, diagnostic.primary.end);
            prop_assert!(start <= end, "diagnostic span is backwards");
            prop_assert!((end as usize) <= file_text.len(), "diagnostic span out of bounds");
            prop_assert!(file_text.is_char_boundary(start as usize));
            prop_assert!(file_text.is_char_boundary(end as usize));

            // A message is one line by contract: no raw control characters.
            prop_assert!(
                !diagnostic.message.contains('\n') && !diagnostic.message.contains('\r'),
                "diagnostic message must not contain a line break: {:?}",
                diagnostic.message
            );
        }

        // The renderers must not panic on any lexer output.
        let _ = crate::dump::dump_tokens(&file, &tokens);
        let mut compact = String::new();
        for diagnostic in &diagnostics {
            compact.push_str(&diagnostic.render_compact(&sources));
        }
    }
}
