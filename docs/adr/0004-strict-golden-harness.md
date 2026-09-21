# ADR 0004 — Harness golden estrito

- **Status:** aceita
- **Data:** M0.1

## Contexto

O harness do M0 (ADR 0003) só comparava o que existia: sem `.out` ou sem `.err`,
ele não verificava nada. Isso permite que um caso passe "por omissão" — por
exemplo, um `orv run` que começa a imprimir lixo, ou um `orv check` que passa a
emitir um aviso, continuaria verde enquanto ninguém criasse o arquivo de
expectativa. Também não havia como afirmar o **exit code**, que é parte do
contrato de SPEC §10.

## Decisão

O harness é estrito; a ausência de um arquivo é uma expectativa, não uma
dispensa:

1. **Sem `.err` ⇒ zero diagnósticos.** Qualquer linha com formato
   `CODE:linha:coluna:` no stderr falha o caso.
2. **Sem `.out` ⇒ stdout vazio.** Qualquer byte em stdout falha o caso.
3. **`// exit: N`** no cabeçalho (padrão `0`) fixa o exit code esperado. Só é
   aceito **depois** da linha `// orv:`, que continua sendo a âncora do
   cabeçalho; chaves desconhecidas são ignoradas para o formato poder crescer.

As três regras são implementadas por um único predicado,
`compare_bytes(what, expected: Option<&str>, actual)`, onde `expected = None`
significa "deve ser vazio". Assim `check_stdout` e `check_diagnostics` não podem
divergir.

## Consequência

- Um caso novo precisa declarar explicitamente tudo o que o comando produz;
  esquecer um `.out` passa a ser um erro alto, não silencioso.
- Testar caminhos de erro exige um `.err` (e, tipicamente, `// exit: 1`), o que
  torna o exit code verificável de ponta a ponta.
- As regras são exercitadas por testes do próprio harness que **devem falhar**:
  `missing_out_requires_empty_stdout`, `missing_err_requires_zero_diagnostics`,
  `present_out_must_match_exactly`,
  `exit_code_check_rejects_matching_status_when_header_disagrees` e
  `an_unknown_subcommand_actually_exits_nonzero` (que impede o `// exit:` de
  passar por acidente num CLI que sempre sai com 0).
- `tests/golden/unknown_subcommand.orv` cobre `// exit: 2` de ponta a ponta, sem
  `.out` nem `.err`: o caso só passa porque o harness agora exige as duas
  ausências.
