//! Aggregating diagnostics from the pipeline stages (SPEC §8, ADR 0008 amend).
//!
//! The lexer runs first and may report errors while still producing
//! best-effort tokens (ADR 0008 A). Later stages therefore parse a stream that
//! covers the whole file, and they *will* find syntax problems in the
//! placeholder tokens. Those follow-up diagnostics are noise: the lexical error
//! already explained the region.
//!
//! [`Diagnostics::new`] takes the lexical diagnostics, and
//! [`Diagnostics::extend_suppressed`] drops any later diagnostic whose primary
//! span intersects a lexical one. The supported hook is the aggregation point
//! the parser will call; no parser exists yet in M2's foundation step.

use crate::diagnostic::Diagnostic;
use crate::span::Span;

/// Diagnostics from every stage that has run, in emission order.
///
/// Lexical diagnostics always come first (ADR 0008 amend, rule 2).
#[derive(Clone, Debug, Default)]
pub struct Diagnostics {
    /// Diagnostics that survived suppression, in emission order.
    diagnostics: Vec<Diagnostic>,
    /// Primary spans of the lexical diagnostics, used as suppression keys.
    lexical_spans: Vec<Span>,
}

impl Diagnostics {
    /// Starts an aggregation with the lexical diagnostics.
    ///
    /// Every diagnostic passed here is kept and becomes a suppression key for
    /// later stages.
    pub fn new(lexical: impl IntoIterator<Item = Diagnostic>) -> Self {
        let lexical: Vec<Diagnostic> = lexical.into_iter().collect();
        let lexical_spans = lexical.iter().map(|d| d.primary).collect();
        Self {
            diagnostics: lexical,
            lexical_spans,
        }
    }

    /// Adds diagnostics from a later stage, dropping those suppressed by a
    /// lexical error.
    ///
    /// A diagnostic is suppressed when its primary span
    /// [`intersects`](Span::intersects) a lexical primary span. Suppression is
    /// silent and intentional: the parser is expected to emit the diagnostic
    /// and let this filter decide, so the parser itself needs no knowledge of
    /// the lexer's output.
    ///
    /// Diagnostics that are themselves lexical must go through
    /// [`Self::new`], not here.
    pub fn extend_suppressed(&mut self, later: impl IntoIterator<Item = Diagnostic>) {
        for diagnostic in later {
            if !self.is_suppressed(diagnostic.primary) {
                self.diagnostics.push(diagnostic);
            }
        }
    }

    /// Whether a span is suppressed by a lexical diagnostic.
    pub fn is_suppressed(&self, span: Span) -> bool {
        self.lexical_spans
            .iter()
            .any(|lexical| span.intersects(*lexical))
    }

