# Changelog

Todas as mudanças relevantes por versão. O formato segue "Keep a Changelog"
(https://keepachangelog.com/pt-BR/1.1.0/) e o versionamento é o de `Cargo.toml`.

## [0.1.1] - 2026-09-22

Sprint `fix-0.1.1`. Ritmo de correção: nada aqui adiciona capacidade nova à
linguagem. Toda mudança de comportamento tem teste que falha antes e passa
depois.

### Corrigido

- **Closure perdia as capturas quando o frame que a definiu retornava.**
  `fn adder(n) { return (x) => x + n }` passava na sema e falhava no runtime com
  `R0010: undefined name 'n'`; o mesmo para lambda aninhada `a => (b) => a + b`.
  A causa era o `Env` compartilhar a cadeia de escopos e o `pop` do retorno
  remover o frame. Agora a closure captura um snapshot com as células de binding
  compartilhadas (ADR 0019). Regressão em `interpret.rs` e golden
  `matrix_m28_lambda_from_frame`.
- **Chave de mapa fora de `{Int, Str, Bool}` passava na sema.** `#{A: 1}` (variante
  de enum), `#{P(x: 1): 1}` (data) e `#{1.5: 1}` (float) eram aceitos pela sema e
  recusados pelo runtime com `R0010`. A sema agora reporta `E0301`
  (`is_valid_map_key`). Regressão em `check/tests.rs` e golden
  `matrix_m29_map_int_bool_keys`.
- **Nome de variante declarado por dois `enum` divergia entre `check` e `run`.**
  `enum A { V(Int) }` + `enum B { V(Str) }` deixava `V("hello")` passar no `check`
  e falhar no `run` (a sema e o runtime escolhiam donos diferentes, por ordem de
  hash). Nome de variante agora é único entre enums (`E0202`, ADR 0020).
  Regressão: `a_variant_name_shared_by_two_enums_reports_e0202`.
- **`break`/`continue` fora de loop passava no `check` e falhava no `run` com
  `R0010`.** A sema agora rastreia profundidade de loop e reporta `E0303`
  (ADR 0021). Regressão: `break_outside_a_loop_reports_e0303`,
  `continue_outside_a_loop_reports_e0303`.
- **`break` dentro de lambda encerrava o loop do chamador, silenciosamente.** O
  `Control::Break` produzido na closure era estacionado em `pending` e relido
  pelo loop do chamador. A sema recusa (`E0303`) e o runtime confina `pending` à
  chamada (ADR 0021). Regressão: `a_break_in_a_lambda_inside_a_loop_reports_e0303`
  (sema), `a_break_inside_a_lambda_cannot_leave_the_call` e
  `a_loop_inside_a_lambda_still_works` (runtime); golden `check_break_in_lambda`.
- **O job `miri` do workflow `Deep tests` nunca rodava.** `dtolnay/rust-toolchain@nightly`
  instalava nightly, mas `rust-toolchain.toml` fixa `stable`, então `cargo miri`
  resolvia para a toolchain errada e abortava com "the 'miri' component ... is not
  available for stable". Passa a usar `cargo +nightly miri` e
  `MIRIFLAGS=-Zmiri-disable-isolation` (a persistência de falhas do proptest usa
  `std::env::current_dir`, bloqueado sob isolamento). Verificado: 262 testes do
  lexer sob Miri, sem UB.

### Adicionado (apenas teste/ferramenta)

- `sema_runtime_agreement.rs` ganhou geradores para tuplas, lambdas/closures,
  campos opcionais e coleções de tipo de usuário — as categorias que o arquivo
  ainda não gerava. Os geradores de lambda reproduzem o bug de captura.
- Linhas de combinações cruzadas em `docs/coverage-matrix.md`.
- Goldens `matrix_m28_lambda_from_frame`, `matrix_m29_map_int_bool_keys`.

### Notas

- A tag `v0.1.0-alpha` e o `CHANGELOG.md` foram criados retroativamente (a tag
  não existia; o merge `alpha-0.1.0 → main` estava feito).
- Nenhuma issue aberta no repositório no início da sprint; as duas correções
  vieram da varredura dirigida.

## [0.1.0-alpha] — primeiro alpha executável

Sprint `alpha-0.1.0` seguida de `fix/0.1.0-stabilization`. `orv run arquivo.orv`
executa um programa real: funções, lambdas, controle de fluxo, coleções,
aritmética/comparação, `data`/`enum`/`match`, opcionais e o prelude. Sem
`use py`/`intent` (recorte em `docs/adr/0012-alpha-scope.md`).

### Corrigido na estabilização

- Variante de enum sem payload construía no `check` e não no `run` (R0010).
- Variante com payload não tinha valor de construtor no runtime (ADR 0017).
- Tipo de usuário aninhado (`fn(..) -> E`, `List<E>`, `Map<_, E>`) resolvia como
  `Data` e acusava `expected E, found E`.
- Bloco não fechado era `E0102`; agora `E0103`.
- Atribuição a campo de `data` passava no `check` e falhava no `run`; agora é
  `E0231` (ADR 0018).
- Profundidade de chamada era `R0010`; agora `R0004`, com fronteira exata (48).

### Limitações conhecidas do alpha

- Interpolação `{expr}` em string imprime o texto literal (ADR 0018).
- Atribuir a campo de `data` (`u.age = 2`) é `E0231` (ADR 0018).
