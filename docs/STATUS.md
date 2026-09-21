# Status das features

Regra (§0.2 item 4 / §11 item 6): uma feature só é **Implemented** se houver
teste golden que a exercita. Sem teste → `Planned`.

## Milestones

| Milestone | Escopo | Status | Gate |
|---|---|---|---|
| M0 | bootstrap do workspace, `Span`/`SourceMap`/`Diagnostic`, harness golden, CI | **Implemented** | `cargo test` roda o harness com 1 caso golden; `orv version` imprime a versão |
| M0.1 | hardening: harness estrito, LF no `version`, licenças, README, CI `--locked` | **Implemented** | `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace` (sem features novas da linguagem) |
| M1 | lexer: tokens de §5.1, interpolação, comentários, `E0001`–`E0006`, `orv tokens` | **Implemented** | `orv tokens x.orv` dumpa tokens estáveis; 104 testes de unidade + 2 proptests |
| M1 | lexer | **Implemented** | `orv tokens x.orv` |
| M2 | parser + `orv ast` | Planned | §4.1–§4.2 parseiam |
| M3 | sema v1 (nomes e tipos) | Planned | `orv check` |
| M4 | interpretador v1 | Planned | `orv run examples/fib.orv` |
| M5 | `data`/`enum`/`match` | Planned | §4.2 roda; `match` não exaustivo rejeitado |
| M6 | intents/strategies/planner | Planned | 12 casos de `tests/golden/intents/` |
| M7 | `use py` e `Host` | Planned | §4.4 com `py.exec` mockado |
| M8 | módulo Python `orvane` | Planned | `pytest tests/python` |
| M9 | `orv fmt` / `orv test` / `orv repl` | Planned | formatador idempotente |
| M10 | generics e módulos | Planned | `data Box<T>` + 2 unidades |
| M11 | (opcional) bytecode VM | Planned | ADR obrigatória |
| M12 | release 0.1.0 | Planned | README honesto + publicação de teste |

## Features do M0

| Feature | Teste que a exercita |
|---|---|
| Workspace com 6 crates + regra de dependência unidirecional | build/clippy de todos os membros; `crates/orv-{sema,runtime,py,pymod}/src/lib.rs` (`crate_is_wired`) |
| `Span` (`file`/`start`/`end`, `to`, `len`, `point`) | unidade em `crates/orv-syntax/src/span.rs` |
| `SourceMap`/`SourceFile`/`FileId` (BOM, `line_col` 1-based) | unidade em `crates/orv-syntax/src/source.rs` |
| `Diagnostic` + formato `.err` estável `CODE:linha:coluna: mensagem` | unidade em `crates/orv-syntax/src/diagnostic.rs` |
| Renderização `ariadne` (ASCII, sem cor, determinística) | unidade em `crates/orv-syntax/src/render.rs` |
| CLI `orv version` | unidade em `crates/orv-cli/src/cli.rs` + golden `tests/golden/version` |
| Harness golden (`.orv` + `.out` + `.err`) | `tests/golden_runner.rs` (`crates/orv-cli/tests/`) |
| Harness estrito: sem `.err` ⇒ 0 diagnósticos; sem `.out` ⇒ stdout vazio | `missing_err_requires_zero_diagnostics`, `missing_out_requires_empty_stdout`, `present_out_must_match_exactly` |
| Cabeçalho `// exit: N` (padrão 0) | `header_defaults_exit_to_zero`, `header_parses_exit_override`, `header_parses_signal_exit_code`, `header_requires_the_command_line_to_come_first`; E2E em `tests/golden/unknown_subcommand.orv` |
| Exit code real conferido (não só o header) | `an_unknown_subcommand_actually_exits_nonzero`, `exit_code_check_rejects_matching_status_when_header_disagrees` |
| `orv version` emite LF em qualquer SO (bytes crus) | `version_stdout_contains_no_carriage_return` |
| Fim de linha estável no golden (Linux + Windows) | `decode_output_normalizes_crlf`, `decode_output_keeps_lf_and_lone_cr`; `.gitattributes` |
| CI Linux + Windows (fmt/clippy/test, `--locked`) | `.github/workflows/ci.yml` |
| `AGENTS.md`, `docs/errors.md`, `docs/adr/` | revisão; ADRs 0001–0007 |
| Licenças MIT **ou** Apache-2.0 | `LICENSE-MIT`, `LICENSE-APACHE`, `license.workspace` |
| README mínimo com status honesto | `README.md` (status gerado de `docs/STATUS.md`) |

