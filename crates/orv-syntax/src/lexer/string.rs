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
    /// The literal is malformed. Diagnostics were already reported and the
    /// cursor consumed up to the end of the literal; the payload holds the
    /// parts read before the error, so the caller can still emit a `Str` token
    /// (ADR 0008).
    Invalid(Vec<StrPart>),
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
                flush(&mut parts, &mut lit);
                return StringOutcome::Invalid(parts);
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
                flush(&mut parts, &mut lit);
                return StringOutcome::Invalid(parts);
            }
            Some(b'\r') => {
                // `\r\n` is one break, so treat the CR the same as a lone LF:
                // it ends the string. Handling it here (instead of letting the
                // CR fall through as content) is what makes LF and CRLF produce
                // the same parts and diagnostics (ADR 0009).
                diagnostics.push(
                    Diagnostic::error(
                        "E0002",
                        "unterminated string literal (newline before closing quote)",
                        span(file, open, start),
                    )
                    .with_label(span(file, start, start + 1), "line break here")
                    .with_help("use `\\n` for a newline inside a string"),
                );
                flush(&mut parts, &mut lit);
                return StringOutcome::Invalid(parts);
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
                        Interpolation::Part(part) => parts.push(part),
                        // An inner error that still closed the interpolation:
                        // keep going so the enclosing string can terminate.
                        Interpolation::Recovered => ok = false,
                        // The interpolation already reported the outermost
                        // `E0002`; stop so the string does not add a second
                        // diagnostic for the same cause (ADR 0008 D).
                        Interpolation::Unterminated => {
                            return StringOutcome::Invalid(parts);
                        }
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
    if ok {
        StringOutcome::Parts(parts)
    } else {
        StringOutcome::Invalid(parts)
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

    // A line break after `\` is not an escape. Report it over the `\` alone and
    // leave the break unconsumed, so it still ends the line for the caller.
    if ch == '\n' || ch == '\r' {
        cursor.rewind_to(backslash + 1);
        diagnostics.push(
            Diagnostic::error(
                "E0004",
                "invalid escape sequence `\\`",
                span(file, backslash, backslash + 1),
            )
            .with_help(
                "valid escapes are \\n \\t \\r \\\\ \\\" \\{ \\}; `\\` cannot escape a line break",
            ),
        );
        return false;
    }

    match ch {
        'n' => lit.push('\n'),
        't' => lit.push('\t'),
        'r' => lit.push('\r'),
        '\\' => lit.push('\\'),
        '"' => lit.push('"'),
        '{' => lit.push('{'),
        '}' => lit.push('}'),
        _ => {
            let rendered = ch.escape_debug().to_string();
            diagnostics.push(
                Diagnostic::error(
                    "E0004",
                    format!("invalid escape sequence `\\{rendered}`"),
                    span(file, backslash, cursor.pos()),
                )
                .with_help("valid escapes are \\n \\t \\r \\\\ \\\" \\{ \\}"),
            );
            return false;
        }
    }
    true
}

/// How an interpolation scan ended.
enum Interpolation {
    /// A usable expression part.
    Part(StrPart),
    /// An error was reported, but the interpolation did close: keep scanning the
    /// enclosing string (e.g. `"{}"` reported `E0006` and the string still ends
    /// with a quote).
    Recovered,
    /// The interpolation never closed. The outermost `E0002` was reported and
    /// the enclosing string must stop (ADR 0008 D).
    Unterminated,
}

/// Scans a `{expr}` interpolation.
///
/// The expression source is captured verbatim (spans point into the file) and
/// nested strings and braces are skipped so `"{f("a")}"` works.
///
/// A `\` inside the interpolation is `E0006` (ADR 0008): the expression is
/// re-lexed by the parser, and nested strings are written raw. Scanning still
/// continues to the closing `}` so the rest of the string is not lost.
fn scan_interpolation(
    cursor: &mut Cursor<'_>,
    file: FileId,
    diagnostics: &mut Vec<Diagnostic>,
) -> Interpolation {
    let open = cursor.pos();
    cursor.advance(1); // the `{`
    let content_start = cursor.pos();

    let mut depth = 1usize;
    loop {
        match cursor.peek() {
            None | Some(b'\n' | b'\r') => {
                // Decision D: a single, outermost `E0002` for the unclosed
                // interpolation, regardless of how many inner problems there were.
                // A lone CR counts as a break here too, so LF and CRLF agree.
                diagnostics.push(
                    Diagnostic::error(
                        "E0002",
                        "unterminated string literal (unclosed interpolation)",
                        span(file, open, cursor.pos()),
                    )
                    .with_help("add a closing `}` and `\"`"),
                );
                return Interpolation::Unterminated;
            }
            Some(b'\\') => {
                // A backslash is not an escape here; nested strings are raw.
                // Report `E0006` **once per interpolation** (ADR 0009), then
                // consume the following character so a quote cannot open a
                // nested string and swallow the closing `}`.
                let start = cursor.pos();
                diagnostics.push(
                    Diagnostic::error(
                        "E0006",
                        "invalid interpolation: `\\` is not an escape",
                        span(file, start, start + 1),
                    )
                    .with_help(r#"write nested strings without escaping: {f("a")}"#),
                );
                cursor.advance(1);
                // A line break after `\` is left unconsumed: it still ends the
                // line, exactly like a `\` in a plain string (ADR 0007/0009).
                if !matches!(cursor.peek(), Some(b'\n' | b'\r')) {
                    cursor.bump_char();
                }
            }
            Some(b'"') => {
                // Skip a nested string so its braces do not affect depth.
                if !skip_nested_string(cursor, file, diagnostics) {
                    return Interpolation::Unterminated;
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
                        return Interpolation::Recovered;
                    }
                    // The expression source is still returned after the `\`
                    // error above so the parser can re-lex it; the diagnostic
                    // already invalidated the string (ADR 0008).
                    return Interpolation::Part(StrPart::Expr {
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
///
/// A `\` immediately before a line break does **not** consume the break (same
/// rule as plain strings and as [`scan_interpolation`]): the break ends the
/// nested string as unterminated, so `"{f("a\<LF>b")}"` reports the same
/// diagnostics whether the break is `\n`, `\r\n` or `\r` (ADR 0009).
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
            Some(b'\r') => {
                // `\r\n` is one break; either way the nested string ends here.
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
                // Leave a line break for the enclosing scanner to handle.
                if !matches!(cursor.peek(), Some(b'\n' | b'\r')) {
                    cursor.bump_char();
                }
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
