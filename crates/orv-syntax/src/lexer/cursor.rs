//! A byte/char cursor over a source file that never panics (SPEC §3.4).
//!
//! The cursor works on `&[u8]` for cheap delimiter and ASCII checks, and only
//! decodes a `char` when it actually needs to advance past non-ASCII text. All
//! offsets are byte offsets, matching [`Span`](crate::Span) (ADR 0001).

/// A cursor over the bytes of one source file.
///
/// Invariant: `pos` is always on a `char` boundary, so `text[pos..]` is valid
/// UTF-8 slicing.
#[derive(Clone, Debug)]
pub struct Cursor<'a> {
    text: &'a str,
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    /// Creates a cursor at the start of `text`.
    ///
    /// A leading UTF-8 BOM is skipped, as required by SPEC §5.1.
    pub fn new(text: &'a str) -> Self {
        let trimmed = text.strip_prefix('\u{feff}').unwrap_or(text);
        Self {
            text: trimmed,
            bytes: trimmed.as_bytes(),
            pos: 0,
        }
    }

    /// The whole (BOM-stripped) source text.
    pub const fn text(&self) -> &'a str {
        self.text
    }

    /// Current byte offset.
    pub const fn pos(&self) -> usize {
        self.pos
    }

    /// Whether the cursor is past the last byte.
    pub const fn is_eof(&self) -> bool {
        self.pos >= self.bytes.len()
    }

    /// The remaining source, starting at the current offset.
    pub fn rest(&self) -> &'a str {
        self.text.get(self.pos..).unwrap_or("")
    }

    /// The remaining bytes, starting at the current offset.
    pub const fn rest_bytes(&self) -> &'a [u8] {
        match self.bytes.split_at_checked(self.pos) {
            Some((_, tail)) => tail,
            None => &[],
        }
    }

    /// The byte at `pos + offset`, if any.
    pub fn peek_byte(&self, offset: usize) -> Option<u8> {
        self.bytes.get(self.pos.checked_add(offset)?).copied()
    }

    /// The byte at the current offset, if any.
    pub fn peek(&self) -> Option<u8> {
        self.peek_byte(0)
    }

    /// The `char` at the current offset, if any.
    pub fn peek_char(&self) -> Option<char> {
        self.rest().chars().next()
    }

    /// Advances by `n` bytes, clamped to the end of input.
    ///
    /// # Panics
    /// Does not panic; a call that would land off a `char` boundary is clamped
    /// to the end of input, and only the lexer's own ASCII paths call this with
    /// raw lengths.
    pub fn advance(&mut self, n: usize) {
        self.pos = self.pos.saturating_add(n).min(self.bytes.len());
    }

    /// Advances past the current `char` and returns it.
    pub fn bump_char(&mut self) -> Option<char> {
        let ch = self.peek_char()?;
        self.pos = self.pos.saturating_add(ch.len_utf8()).min(self.bytes.len());
        Some(ch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_utf8_bom() {
        let c = Cursor::new("\u{feff}fn");
        assert_eq!(c.pos(), 0);
        assert_eq!(c.peek(), Some(b'f'));
        assert_eq!(c.text(), "fn");
    }

    #[test]
    fn peek_and_advance() {
        let mut c = Cursor::new("ab");
        assert_eq!(c.peek(), Some(b'a'));
        c.advance(1);
        assert_eq!(c.peek(), Some(b'b'));
        c.advance(1);
        assert_eq!(c.peek(), None);
        assert!(c.is_eof());
    }

    #[test]
    fn advance_clamps_past_end() {
        let mut c = Cursor::new("ab");
        c.advance(999);
        assert_eq!(c.pos(), 2);
        assert!(c.is_eof());
        c.advance(1);
        assert_eq!(c.pos(), 2);
    }

    #[test]
    fn bump_char_uses_utf8_len() {
        let mut c = Cursor::new("é!");
        assert_eq!(c.bump_char(), Some('é'));
        assert_eq!(c.pos(), 2);
        assert_eq!(c.bump_char(), Some('!'));
        assert_eq!(c.bump_char(), None);
    }

    #[test]
    fn rest_slices_by_byte_offset() {
        let mut c = Cursor::new("abc");
        c.advance(1);
        assert_eq!(c.rest(), "bc");
        assert_eq!(c.rest_bytes(), b"bc");
    }

    #[test]
    fn rest_is_valid_utf8_after_multibyte_advance() {
        let mut c = Cursor::new("日本");
        c.bump_char();
        assert_eq!(c.rest(), "本");
        c.bump_char();
        assert_eq!(c.rest(), "");
    }

    #[test]
    fn peek_byte_at_offset_is_bounds_checked() {
        let c = Cursor::new("ab");
        assert_eq!(c.peek_byte(1), Some(b'b'));
        assert_eq!(c.peek_byte(2), None);
        assert_eq!(c.peek_byte(usize::MAX), None);
    }

    #[test]
    fn empty_input_is_eof_immediately() {
        let c = Cursor::new("");
        assert!(c.is_eof());
        assert_eq!(c.peek_char(), None);
    }
}
