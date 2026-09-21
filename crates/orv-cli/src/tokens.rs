//! The `orv tokens` debug subcommand (SPEC §12 M1, ADR 0008).
//!
//! Not counted against the 8-subcommand budget and scheduled for removal in
//! M12; it exists so golden tests can pin the lexer's output.

use std::io::IsTerminal;
use std::path::Path;

use orv_syntax::{SourceMap, dump, lex};

use crate::cli::SubcommandOutcome;
use crate::diag;

/// Lexes `path` and prints one line per token to stdout.
///
/// Tokens go to stdout; diagnostics go to stderr and set the exit code to `1`
/// (SPEC §10). Tokens are printed even when diagnostics exist, so a golden case
/// can assert both streams.
pub fn run(path: &Path) -> SubcommandOutcome {
    let mut sources = SourceMap::new();
    let id = match sources.load(path) {
        Ok(id) => id,
        Err(err) => {
            eprintln!("error: cannot read {}: {err}", path.display());
            return SubcommandOutcome::Usage;
        }
    };

    let Some(file) = sources.file(id) else {
        eprintln!("error: internal source map failure for {}", path.display());
        return SubcommandOutcome::Usage;
    };

    let (tokens, diagnostics) = lex(file);
    print!("{}", dump::dump_tokens(file, &tokens));

    if diagnostics.is_empty() {
        return SubcommandOutcome::Ok;
    }

    // On a terminal, show the readable rendering; otherwise emit the stable
    // `CODE:line:col: message` lines that golden `.err` files compare (ADR 0004).
    let pretty = std::io::stderr().is_terminal();
    for diagnostic in &diagnostics {
        if pretty {
            diag::render_or_compact(&sources, diagnostic);
        } else {
            diag::print_compact(&sources, diagnostic);
        }
    }
    SubcommandOutcome::Failure
}
