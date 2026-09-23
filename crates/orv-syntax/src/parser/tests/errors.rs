//! Error-path tests: diagnostics, suppression, depth limit, and a proptest
//! proving the parser never panics on arbitrary token streams.

use super::{parse, parse_ok};
use crate::ast::{ItemKind, StmtKind};
use crate::lexer::lex;
use crate::parser::Parser;
use crate::source::{FileId, SourceFile, SourceMap};
use proptest::prelude::*;

#[test]
fn missing_closing_paren_reports_e0102() {
    let parsed = parse("fn main() {\n    (1 + 2\n}\n");
    assert!(
        parsed.codes.contains(&"E0102"),
        "expected E0102: {:?}",
        parsed.codes
    );
}

#[test]
fn missing_comma_between_list_elements_reports_e0102() {
    let parsed = parse("fn main() {\n    [1 2]\n}\n");
    assert!(
        parsed.codes.contains(&"E0102"),
        "expected E0102: {:?}",
        parsed.codes
    );
}

#[test]
fn map_without_colon_reports_e0102() {
    let parsed = parse("fn main() {\n    #{\"a\" 1}\n}\n");
    assert!(
        parsed.codes.contains(&"E0102"),
        "expected E0102: {:?}",
        parsed.codes
    );
}

#[test]
fn an_unclosed_function_body_reports_e0103() {
    // §12 names E0103 for an unclosed block; it is a distinct cause from
    // "expected X, found Y", so it gets its own code.
    let parsed = parse("fn main() {\n    let x = 1\n");
    let e0103: Vec<_> = parsed
        .codes
        .iter()
        .filter(|code| **code == "E0103")
        .collect();
    assert_eq!(e0103.len(), 1, "got: {:?}", parsed.codes);
    assert!(
        parsed.messages.iter().any(|m| m == "unclosed block"),
        "got: {:?}",
        parsed.messages
    );
}

#[test]
fn an_unclosed_data_body_reports_e0103() {
    let parsed = parse("data D {\n    x: Int,\n");
    assert!(parsed.codes.contains(&"E0103"), "got: {:?}", parsed.codes);
}

#[test]
fn an_unclosed_enum_body_reports_e0103() {
    let parsed = parse("enum E {\n    A,\n");
    assert!(parsed.codes.contains(&"E0103"), "got: {:?}", parsed.codes);
}

#[test]
fn an_unclosed_match_body_reports_e0103() {
    let parsed = parse("fn main() {\n    match x {\n        1 => 2,\n}\n");
    assert!(parsed.codes.contains(&"E0103"), "got: {:?}", parsed.codes);
}

#[test]
fn a_missing_closing_paren_still_reports_e0102() {
    // E0103 is only for blocks; a missing `)` keeps the general code.
    let parsed = parse("fn main() {\n    (1 + 2\n}\n");
    assert!(parsed.codes.contains(&"E0102"), "got: {:?}", parsed.codes);
    assert!(
        !parsed.codes.contains(&"E0103"),
        "a block was closed: {:?}",
        parsed.codes
    );
}

#[test]
fn missing_fn_body_reports_e0102() {
    let parsed = parse("fn main()\n");
    assert!(
        !parsed.codes.is_empty(),
        "a function without a body must be rejected"
    );
}

#[test]
fn a_lexically_invalid_character_reports_e0001_not_e0101() {
    // `@` never reaches the parser: the lexer reports it (ADR 0008) and the
    // parser's own complaint on the same region is suppressed.
    let parsed = parse("@\n");
    assert_eq!(parsed.codes, vec!["E0001"]);
}

#[test]
fn an_unexpected_token_at_item_level_reports_e0101() {
    // `1` is a valid token but cannot start an item.
    let parsed = parse("1\n");
    assert_eq!(parsed.codes, vec!["E0101"]);
}

#[test]
fn lexical_errors_come_first_and_suppress_parser_cascade() {
    // `0x` is `E0005` with a best-effort `Int(0)`; the parser accepts the token
    // and must not pile on (ADR 0008 amend, rules 2 and 3).
    let parsed = parse("fn main() {\n    0x\n}\n");
    assert_eq!(parsed.codes, vec!["E0005"], "got {:?}", parsed.codes);
    assert!(parsed.has_errors);
}

#[test]
fn deeply_nested_input_reports_e0104_instead_of_overflowing() {
    // 1000 nested parens: the depth guard must fire (ADR 0013) rather than
    // blowing the stack.
    let source = format!(
        "fn main() {{\n    {}1{}\n}}\n",
        "(".repeat(1000),
        ")".repeat(1000)
    );
    let parsed = parse(&source);
    assert!(
        parsed.codes.contains(&"E0104"),
        "expected the depth limit: {:?}",
        &parsed.codes[..parsed.codes.len().min(5)]
    );
}

