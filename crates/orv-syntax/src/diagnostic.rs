//! User-facing diagnostics (SPEC §8).
//!
//! Every user-visible error in Orvane is a [`Diagnostic`]; `String`s never
//! escape as errors (SPEC §0.2 rule 5).

use crate::source::SourceMap;
use crate::span::Span;

/// Diagnostic severity. v0.1 only ever emits `Error`, but warnings are part of
/// the contract from day one.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Severity {
    #[default]
    Error,
    Warning,
}

impl Severity {
    /// Whether this severity blocks the pipeline (ADR 0008 amend, rule 5).
    pub const fn is_error(self) -> bool {
        matches!(self, Severity::Error)
    }

    /// Stable lowercase name, used in golden `.err` output.
    pub const fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

/// A secondary location attached to a diagnostic, with its own message.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Label {
    pub span: Span,
    pub message: String,
}

impl Label {
    pub fn new(span: Span, message: impl Into<String>) -> Self {
        Self {
            span,
            message: message.into(),
        }
    }
}

/// A compiler or runtime diagnostic.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Diagnostic {
    /// Stable code such as `E0001`; see `docs/errors.md`.
    pub code: &'static str,
    pub severity: Severity,
    pub message: String,
    pub primary: Span,
    pub labels: Vec<Label>,
    pub help: Option<String>,
}

impl Diagnostic {
    /// Creates an error diagnostic with a single primary span.
    pub fn error(code: &'static str, message: impl Into<String>, primary: Span) -> Self {
        Self {
            code,
            severity: Severity::Error,
            message: message.into(),
            primary,
            labels: Vec::new(),
            help: None,
        }
    }

    /// Creates a warning diagnostic with a single primary span.
    pub fn warning(code: &'static str, message: impl Into<String>, primary: Span) -> Self {
        Self {
            severity: Severity::Warning,
            ..Self::error(code, message, primary)
        }
    }

    /// Adds a secondary label.
    #[must_use]
    pub fn with_label(mut self, span: Span, message: impl Into<String>) -> Self {
        self.labels.push(Label::new(span, message));
        self
    }

    /// Adds a help sentence.
    #[must_use]
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// The file the primary span points into.
    pub fn file(&self) -> crate::source::FileId {
        self.primary.file
    }

    /// Renders the stable, single-line test form required by SPEC §8.1:
    /// `CODE:linha:coluna: mensagem`.
    ///
    /// This is what golden `.err` files are compared against, so it must stay
    /// byte-for-byte stable.
    pub fn render_compact(&self, sources: &SourceMap) -> String {
        let (line, column) = match sources.file(self.primary.file) {
            Some(file) => file.line_col(self.primary.start),
            None => (0, 0),
        };
        format!("{}:{}:{}: {}", self.code, line, column, self.message)
    }

    /// Renders one compact line per diagnostic.
    pub fn render_compact_all(sources: &SourceMap, diagnostics: &[Diagnostic]) -> String {
        let mut out = String::new();
        for diag in diagnostics {
            out.push_str(&diag.render_compact(sources));
            out.push('\n');
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(file: u32, start: u32, end: u32) -> Span {
        Span::new(crate::source::FileId(file), start, end)
    }

    #[test]
    fn error_has_expected_defaults() {
        let d = Diagnostic::error("E0001", "invalid character", span(0, 0, 1));
        assert_eq!(d.code, "E0001");
        assert_eq!(d.severity, Severity::Error);
        assert_eq!(d.message, "invalid character");
        assert!(d.labels.is_empty());
        assert!(d.help.is_none());
    }

    #[test]
    fn builders_accumulate_labels_and_help() {
        let d = Diagnostic::error("E0301", "mismatch", span(0, 0, 1))
            .with_label(span(0, 4, 5), "expected Int here")
            .with_help("annotate the binding");
        assert_eq!(d.labels.len(), 1);
        assert_eq!(d.labels[0].message, "expected Int here");
        assert_eq!(d.help.as_deref(), Some("annotate the binding"));
    }

    #[test]
    fn warning_keeps_code_and_message() {
        let d = Diagnostic::warning("E0001", "unused", span(0, 0, 1));
        assert_eq!(d.severity, Severity::Warning);
        assert_eq!(d.severity.as_str(), "warning");
    }

    #[test]
    fn render_compact_uses_one_based_line_and_column() {
        let mut sm = SourceMap::new();
        let id = sm.add("x.orv", "ab\nwrong\n");
        let d = Diagnostic::error("E0101", "unexpected token", Span::new(id, 3, 8));
        assert_eq!(d.render_compact(&sm), "E0101:2:1: unexpected token");
    }

    #[test]
    fn render_compact_all_emits_one_line_per_diagnostic() {
        let mut sm = SourceMap::new();
        let id = sm.add("x.orv", "abc");
        let ds = vec![
            Diagnostic::error("E0001", "one", Span::new(id, 0, 1)),
            Diagnostic::error("E0002", "two", Span::new(id, 2, 3)),
        ];
        assert_eq!(
            Diagnostic::render_compact_all(&sm, &ds),
            "E0001:1:1: one\nE0002:1:3: two\n"
        );
    }

    #[test]
    fn render_compact_all_of_nothing_is_empty() {
        let sm = SourceMap::new();
        assert_eq!(Diagnostic::render_compact_all(&sm, &[]), "");
    }

    #[test]
    fn render_compact_tolerates_unknown_file() {
        let sm = SourceMap::new();
        let d = Diagnostic::error("E0001", "boom", span(3, 0, 1));
        assert_eq!(d.render_compact(&sm), "E0001:0:0: boom");
    }
}
