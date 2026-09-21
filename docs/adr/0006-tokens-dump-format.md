# ADR 0006 — Formato de `orv tokens`

- **Status:** aceita
- **Data:** M1

## Contexto

O gate do M1 é `orv tokens x.orv` dumpando tokens de forma estável (SPEC §12).
O subcomando é de debug, não conta no orçamento de 8 subcomandos de §10 e será
removido no M12. Ainda assim, goldens comparam sua saída byte a byte, então o
formato precisa ser especificado, não incidental.

## Decisão

Uma linha por token, incluindo o `Eof` final:

```
LINHA:COLUNA Kind(payload)
```

- `LINHA` é 1-based; `COLUNA` é 1-based **em caracteres**, como em `.err`
  (ADR 0001), e aponta para o **início** do span do token.
- Rendering por kind (fixo, sem espaços extras):

| Kind | Saída |
|---|---|
| `Int(42)` | `Int(42)` |
| `Float(1.0)` | `Float(1.0)` (sempre com `.0` ou expoente) |
| `Str` | `Str[Lit("a"), Expr("b + 1")]`; string vazia é `Str[]` |
| `Ident(main)` | `Ident(main)` |
| keyword | `Kw(fn)` |
| `Newline` / `Eof` | `Newline` / `Eof` |
| operador/pontuação | o nome do variante: `LParen`, `DotDotEq`, `HashLBrace`, … |

- Dentro de `Lit(...)`/`Expr(...)` a string é entre aspas duplas com escapes
  mínimos (`\"`, `\\`, `\n`, `\t`, `\r`), para caber em uma linha.
- O subcomando **não** aparece em `--help` (`#[command(hide = true)]`), já que é
  ferramenta interna até o M12.

## Consequência

- Golden tests pinam exatamente o que o lexer produz, com `Eof` explícito.
- Tokens são impressos em stdout **mesmo quando há diagnósticos**; os
  diagnósticos vão para stderr e o exit code é `1`. Assim um caso golden pode
  afirmar os dois streams de uma vez.
- Em um terminal, os diagnósticos usam o rendering legível do `ariadne`; quando
  stderr **não** é um terminal (CI, redirecionamento), sai o formato compacto
  `CODE:linha:coluna: mensagem` de §8.1, que é o que os `.err` contêm (ADR 0004).
  Sem isso, o harness não reconheceria diagnóstico algum e a regra "sem `.err`
  ⇒ zero diagnósticos" passaria por acidente.
- Mudar o formato é uma edição deliberada e visível em todos os goldens.
