//! Shared diagnostic output for the CLI.
//!
//! Two formats coexist (ADR 0006):
//!
//! * the readable `ariadne` rendering, for a human at a terminal;
//! * the stable single-line `CODE:line:col: message` form from SPEC §8.1, which
//!   is what golden `.err` files contain and what CI sees.
//!
//! Neither function panics on a broken pipe: they return [`io::Result`] and the
//! caller decides what a failed write means.

use std::io::{self, Write};

use orv_syntax::{Diagnostic, SourceMap};

/// Writes the compact `CODE:line:col: message` line to stderr.
///
/// # Errors
/// Propagates I/O errors from stderr.
pub fn print_compact(sources: &SourceMap, diagnostic: &Diagnostic) -> io::Result<()> {
    let stderr = io::stderr();
    let mut lock = stderr.lock();
    lock.write_all(diagnostic.render_compact(sources).as_bytes())?;
    lock.write_all(b"\n")?;
    lock.flush()
}

/// Writes the readable rendering to stderr, degrading to the compact form if the
/// rendering itself fails.
///
/// # Errors
/// Propagates I/O errors from stderr after the fallback also failed.
pub fn render_or_compact(sources: &SourceMap, diagnostic: &Diagnostic) -> io::Result<()> {
    if sources.render(diagnostic).is_ok() {
        return Ok(());
    }
    print_compact(sources, diagnostic)
}
