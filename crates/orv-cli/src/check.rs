//! The `orv check` subcommand (SPEC §10).
//!
//! Lexes, parses and type-checks a file without executing it. Diagnostics go to
//! stderr in the stable compact form (§8.1) when stderr is not a terminal, and
//! the exit code is `1` when anything failed (SPEC §10).

use std::io::{IsTerminal, Write};
use std::path::Path;

use orv_sema::check as sema_check;
use orv_syntax::{Parser, SourceMap, lex};

use crate::cli::SubcommandOutcome;
use crate::diag;

/// Runs the front end over `path`.
pub fn run(path: &Path) -> SubcommandOutcome {
    let mut sources = SourceMap::new();
    let id = match sources.load(path) {
        Ok(id) => id,
        Err(err) => {
            let _ = writeln!(
                std::io::stderr(),
                "error: cannot read {}: {err}",
                path.display()
            );
            return SubcommandOutcome::Usage;
        }
    };

    let Some(file) = sources.file(id) else {
        let _ = writeln!(
            std::io::stderr(),
            "error: internal source map failure for {}",
            path.display()
        );
        return SubcommandOutcome::Usage;
    };

    // Stage 1: lex (diagnostics are lexical-first, ADR 0008 amend rule 2).
    let (tokens, lexical) = lex(file);
    // Stage 2: parse, suppressing parser diagnostics that intersect a lexical
    // one (rule 3), and never re-lexing a best-effort `Expr.src` (rule 4).
    let mut parser = Parser::new(&tokens, lexical);
    let parsed = parser.parse_program();

    let mut diagnostics: Vec<orv_syntax::Diagnostic> = parsed.diagnostics.into_vec();

    // Stage 3: type check, but only when stages 1 and 2 were clean — a broken
    // parse has no meaningful types to check (rule 5). The AST is still passed
    // through when there were no errors in the previous stages.
    if parsed.program.is_some() && !has_error(&diagnostics) {
        let Some(program) = parsed.program else {
            return SubcommandOutcome::Failure;
        };
        let checked = sema_check(&program);
        diagnostics.extend(checked.diagnostics);
    }

    let failed = has_error(&diagnostics);
    report(&sources, &diagnostics);

    if failed {
        SubcommandOutcome::Failure
    } else {
        SubcommandOutcome::Ok
    }
}

/// Whether any diagnostic is an error.
fn has_error(diagnostics: &[orv_syntax::Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|d| d.severity == orv_syntax::Severity::Error)
}

/// Writes every diagnostic to stderr.
fn report(sources: &SourceMap, diagnostics: &[orv_syntax::Diagnostic]) {
    let pretty = std::io::stderr().is_terminal();
    for diagnostic in diagnostics {
        let _ = if pretty {
            diag::render_or_compact(sources, diagnostic)
        } else {
            diag::print_compact(sources, diagnostic)
        };
    }
}
