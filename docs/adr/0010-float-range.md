# ADR 0010 — Float fora da faixa é erro léxico

- **Status:** aceita
- **Data:** M1.2

## Contexto

`1e999` é sintaticamente um float válido, mas não tem representação finita em
`f64`: `"1e999".parse::<f64>()` devolve `Ok(f64::INFINITY)`. Aceitá-lo faria o
programa carregar um `Float(inf)` silencioso, e o dump deixa de ser legível
(`inf.0`). Um literal fora da faixa é manifestamente um engano de quem escreveu,
não um valor que o programa quis produzir.

## Decisão

- Um literal de float cujo valor não é **finito** é `E0005` com help
  `float literal out of range`, e o token de melhor esforço é `Float(0.0)`
  (ADR 0008 A).
- O valor finito mais próximo ainda é aceito: `1e308` passa, `1e309` não.
- O mesmo vale para `NaN` (não escrevível como literal) por consequência da
  regra de finitude.
- `dump::float_text` renderiza não-finitos como `inf`/`-inf`/`NaN` caso algum
  apareça por outro caminho (placeholder, `--json` futuro), em vez de `inf.0`.

## Consequência

- Nenhum `Float` na stream é infinito, o que simplifica o interpretador do M4
  (comparações, `Display`, serialização) e evita propagar um valor que o usuário
  não pediu.
- Um programa que dependa de "estouro silencioso para infinito" não é
  expressável; se isso for necessário, o caminho é uma operação explícita em
  runtime (divisão por zero, por exemplo), que já é `Failure` (§5.3).
- Cobertura: `float_out_of_range_reports_e0005_with_a_float_placeholder`,
  `largest_finite_float_is_still_accepted`,
  `renders_non_finite_floats_explicitly`,
  `out_of_range_float_dumps_as_the_placeholder`; golden
  `tests/golden/tokens_err_float_range.orv`.