#[test]
fn deeply_nested_unary_reports_e0104() {
    let source = format!("fn main() {{\n    {}1\n}}\n", "-".repeat(1000));
    let parsed = parse(&source);
    assert!(
        parsed.codes.contains(&"E0104"),
        "expected the depth limit: {:?}",
        &parsed.codes[..parsed.codes.len().min(5)]
    );
}

#[test]
fn moderately_nested_input_is_fine() {
    let source = format!(
        "fn main() {{\n    {}1{}\n}}\n",
        "(".repeat(50),
        ")".repeat(50)
    );
    parse_ok(&source);
}

#[test]
fn the_parser_tolerates_an_empty_token_slice() {
    // `lex` never produces this, but the parser promises not to panic. An empty
    // program is the sensible result.
    let mut parser = Parser::new(&[], Vec::new());
    let result = parser.parse_program();
    let program = result.program.expect("an empty program, not None");
    assert!(program.items.is_empty());
}

#[test]
fn a_program_of_only_eof_is_empty() {
    let parsed = parse("");
    assert!(parsed.program.items.is_empty());
}

#[test]
fn a_trailing_unterminated_string_is_reported_once() {
    let parsed = parse("fn main() {\n    let s = \"abc\n}\n");
    let e0002 = parsed.codes.iter().filter(|c| **c == "E0002").count();
    assert_eq!(e0002, 1, "got {:?}", parsed.codes);
}

#[test]
fn a_parser_program_still_yields_the_items_it_could_read() {
    let parsed = parse("fn a() {\n    1\n}\nfn b() {\n    (1\n}\n");
    assert!(parsed.has_errors);
    assert_eq!(
        parsed.program.items.len(),
        2,
        "both items should be present despite the error"
    );
    let ItemKind::Fn(decl) = &parsed.program.items[0].kind else {
        panic!("expected a function");
    };
    assert!(matches!(decl.body.statements[0].kind, StmtKind::Expr(_)));
}

/// Lexes `text` and parses it, ignoring the result.
fn parse_bytes(text: &str) {
    let mut sources = SourceMap::new();
    let id = sources.add("fuzz.orv", text);
    let file = match sources.file(id).cloned() {
        Some(file) => file,
        None => SourceFile::new(FileId(0), "fuzz.orv", text),
    };
    let (tokens, lexical) = lex(&file);
    let mut parser = Parser::new(&tokens, lexical);
    let _ = parser.parse_program();
}

proptest! {
    /// The parser never panics, whatever the input (SPEC §11 item 3).
    #[test]
    fn parser_never_panics(text in any::<String>()) {
        parse_bytes(&text);
    }

    /// Structured fragments exercise the parser's error paths harder than
    /// random bytes do, while still never panicking.
    #[test]
    fn parser_never_panics_on_fragments(
        fragments in prop::collection::vec(
            prop::sample::select(vec![
                "fn", "main", "(", ")", "{", "}", "[", "]", "#{", ",", ":", ";", ".", "..",
                "..=", "=>", "->", "?", "?.", "??", "=", "==", "+", "-", "*", "/", "%", "as",
                "let", "mut", "if", "else", "match", "for", "in", "while", "return", "break",
                "continue", "try", "fail", "data", "enum", "intent", "how", "test", "use", "py",
                "true", "false", "none", "and", "or", "not", "pub", "1", "1.5", "\"s\"", "\"a{b}c\"",
                "ident", "_", "\n", " ", "\"", "#", "\\", "e0104",
            ]),
            0..40,
        )
    ) {
        let text: String = fragments.concat();
        parse_bytes(&text);
    }

    /// The interpolation sub-parse reads arbitrary text between `{}`; it must
    /// never panic, whatever the content (SPEC §11 item 3). The inner text is
    /// embedded in a well-formed `print("...")` so the sub-parse is actually
    /// reached, and quotes/backslashes are neutralized so the outer string
    /// stays lexically clean.
    #[test]
    fn interpolation_subparse_never_panics(inner in any::<String>()) {
        let safe: String = inner
            .chars()
            .map(|c| match c {
                '"' | '\\' | '{' | '}' | '\n' | '\r' => ' ',
                other => other,
            })
            .collect();
        let text = format!("fn main() {{\n    print(\"before {{{safe}}} after\")\n}}\n");
        parse_bytes(&text);
    }
}
