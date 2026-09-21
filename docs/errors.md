# Códigos de diagnóstico

Formato estável para golden tests (§8.1):

```
CODE:linha:coluna: mensagem
```

`linha` e `coluna` são 1-based e derivados do **início do span primário**. Uma
linha por diagnóstico, na ordem em que foram emitidos.

Todo código novo **deve** ser adicionado aqui com exemplo mínimo (§8.2).

## E00xx — léxico

| Código | Mensagem | Exemplo mínimo | Status |
|---|---|---|---|
| `E0001` | caractere inválido | `§` · `@` · `!` (sozinho) · identificador não-ASCII | **M1** |
| `E0002` | string não terminada | `"abc` · `"abc<newline>` · `"{a` | **M1** |
| `E0003` | comentário não fechado | `/* abc` | **M1** |
| `E0004` | escape inválido | `"a\q"` · `"a\<quebra de linha>` | **M1** |
| `E0005` | literal numérico inválido | `0x` · `0b` · `1__0` · `1_` · `1e` · `9223372036854775808` · `1e999` | **M1** |
| `E0006` | interpolação inválida | `"a}b"` · `"{}"` · `"{f(\"a\")}"` | **M1** |

Notas de comportamento (M1):

- `E0001`: o caractere inválido é consumido e o lexer **continua**; `!` só é
  válido como `!=`. Caractere não-ASCII fora de string/comentário leva o help
  "identifiers are ASCII-only in v0.1". **Só o primeiro BOM é ignorado** (por
  `SourceMap::add`); um segundo BOM é `E0001`.
- `E0002`: uma quebra de linha crua dentro da string também é `E0002`; use `\n`.
  Uma interpolação não fechada gera **um único** `E0002`, o mais externo.
- `E0003`: comentários de bloco são aninháveis; um `/*` sem `*/` consome o resto
  do arquivo e é reportado uma única vez.
- `E0004`: a mensagem usa `escape_debug`, então nunca contém caractere de
  controle cru. `\` seguido de **quebra de linha** é `E0004` com span apenas
  sobre a `\`, e a quebra **não** é consumida (segue valendo como fim de linha)
  — o mesmo vale dentro de `{...}` e de string aninhada (ADR 0009), de modo que
  LF, CRLF e CR produzem os mesmos diagnósticos.
- `E0005`: a mensagem não distingue os casos; o `help` diz o motivo
  (`_` fora de dígitos, prefixo sem dígitos, literal fora de `i64`, ou
  `float literal out of range` para `1e309`/`1e999`).
- `E0006`: `}` sem `{` (help "use }}"), interpolação vazia `{}`, e `\` dentro de
  `{...}` (help: escreva `{f("a")}` sem escape) — **um** diagnóstico por
  interpolação, não um por barra.

**Tokens de melhor esforço (ADR 0008).** Todo erro léxico emite, além do
diagnóstico, um token que cobre o trecho lido: `Str(parts)` com as partes já
lidas para `E0002`/`E0004`/`E0006`, e `Int(0)`/`Float(0.0)` para `E0005`. A
stream continua cobrindo o arquivo; a validade é decidida pelos diagnósticos.

### Ordem e supressão de diagnósticos (ADR 0008, emenda do M2)

Quando há erro léxico, o programa **não** para no primeiro caractere ruim:

1. diagnósticos léxicos são emitidos **primeiro**;
2. o parser roda mesmo assim, mas um diagnóstico do parser cujo span primário
   **intersecta** o de um diagnóstico léxico é **suprimido** (a construção veio
   de um token de melhor esforço);
3. `StrPart::Expr.src` de um `Str` que contém diagnóstico léxico **não** é
   re-parseado;
4. com qualquer erro — léxico ou sintático — `orv check` não roda sema e
   `orv run` não executa.

## E01xx — parser

| Código | Mensagem | Exemplo mínimo | Status |
|---|---|---|---|
| `E0101` | token inesperado | `fn () {}` | M2 |
| `E0102` | esperado X, encontrado Y | `let = 1` | M2 |
| `E0103` | bloco não fechado | `fn main() {` | M2 |

## E02xx — resolução

| Código | Mensagem | Exemplo mínimo | Status |
|---|---|---|---|
| `E0201` | nome indefinido | `print(x)` | M3 |
| `E0202` | definição duplicada | `fn a() {} fn a() {}` | M3 |
| `E0230` | atribuição a imutável | `let x = 1` + `x = 2` | M3 |
| `E0250` | ciclo de import | `a.orv` ↔ `b.orv` | M10 |
| `E0251` | item não `pub` importado | `use util` com `fn helper()` | M10 |

## E03xx — tipos

| Código | Mensagem | Exemplo mínimo | Status |
|---|---|---|---|
| `E0301` | tipos incompatíveis | `let x: Int = "s"` | M3 |
| `E0302` | aridade incorreta | `f(1, 2)` com `f(a: Int)` | M3 |
| `E0305` | opcional não tratado | `let a: Int = opt_int` | M5 |
| `E0310` | lambda sem contexto de tipo | `let f = x => x` | M3 |
| `E0311` | cast inválido | `"s" as Int` | M3 |
| `E0312` | truthiness não existe | `if py_value {}` | M7 |
| `E0320` | `match` não exaustivo | `match b { true => 1 }` | M5 |

## E04xx — intents

| Código | Mensagem | Exemplo mínimo | Status |
|---|---|---|---|
| `E0401` | intent sem strategies nem corpo | `intent f() -> Int` | M6 |
| `E0402` | `how` referencia intent inexistente | `how g() {}` | M6 |
| `E0403` | aridade do `how` difere da intent | `how f(a, b) {}` p/ `intent f(a)` | M6 |
| `E0404` | `given`/`ensure`/`when` não é `Bool` | `given 1` | M6 |
| `E0405` | `via` duplicado | dois `how f() via x` | M6 |
| `E0406` | `result` fora de `ensure` | `fn f() { print(result) }` | M6 |
| `E0410` | intent chamada em `given`/`ensure`/`when` | `ensure g() > 0` | M6 |
| `E0411` | corpo do `how` retorna tipo ≠ intent | `how f() -> Int` retorna `Str` | M6 |

## E05xx — interop

| Código | Mensagem | Exemplo mínimo | Status |
|---|---|---|---|
| `E0501` | `use py` fora do topo | `fn f() { use py math }` | M7 |
| `E0502` | `Py` onde tipo concreto é exigido | `let x: Int = np.pi` | M7 |

## Rxxxx — runtime

| Código | Mensagem | Exemplo mínimo | Status |
|---|---|---|---|
| `R0001` | divisão por zero | `1 / 0` | M4 |
| `R0002` | overflow de inteiro | `i64::MAX + 1` | M4 |
| `R0003` | índice fora dos limites | `[1][5]` | M4 |
| `R0004` | estouro de pilha | intent recursiva | M6 |
| `R0010` | intent insatisfeita | todas as strategies falham | M6 |
| `R0020` | erro Python | exceção no host | M7 |

> M0 não introduz código de diagnóstico novo: apenas o tipo `Diagnostic`, o
> formato de renderização acima e sua versão legível com `ariadne`.
