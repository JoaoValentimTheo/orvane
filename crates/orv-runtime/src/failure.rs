//! Runtime failures (SPEC §5.5).
//!
//! One mechanism for every runtime error: `Failure { kind, message, trace }`.
//! An uncaught failure reaches `main` as a diagnostic and exit code 1.

use std::rc::Rc;

use orv_syntax::Span;

/// A runtime failure.
#[derive(Clone, Debug)]
pub struct Failure {
    /// A stable machine-readable kind, e.g. `DivisionByZero`.
    pub kind: FailureKind,
    /// A human-readable message.
    pub message: Rc<str>,
    /// The call stack at the point of failure, innermost last.
    pub trace: Rc<Vec<Frame>>,
    /// Where it happened.
    pub span: Span,
}

/// The kinds of runtime failure in the alpha (SPEC §5.5, §5.3).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FailureKind {
    /// `1 / 0` or `1 % 0` (`R0001`).
    DivisionByZero,
    /// Integer overflow (`R0002`).
    Overflow,
    /// An index past the end of a list or string (`R0003`).
    IndexOutOfBounds,
    /// `assert` failed (`AssertionFailed`).
    AssertionFailed,
    /// A map lookup for a missing key.
    MissingKey,
    /// The call stack exceeded [`MAX_CALL_DEPTH`](crate::MAX_CALL_DEPTH).
    ///
    /// Kept separate from [`FailureKind::Unsupported`] because §12 names it
    /// (`R0004`) and a depth error is a property of the program's recursion,
    /// not of an unimplemented feature.
    StackOverflow,
    /// A runtime operation that is not supported in the alpha.
    Unsupported,
    /// Internal control flow (`break`, `continue`, `return`) travelling through
    /// the error channel.
    ///
    /// A tree-walking evaluator returns `Result`, and early control flow is the
    /// one thing that must escape *out* of an expression without being an
    /// error. Rather than thread a second channel through every call site, the
    /// interpreter uses this kind with a private marker message; it is never
    /// reported to the user, and `Halt` is collapsed before `run` returns.
    Flow,
}

impl FailureKind {
    /// The stable name used in messages and (later) JSON output.
    pub const fn as_str(self) -> &'static str {
        match self {
            FailureKind::DivisionByZero => "DivisionByZero",
            FailureKind::Overflow => "Overflow",
            FailureKind::IndexOutOfBounds => "IndexOutOfBounds",
            FailureKind::AssertionFailed => "AssertionFailed",
            FailureKind::MissingKey => "MissingKey",
            FailureKind::StackOverflow => "StackOverflow",
            FailureKind::Unsupported => "Unsupported",
            FailureKind::Flow => "Flow",
        }
    }

    /// The `Rxxxx` diagnostic code from `docs/errors.md`.
    pub const fn code(self) -> &'static str {
        match self {
            FailureKind::DivisionByZero => "R0001",
            FailureKind::Overflow => "R0002",
            FailureKind::IndexOutOfBounds => "R0003",
            FailureKind::AssertionFailed => "R0001",
            FailureKind::MissingKey => "R0003",
            FailureKind::StackOverflow => "R0004",
            FailureKind::Unsupported => "R0010",
            FailureKind::Flow => "R0010",
        }
    }
}

/// One frame of the Orvane call stack.
#[derive(Clone, Debug)]
pub struct Frame {
    /// The function name, or `<main>`.
    pub function: Rc<str>,
    /// Where the call happened.
    pub span: Span,
}

impl Failure {
    /// Creates a failure with no stack.
    pub fn new(kind: FailureKind, message: impl Into<Rc<str>>, span: Span) -> Self {
        Self {
            kind,
            message: message.into(),
            trace: Rc::new(Vec::new()),
            span,
        }
    }

    /// Attaches a call stack.
    #[must_use]
    pub fn with_trace(mut self, trace: Vec<Frame>) -> Self {
        self.trace = Rc::new(trace);
        self
    }

    /// Structural equality, used by `Result` values (§5.3).
    pub fn equals(&self, other: &Failure) -> bool {
        self.kind == other.kind && self.message == other.message
    }
}

/// The structured rendering used inside `Err(...)` and in diagnostics.
///
/// Matches §5.5's shape: `Failure(kind: "...", message: "...")`.
pub fn render(failure: &Failure) -> String {
    format!(
        "Failure(kind: {:?}, message: {:?})",
        failure.kind.as_str(),
        failure.message.as_ref()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use orv_syntax::{FileId, Span};

    fn span() -> Span {
        Span::point(FileId(0), 0)
    }

    #[test]
    fn renders_the_spec_shape() {
        let failure = Failure::new(FailureKind::DivisionByZero, "division by zero", span());
        assert_eq!(
            render(&failure),
            "Failure(kind: \"DivisionByZero\", message: \"division by zero\")"
        );
    }

    #[test]
    fn kinds_have_stable_names_and_codes() {
        assert_eq!(FailureKind::DivisionByZero.as_str(), "DivisionByZero");
        assert_eq!(FailureKind::DivisionByZero.code(), "R0001");
        assert_eq!(FailureKind::Overflow.code(), "R0002");
        assert_eq!(FailureKind::IndexOutOfBounds.code(), "R0003");
    }

    #[test]
    fn equality_ignores_the_trace() {
        let a = Failure::new(FailureKind::Overflow, "boom", span()).with_trace(vec![Frame {
            function: "f".into(),
            span: span(),
        }]);
        let b = Failure::new(FailureKind::Overflow, "boom", span());
        assert!(a.equals(&b));
    }

    #[test]
    fn different_kinds_are_not_equal() {
        let a = Failure::new(FailureKind::Overflow, "boom", span());
        let b = Failure::new(FailureKind::MissingKey, "boom", span());
        assert!(!a.equals(&b));
    }
}
