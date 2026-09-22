//! Numeric literal scanning (SPEC §5.1, ADR 0007).
//!
//! Supported forms:
//!
//! * decimal `Int`: `123`, `1_000`
//! * hexadecimal `Int`: `0xFF`, `0x1_0`
//! * binary `Int`: `0b1010`, `0b1_0`
//! * `Float`: `1.5`, `1_0.5`, `2e10`, `1e-3`, `1.5e+3`
//!
//! `_` is allowed **between** digits but not leading, trailing, or doubled.
//! A `.` starts a fractional part only when followed by a digit, so `1..5` is
//! `Int(1) DotDot Int(5)` and `1.foo` is `Int(1) Dot Ident(foo)`.
//!
//! Everything malformed (`0x` with no digits, `1__0`, `1_`, an `Int` outside
//! `i64`, a float without an exponent value) produces `E0005`.

use crate::diagnostic::Diagnostic;
use crate::span::Span;

use super::cursor::Cursor;

/// The outcome of scanning a numeric literal.
pub(super) enum NumberOutcome {
    /// A well-formed integer literal.
    Int(i64),
    /// A well-formed float literal.
    Float(f64),
    /// The literal is malformed. A diagnostic was reported and the cursor
    /// consumed the offending characters; the boolean says whether the text
    /// looked like a float, so the caller can emit `Float(0.0)` instead of
    /// `Int(0)` (ADR 0008).
    Invalid { looked_like_float: bool },
}

/// Scans a numeric literal starting at a decimal or `0` digit.
///
/// The caller guarantees `cursor` is positioned at `[0-9]`.
pub(super) fn scan_number(
    cursor: &mut Cursor<'_>,
    file: crate::source::FileId,
    diagnostics: &mut Vec<Diagnostic>,
) -> NumberOutcome {
    let start = cursor.pos();
    let radix = match radix_prefix(cursor) {
        Some(radix) => radix,
        None => return scan_decimal(cursor, file, diagnostics, start),
    };

    // Skip the `0x` / `0b` prefix and read digits in that radix.
    cursor.advance(2);
    let (digits, malformed) = scan_digits(cursor, radix);
    if malformed || digits.is_empty() {
        return invalid(cursor, file, diagnostics, start, cursor.pos(), None);
    }

    let end = cursor.pos();
    // Parse the separator-free digit string, not the raw slice: `0x1_0` would
    // otherwise be rejected by `from_str_radix`.
    match i64::from_str_radix(&digits, radix) {
        Ok(value) => NumberOutcome::Int(value),
        Err(_) => invalid(
            cursor,
            file,
            diagnostics,
            start,
            end,
            Some("integer literal does not fit in i64"),
        ),
    }
}

/// Decimal (or float) literal: `[0-9]` digits, optional fraction, optional exponent.
fn scan_decimal(
    cursor: &mut Cursor<'_>,
    file: crate::source::FileId,
    diagnostics: &mut Vec<Diagnostic>,
    start: usize,
) -> NumberOutcome {
    let (int_digits, malformed) = scan_digits(cursor, 10);
    if malformed || int_digits.is_empty() {
        return invalid(cursor, file, diagnostics, start, cursor.pos(), None);
    }
    let mut is_float = false;

    // A `.` only starts a fraction when followed by a digit.
    if next_starts_fraction(cursor) {
        is_float = true;
        cursor.advance(1); // the `.`
        let (frac, malformed) = scan_digits(cursor, 10);
        if malformed || frac.is_empty() {
            return invalid(cursor, file, diagnostics, start, cursor.pos(), None);
        }
    }

    // Exponent: `e`/`E`, optional sign, then at least one digit.
    if let Some(exponent_len) = exponent_length(cursor) {
        is_float = true;
        let after_e = cursor.pos() + 1;
        cursor.advance(exponent_len);
        let sign_len = usize::from(matches!(
            cursor.text().as_bytes().get(after_e),
            Some(b'+' | b'-')
        ));
        let exp_digits_start = after_e + sign_len;
        let exp_digits = cursor
            .text()
            .get(exp_digits_start..cursor.pos())
            .unwrap_or("");
        if exp_digits.is_empty() || !exp_digits.bytes().all(|b| b.is_ascii_digit()) {
            return invalid_float(
                cursor,
                file,
                diagnostics,
                start,
                cursor.pos(),
                Some("exponent has no digits"),
            );
        }
    }

    let end = cursor.pos();
    let raw = cursor.text().get(start..end).unwrap_or("");
    let cleaned: String = raw.chars().filter(|c| *c != '_').collect();

    if is_float {
        match cleaned.parse::<f64>() {
            // `1e999` parses to infinity: the literal is out of range, not a
            // valid float, so it is `E0005` with a float placeholder (ADR 0010).
            Ok(value) if value.is_finite() => NumberOutcome::Float(value),
            Ok(_) => invalid_float(
                cursor,
                file,
                diagnostics,
                start,
                end,
                Some("float literal out of range"),
            ),
            Err(_) => invalid_float(cursor, file, diagnostics, start, end, None),
        }
    } else {
        match cleaned.parse::<i64>() {
            Ok(value) => NumberOutcome::Int(value),
            Err(_) => invalid(
                cursor,
                file,
                diagnostics,
                start,
                end,
                Some("integer literal does not fit in i64"),
            ),
        }
    }
}