    /// Whether any diagnostic of severity [`Severity::Error`] is present.
    ///
    /// The gate for "never advance to sema/run" (ADR 0008 amend, rule 5); it
    /// ignores warnings so a warning-only program still runs.
    ///
    /// [`Severity::Error`]: crate::diagnostic::Severity::Error
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == crate::diagnostic::Severity::Error)
    }

    /// The retained diagnostics, in emission order.
    pub fn as_slice(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// The retained diagnostics, consuming the aggregation.
    pub fn into_vec(self) -> Vec<Diagnostic> {
        self.diagnostics
    }

    /// Number of retained diagnostics.
    pub fn len(&self) -> usize {
        self.diagnostics.len()
    }

    /// Whether no diagnostic survived.
    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::FileId;

    fn span(start: u32, end: u32) -> Span {
        Span::new(FileId(0), start, end)
    }

    #[test]
    fn lexical_diagnostics_are_never_suppressed() {
        // Their own spans intersect themselves; `new` keeps them regardless.
        let lexical = Diagnostic::error("E0005", "invalid numeric literal", span(8, 10));
        let all = Diagnostics::new([lexical.clone()]);
        assert_eq!(all.as_slice(), &[lexical]);
    }

    #[test]
    fn a_parser_diagnostic_on_the_same_span_is_suppressed() {
        // (a) the rule that matters: the lexical one survives, the parser's does not.
        let lexical = Diagnostic::error("E0005", "invalid numeric literal", span(8, 10));
        let parser = Diagnostic::error("E0101", "unexpected token", span(8, 10));

        let mut all = Diagnostics::new([lexical.clone()]);
        all.extend_suppressed([parser]);

        assert_eq!(all.len(), 1, "only the lexical diagnostic survives");
        assert_eq!(all.as_slice(), &[lexical]);
    }

    #[test]
    fn overlapping_and_containing_spans_are_suppressed() {
        let lexical = Diagnostic::error("E0005", "invalid numeric literal", span(8, 12));

        let mut all = Diagnostics::new([lexical.clone()]);
        all.extend_suppressed([
            Diagnostic::error("E0101", "partial overlap", span(10, 20)),
            Diagnostic::error("E0101", "contained", span(9, 11)),
            Diagnostic::error("E0101", "touching the start", span(0, 8)),
            Diagnostic::error("E0101", "touching the end", span(12, 20)),
        ]);

        assert_eq!(all.len(), 1, "every touching span intersects: {all:?}");
        assert_eq!(all.as_slice(), &[lexical]);
    }

    #[test]
    fn unrelated_parser_diagnostics_survive() {
        let lexical = Diagnostic::error("E0005", "invalid numeric literal", span(8, 10));

        let mut all = Diagnostics::new([lexical.clone()]);
        let before = Diagnostic::error("E0101", "before", span(0, 7));
        let after = Diagnostic::error("E0101", "after", span(11, 20));
        all.extend_suppressed([before.clone(), after.clone()]);

        assert_eq!(all.len(), 3, "nothing here intersects: {all:?}");
        assert_eq!(all.as_slice(), &[lexical, before, after]);
    }

    #[test]
    fn suppression_uses_the_primary_span_only() {
        // A parser diagnostic *about* a lexical region, but anchored elsewhere,
        // survives: the contract is about where the error points.
        let lexical = Diagnostic::error("E0005", "invalid numeric literal", span(8, 10));
        let elsewhere = Diagnostic::error("E0101", "unexpected token", span(30, 31))
            .with_label(span(8, 10), "while parsing this literal");

        let mut all = Diagnostics::new([lexical.clone()]);
        all.extend_suppressed([elsewhere.clone()]);

        assert_eq!(all.as_slice(), &[lexical, elsewhere]);
    }

    #[test]
    fn suppression_does_not_cross_files() {
        let lexical = Diagnostic::error(
            "E0005",
            "invalid numeric literal",
            Span::new(FileId(0), 8, 10),
        );
        let other_file =
            Diagnostic::error("E0101", "unexpected token", Span::new(FileId(1), 8, 10));

        let mut all = Diagnostics::new([lexical.clone()]);
        all.extend_suppressed([other_file.clone()]);

        assert_eq!(all.as_slice(), &[lexical, other_file]);
    }

    #[test]
    fn order_is_lexical_first_then_surviving_later_stages() {
        let lexical = Diagnostic::error("E0005", "invalid numeric literal", span(8, 10));
        let parser = Diagnostic::error("E0101", "unrelated", span(20, 21));

        let mut all = Diagnostics::new([lexical.clone()]);
        all.extend_suppressed([parser.clone()]);

        assert_eq!(all.as_slice(), &[lexical, parser]);
    }

    #[test]
    fn has_errors_ignores_warnings() {
        let warning = Diagnostic::warning("W0001", "unused", span(0, 1));
        let all = Diagnostics::new([warning]);
        assert!(
            !all.has_errors(),
            "a warning-only program must still be allowed to run"
        );

        let mut all = Diagnostics::new([Diagnostic::warning("W0001", "unused", span(0, 1))]);
        all.extend_suppressed([Diagnostic::error("E0101", "boom", span(5, 6))]);
        assert!(all.has_errors());
    }

    #[test]
    fn empty_input_yields_an_empty_aggregation() {
        let all = Diagnostics::new([]);
        assert!(all.is_empty());
        assert!(!all.has_errors());
        assert_eq!(all.len(), 0);
    }

    #[test]
    fn no_lexical_diagnostics_suppresses_nothing() {
        let mut all = Diagnostics::new([]);
        let parser = Diagnostic::error("E0101", "unexpected token", span(0, 1));
        all.extend_suppressed([parser.clone()]);
        assert_eq!(all.as_slice(), &[parser]);
    }

    #[test]
    fn into_vec_returns_the_retained_set() {
        let lexical = Diagnostic::error("E0005", "invalid numeric literal", span(8, 10));
        let mut all = Diagnostics::new([lexical.clone()]);
        all.extend_suppressed([Diagnostic::error("E0101", "suppressed", span(9, 10))]);
        assert_eq!(all.into_vec(), vec![lexical]);
    }
}
