# Status das features

Regra (§0.2 item 4 / §11 item 6): uma feature só é **Implemented** se houver
teste golden que a exercita. Sem teste → `Planned`.

## Milestones

| Milestone | Escopo | Status | Gate |
|---|---|---|---|
| M0 | bootstrap do workspace, `Span`/`SourceMap`/`Diagnostic`, harness golden, CI | **Implemented** | `cargo test` roda o harness com 1 caso golden; `orv version` imprime a versão |
| M0.1 | hardening: harness estrito, LF no `version`, licenças, README, CI `--locked` | **Implemented** | `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace` (sem features novas da linguagem) |
| M1 | lexer: tokens de §5.1, interpolação, comentários, `E0001`–`E0006`, `orv tokens` | **Implemented** | `orv tokens x.orv` dumpa tokens estáveis; 128 testes de unidade + 2 proptests |
| M1.1 | correções pós-revisão: `-->` rejeitado, BOM, recuperação de erro (ADR 0008), LF×CR, docs | **Implemented** | mesmo gate; 16 goldens `tokens_*` |
| M1.2 | últimos ajustes: teste de stdout determinístico, paridade LF/CRLF com `\`, `E0006` por interpolação, float fora de faixa, docs, testes divididos | **Implemented** | mesmo gate; 17 goldens `tokens_*` |
| M2 | parser + `orv ast` | **Implemented (alpha)** | §4.1, §4.2 e §A1–A2 parseiam; `orv ast` dumpa AST estável |
| — | exemplos executáveis em `examples/` (6 programas) | **Implemented** | goldens `example_{hello,fizzbuzz,fib,data,shapes,closures}` |
| M3 | sema: nomes e tipos do recorte | **Implemented (alpha)** | `orv check` aceita válidos e rejeita cada caso com o código certo |
| M4 | interpretador: `Value`, closures, controle, coleções, prelude | **Implemented (alpha)** | `orv run` executa §4.1/§4.2/§A1/§A2 com a saída correta; `R0001`/`R0002` com exit 1 |
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
| `Span` (`file`/`start`/`end`, `to`, `len`, `point`, `intersects`) | unidade em `crates/orv-syntax/src/span.rs` |
| `SourceMap`/`SourceFile`/`FileId` (BOM, `line_col` 1-based, índice de inícios de linha O(log n)) | unidade em `crates/orv-syntax/src/source.rs` |
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
| `AGENTS.md`, `docs/errors.md`, `docs/adr/` | revisão; ADRs 0001–0010 |
| Licenças MIT **ou** Apache-2.0 | `LICENSE-MIT`, `LICENSE-APACHE`, `license.workspace` |
| README mínimo com status honesto | `README.md` (status gerado de `docs/STATUS.md`) |

## Features do M4 (runtime)

| Feature | Teste que a exercita |
|---|---|
| `Value` com igualdade estrutural (listas, mapas, tuplas, `data`, `enum`) | `value::tests::*` (11 casos), `runs_list_equality`, `runs_data_construction_and_display` |
| Aritmética com promoção `Int`→`Float`, concatenação de `Str` e listas | `runs_arithmetic`, `runs_string_concatenation` |
| Comparações e lógica com curto-circuito | `runs_comparisons_and_logic` |
| `fn` com retorno, recursão, closures capturando `let mut` | `runs_a_function_call`, `runs_recursion`, `runs_a_closure_capturing_a_mutable_binding`, `runs_a_lambda_passed_to_a_function` |
| `let`/`let mut`, atribuição e operadores compostos | `runs_while_with_mutation`, `accepts_mut_reassignment` |
| `if`/`while`/`for`/`break`/`continue` | `runs_if_as_an_expression`, `runs_while_with_mutation`, `runs_for_over_a_range`, `runs_break_and_continue` |
| Listas, mapas, `len`, indexação | `runs_list_operations`, `runs_map_operations`, `runs_for_over_a_list` |
| `data` (construtor nomeado/posicional, campos, `Display`) | `runs_data_construction_and_display`, `runs_field_access` |
| `enum` + `match` (payload, guarda, literal, wildcard) | `runs_the_enum_and_match_example`, `runs_match_on_integers` |
| Prelude §5.6 | `builtins::tests::*` (13 casos) |
| `R0001` divisão por zero, `R0002` overflow, `R0003` índice/chave, `R0004` profundidade | `division_by_zero_is_a_failure`, `integer_overflow_is_a_failure`, `index_out_of_bounds_is_a_failure`, `missing_map_key_is_a_failure`, `infinite_recursion_is_a_failure_not_a_crash` |
| Fronteira exata de profundidade: `main` + 47 frames = 48; `f(46)` ok, `f(47)` `R0004` | `the_call_depth_boundary_is_exact` (ADR 0016) |
| proptest: runtime nunca dá panic | `runtime_never_panics`, `runtime_never_panics_on_fragments` |
| `orv run` (stdout = saída do programa; exit 1 em Failure) | goldens `run_fizzbuzz`, `run_data`, `run_shape`, `run_division_by_zero`, `run_overflow` |

## Instrumentação para 0.1.1 (proptest de alto volume e fuzzing)

O objetivo do 0.1.1 é "rodar isto e consertar o que encontrar". Nada aqui é
bloqueante para o alpha.

**CI:** `.github/workflows/deep-tests.yml` (`Deep tests`, disparo manual ou semanal)
roda três jobs, todos com `continue-on-error: true`:

1. `proptest-deep` — os proptests de nunca-panic com `PROPTEST_CASES=200000` por
   camada (lexer/parser, sema, runtime);
2. `fuzz-front-end` — o harness de bytes `orv-tools --bin fuzz-bytes` com 200000
   iterações;
3. `miri-lexer` — `cargo miri test -p orv-syntax --lib` (pega aritmética de byte
   fora dos limites que o modo release pode esconder).

**Manual (local):**

```console
$ PROPTEST_CASES=200000 cargo test --locked -p orv-syntax --lib
$ PROPTEST_CASES=200000 cargo test --locked -p orv-sema --lib
$ PROPTEST_CASES=200000 cargo test --locked -p orv-runtime
$ cargo run --locked --release -p orv-tools --bin fuzz-bytes -- 1000000
```

O harness é determinístico (xorshift de semente fixa), então uma falha é
reproduzível; ele mistura fragmentos que chegam aos caminhos interessantes com
bytes crus. `orv-tools` é `publish = false` e existe só para isso.

## Features do M3 (sema)

| Feature | Teste que a exercita |
|---|---|
| Resolução de nomes com escopos, shadowing e prelude (§5.6) | `accepts_nested_blocks_and_shadowing`, `shadowing_in_an_inner_scope_is_allowed`, `calls_can_refer_to_later_functions`; `scopes::tests::*` |
| Inferência local (`let x = 1`) e anotação explícita | `accepts_arithmetic_with_inference`, `accepts_annotated_let` |
| Primitivos, `List`, `Map`, tuplas, opcionais, `Result` | `ty::tests::*`, `accepts_lists_and_indexing`, `accepts_maps`, `accepts_optional_coalesce` |
| Aritmética com promoção `Int`→`Float` e concatenação de `Str` | `accepts_int_float_promotion`, `accepts_string_concatenation`, `numeric_result` |
| `data`/`enum`: campos, variantes, construção posicional/nomeada | `accepts_data_construction`, `accepts_enum_and_match`, `duplicate_field_reports_e0202` |
| `match` com patterns, guarda e tipos de arm unificados | `accepts_enum_and_match`, `match_arm_type_mismatch_reports_e0301`, `variant_pattern_field_count_reports_e0302` |
| `E0201` nome/tipo indefinido | `undefined_name_reports_e0201`, `undefined_type_reports_e0201` |
| `E0202` definição duplicada | `duplicate_definition_reports_e0202`, `duplicate_parameter_reports_e0202` |
| `E0230` atribuição a imutável | `assigning_to_an_immutable_reports_e0230`, `assigning_to_a_field_of_an_immutable_is_allowed` |
| `E0301`–`E0313`, `E0321` (tabela em `docs/errors.md`) | um teste por código |
| **Sem cascata**: um erro por causa (`Ty::Unknown` compatível) | `one_undefined_name_yields_one_diagnostic`, `a_mistake_in_a_condition_does_not_cascade_into_the_body` |
| proptest: sema nunca dá panic para AST arbitrária | `sema_never_panics`, `sema_never_panics_on_fragments` |
| `orv check` (exit 1 com diagnóstico, 0 sem) | goldens `check_ok`, `check_errors` |

## Fundações do M2 (parser)

Ainda não há parser; estes itens preparam o M2 e são pré-requisito do contrato do
pipeline (ADR 0008, emenda).

| Feature | Teste que a exercita |
|---|---|
| Índice de inícios de linha em `SourceFile` (`line_col` deixa de ser O(n)) | `line_starts_index_every_line`, `line_starts_of_an_empty_file_is_just_zero`, `line_starts_treat_crlf_as_one_break`, `line_starts_ignore_a_lone_carriage_return`, `line_col_matches_a_naive_scan_on_every_offset` (offset a offset), `line_col_is_at_least_linear_only_once_per_file` |
| `Span::intersects` para supressão de diagnóstico do parser | `intersects_detects_overlap`, `intersects_treats_a_point_span_as_inside`, `intersects_requires_the_same_file`, `intersects_two_point_spans` |
| Contrato do pipeline com erro léxico (léxico primeiro; suprimir parser por interseção; sem sub-parse de `Expr.src`; nunca sema/run) | **contrato** (ADR 0008, emenda) — a implementação é do M2. Material bruto coberto por `lexical_diagnostics_cover_their_best_effort_token`, `a_newline_after_an_erroneous_token_does_not_overlap_it` |
| `i64::MIN` sem literal: `(-9223372036854775807) - 1` (ADR 0007 §6.2) | `i64_min_cannot_be_written_as_a_literal`, `the_overflowing_magnitude_is_not_recoverable_from_tokens` |
| `Float Dot Int` (`1.2.3`) é erro **sintático** `E0102`, não léxico (ADR 0011) | decisão registrada; `float_dot_int_is_a_syntax_error_by_grammar` (lexer) |
| AST de primary: `Literal`/`Ident`/`Paren`/`Block`, `Span` em todo nó | `parser::tests::*` |
| Parser completo de §5.2: expressões, statements, itens, tipos, patterns | suíte `parser::tests` (97 casos) |
| Pratt com a precedência de §5.2.1 | `parses_arithmetic_with_precedence`, `subtraction_is_left_associative`, `coalesce_binds_looser_than_or`, `and_binds_tighter_than_or`, `comparison_binds_looser_than_addition`, `unary_binds_tighter_than_multiplication`, `precedence_of_cast_relative_to_arithmetic` |
| Lambda (1 e N params, corpo bloco) | `parses_single_parameter_lambda`, `parses_multi_parameter_lambda`, `parses_lambda_with_block_body`, `parenthesised_expression_is_not_a_lambda` |
| Coleções `[]`, `#{}` | `parses_list_literal`, `parses_empty_list`, `parses_list_with_trailing_comma`, `parses_map_literal`, `parses_empty_map` |
| `if`/`else if`, `match` com guarda, `try`/`fail` | `parses_if_expression`, `allows_a_newline_before_else`, `parses_match_expression`, `parses_match_with_guard`, `parses_try_expression`, `parses_fail_as_a_statement` |
| `fn`/`data`/`enum`/`use` | `parses_a_function_declaration`, `parses_data_declaration`, `parses_enum_declaration`, `parses_use_declaration`, `parses_parameter_defaults` |
| Tipos de §5.2 (`List<T>`, tuplas, `fn(...) -> ...`, `T?`, `()`) | `parses_generic_type_annotation`, `parses_tuple_and_fn_types`, `parses_unit_type` |
| Recuperação: um erro por statement, múltiplos por arquivo | `a_broken_statement_does_not_hide_the_next_one`, `several_errors_in_one_file_are_all_reported`, `a_broken_item_does_not_hide_the_next_one` |
| `E0104` aninhamento profundo (ADR 0013) | `deeply_nested_input_reports_e0104_instead_of_overflowing`, `deeply_nested_unary_reports_e0104`, `moderately_nested_input_is_fine` |
| `E0105` recusa explícita do que está fora do alpha | `intent_is_refused_with_e0105`, `how_is_refused_with_e0105`, `test_block_is_refused_with_e0105`, `use_py_is_refused_with_e0105`, `pub_data_is_refused_with_e0105` |
| proptest: parser nunca dá panic | `parser_never_panics`, `parser_never_panics_on_fragments` |
| `orv ast` + dump estável (ADR 0014) | goldens `ast_hello`, `ast_fizzbuzz`, `ast_shape` |
| Programas de referência §4.1/§4.2/§A1/§A2 parseiam | `parses_the_appendix_a1_fizzbuzz`, `parses_the_appendix_a2_data_example`, `parses_the_appendix_a2_enum_and_match_example` |
| `E0102` quando falta `)` ou `}` | `missing_closing_paren_reports_e0102`, `unclosed_block_reports_e0102`, `empty_parens_report_e0102` |
| Diagnóstico do parser suprimido por erro léxico (ADR 0008 regra 3) | `a_parser_error_inside_a_broken_literal_is_suppressed`, `without_suppression_that_parser_error_would_exist` |
| `should_subparse_expr` respeitado no parser (ADR 0008 regra 4) | `a_string_with_a_lexical_error_blocks_the_subparse_and_adds_nothing`, `the_subparse_guard_is_consulted_for_strings` |
| Gate sema/run documentado (`ParseResult::has_errors`) | `no_errors_means_the_sema_gate_is_open`, `lexical_diagnostics_come_first_and_suppress_the_parser` |

