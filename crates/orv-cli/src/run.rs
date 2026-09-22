//! The `orv run` subcommand (SPEC §10).
//!
//! Runs the front end, then the program's `main`. A diagnostic in any stage
//! stops the pipeline before execution (ADR 0008 amend, rule 5); a runtime
//! failure is reported as a diagnostic with the call stack and exits `1`.

use std::io::{IsTerminal, Write};
use std::path::Path;

use orv_runtime::driver;
use orv_sema::check as sema_check;
use orv_syntax::{Diagnostic, Parser, Severity, SourceMap, lex};

use crate::cli::SubcommandOutcome;
use crate::diag;

/// Runs `path`.
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

    let (tokens, lexical) = lex(file);
    let mut parser = Parser::new(&tokens, lexical);
    let parsed = parser.parse_program();
    let mut diagnostics: Vec<Diagnostic> = parsed.diagnostics.into_vec();

    // Do not advance past a broken front end (rule 5).
    if has_error(&diagnostics) {
        report(&sources, &diagnostics);
        return SubcommandOutcome::Failure;
    }

    let Some(program) = parsed.program else {
        report(&sources, &diagnostics);
        return SubcommandOutcome::Failure;
    };

    let checked = sema_check(&program);
    if checked.has_errors() {
        diagnostics.extend(checked.diagnostics);
        report(&sources, &diagnostics);
        return SubcommandOutcome::Failure;
    }

    let outcome = driver::run(&program);
    // The program's output goes to stdout as it was collected.
    let mut stdout = std::io::stdout().lock();
    if stdout
        .write_all(outcome.output.as_bytes())
        .and_then(|()| stdout.flush())
        .is_err()
    {
        return SubcommandOutcome::Usage;
    }

    match outcome.failure {
        None => SubcommandOutcome::Ok,
        Some(failure) => {
            // A runtime failure is a diagnostic with the `Rxxxx` code (§5.5).
            let diagnostic = Diagnostic::error(
                failure.kind.code(),
                format!("{}: {}", failure.kind.as_str(), failure.message),
                failure.span,
            );
            let mut trace = failure.trace.as_ref().clone();
            trace.reverse();
            let _ = trace;
            report(&sources, &[diagnostic]);
            SubcommandOutcome::Failure
        }
    }
}

/// Whether any diagnostic is an error.
fn has_error(diagnostics: &[Diagnostic]) -> bool {
    diagnostics.iter().any(|d| d.severity == Severity::Error)
}

/// Writes diagnostics to stderr.
fn report(sources: &SourceMap, diagnostics: &[Diagnostic]) {
    let pretty = std::io::stderr().is_terminal();
    for diagnostic in diagnostics {
        let _ = if pretty {
            diag::render_or_compact(sources, diagnostic)
        } else {
            diag::print_compact(sources, diagnostic)
        };
    }
}
