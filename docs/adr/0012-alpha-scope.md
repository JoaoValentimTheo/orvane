# ADR 0012 — Recorte da 0.1.0-alpha

- **Status:** aceita
- **Data:** sprint `alpha-0.1.0`
- **Relacionada:** §12 (roadmap), §13 (DoD)

## Contexto

O roadmap de §12 vai de M0 a M12 (~14k LOC) e a ordem é por camada, não por
"programa executável". O objetivo desta sprint é diferente do roadmap: chegar a
uma **0.1.0-alpha em que `orv run arquivo.orv` execute um programa de verdade**
— funções, lambdas, controle de fluxo, coleções, aritmética/comparação,
interpolação e `print` — sem abrir mão do rigor de qualidade (sem
`unwrap`/`panic` fora de teste, proptest de "nunca panica" por camada, gate
verde em Linux e Windows).

Isso obriga a cortar §12 transversalmente: pegar a fatia de cada milestone que o
alpha precisa e adiar o resto.

## Decisão

**Subconjunto do §12 que entra no alpha**, na ordem em que será implementado:

| Ordem | Fatia | Vem de | Por quê |
|---|---|---|---|
| 1 | Parser completo de §5.2 (menos `intent/how/test/use py`) | M2 | sem AST completa não há o que executar |
| 2 | Sema mínima: nomes, tipos primitivos, `List`/`Map`, funções, lambdas, `as`, truthiness | M3 (parte) | o runtime precisa de `Value` tipado e de erros `E02xx`/`E03xx` |
| 3 | Runtime: `Value`, `fn`/closures, `let`/`let mut`, `if`/`while`/`for`, `return`, `fail`, `try`, `print`, prelude, `std.text`/`std.list`/`std.math` básicos | M4 (parte) | é o núcleo que faz `orv run` funcionar |
| 4 | `data`/`enum`/`match` + opcionais (`??`, `?.`) | M5 | §4.2 e `match` aparecem nos programas de referência; coleções e ADTs são o mínimo útil |
| 5 | `orv run`, `orv check`, `orv ast` | M2/M4/M10 (parte) | os três subcomandos que tornam o alpha verificável |
| 6 | Colisões: `orv fmt`/`repl`/`doctor` **não** entram | M9/M7 | fora do recorte |

**Fora do alpha (explicitamente):**
- **`use py`, `Py`, pyo3 inteiro** (M7) e o wheel Python (M8): o alpha é
  auto-contido, sem host Python.
- **`intent`/`how`/`given`/`ensure`/planner/`Trace`/`--explain`** (M6): é o
  coração do projeto, mas depende de um runtime já maduro; §12 diz "não apresse".
  O alpha **parseia** a sintaxe? Não: `intent`/`how` ficam fora do parser
  também, para não reservar semântica que não existe.
- **`test` e `orv test`**, **`use`/módulos multi-arquivo**, **generics** (M10).
- **`orv fmt`/`repl`/`doctor`** (M9, M7).
- **Bytecode VM** (M11).
- **Stdlib além do imprescindível:** só `print`, `len`, `range`, `str`, `int`,
  `float`, `assert` (prelude) e `std.text`/`std.list`/`std.math` no mínimo.
- **`--json`**, **`--explain`**, e `orv tokens` continua sendo debug interno.

**Regra de saída:** se uma fatia do roadmap conflitar com o recorte, o recorte
vence; se algo do recorte exigir sintaxe nova, para (não inventar).

## Consequência

- O alpha **não** demonstra a proposta da linguagem (`intent`), só a linguagem
  imperativa com coleções. A identidade fica para o M6, que é o próximo marco
  depois do alpha — e é isso que torna o alpha útil: valida lexer, parser, sema
  e runtime de ponta a ponta antes de investir no planner.
- `docs/STATUS.md` passa a distinguir "alpha" de "§12": features do recorte com
  status real, o resto "Planned (pós-alpha)".
- Cada fatia é um branch + PR contra `alpha-0.1.0`, com merge automático só com
  gate verde em Linux e Windows. `alpha-0.1.0` → `main` fica com o humano.
- O risco maior é a sema: é a fatia mais fácil de crescer sem limite (§5.3 é
  grande). Mitigação: só os tipos que os programas de referência exercitam, e
  erro explícito para o resto.
