//! Parser test helpers.

use crate::ast::{Expr, Program};
use crate::lexer::lex;
use crate::parser::Parser;
use crate::source::{FileId, SourceFile, SourceMap};

mod errors;
mod exprs;
mod items;
mod stmts;

/// The outcome of lexing and parsing `text`.
pub(super) struct Parsed {
    pub program: Program,
    pub codes: Vec<&'static str>,
    pub messages: Vec<String>,
    /// `(start, end)` byte ranges of each diagnostic's primary span, in the
    /// same order as `codes`.
    pub spans: Vec<(u32, u32)>,
    pub has_errors: bool,
}

/// Lexes and parses `text`, keeping every diagnostic for assertions.
pub(super) fn parse(text: &str) -> Parsed {
    let mut sources = SourceMap::new();
    let id = sources.add("test.orv", text);
    let file = match sources.file(id).cloned() {
        Some(file) => file,
        None => SourceFile::new(FileId(0), "test.orv", text),
    };
    let (tokens, lexical) = lex(&file);
    let mut parser = Parser::new(&tokens, lexical);
    let result = parser.parse_program();
    let has_errors = result.has_errors();
    let diagnostics = result.diagnostics.as_slice();
    let codes = diagnostics.iter().map(|d| d.code).collect();
    let messages = diagnostics.iter().map(|d| d.message.clone()).collect();
    let spans = diagnostics
        .iter()
        .map(|d| (d.primary.start, d.primary.end))
        .collect();
    Parsed {
        program: result.program.unwrap_or(Program {
            items: Vec::new(),
            span: crate::Span::point(file.id, 0),
        }),
        codes,
        messages,
        spans,
        has_errors,
    }
}

/// Parses `text` expecting no diagnostics.
pub(super) fn parse_ok(text: &str) -> Program {
    let parsed = parse(text);
    assert!(
        parsed.codes.is_empty(),
        "unexpected diagnostics for {text:?}: {:?}",
        parsed.messages
    );
    parsed.program
}

/// The first item of a program parsed cleanly.
pub(super) fn first_item(text: &str) -> crate::ast::Item {
    let program = parse_ok(text);
    match program.items.into_iter().next() {
        Some(item) => item,
        None => panic!("expected an item in {text:?}"),
    }
}

/// Parses a single expression statement inside `fn main`, and returns it.
pub(super) fn expr_of(text: &str) -> Expr {
    let source = format!("fn main() {{\n    {text}\n}}\n");
    let program = parse_ok(&source);
    let Some(crate::ast::Item {
        kind: crate::ast::ItemKind::Fn(decl),
        ..
    }) = program.items.first()
    else {
        panic!("expected a function in {source:?}");
    };
    match decl.body.statements.first() {
        Some(crate::ast::Stmt {
            kind: crate::ast::StmtKind::Expr(expr),
            ..
        }) => expr.clone(),
        other => panic!("expected an expression statement, got {other:?}"),
    }
}

/// The statements of `fn main` in `text`.
pub(super) fn main_statements(text: &str) -> Vec<crate::ast::Stmt> {
    let source = format!("fn main() {{\n{text}\n}}\n");
    let program = parse_ok(&source);
    let Some(crate::ast::Item {
        kind: crate::ast::ItemKind::Fn(decl),
        ..
    }) = program.items.first()
    else {
        panic!("expected a function in {source:?}");
    };
    decl.body.statements.clone()
}