## Features do M1 (lexer)

| Feature | Teste que a exercita |
|---|---|
| 33 keywords reservadas; `py` como `Ident` | `lexes_every_reserved_keyword`, `py_is_lexed_as_an_identifier`, `keyword_table_covers_spec_list`, `py_is_not_a_reserved_keyword` |
| Int: decimal, `_`, `0x`, `0b`, limites de `i64` | `lexes_simple_int`, `lexes_int_with_separators`, `lexes_hex_and_binary`, `lexes_i64_boundaries` |
| Float: `1.5`, `2e10`, `1e-3`, `1_0.5` | `lexes_float_forms` |
| `.` só inicia fração antes de dígito | `dot_only_starts_a_fraction_before_a_digit`, `chained_dots_are_not_a_number_error` |
| `E0005` literal numérico inválido | `invalid_numbers_report_e0005`; golden `tokens_err_bad_number` |
| Str com escapes, `{{`/`}}`, interpolação | `lexes_plain_string_with_escapes`, `lexes_interpolation`, `literal_braces_via_doubling` |
| Interpolação aninhada (strings e chaves) | `interpolation_allows_nested_strings_and_braces`, `escaped_quotes_inside_interpolation_do_not_close_the_string`, `literal_braces_around_an_interpolation` |
| `E0002` / `E0004` / `E0006` | `unterminated_string_reports_e0002`, `raw_newline_in_string_reports_e0002`, `unknown_escape_reports_e0004`, `lone_closing_brace_reports_e0006`, `empty_interpolation_reports_e0006`, `unclosed_interpolation_reports_e0002`; goldens `tokens_err_*` |
| Todos os operadores + maximal munch | `lexes_all_operators`, `maximal_munch_prefers_longer_operators` |
| `E0001` caractere inválido (e não-ASCII) | `invalid_characters_report_e0001_and_continue`, `non_ascii_outside_strings_reports_e0001_with_ascii_help`; golden `tokens_err_invalid_char` |
| Comentários `//` e `/* */` aninhável; `E0003` | `line_comment_does_not_consume_the_newline`, `block_comments_are_nestable`, `block_comment_with_newline_counts_as_one_newline`, `unterminated_block_comment_reports_e0003`; goldens `tokens_comments`, `tokens_err_block_comment` |
| Newline: colapso, CRLF, `\r` isolado, BOM | `no_newline_at_start_of_file`, `consecutive_newlines_collapse`, `crlf_is_one_line_break`, `lone_carriage_return_is_whitespace`, `bom_is_ignored` |
| Pilha de delimitadores (§5.1 regra 1) | `parens_suppress_newlines`, `brackets_suppress_newlines`, `hash_brace_suppresses_newlines`, `block_brace_keeps_newlines_significant_inside_parens`, `nested_delimiters_use_the_innermost_context`, `newline_after_closing_hash_brace_is_emitted` |
| **Teste obrigatório**: lambda com bloco em chamada | `block_brace_keeps_newlines_significant_inside_parens`; golden `tokens_block_lambda` |
| **Teste obrigatório**: `#{ }` multilinha | `hash_brace_suppresses_newlines`; golden `tokens_map_multiline` |
| Fechamento sem abertura não dá panic | `unmatched_closers_do_not_panic` |
| Invariantes: `Eof` final, spans no arquivo/ordenados/disjuntos | `always_ends_with_eof`, `spans_are_ordered_and_within_the_file`, `token_spans_match_the_source_text`, `assert_spans_are_valid` |
| proptest: `lex` nunca dá panic | `lex_never_panics` |
| proptest: invariantes com alfabeto de fragmentos | `lex_invariants_hold` |
| Formato de dump estável | `dump::tests::*`; goldens `tokens_*` |
| `orv tokens` (stdout/`Eof`/exit 1 com diagnóstico) | goldens `tokens_*`, `tokens_err_*`; `tokens_subcommand_parses_with_a_file` |
