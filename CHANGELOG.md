# Changelog

Todas as mudanças relevantes por versão. O formato segue "Keep a Changelog"
(https://keepachangelog.com/pt-BR/1.1.0/) e o versionamento é o de `Cargo.toml`.

## [0.1.3] - 2026-09-23

Sprint `fix-0.1.3`: correção de bugs na superfície introduzida pela 0.1.2
(interpolação avaliada + mutação de campo de `data`). Sem feature nova.

### Corrigido

- **P0 — interpolação profundamente aninhada estourava a pilha nativa.**
  O sub-parse de `{expr}` criava um `Parser` novo com `depth: 0`, então o
  limite de recursão `E0104` (ADR 0013) não contava os níveis de interpolação
  aninhada (`"{"{"{...}"}"}"`) e milhares de níveis abortavam o processo com
  *stack overflow* (SIGABRT), não um diagnóstico. O sub-parse agora **compartilha**
  o orçamento de profundidade do parser externo e reporta `E0104`.
  Regressão: `deeply_nested_interpolation_reports_e0104_instead_of_overflowing`,
  o proptest `nested_interpolation_never_overflows` e a estrutura no
  `fuzz-bytes`.

### Notas

- A auditoria da superfície nova (itens b–e) **não** encontrou mais bugs:
  custo O(n) para N interpolações sequenciais, igualdade estrutural de `data`
  independente (inclusive mutar/desmutar), mutação cruzando as fronteiras de
  lambda/variante/`break` e span/código de erro vindos de dentro da
  interpolação estão todos cobertos por teste novo (ver
  `docs/coverage-matrix.md`, seção "Auditoria fix-0.1.3").
- Nenhuma issue aberta no repositório no início da sprint (mesma situação da
  0.1.1); o P0 veio da auditoria dirigida, não de reporte de usuário.

## [0.1.2] - 2026-09-23

Sprint `feat-0.1.2-interpolation-and-mutation`: duas features, sem correção de
bug avulsa. Cada feature tem teste de aceitação e golden.

### Adicionado

- **Interpolação de string avaliada.** `"{expr}"` produz o mesmo texto que
  `print(expr)`, em todo tipo de valor (Int, Float, Str, Bool, enum, `data`,
  List, Map, none), porque reusa `value::display`. O parser faz o sub-parse de
  cada `Expr.src` com spans deslocados (SPEC §5.1) e a sema type-checa a
  expressão no escopo onde a string aparece; uma referência indefinida dentro
  de `{}` dá o mesmo `E0201` de fora. A guarda do ADR 0008 (não sub-parsear
  quando o token tem erro léxico) continua valendo. Antes, `{expr}` era
  impresso literalmente (ADR 0018, item 1, agora superado).
- **Mutação de campo de `data`.** `let mut u = User(...); u.age = 2` funciona.
  `data` passa a ter **semântica de valor**: `let b = a` copia, então mutar uma
  cópia não afeta a original (ADR 0022). `Value::Data` guarda os campos atrás
  de `RefCell` e `Value` tem `Clone` manual. Campo de `data` imutável é
  `E0230`; campo opcional (`u?.x = 1`) e receptor aninhado (`a.b.c = 1`) seguem
  `E0231`.

### Notas

- O ADR 0018 fica superado nos dois itens que documentava.
- `docs/errors.md`: `E0230` passa a cobrir escrita em campo de `data` imutável;
  `E0231` fica restrito a campo opcional/receptor aninhado.
- Sem mudança de comportamento em igualdade estrutural, `Display` ou no
  formato do dump de AST (ADR 0014).

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
