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

    /// Whether `self` and `other` overlap, treating a zero-width span as a
    /// single point.
    ///
    /// Spans in different files never intersect. A point span counts as inside
    /// `other` when it lies in `[other.start, other.end]`, so a `Newline` point
    /// at the end of an erroneous token is still suppressed (ADR 0008 amenda).
    pub const fn intersects(self, other: Span) -> bool {
        // Compare the raw ids: `FileId`'s derived `PartialEq` is not `const`.
        if self.file.0 != other.file.0 {
            return false;
        }
        self.start <= other.end && other.start <= self.end
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
    fn intersects_detects_overlap() {
        let a = Span::new(file(), 10, 20);
        assert!(a.intersects(Span::new(file(), 15, 25)), "partial overlap");
        assert!(a.intersects(Span::new(file(), 12, 14)), "contained");
        assert!(a.intersects(a), "identical");
        assert!(a.intersects(Span::new(file(), 0, 10)), "touching at start");
        assert!(a.intersects(Span::new(file(), 20, 30)), "touching at end");
        assert!(!a.intersects(Span::new(file(), 0, 9)), "before");
        assert!(!a.intersects(Span::new(file(), 21, 30)), "after");
    }

    #[test]
    fn intersects_treats_a_point_span_as_inside() {
        // A `Newline` point at an erroneous token's end must be suppressed.
        let token = Span::new(file(), 10, 20);
        assert!(Span::point(file(), 10).intersects(token));
        assert!(Span::point(file(), 15).intersects(token));
        assert!(Span::point(file(), 20).intersects(token));
        assert!(!Span::point(file(), 21).intersects(token));
        assert!(!Span::point(file(), 9).intersects(token));
    }

    #[test]
    fn intersects_requires_the_same_file() {
        let a = Span::new(FileId(0), 0, 10);
        let b = Span::new(FileId(1), 0, 10);
        assert!(!a.intersects(b));
        assert!(!Span::point(FileId(1), 5).intersects(a));
    }

    #[test]
    fn intersects_two_point_spans() {
        assert!(Span::point(file(), 7).intersects(Span::point(file(), 7)));
        assert!(!Span::point(file(), 7).intersects(Span::point(file(), 8)));
    }

    #[test]
    #[should_panic(expected = "different files")]
    fn to_rejects_mixed_files() {
        let a = Span::new(FileId(0), 0, 1);
        let b = Span::new(FileId(1), 0, 1);
        let _ = a.to(b);
    }
}
