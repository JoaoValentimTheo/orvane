use super::*;
// --- Property tests --------------------------------------------------------

proptest! {
    /// `lex` never panics, for any input at all (SPEC §11 item 3).
    #[test]
    fn lex_never_panics(text in any::<String>()) {
        let (_tokens, _diagnostics) = lex_src(&text);
    }

    /// Structured fragments exercise the interaction of strings, comments,
    /// braces and numbers, checking the invariants on the result — including
    /// on the spans attached to diagnostics, and that the downstream renderers
    /// never panic either.
    #[test]
    fn lex_invariants_hold(
        fragments in prop::collection::vec(
            prop::sample::select(vec![
                "fn", "main", "{", "}", "(", ")", "[", "]", "#{", "\"", "{{", "}}", "\\",
                "\\n", "/*", "*/", "//", "1", "1.", "1..5", "0x", "0xFF", "_", "??", "?.", "..=",
                "=>", "->", "\n", "\r\n", " ", "a", "é", "!", "x => {\n", "\"a{b}c\"",
                "#{\n1: 2\n}", ";", ", ",
                // Added in M1.1: BOM, bare CR, astral char, the rejected `-->`
                // pseudo-operator, and a backslash followed by a real newline.
                "\u{feff}", "\r", "😀", "-->", "\\\n",
            ]),
            0..24,
        )
    ) {
        let text: String = fragments.concat();
        let (sources, file, tokens, diagnostics) = lex_with_sources(&text);
        // Spans are offsets into `SourceFile::text()`, which is the input with
        // the leading BOM already stripped by `SourceMap::add`.
        let file_text = file.text();

        prop_assert!(tokens.last().is_some_and(|t| t.is_eof()), "must end with Eof");

        let mut previous_end = 0u32;
        for token in &tokens {
            let (start, end) = (token.span.start, token.span.end);
            prop_assert!(start <= end);
            prop_assert!((end as usize) <= file_text.len());
            prop_assert!(start >= previous_end, "overlapping spans");
            prop_assert!(file_text.is_char_boundary(start as usize));
            prop_assert!(file_text.is_char_boundary(end as usize));
            // ADR 0007: a `Newline` is a zero-width point, so it cannot cover
            // bytes that belong to another token.
            if token.is_newline() {
                prop_assert_eq!(start, end, "Newline must be zero-width");
            }
            previous_end = end;
        }

        // Diagnostic spans obey the same bounds and char-boundary rules, and
        // must not start after they end.
        for diagnostic in &diagnostics {
            let (start, end) = (diagnostic.primary.start, diagnostic.primary.end);
            prop_assert!(start <= end, "diagnostic span is backwards");
            prop_assert!((end as usize) <= file_text.len(), "diagnostic span out of bounds");
            prop_assert!(file_text.is_char_boundary(start as usize));
            prop_assert!(file_text.is_char_boundary(end as usize));

            // A message is one line by contract: no raw control characters.
            prop_assert!(
                !diagnostic.message.contains('\n') && !diagnostic.message.contains('\r'),
                "diagnostic message must not contain a line break: {:?}",
                diagnostic.message
            );
        }

        // The renderers must not panic on any lexer output.
        let _ = crate::dump::dump_tokens(&file, &tokens);
        let mut compact = String::new();
        for diagnostic in &diagnostics {
            compact.push_str(&diagnostic.render_compact(&sources));
        }

        // Line-ending invariance: replacing every `\n` with `\r\n` must not
        // change the sequence of token kinds nor the diagnostic codes. Spans
        // are allowed to shift, so the comparison strips the `L:C` prefixes.
        if !text.contains('\r') {
            let crlf = text.replace('\n', "\r\n");
            let (_sources2, file2, tokens2, diagnostics2) = lex_with_sources(&crlf);
            prop_assert_eq!(
                kind_sequence(&file, &tokens),
                kind_sequence(&file2, &tokens2),
                "LF and CRLF must lex to the same kinds"
            );
            let codes: Vec<&str> = diagnostics.iter().map(|d| d.code).collect();
            let codes2: Vec<&str> = diagnostics2.iter().map(|d| d.code).collect();
            prop_assert_eq!(codes, codes2, "LF and CRLF must report the same codes");
        }
    }
}

/// The dump of `tokens`, with the `line:col` prefix removed from each line.
///
/// Comparing this ignores span positions while keeping the kind and payload, so
/// it can assert that LF and CRLF yield the same token sequence (ADR 0009).
fn kind_sequence(file: &SourceFile, tokens: &[Token]) -> Vec<String> {
    crate::dump::dump_tokens(file, tokens)
        .lines()
        .map(|line| match line.split_once(' ') {
            Some((_position, kind)) => kind.to_owned(),
            None => line.to_owned(),
        })
        .collect()
}
