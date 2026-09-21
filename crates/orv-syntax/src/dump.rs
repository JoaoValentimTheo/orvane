//! Stable textual rendering of tokens, used by `orv tokens` (ADR 0008).
//!
//! The format is one line per token:
//!
//! ```text
//! LINE:COL Kind(payload)
//! ```
//!
//! `LINE` is 1-based and `COL` is 1-based in **characters**, matching the `.err`
//! convention from ADR 0001. The kind spelling is fixed so golden tests can rely
//! on it; changing it is a deliberate, visible edit.

use std::fmt::Write as _;

use crate::source::SourceFile;

use crate::lexer::{StrPart, Token, TokenKind};

/// Renders a whole token stream.
pub fn dump_tokens(file: &SourceFile, tokens: &[Token]) -> String {
    let mut out = String::new();
    for token in tokens {
        let (line, col) = file.line_col(token.span.start);
        let _ = writeln!(out, "{line}:{col} {}", kind_text(&token.kind));
    }
    out
}

/// The stable one-line spelling of a token kind, e.g. `Kw(fn)` or `Int(42)`.
pub fn kind_text(kind: &TokenKind) -> String {
    match kind {
        TokenKind::Int(value) => format!("Int({value})"),
        TokenKind::Float(value) => format!("Float({})", float_text(*value)),
        TokenKind::Str(parts) => {
            let rendered: Vec<String> = parts
                .iter()
                .map(|part| match part {
                    StrPart::Lit(text) => format!("Lit({})", quote(text)),
                    StrPart::Expr { src, .. } => format!("Expr({})", quote(src)),
                })
                .collect();
            format!("Str[{}]", rendered.join(", "))
        }
        TokenKind::Ident(name) => format!("Ident({name})"),
        TokenKind::Kw(kw) => format!("Kw({})", kw.as_str()),
        TokenKind::Newline => "Newline".to_owned(),
        TokenKind::Eof => "Eof".to_owned(),
        other => punct_text(other).to_owned(),
    }
}

/// Renders a float so `1.0` stays `1.0` instead of `1`.
fn float_text(value: f64) -> String {
    let mut text = format!("{value}");
    if !text.contains(['.', 'e', 'E']) {
        text.push_str(".0");
    }
    text
}

/// A double-quoted string with a minimal escape set, so output stays one line
/// and unambiguous.
fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

/// The fixed spelling for operator and punctuation tokens.
const fn punct_text(kind: &TokenKind) -> &'static str {
    match kind {
        TokenKind::LParen => "LParen",
        TokenKind::RParen => "RParen",
        TokenKind::LBracket => "LBracket",
        TokenKind::RBracket => "RBracket",
        TokenKind::LBrace => "LBrace",
        TokenKind::RBrace => "RBrace",
        TokenKind::HashLBrace => "HashLBrace",
        TokenKind::Comma => "Comma",
        TokenKind::Colon => "Colon",
        TokenKind::Semicolon => "Semicolon",
        TokenKind::Dot => "Dot",
        TokenKind::DotDot => "DotDot",
        TokenKind::DotDotEq => "DotDotEq",
        TokenKind::Arrow => "Arrow",
        TokenKind::FatArrow => "FatArrow",
        TokenKind::Question => "Question",
        TokenKind::QuestionDot => "QuestionDot",
        TokenKind::QuestionQuestion => "QuestionQuestion",
        TokenKind::Eq => "Eq",
        TokenKind::EqEq => "EqEq",
        TokenKind::NotEq => "NotEq",
        TokenKind::Lt => "Lt",
        TokenKind::Le => "Le",
        TokenKind::Gt => "Gt",
        TokenKind::Ge => "Ge",
        TokenKind::Plus => "Plus",
        TokenKind::Minus => "Minus",
        TokenKind::Star => "Star",
        TokenKind::Slash => "Slash",
        TokenKind::Percent => "Percent",
        TokenKind::PlusEq => "PlusEq",
        TokenKind::MinusEq => "MinusEq",
        TokenKind::StarEq => "StarEq",
        TokenKind::SlashEq => "SlashEq",
        // Handled by `kind_text` before reaching here.
        TokenKind::Int(_)
        | TokenKind::Float(_)
        | TokenKind::Str(_)
        | TokenKind::Ident(_)
        | TokenKind::Kw(_)
        | TokenKind::Newline
        | TokenKind::Eof => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::source::SourceMap;

    fn dump(text: &str) -> String {
        let mut sources = SourceMap::new();
        let id = sources.add("t.orv", text);
        let file = sources.file(id).cloned();
        let file = match file {
            Some(file) => file,
            None => panic!("file missing"),
        };
        let (tokens, _) = lex(&file);
        dump_tokens(&file, &tokens)
    }

    #[test]
    fn renders_kinds_with_payloads() {
        assert_eq!(dump("fn"), "1:1 Kw(fn)\n1:3 Eof\n");
        assert_eq!(dump("main"), "1:1 Ident(main)\n1:5 Eof\n");
        assert_eq!(dump("42"), "1:1 Int(42)\n1:3 Eof\n");
        assert_eq!(dump("1.5"), "1:1 Float(1.5)\n1:4 Eof\n");
    }

    #[test]
    fn renders_integral_floats_with_a_decimal_point() {
        assert_eq!(dump("1.0"), "1:1 Float(1.0)\n1:4 Eof\n");
    }

    #[test]
    fn column_is_one_based_in_chars() {
        // `á` is invalid (non-ASCII identifier), so no token is produced, but
        // Eof still reports column 2 for byte offset 2: columns are in chars.
        assert_eq!(dump("á"), "1:2 Eof\n");
        // A multi-byte char inside a string shifts the *byte* span but the
        // reported column stays a char count.
        assert_eq!(dump("\"á\""), "1:1 Str[Lit(\"á\")]\n1:4 Eof\n");
    }

    #[test]
    fn renders_newlines_and_spans_across_lines() {
        // A `Newline` is a zero-width span at the position *after* the line
        // break, so it reports the first column of the next line. That is the
        // position the parser resumes from, and it keeps spans disjoint.
        assert_eq!(
            dump("a\nb"),
            "1:1 Ident(a)\n2:1 Newline\n2:1 Ident(b)\n2:2 Eof\n"
        );
    }

    #[test]
    fn renders_strings_with_parts() {
        assert_eq!(
            dump("\"Olá {name}\""),
            "1:1 Str[Lit(\"Olá \"), Expr(\"name\")]\n1:13 Eof\n"
        );
    }

    #[test]
    fn renders_escapes_in_literal_parts() {
        assert_eq!(dump(r#""a\nb""#), "1:1 Str[Lit(\"a\\nb\")]\n1:7 Eof\n");
    }

    #[test]
    fn renders_punctuation() {
        assert_eq!(dump("#{"), "1:1 HashLBrace\n1:3 Eof\n");
        assert_eq!(dump("..="), "1:1 DotDotEq\n1:4 Eof\n");
        assert_eq!(dump("??"), "1:1 QuestionQuestion\n1:3 Eof\n");
    }
}
