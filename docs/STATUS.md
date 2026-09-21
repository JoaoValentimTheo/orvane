# Status das features

Regra (§0.2 item 4 / §11 item 6): uma feature só é **Implemented** se houver
teste golden que a exercita. Sem teste → `Planned`.

## Milestones

| Milestone | Escopo | Status | Gate |
|---|---|---|---|
| M0 | bootstrap do workspace, `Span`/`SourceMap`/`Diagnostic`, harness golden, CI | **Implemented** | `cargo test` roda o harness com 1 caso golden; `orv version` imprime a versão |
| M0.1 | hardening: harness estrito, LF no `version`, licenças, README, CI `--locked` | **Implemented** | `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace` (sem features novas da linguagem) |
| M1 | lexer | Planned | `orv tokens x.orv` |
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
| `AGENTS.md`, `docs/errors.md`, `docs/adr/` | revisão; ADRs 0001–0005 |
| Licenças MIT **ou** Apache-2.0 | `LICENSE-MIT`, `LICENSE-APACHE`, `license.workspace` |
| README mínimo com status honesto | `README.md` (status gerado de `docs/STATUS.md`) |
