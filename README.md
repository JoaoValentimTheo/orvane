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

## Status: pré-alfa — M1 concluído

O M0 entregou a fundação (workspace, `Span`, `SourceMap`, `Diagnostic`, harness
golden, CI). O M1 entrega o **lexer** de §5.1: tokens, strings com interpolação,
comentários aninhados, a regra de `Newline` por pilha de delimitadores e os
diagnósticos `E0001`–`E0006`.

**Ainda não há linguagem executável**: parser, sema e interpretador são M2+.
Os subcomandos funcionais são `orv version` e o interno `orv tokens <f.orv>`.

O que está pronto e testado está em [`docs/STATUS.md`](docs/STATUS.md); a
tabela usa "Implemented" só quando há teste golden que exercita a feature.

## Uso

```console
$ cargo run -p orv-cli -- version
orv 0.1.0 (orvane 0.1.0)

$ cargo run -p orv-cli -- tokens tests/golden/tokens_block_lambda.orv
4:1 Ident(xs)
4:3 Dot
4:4 Ident(map)
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
