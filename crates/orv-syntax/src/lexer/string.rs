//! String literal scanning, including interpolation (SPEC §5.1, ADR 0007).
//!
//! A string produces `Str(Vec<StrPart>)` where each part is either literal text
//! (escapes already resolved) or an interpolated expression. Interpolation
//! supports nested strings and braces, so `"{f("a")}"` is one expression part.
//!
//! Errors:
//!
//! * `E0002` — unterminated string (including a raw newline inside it).
//! * `E0004` — unknown escape.
//! * `E0006` — malformed interpolation (`}` without `{`, or empty `{}`).

use crate::diagnostic::Diagnostic;
use crate::source::FileId;
use crate::span::Span;

use super::cursor::Cursor;
use super::token::StrPart;

/// The outcome of scanning a string literal.
pub(super) enum StringOutcome {
    /// A well-formed string, with at least one part (possibly empty text).
    Parts(Vec<StrPart>),
    /// The literal is malformed; diagnostics were already reported.
    Invalid,
}

/// Scans a string literal. The caller guarantees `cursor` is at the opening `"`.
pub(super) fn scan_string(
    cursor: &mut Cursor<'_>,
    file: FileId,
    diagnostics: &mut Vec<Diagnostic>,
) -> StringOutcome {
    let open = cursor.pos();
    cursor.advance(1); // opening quote

    let mut parts: Vec<StrPart> = Vec::new();
    let mut lit = String::new();
    let mut ok = true;

    loop {
        let start = cursor.pos();
        match cursor.peek() {
            None => {
                // Ran off the end of the file without a closing quote.
                diagnostics.push(
                    Diagnostic::error(
                        "E0002",
                        "unterminated string literal",
                        span(file, open, start),
                    )
                    .with_help("add a closing `\"`"),
                );
                return StringOutcome::Invalid;
            }
            Some(b'\n') => {
                // A raw newline is not allowed inside a string (SPEC §5.1).
                diagnostics.push(
                    Diagnostic::error(
                        "E0002",
                        "unterminated string literal (newline before closing quote)",
                        span(file, open, start),
                    )
                    .with_label(span(file, start, start + 1), "line break here")
                    .with_help("use `\\n` for a newline inside a string"),
                );
                return StringOutcome::Invalid;
            }
            Some(b'"') => {
                cursor.advance(1);
                break;
            }
            Some(b'\\') => {
                if !scan_escape(cursor, file, &mut lit, diagnostics) {
                    ok = false;
                }
            }
            Some(b'{') => {
                if cursor.peek_byte(1) == Some(b'{') {
                    lit.push('{');
                    cursor.advance(2);
                } else {
                    flush(&mut parts, &mut lit);
                    match scan_interpolation(cursor, file, diagnostics) {
                        Some(part) => parts.push(part),
                        None => ok = false,
                    }
                }
            }
            Some(b'}') => {
                if cursor.peek_byte(1) == Some(b'}') {
                    lit.push('}');
                    cursor.advance(2);
                } else {
                    diagnostics.push(
                        Diagnostic::error(
                            "E0006",
                            "invalid interpolation: `}` without a matching `{`",
                            span(file, start, start + 1),
                        )
                        .with_help("use }} to write a literal `}`"),
                    );
                    cursor.advance(1);
                    ok = false;
                }
            }
            Some(_) => match cursor.bump_char() {
                Some(ch) => lit.push(ch),
                None => break,
            },
        }
    }

    flush(&mut parts, &mut lit);
    if !ok {
        StringOutcome::Invalid
    } else {
        StringOutcome::Parts(parts)
    }
}

