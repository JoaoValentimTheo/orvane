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

Notas de comportamento (M2):

- `E0102` cobre "faltou algo": `)`, `]`, `}`, `,`, `:`, `=>`, `=`, um operando, um
  tipo, um nome. A recuperação sincroniza no próximo `Newline`/`}` e continua, de
  modo que **um erro por statement**, não uma cascata.
- `E0104` existe para que entrada patológica não dê estouro de pilha: o parser é
  recursive-descent e o limite é 256 níveis (ADR 0013).
- `E0105` é a recusa explícita do que está **fora do alpha** (ADR 0012):
  `intent`/`how`/`test`/`use py`/`pub data`/`pub enum`. A declaração inteira é
  pulada, então sai um diagnóstico por causa, não um por linha do corpo.

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
| `E0101` | token inesperado | `1` no nível de item | **M2** |
| `E0102` | esperado X, encontrado Y | `let = 1` · `1.2.3` · `f(1` | **M2** |
| `E0103` | bloco não fechado | `fn main() {` | M2 (via `E0102` hoje) |
| `E0104` | aninhamento profundo demais | `((((…1…))))` com 1000 níveis | **M2** |
| `E0105` | construção fora do alpha | `intent f() -> Int` · `use py math` · `pub data D` | **M2** |

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
| `E0301` | tipos incompatíveis | `let x: Int = "s"` · `"a" - "b"` | **M3** (alpha) |
| `E0302` | aridade incorreta | `f(1, 2)` com `f(a: Int)` · variant com nº errado de campos | **M3** (alpha) |
| `E0303` | não é chamável/iterável/indexável | `let x = 1` + `x()` · `x[0]` | **M3** (alpha) |
| `E0304` | campo inexistente | `u.age` onde `data User { name: Str }` | **M3** (alpha) |
| `E0305` | opcional não tratado | `let a: Int = opt_int` | M5 |
| `E0310` | lambda sem contexto de tipo | `let f = x => x` | **M3** (alpha) |
| `E0311` | cast inválido | `"s" as Int` | **M3** (alpha) |
| `E0312` | truthiness não existe | `if 1 {}` · `while "x" {}` | **M3** (alpha) |
| `E0313` | `fail` usado onde não é suportado | `fail "x"` como expressão | **M3** (alpha) |
| `E0320` | `match` não exaustivo | `match b { true => 1 }` | M5 |
| `E0321` | variante inexistente | padrão de variante de outro enum | **M3** (alpha) |

Notas de comportamento (M3, ADR 0012):

- **Um erro por causa.** Uma sub-expressão que falha vira o tipo `?` (Unknown),
  compatível com tudo, então `nope + 1 + 2` reporta **um** `E0201`, não três
  erros de tipo.
- `E0312` implementa §5.3 ("Truthiness: inexistente"): `if`/`while` exigem `Bool`.
- `E0311` só permite `Int as Float` (e o cast para o mesmo tipo); `Py as T` é do
  M7 e está fora do alpha.
- `E0321` vale quando o nome **é** uma variante declarada e o scrutinee não é
  aquele enum. Um nome de variante **escrito errado** liga em vez de errar —
  consequência inevitável de §5.2, registrada no ADR 0015.
- `E0201` cobre também tipo indefinido numa anotação (`x: Missing`).

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
| `R0001` | divisão por zero | `1 / 0` · `1 % 0` | **M4** (alpha) |
| `R0002` | overflow de inteiro | `9223372036854775807 + 1` | **M4** (alpha) |
| `R0003` | índice fora dos limites / chave ausente | `[1][5]` · `m["z"]` | **M4** (alpha) |
| `R0004` | recursão profunda demais | `fn f() { f() }` | **M4** (alpha) |
| `R0010` | operação não suportada no alpha | atribuir a campo de `data` | **M4** (alpha) |
| `R0020` | erro Python | exceção no host | M7 |

> M0 não introduz código de diagnóstico novo: apenas o tipo `Diagnostic`, o
> formato de renderização acima e sua versão legível com `ariadne`.
