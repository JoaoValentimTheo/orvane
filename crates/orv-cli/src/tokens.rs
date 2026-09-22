//! The `orv tokens` debug subcommand (SPEC §12 M1, ADR 0006).
//!
//! Not counted against the 8-subcommand budget and scheduled for removal in
//! M12; it exists so golden tests can pin the lexer's output.
//!
//! Nothing here writes through `print!`/`eprintln!`: those panic on a broken
//! pipe (e.g. `orv tokens x.orv | head`), and SPEC §0.2 rule 5 forbids a panic
//! for a user-visible condition. All output goes through [`std::io::Write`] and
//! a failed stdout write becomes [`SubcommandOutcome::Usage`].

use std::io::{IsTerminal, Write};

use orv_syntax::{SourceMap, dump, lex};

use crate::cli::SubcommandOutcome;
use crate::diag;

/// Lexes `path` and writes one line per token to stdout.
///
/// Tokens go to stdout; diagnostics go to stderr and set the exit code to `1`
/// (SPEC §10). Tokens are written even when diagnostics exist, so a golden case
/// can assert both streams.
pub fn run(path: &std::path::Path) -> SubcommandOutcome {
    let mut sources = SourceMap::new();
    let id = match sources.load(path) {
        Ok(id) => id,
        Err(err) => {
            return report(&format!("cannot read {}: {err}", path.display()));
        }
    };

    let Some(file) = sources.file(id) else {
        return report(&format!(
            "internal source map failure for {}",
            path.display()
        ));
    };

    let (tokens, diagnostics) = lex(file);

    let mut stdout = std::io::stdout().lock();
    // A broken pipe or a full disk is an environment problem, not a program
    // error, so it maps to exit code 2 (SPEC §10) and never panics.
    if stdout
        .write_all(dump::dump_tokens(file, &tokens).as_bytes())
        .and_then(|()| stdout.flush())
        .is_err()
    {
        return SubcommandOutcome::Usage;
    }

    if diagnostics.is_empty() {
        return SubcommandOutcome::Ok;
    }

    // On a terminal, show the readable rendering; otherwise emit the stable
    // `CODE:line:col: message` lines that golden `.err` files compare (ADR 0004).
    let pretty = std::io::stderr().is_terminal();
    for diagnostic in &diagnostics {
        let _ = if pretty {
            diag::render_or_compact(&sources, diagnostic)
        } else {
            diag::print_compact(&sources, diagnostic)
        };
    }
    SubcommandOutcome::Failure
}

/// Writes a one-line environment error to stderr, best effort.
fn report(message: &str) -> SubcommandOutcome {
    let _ = writeln!(std::io::stderr(), "error: {message}");
    SubcommandOutcome::Usage
}
