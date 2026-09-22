# ADR 0014 — Formato do dump da AST (`orv ast`)

- **Status:** aceita
- **Data:** M2 (alpha)

## Contexto

§10 dá a `orv ast` a função de "debug: dump da AST (texto estável)" e §12 M2
pede goldens de AST. Como no `orv tokens` (ADR 0006), o texto é comparado byte a
byte por golden, então precisa ser especificado, não incidental.

## Decisão

`ast_dump::dump_program` renderiza o programa assim:

- um nó por linha, indentado com **dois espaços** por nível;
- a primeira palavra da linha é o tipo do nó (`Fn`, `Block`, `Let`, `Binary`, …);
- os campos que identificam o nó vêm em seguida, na mesma linha quando são só
  um nome ou operador (`Fn main`, `Binary +`, `Field name`);
- sub-nós vão em linhas mais indentadas, na ordem do fonte;
- **spans não aparecem.** O dump é sobre estrutura; posições já são testadas em
  unidades específicas e incluí-las tornaria cada golden sensível a qualquer
  edição de linha em branco no arquivo de entrada;
- literais: `Int(42)`, `Float(1.0)`, `Str` com as partes
  (`"a" {x} "b"`), `true`/`false`/`none` como texto simples.

Exemplo:

```
Fn main
  Block
    Let x =
      Literal 42
    Binary +
      Ident x
      Literal 1
```

## Consequência

- Goldens de AST são legíveis e estáveis; mudar o dump é uma edição deliberada e
  visível em todos eles.
- Perder spans no dump significa que um bug de span não é pego por golden — é
  coberto por testes de unidade que afirmam `(start, end)` diretamente (como já
  acontece no lexer e nos testes de expressão do parser).
- `orv ast` é debug interno; como `orv tokens`, não conta no orçamento de 8
  subcomandos de §10 e será reavaliado no M12 (§12: "remover `orv tokens`").
