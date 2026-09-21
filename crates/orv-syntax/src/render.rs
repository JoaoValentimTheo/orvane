//! Renders [`Diagnostic`]s to a terminal with `ariadne`.
//!
//! The pretty, multi-line rendering is for humans. Machine-readable output goes
//! through [`Diagnostic::render_compact`](crate::Diagnostic::render_compact),
//! which is what golden `.err` files contain (SPEC §8.1).

use std::io::{self, Write};

use ariadne::{CharSet, Config, IndexType, Label, Report, ReportKind, Source};

use crate::diagnostic::{Diagnostic, Severity};
use crate::source::{FileId, SourceMap};
use crate::span::Span;

/// Renders `diag` to `writer` using `ariadne`.
///
/// Colors are disabled and the character set is ASCII so output is stable
/// across terminals and platforms (SPEC §3.4: determinism).
///
/// # Errors
/// Propagates I/O errors from `writer`.
pub fn render_to<W: Write>(sources: &SourceMap, diag: &Diagnostic, writer: W) -> io::Result<()> {
    let (id, source) = match sources.file(diag.primary.file) {
        Some(file) => (file.name.clone(), Source::from(file.text().to_owned())),
        None => (
            format!("<file {}>", diag.primary.file.0),
            Source::from(String::new()),
        ),
    };

    let kind = match diag.severity {
        Severity::Error => ReportKind::Error,
        Severity::Warning => ReportKind::Warning,
    };

    let config = Config::default()
        .with_color(false)
        .with_char_set(CharSet::Ascii)
        .with_index_type(IndexType::Byte);

    let primary = Label::new((id.clone(), range(diag.primary))).with_message(diag.message.clone());

    let mut builder = Report::build(kind, (id.clone(), range(diag.primary)))
        .with_config(config)
        .with_code(diag.code)
        .with_message(diag.message.clone())
        .with_label(primary);

    for label in &diag.labels {
        // Secondary labels live in the primary's file in M0; files beyond the
        // primary are rendered as a range in a synthetic source name.
        let label_id = file_name_of(sources, label.span.file).unwrap_or_else(|| id.clone());
        builder = builder.with_label(
            Label::new((label_id, range(label.span))).with_message(label.message.clone()),
        );
    }

    if let Some(help) = &diag.help {
        builder = builder.with_help(help.clone());
    }

    builder.finish().write((id, source), writer)
}

/// Renders `diag` to stderr.
///
/// # Errors
/// Propagates I/O errors from stderr.
pub fn render(sources: &SourceMap, diag: &Diagnostic) -> io::Result<()> {
    render_to(sources, diag, io::stderr())
}

/// Renders `diag` into a `String`, for tests and `--json`-free debugging.
pub fn render_to_string(sources: &SourceMap, diag: &Diagnostic) -> String {
    let mut buf = Vec::new();
    // Writing into a `Vec` cannot fail; a failure here is a bug, and M0 has no
    // `expect()`, so fall back to the compact form.
    if render_to(sources, diag, &mut buf).is_err() {
        return diag.render_compact(sources);
    }
    String::from_utf8(buf).unwrap_or_else(|_| diag.render_compact(sources))
}

fn range(span: Span) -> std::ops::Range<usize> {
    let start = span.start as usize;
    let end = span.end as usize;
    start..end
}

fn file_name_of(sources: &SourceMap, id: FileId) -> Option<String> {
    sources.file(id).map(|f| f.name.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sources() -> (SourceMap, FileId) {
        let mut sm = SourceMap::new();
        let id = sm.add("sample.orv", "fn main() {\n    let = 3\n}\n");
        (sm, id)
    }

    #[test]
    fn render_to_string_contains_code_and_message() {
        let (sm, id) = sources();
        let diag = Diagnostic::error("E0101", "unexpected token", Span::new(id, 16, 19));
        let out = render_to_string(&sm, &diag);
        assert!(out.contains("E0101"), "got: {out}");
        assert!(out.contains("unexpected token"), "got: {out}");
        assert!(out.contains("sample.orv"), "got: {out}");
    }

    #[test]
    fn render_to_string_includes_labels_and_help() {
        let (sm, id) = sources();
        let diag = Diagnostic::error("E0301", "type mismatch", Span::new(id, 16, 19))
            .with_label(Span::new(id, 4, 8), "expected Int")
            .with_help("add an annotation");
        let out = render_to_string(&sm, &diag);
        assert!(out.contains("expected Int"), "got: {out}");
        assert!(out.contains("add an annotation"), "got: {out}");
    }

    #[test]
    fn render_is_deterministic() {
        let (sm, id) = sources();
        let diag = Diagnostic::error("E0001", "invalid character", Span::new(id, 0, 2));
        assert_eq!(
            render_to_string(&sm, &diag),
            render_to_string(&sm, &diag),
            "rendering must not depend on hash order"
        );
    }

    #[test]
    fn render_warning_uses_warning_kind() {
        let (sm, id) = sources();
        let diag = Diagnostic::warning("E0001", "careful", Span::new(id, 0, 2));
        let out = render_to_string(&sm, &diag);
        assert!(out.to_lowercase().contains("warning"), "got: {out}");
    }

    #[test]
    fn render_to_writer_reports_io_errors() {
        struct Failing;
        impl Write for Failing {
            fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
                Err(io::Error::other("nope"))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let (sm, id) = sources();
        let diag = Diagnostic::error("E0001", "boom", Span::new(id, 0, 1));
        assert!(render_to(&sm, &diag, Failing).is_err());
    }
}
