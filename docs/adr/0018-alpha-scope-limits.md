# ADR 0018 — Limitações de escopo da 0.1.0-alpha, documentadas retroativamente

- **Status:** aceita
- **Data:** sprint `fix/0.1.0-stabilization`
- **Relacionada:** ADR 0012 (recorte do alpha), ADR 0017 (variantes)

## Contexto

A auditoria de estabilização pediu confirmação de que dois comportamentos
**já existentes** estavam registrados como decisão, não só como acidente de
implementação:

1. **Interpolação `{expr}` em string não é avaliada.** `print("n = {len(xs)}")`
   imprime o texto literal `n = {len(xs)}`. O lexer produz os `StrPart::Expr`
   com span correto, mas nem a sema nem o runtime os avaliam.
2. **Atribuir a campo de `data` não é suportado.** `u.age = 2` não tem caminho
   no runtime: `Value::Data` é imutável atrás do `Rc<Vec<..>>`.

Ao reler o **ADR 0012**, nenhum dos dois estava listado. Isso significa que a
decisão nunca foi registrada — o comportamento existe por consequência do
recorte, não por escolha escrita. Esta ADR corrige isso, **documentando o que já
existe**; não muda comportamento nem implementa feature.

## Decisão

### 1. Interpolação de string fica fora da 0.1.0

`StrPart::Expr` é produzido pelo lexer (§5.1) e preservado na AST, mas não é
avaliado em 0.1.0: as partes `Lit` são concatenadas e as `Expr` são mantidas
verbatim, incluindo as chaves. A interpolação **avaliada** é trabalho de 0.1.2.

Justificativa: avaliar `{expr}` exige sub-parse do texto (o parser tem o `src` e
o span prontos), checagem de tipo do resultado e conversão para `Str` — três
camadas, não um ajuste. É feature, não correção.

### 2. Atribuição a campo de `data` fica fora da 0.1.0

`u.age = 2` é **erro de sema** (`E0231`), com a mensagem "assigning to field
`age` is not supported in 0.1.0-alpha; rebuild the value instead".

Justificativa: `Value::Data` é imutável atrás do `Rc`, e mutar exigiria ou
`RefCell` por campo (custo em todo acesso) ou um modelo de lugar (place)
explícito que o runtime ainda não tem. É feature.

**Nota de correção (esta sprint):** antes, a sema *aceitava* `u.age = 2` e o
runtime falhava com `R0010` em execução. Isso era uma divergência sema↔runtime —
a mesma classe do bug das variantes de enum (ADR 0017) — então aqui a correção
**é** de estabilização: a sema passou a recusar. A feature continua fora.

### 3. O que permanece suportado

- `xs[i] = v` (lista) e `m[k] = v` (mapa): mutação de elemento, suportada pelo
  runtime e aceita pela sema.
- `let mut` e reatribuição de variável, com operadores compostos.
- `d?.campo` (leitura de campo opcional) e `d.campo` (leitura): só escrita de
  campo é recusada.

## Consequência

- O README e o topo de `docs/STATUS.md` avisam sobre a interpolação literal.
- `E0231` entra em `docs/errors.md`; o gate quanto a sema↔runtime fica coberto
  por `assigning_to_a_field_is_rejected_as_out_of_scope` e pelos probes de
  auditoria que comparam `check` e `run` para as mesmas construções.
- Quem precisar mudar um campo na 0.1.0 reconstrói o valor:
  `let u2 = User(name: u.name, age: 2)`.
