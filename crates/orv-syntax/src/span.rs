//! Byte spans into a source file.

use crate::source::FileId;

/// A half-open byte range `[start, end)` inside `file`.
///
/// Spans are attached to every AST node (SPEC §3.4). They are byte offsets
/// (never char offsets) so slicing the source is always valid at UTF-8
/// boundaries produced by the lexer.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Span {
    pub file: FileId,
    pub start: u32,
    pub end: u32,
}

impl Span {
    /// Creates a span. `start`/`end` are byte offsets; callers are responsible
    /// for `start <= end`.
    pub const fn new(file: FileId, start: u32, end: u32) -> Self {
        Self { file, start, end }
    }

    /// A zero-width span at `offset`.
    pub const fn point(file: FileId, offset: u32) -> Self {
        Self {
            file,
            start: offset,
            end: offset,
        }
    }

    /// Smallest span covering both `self` and `other`.
    ///
    /// # Panics
    /// Panics if the spans belong to different files; that is a compiler bug,
    /// not a user error.
    pub fn to(self, other: Span) -> Span {
        assert_eq!(
            self.file, other.file,
            "cannot join spans from different files"
        );
        Span {
            file: self.file,
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    /// Length in bytes.
    pub const fn len(self) -> u32 {
        self.end.saturating_sub(self.start)
    }

    /// Whether the span is empty.
    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file() -> FileId {
        FileId(0)
    }

    #[test]
    fn len_and_is_empty() {
        let s = Span::new(file(), 3, 7);
        assert_eq!(s.len(), 4);
        assert!(!s.is_empty());
        assert!(Span::point(file(), 3).is_empty());
    }

    #[test]
    fn to_covers_both_spans() {
        let a = Span::new(file(), 3, 5);
        let b = Span::new(file(), 9, 12);
        assert_eq!(a.to(b), Span::new(file(), 3, 12));
        assert_eq!(b.to(a), Span::new(file(), 3, 12));
    }

    #[test]
    fn to_is_idempotent_on_self() {
        let a = Span::new(file(), 4, 6);
        assert_eq!(a.to(a), a);
    }

    #[test]
    fn point_span_has_zero_len() {
        let p = Span::point(file(), 42);
        assert_eq!(p.start, 42);
        assert_eq!(p.end, 42);
        assert_eq!(p.len(), 0);
    }

    #[test]
    #[should_panic(expected = "different files")]
    fn to_rejects_mixed_files() {
        let a = Span::new(FileId(0), 0, 1);
        let b = Span::new(FileId(1), 0, 1);
        let _ = a.to(b);
    }
}