/// Recognises a `0x`/`0X` or `0b`/`0B` prefix and returns the radix.
fn radix_prefix(cursor: &Cursor<'_>) -> Option<u32> {
    if cursor.peek() != Some(b'0') {
        return None;
    }
    match cursor.peek_byte(1) {
        Some(b'x' | b'X') => Some(16),
        Some(b'b' | b'B') => Some(2),
        _ => None,
    }
}

/// Consumes digits in `radix`, allowing single `_` separators between digits.
///
/// Returns the digit text (without `_`) and whether the separator placement was
/// malformed.
fn scan_digits(cursor: &mut Cursor<'_>, radix: u32) -> (String, bool) {
    let mut digits = String::new();
    let mut malformed = false;
    let mut last_was_sep = false;
    let mut saw_digit = false;

    while let Some(b) = cursor.peek() {
        if b == b'_' {
            // A separator is only valid between two digits.
            if !saw_digit || last_was_sep {
                malformed = true;
            }
            last_was_sep = true;
            cursor.advance(1);
            continue;
        }
        if is_digit_in_radix(b, radix) {
            digits.push(b as char);
            saw_digit = true;
            last_was_sep = false;
            cursor.advance(1);
            continue;
        }
        break;
    }

    // A trailing `_` is malformed.
    if last_was_sep {
        malformed = true;
    }
    (digits, malformed)
}

/// Whether a byte is a digit in the given radix.
fn is_digit_in_radix(b: u8, radix: u32) -> bool {
    match radix {
        2 => matches!(b, b'0' | b'1'),
        10 => b.is_ascii_digit(),
        16 => b.is_ascii_hexdigit(),
        _ => false,
    }
}

/// Whether the next two bytes are `.<digit>`.
fn next_starts_fraction(cursor: &Cursor<'_>) -> bool {
    cursor.peek() == Some(b'.') && cursor.peek_byte(1).is_some_and(|b| b.is_ascii_digit())
}

/// The byte length of the exponent part (`e`, optional sign, digits), or `None`
/// when there is no exponent.
///
/// Returns the full length so a malformed exponent can still be consumed for
/// error reporting.
fn exponent_length(cursor: &Cursor<'_>) -> Option<usize> {
    if !matches!(cursor.peek(), Some(b'e' | b'E')) {
        return None;
    }
    let mut len = 1;
    if matches!(cursor.peek_byte(1), Some(b'+' | b'-')) {
        len += 1;
    }
    // Consume digits, validating separators the same way as the mantissa.
    let bytes = cursor.rest_bytes();
    while let Some(b) = bytes.get(len) {
        if b.is_ascii_digit() || *b == b'_' {
            len += 1;
        } else {
            break;
        }
    }
    Some(len)
}

/// Builds an `E0005` diagnostic and returns [`NumberOutcome::Invalid`].
///
/// `looked_like_float` picks the placeholder token kind the caller emits
/// (ADR 0008).
fn invalid(
    _cursor: &Cursor<'_>,
    file: crate::source::FileId,
    diagnostics: &mut Vec<Diagnostic>,
    start: usize,
    end: usize,
    help: Option<&str>,
) -> NumberOutcome {
    invalid_as(_cursor, file, diagnostics, start, end, help, false)
}

/// Like [`invalid`], but records whether the literal had a fractional or
/// exponent part so the placeholder token is a `Float`.
fn invalid_float(
    _cursor: &Cursor<'_>,
    file: crate::source::FileId,
    diagnostics: &mut Vec<Diagnostic>,
    start: usize,
    end: usize,
    help: Option<&str>,
) -> NumberOutcome {
    invalid_as(_cursor, file, diagnostics, start, end, help, true)
}

fn invalid_as(
    _cursor: &Cursor<'_>,
    file: crate::source::FileId,
    diagnostics: &mut Vec<Diagnostic>,
    start: usize,
    end: usize,
    help: Option<&str>,
    looked_like_float: bool,
) -> NumberOutcome {
    let span = Span::new(file, start as u32, end as u32);
    let mut diagnostic = Diagnostic::error("E0005", "invalid numeric literal", span);
    if let Some(help) = help {
        diagnostic = diagnostic.with_help(help);
    } else {
        diagnostic = diagnostic
            .with_help("`_` may only appear between digits; `0x`/`0b` need at least one digit");
    }
    diagnostics.push(diagnostic);
    NumberOutcome::Invalid { looked_like_float }
}
