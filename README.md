# Orvane

**Diga o que precisa acontecer. Orvane escolhe como.**

Orvane é uma linguagem orientada a **intenção**: você declara *o quê* se quer
(assinatura + pré-condições `given` + garantias `ensure`), bibliotecas declaram
*como* cumprir (`how`, com guarda `when` e `priority`), e um planner
determinístico (sem LLM) escolhe, executa, verifica as garantias e **cai para a
próxima estratégia** quando a escolhida falha. Toda decisão é explicável.

Interoperabilidade com Python é bidirecional: Orvane importa qualquer pacote do
PyPI (`use py numpy as np`) e pode ser chamado de Python (`import orvane`).

- **CLI:** `orv` · **extensão:** `.orv` · **módulo Python:** `orvane`
- Especificação completa: [`docs/SPEC.md`](docs/SPEC.md)

## Status: 0.1.0-alpha (em `alpha-0.1.0`)

A alpha executa programas de ponta a ponta:

```console
$ orv run examples/fizzbuzz.orv
1
2
Fizz
...
FizzBuzz

$ orv check examples/data.orv   # sem executar
$ orv ast examples/shapes.orv   # dump da AST
```

**O que funciona:** funções e closures, `let`/`let mut`, `if`/`while`/`for`,
`break`/`continue`, listas (`[]`) e mapas (`#{}`), tuplas, aritmética e
comparação, `data`/`enum`/`match`, `try`, `as`, o prelude de §5.6
(`print`, `len`, `range`, `str`, `int`, `float`, `assert`) e erros de runtime
`R0001`–`R0004` como diagnóstico com exit 1.

**O que ainda não existe:** `intent`/`how` (o núcleo da proposta, M6),
interoperabilidade com Python (`use py`, M7), `orv test`/`fmt`/`repl`,
módulos multi-arquivo e generics. O recorte está no
[ADR 0012](docs/adr/0012-alpha-scope.md).

> **Dois recursos entregues na 0.1.2:**
> 1. **Interpolação `{expr}` avaliada**: `print("n = {len(xs)}")` imprime o
>    valor. `"{x}"` produz o mesmo texto que `str(x)`
>    ([ADR 0022](docs/adr/0022-data-field-mutation.md)).
> 2. **Mutação de campo de `data`**: `let mut u = User(..); u.age = 2`
>    funciona, com **semântica de valor** — `let b = a` copia, então mutar uma
>    cópia não afeta a original. Campo de `data` imutável é `E0230`; campo
>    opcional (`u?.x = 1`) e receptor aninhado (`a.b.c = 1`) seguem `E0231`
>    ([ADR 0022](docs/adr/0022-data-field-mutation.md)).

Nada aqui é "1.0": a alpha valida lexer, parser, sema e runtime de ponta a
ponta antes de investir no planner. O detalhe feature-a-feature (com o teste
que exercita cada uma) está em [`docs/STATUS.md`](docs/STATUS.md); "Implemented"
só aparece quando há teste golden.

`examples/` tem seis programas executáveis; todos são também goldens
(`tests/golden/example_*.orv`).

## Uso

```console
$ cargo run -p orv-cli -- version
orv 0.1.0 (orvane 0.1.0)

$ cargo run -p orv-cli -- run examples/fib.orv
0
1
1
2
...
```

## Rodando o gate

```console
$ cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```

Em CI, os mesmos comandos rodam com `--locked` em Linux e Windows
(`.github/workflows/ci.yml`).

## Licença

Licenciado sob MIT **ou** Apache-2.0, à sua escolha
([`LICENSE-MIT`](LICENSE-MIT) / [`LICENSE-APACHE`](LICENSE-APACHE)).
