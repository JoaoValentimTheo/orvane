//! The `orv ast` debug subcommand (SPEC §10, ADR 0014).
//!
//! Not counted against the 8-subcommand budget; like `orv tokens` it is internal
//! tooling.

use std::io::{IsTerminal, Write};
use std::path::Path;

use orv_syntax::{Parser, SourceMap, ast_dump, lex};

use crate::cli::SubcommandOutcome;
use crate::diag;

/// Lexes and parses `path`, then writes the AST dump to stdout.
///
/// Tokens go through the same pipeline as `orv run`: a lexical error does not
/// stop the parse (ADR 0008), and parser diagnostics are suppressed when they
/// intersect a lexical one.
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
    let result = parser.parse_program();
    let has_errors = result.has_errors();
    let dump = match &result.program {
        Some(program) => ast_dump::dump_program(program),
        None => String::new(),
    };

    let mut stdout = std::io::stdout().lock();
    if stdout
        .write_all(dump.as_bytes())
        .and_then(|()| stdout.flush())
        .is_err()
    {
        return SubcommandOutcome::Usage;
    }

    if !has_errors {
        return SubcommandOutcome::Ok;
    }
    let pretty = std::io::stderr().is_terminal();
    for diagnostic in result.diagnostics.as_slice() {
        let _ = if pretty {
            diag::render_or_compact(&sources, diagnostic)
        } else {
            diag::print_compact(&sources, diagnostic)
        };
    }
    SubcommandOutcome::Failure
}
