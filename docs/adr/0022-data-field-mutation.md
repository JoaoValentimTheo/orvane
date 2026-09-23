# ADR 0022 — Mutação de campo de `data` e semântica de valor

- **Status:** aceita
- **Data:** sprint `feat-0.1.2-interpolation-and-mutation`
- **Relacionada:** ADR 0018 (limitação que esta ADR remove), ADR 0012 (recorte)

## Contexto

O ADR 0018 registrou que `u.age = 2` era **recusado** na 0.1.0 com `E0231`,
porque `Value::Data` era imutável atrás de `Rc<Vec<..>>`. A 0.1.2 traz a
mutação de campo como feature.

Duas perguntas de semântica precisavam de decisão, e o SPEC §5.3 é omisso nas
duas:

1. `let b = a; b.x = 1` muda `a.x`? Ou seja, `data` tem semântica de
   **referência** ou de **valor**?
2. Que erro sai ao escrever num campo de um `data` ligado a um `let` imutável?

## Decisão

### 1. `data` tem semântica de VALOR

`data` é uma struct simples (§4.2: "dados sem cerimônia", construída por
chamada, com `==` e `Display` automáticos). A leitura mais natural de uma
struct é **cópia na atribuição**: `let b = a` cria um valor independente.

Consequência: `let mut a = User(...); let b = a; a.age = 99` deixa `b.age`
inalterado. O teste `data_has_value_semantics_on_assignment` fixa isso — não
como detalhe implícito, mas como contrato.

### 2. Escrita em campo exige binding mutável

`u.age = 2` é uma **mutação de valor** (reconstrói efetivamente o `data` com um
campo trocado), então a mesma regra de reatribuição de variável vale: a
variável precisa ser `let mut`.

- `let mut u = User(...); u.age = 2` → aceito.
- `let u = User(...); u.age = 2` → **`E0230`** ("cannot assign to a field of
  immutable `u`"), coerente com `x = 2` em `let x`.
- `u?.campo = x` (campo opcional) → continua `E0231`.
- `a.b.c = 1` (receptor aninhado) → `E0231`; só um binding direto é um *place*
  que o runtime sabe reescrever (ver "Consequências").

### 3. Implementação

- `Value::Data.fields` passa de `Rc<Vec<(Rc<str>, Value)>>` para
  `Rc<RefCell<Vec<(Rc<str>, Value)>>>`, permitindo reescrever um campo.
- `Value` passa a ter `Clone` **manual**: `data` faz cópia profunda dos campos
  (dá a semântica de valor do item 1), enquanto lista/mapa/tupla/`enum`
  continuam compartilhando o `Rc` como antes.
- `assign` a `ExprKind::Field` com receptor `Ident`: lê o valor, troca o campo
  em uma cópia e grava de volta via `Env::assign` (que respeita `mut`.

## Consequências

- `E0231` deixa de cobrir campo de `data` mutável; continua para campo
  opcional e receptor aninhado. `docs/errors.md` e o aviso do topo de
  `docs/STATUS.md`/`README.md` foram atualizados (a limitação "interpolação
  literal" sai junto, pela Feature 1).
- Igualdade estrutural, `Display` e a interpolação da Feature 1 continuam
  idênticas: eles leem `fields.borrow()` e não observam o `RefCell`.
- **Custo**: `Value::Data` ganha um `RefCell`; uma cópia de `data` é profunda.
  Aceitável para a 0.1.2 e coerente com semântica de valor.
- **Limite**: mutação de campo aninhado (`a.b.c = 1`) e mutação através de um
  elemento de lista/mapa (`list[0].x = 1`) não são suportadas — precisariam de
  um modelo de *place* recursivo. São `E0231` (nested) ou fora da gramática de
  `lvalue` da 0.1. Escopo de milestone futuro, não bug.
- Regressão: `mutating_a_data_field_is_visible_on_the_binding`,
  `data_has_value_semantics_on_assignment`,
  `interpolating_a_mutated_field_shows_the_new_value`,
  `a_field_can_be_mutated_inside_a_loop`,
  `a_data_value_can_be_captured_and_mutated_by_a_closure_binding` (runtime);
  `assigning_to_a_field_of_a_mutable_binding_is_allowed`,
  `assigning_to_a_field_requires_a_mutable_binding`,
  `assigning_to_a_nested_field_is_rejected` (sema);
  `cloning_a_data_makes_an_independent_copy` (value).