## Features do M1 (lexer)

| Feature | Teste que a exercita |
|---|---|
| 33 keywords reservadas; `py` como `Ident` | `lexes_every_reserved_keyword`, `py_is_lexed_as_an_identifier`, `keyword_table_covers_spec_list`, `py_is_not_a_reserved_keyword` |
| Int: decimal, `_`, `0x`, `0b`, limites de `i64` | `lexes_simple_int`, `lexes_int_with_separators`, `lexes_hex_and_binary`, `lexes_i64_boundaries` |
| Float: `1.5`, `2e10`, `1e-3`, `1_0.5` | `lexes_float_forms` |
| `.` só inicia fração antes de dígito | `dot_only_starts_a_fraction_before_a_digit`, `chained_dots_are_not_a_number_error` |
| `E0005` literal numérico inválido | `invalid_numbers_report_e0005`; golden `tokens_err_bad_number` |
| Str com escapes, `{{`/`}}`, interpolação | `lexes_plain_string_with_escapes`, `lexes_interpolation`, `literal_braces_via_doubling` |
| Interpolação aninhada (strings e chaves) | `interpolation_allows_nested_strings_and_braces`, `interpolation_with_a_raw_nested_string_is_valid`, `literal_braces_around_an_interpolation` |
| `E0002` / `E0004` / `E0006` | `unterminated_string_reports_e0002`, `raw_newline_in_string_reports_e0002`, `unknown_escape_reports_e0004`, `lone_closing_brace_reports_e0006`, `empty_interpolation_reports_e0006`, `unclosed_interpolation_reports_e0002`, `unclosed_interpolation_reports_a_single_e0002`, `backslash_inside_interpolation_is_e0006`, `backslash_before_a_line_break_reports_e0004_on_the_backslash_only`, `unknown_escape_message_uses_escape_debug`; goldens `tokens_err_*` |
| Todos os operadores + maximal munch | `lexes_all_operators`, `maximal_munch_prefers_longer_operators` |
| `E0001` caractere inválido (e não-ASCII) | `invalid_characters_report_e0001_and_continue`, `non_ascii_outside_strings_reports_e0001_with_ascii_help`; golden `tokens_err_invalid_char` |
| Comentários `//` e `/* */` aninhável; `E0003` | `line_comment_does_not_consume_the_newline`, `block_comments_are_nestable`, `block_comment_with_newline_counts_as_one_newline`, `unterminated_block_comment_reports_e0003`; goldens `tokens_comments`, `tokens_err_block_comment` |
| Newline: colapso, CRLF, `\r` isolado | `no_newline_at_start_of_file`, `consecutive_newlines_collapse`, `crlf_is_one_line_break`, `lone_carriage_return_is_whitespace`, `lone_carriage_return_does_not_end_a_line_comment`, `carriage_return_newline_does_end_a_line_comment`, `lone_carriage_return_inside_a_block_comment_is_not_a_newline`, `crlf_inside_a_block_comment_is_one_newline`; golden `tokens_crlf` |
| BOM (primeiro é ignorado; o segundo é `E0001`) | **unit-only** — `bom_is_ignored`, `second_bom_is_an_invalid_character`, `cursor_does_not_strip_a_bom`. O harness não consegue representar BOM no cabeçalho, então não há golden |
| Pilha de delimitadores (§5.1 regra 1) | `parens_suppress_newlines`, `brackets_suppress_newlines`, `hash_brace_suppresses_newlines`, `block_brace_keeps_newlines_significant_inside_parens`, `nested_delimiters_use_the_innermost_context`, `newline_after_closing_hash_brace_is_emitted` |
| **Teste obrigatório**: lambda com bloco em chamada | `block_brace_keeps_newlines_significant_inside_parens`; golden `tokens_block_lambda` |
| **Teste obrigatório**: `#{ }` multilinha | `hash_brace_suppresses_newlines`; golden `tokens_map_multiline` |
| Fechamento sem abertura não dá panic | `unmatched_closers_do_not_panic` |
| Pilha desenrola até o abridor correspondente (ADR 0008 B) | `a_closer_unwinds_mismatched_openers_above_its_match`, `a_closer_with_no_match_leaves_the_stack_alone` |
| Token de melhor esforço em erro léxico (ADR 0008 A) | `error_tokens_are_best_effort`, `lexes_i64_boundaries`, `unterminated_string_does_not_stop_the_lexer`; goldens `tokens_err_*` |
| `-->` não é operador | `triple_dash_is_not_a_single_operator` |
| `orv tokens` não dá panic com stdout fechado | `tokens_with_closed_stdout_does_not_panic`, `tokens_on_a_missing_file_reports_usage_error` (`crates/orv-cli/tests/output_failures.rs`) |
| Invariantes: `Eof` final, spans no arquivo/ordenados/disjuntos | `always_ends_with_eof`, `spans_are_ordered_and_within_the_file`, `token_spans_match_the_source_text`, `assert_spans_are_valid` |
| proptest: `lex` nunca dá panic | `lex_never_panics` |
| proptest: invariantes com alfabeto de fragmentos | `lex_invariants_hold` |
| Formato de dump estável | `dump::tests::*`; goldens `tokens_*` |
| `orv tokens` (stdout/`Eof`/exit 1 com diagnóstico) | goldens `tokens_*`, `tokens_err_*`; `tokens_subcommand_parses_with_a_file` |
| Paridade LF/CRLF/CR com `\` (ADR 0009) | `backslash_before_a_line_break_inside_interpolation_is_reported`, `line_break_after_backslash_inside_interpolation_is_not_swallowed`, `lf_and_crlf_agree_inside_a_nested_string`; invariante `kind_sequence` em `lex_invariants_hold` |
| Um `E0006` por interpolação | `backslash_inside_interpolation_is_e0006`; golden `tokens_err_interp_escape` |
| Float fora da faixa é `E0005` (ADR 0010) | `float_out_of_range_reports_e0005_with_a_float_placeholder`, `largest_finite_float_is_still_accepted`, `renders_non_finite_floats_explicitly`, `out_of_range_float_dumps_as_the_placeholder`; golden `tokens_err_float_range` |
| `0X`/`0B`/`E` maiúsculos; `i64::MIN` não escrevível (ADR 0007 §6.1/6.2) | `radix_prefixes_and_exponents_accept_uppercase`, `i64_min_cannot_be_written_as_a_literal`, `i64_max_magnitude_is_fine` |
| stdout fechado/cheio não dá panic (determinístico no Linux) | `tokens_with_closed_stdout_does_not_panic`, `tokens_with_full_stdout_reports_usage_error` (Linux) |
| Testes do lexer divididos por tema | `crates/orv-syntax/src/lexer/tests/{keywords,numbers,strings,operators,comments,newlines,invariants,properties}.rs` (81 testes, nomes e corpos inalterados) |
