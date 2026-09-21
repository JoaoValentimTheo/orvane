//! Shared diagnostic output for the CLI.
//!
//! Two formats coexist (ADR 0004):
//!
//! * the readable `ariadne` rendering, for a human at a terminal;
//! * the stable single-line `CODE:line:col: message` form from SPEC §8.1, which
//!   is what golden `.err` files contain and what CI sees.
//!
//! Nothing here panics on a broken pipe: a failed write falls back to the
//! compact form, and a failed compact write is simply dropped.

use orv_syntax::{Diagnostic, SourceMap};

/// Prints the compact `CODE:line:col: message` line to stderr.
pub fn print_compact(sources: &SourceMap, diagnostic: &Diagnostic) {
    eprintln!("{}", diagnostic.render_compact(sources));
}

/// Prints the readable rendering to stderr, degrading to the compact form if the
/// terminal writer fails.
pub fn render_or_compact(sources: &SourceMap, diagnostic: &Diagnostic) {
    if sources.render(diagnostic).is_err() {
        print_compact(sources, diagnostic);
    }
}
