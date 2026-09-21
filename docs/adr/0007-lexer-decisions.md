# ADR 0007 — Newline, interpolação e números: decisões do lexer

- **Status:** aceita
- **Data:** M1

## Contexto

§5.1 deixa alguns detalhes em aberto que o lexer precisa fixar. Cada um abaixo
foi escolhido pelo padrão mais simples que não perde informação, e todos estão
cobertos por teste.

## Decisões

### 1. Span do token `Newline`

`Newline` é um span **vazio** (zero bytes) na posição **depois** da quebra, não
sobre a quebra. Consequências:

- os spans continuam disjuntos e em ordem crescente (invariante do M1), o que
  seria falso se `Newline` cobrisse os bytes `\n` que também pertencem ao
  intervalo entre dois tokens;
- a coluna reportada é a primeira da linha seguinte, que é exatamente onde o
  parser retoma.

### 2. Sem `Newline` no início do arquivo

Linhas em branco no topo não produzem token. A regra é aplicada no momento de
emitir, não no lexer inteiro: um `Newline` só é suprimido enquanto **nenhum
token real** foi emitido.

### 3. Comentário de bloco com quebra de linha

Conta como **um** `Newline` (como no Go), emitido depois do `*/`. Um comentário
de bloco **sem** quebra de linha não emite `Newline` algum. Um comentário de
linha nunca emite: ele termina antes do `\n`, que segue o fluxo normal.

### 4. `#{` na pilha de delimitadores

`#{` empilha um contexto que **suprime** `Newline`, e é fechado por `}`. Como
`}` também fecha um bloco `{`, o fechamento casa com o topo da pilha quando ele
for `{` **ou** `#{`; fechar sem abertura correspondente apenas emite o token
(o parser reporta), nunca dá panic.

### 5. Interpolação: fonte verbatim e profundidade

- O `src` de `StrPart::Expr` é o texto **cru** entre `{` e `}`, incluindo
  espaços (`"{ {1: 2} }"` → `src = " {1: 2} "`). Preservar o texto permite que o
  parser re-lexe com spans corretos, sem reconstruir nada.
- Profundidade conta `{`/`}` aninhados; `{{`/`}}` são chaves literais.
- `\` dentro da interpolação escapa o próximo caractere, então `\"` é aspa
  literal e **não** abre uma string aninhada — é assim que `"{f(\"a\")}"`
  funciona. (Uma aspa crua dentro de uma string Orvane é impossível por
  construção.)
- `{}` vazio e `}` solto são `E0006`.

### 6. Números

- `_` só entre dígitos: nem inicial, nem final, nem duplicado. O valor é
  calculado sobre a cadeia **sem** separadores (`0x1_0` = 16), não sobre o
  slice cru.
- Um `.` inicia parte fracionária **apenas** se seguido de dígito. Logo
  `1..5` = `Int(1) DotDot Int(5)` e `1.foo` = `Int(1) Dot Ident(foo)`;
  `1.2.3` = `Float(1.2) Dot Int(3)`, que não é erro léxico.
- Expoente exige ao menos um dígito (`1e`, `1e+` são `E0005`).
- Literal decimal ou hex/bin fora de `i64` é `E0005`, sem wrap nem truncamento.

### 7. Recuperação de erro

Todo erro léxico consome o mínimo necessário e a varredura continua. Uma string
não terminada pode gerar um segundo `E0002` no restante da linha, porque o texto
seguinte realmente começa outra string; é cascade esperado, e o primeiro
diagnóstico aponta o erro real.

## Consequência

- O parser (M2) pode assumir: `Eof` sempre presente; spans disjuntos, ordenados
  e em fronteira de char; `Newline` significativo apenas onde §5.1 manda.
- `1.2.3` ser válido léxico empurra a decisão para o parser, que é onde ela
  pertence (não existe sintaxe léxica de "número com dois pontos").
