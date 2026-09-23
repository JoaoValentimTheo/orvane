# ADR 0011 — `Float Dot Int` (`1.2.3`) é erro sintático

- **Status:** aceita
- **Data:** M2
- **Relacionada:** ADR 0007 §6 (léxico de números), ADR 0008

## Contexto

O lexer de §5.1 faz maximal munch em números: `1.2.3` vira `Float(1.2) Dot Int(3)`,
porque o primeiro `.` inicia a fração (`1.2`) e o segundo já não pertence ao
número. Isso é intencional e está fixado no ADR 0007 §6; o ADR deixou explícito
que **a decisão sobre rejeitar ou não pertence ao parser**.

A pergunta: o parser deve rejeitar essa sequência (`E010x`) ou aceitá-la
sintaticamente para a sema reclamar depois?

## Decisão

**Opção A — erro sintático.** O parser rejeita com o código já reservado
`E0102` ("esperado X, encontrado Y").

Não é preciso inventar regra nova: a gramática de §5.2 já decide. Em

```
postfix = primary { call | index | field | "?." IDENT } ;
field   = "." IDENT ;
```

o `field` exige `IDENT` depois do `.`. Em `1.2.3` o token seguinte é `Int(3)`,
que não é `IDENT`, então a sequência **não é derivável** pela gramática. Rejeitar
é a leitura literal de §5.2; aceitar exigiria *acrescentar* uma produção que a
spec não tem.

Mensagem e código: `E0102` com mensagem na forma "expected identifier after `.`,
found `Int`" (o `E0102` já é o código para "esperado X, encontrado Y").

### Por que não a opção B

Aceitar `1.2.3` para a sema rejeitar depois tem três custos e nenhum ganho:

1. **A sema não teria o que dizer.** `Float(1.2) . Ident` nem chega a ser uma
   expressão bem tipada — não existe tipo com campo `3`. A sema teria de
   reintroduzir a mesma checagem ("campo de um literal numérico"), só que mais
   tarde e com menos contexto.
2. **A mensagem ficaria pior.** Na sema, o erro mais provável seria "tipo X não
   tem campo `3`", que não menciona o problema real (um ponto a mais no número).
   No parser, `expected identifier after `.`` aponta exatamente o que falta.
3. **A recuperação fica mais simples.** O parser já sincroniza em `Newline`, e a
   sequência continua a mesma: `1.2.3` termina a expressão no `Int(3)`. Nada de
   especial a fazer.

Também não é opção C (por exemplo, reinterpretar `1.2.3` como número inválido ou
como `(1.2).3`): reinterpretar seria invenção de sintaxe (§0.2 regra 1), e o
lexer não pode reportar erro porque `Float(1.2) Dot Int(3)` é léxico válido por
§5.1.

## Consequência

- `1.foo` (Int `Dot` Ident) e `1..5` (Int `DotDot` Int) **não** são afetados:
  ambos já seguem a gramática. Só `Float Dot <não-IDENT>` é erro, e só pelo
  motivo geral "`field` exige `IDENT`".
- Nenhum código novo de diagnóstico. `E0102` ganha `1.2.3` como exemplo em
  `docs/errors.md`.
- O teste `float_dot_int_is_a_syntax_error_by_grammar` pina a intenção: o lexer
  continua emitindo `Float Dot Int` (é a entrada que o parser vai rejeitar), de
  modo que a decisão não seja "consertada" no lexer.
