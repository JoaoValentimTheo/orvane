//! Unit tests for the lexer (SPEC §5.1, §11 item 1).
//!
//! Split by theme in M1.2 (§3.4 keeps modules small). The tests themselves are
//! unchanged; each submodule owns one area and the shared helpers live here.
//!
//! Coverage overall: one case per token kind, every error code
//! (`E0001`–`E0006`) and each rule from the milestone — CRLF, BOM, `1..5`,
//! `a ?? b`, `#{`, block comments containing a newline, and nested
//! interpolation.

use crate::diagnostic::Diagnostic;
use crate::lexer::{StrPart, Token, TokenKind, lex};
use crate::source::{FileId, SourceFile, SourceMap};

/// Re-exported so submodules reach them through `use super::*`.
///
/// `Keyword` is used by the keyword submodule; `proptest` brings the macro and
/// `prop` module into scope for the property tests.
pub(super) use crate::lexer::Keyword;
pub(super) use proptest::prelude::*;

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

mod comments;
mod invariants;
mod keywords;
mod newlines;
mod numbers;
mod operators;
mod properties;
mod strings;