/// Resolves one escape sequence into `lit`.
///
/// Returns `false` (and reports `E0004`) for an unknown escape.
fn scan_escape(
    cursor: &mut Cursor<'_>,
    file: FileId,
    lit: &mut String,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let backslash = cursor.pos();
    cursor.advance(1); // the `\`
    let Some(ch) = cursor.bump_char() else {
        // A trailing backslash means the string never terminated.
        diagnostics.push(
            Diagnostic::error(
                "E0002",
                "unterminated string literal",
                span(file, backslash, cursor.pos()),
            )
            .with_help("add a closing `\"`"),
        );
        return false;
    };

    match ch {
        'n' => lit.push('\n'),
        't' => lit.push('\t'),
        'r' => lit.push('\r'),
        '\\' => lit.push('\\'),
        '"' => lit.push('"'),
        '{' => lit.push('{'),
        '}' => lit.push('}'),
        _ => {
            diagnostics.push(
                Diagnostic::error(
                    "E0004",
                    format!("invalid escape sequence `\\{ch}`"),
                    span(file, backslash, cursor.pos()),
                )
                .with_help("valid escapes are \\n \\t \\r \\\\ \\\" \\{ \\}"),
            );
            return false;
        }
    }
    true
}

/// Scans a `{expr}` interpolation, returning the expression part.
///
/// The expression source is captured verbatim (spans point into the file) and
/// nested strings and braces are skipped so `"{f("a")}"` works.
fn scan_interpolation(
    cursor: &mut Cursor<'_>,
    file: FileId,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<StrPart> {
    let open = cursor.pos();
    cursor.advance(1); // the `{`
    let content_start = cursor.pos();

    let mut depth = 1usize;
    loop {
        match cursor.peek() {
            None | Some(b'\n') => {
                diagnostics.push(
                    Diagnostic::error(
                        "E0002",
                        "unterminated string literal (unclosed interpolation)",
                        span(file, open, cursor.pos()),
                    )
                    .with_help("add a closing `}` and `\"`"),
                );
                return None;
            }
            Some(b'\\') => {
                // Outside a nested string, `\` escapes the next character, so
                // `\"` is a literal quote and must not open a nested string.
                // This is what makes `"{f(\"a\")}"` work.
                cursor.advance(1);
                cursor.bump_char();
            }
            Some(b'"') => {
                // Skip a nested string so its braces do not affect depth.
                if !skip_nested_string(cursor, file, diagnostics) {
                    return None;
                }
            }
            Some(b'{') => {
                depth += 1;
                cursor.advance(1);
            }
            Some(b'}') => {
                depth -= 1;
                if depth == 0 {
                    let content_end = cursor.pos();
                    cursor.advance(1); // the `}`
                    let src = cursor
                        .text()
                        .get(content_start..content_end)
                        .unwrap_or("")
                        .to_owned();
                    if src.trim().is_empty() {
                        diagnostics.push(
                            Diagnostic::error(
                                "E0006",
                                "invalid interpolation: empty expression",
                                span(file, open, cursor.pos()),
                            )
                            .with_help("write an expression between `{` and `}`"),
                        );
                        return None;
                    }
                    return Some(StrPart::Expr {
                        src,
                        span: span(file, content_start, content_end),
                    });
                }
                cursor.advance(1);
            }
            Some(_) => cursor.advance(1),
        }
    }
}

/// Skips a nested `"..."` string inside an interpolation.
///
/// Only nesting matters here; the nested string is re-lexed later by the parser,
/// so its escapes are consumed without validation beyond finding the end.
fn skip_nested_string(
    cursor: &mut Cursor<'_>,
    file: FileId,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let open = cursor.pos();
    cursor.advance(1);
    loop {
        match cursor.peek() {
            None => {
                diagnostics.push(
                    Diagnostic::error(
                        "E0002",
                        "unterminated string literal",
                        span(file, open, cursor.pos()),
                    )
                    .with_help("add a closing `\"`"),
                );
                return false;
            }
            Some(b'\n') => {
                diagnostics.push(
                    Diagnostic::error(
                        "E0002",
                        "unterminated string literal (newline before closing quote)",
                        span(file, open, cursor.pos()),
                    )
                    .with_help("use `\\n` for a newline inside a string"),
                );
                return false;
            }
            Some(b'\\') => {
                cursor.advance(1);
                cursor.bump_char();
            }
            Some(b'"') => {
                cursor.advance(1);
                return true;
            }
            Some(_) => cursor.advance(1),
        }
    }
}

fn flush(parts: &mut Vec<StrPart>, lit: &mut String) {
    if !lit.is_empty() {
        parts.push(StrPart::Lit(std::mem::take(lit)));
    }
}

fn span(file: FileId, start: usize, end: usize) -> Span {
    Span::new(file, start as u32, end as u32)
}
