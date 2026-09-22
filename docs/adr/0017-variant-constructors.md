# ADR 0017 — Construtores de variante são valores de primeira classe

- **Status:** aceita
- **Data:** sprint `fix/0.1.0-stabilization`
- **Relacionada:** ADR 0012 (escopo do alpha), ADR 0015 (padrões)

## Contexto

A auditoria de estabilização encontrou uma assimetria sema↔runtime:

- `check` aceita `let ctor = Circle`, onde `enum Shape { Circle(Float) }`. A sema
  tipa o nome nu como `Ty::Fn { params: [Float], ret: Shape }`, porque "um
  variant com payload se comporta como uma função do payload para o enum".
- `run` **não sabia produzir esse valor**: `eval_ident` só conhecia bindings
  locais e `fn` declaradas, então caía em `R0010 "undefined name"`.

O mesmo valia para passar `Circle` como argumento de uma função de ordem
superior. Ou seja: a sema prometia um tipo que o runtime não tinha como
materializar.

O caso irmão (variante **sem** payload, `Active`) tinha o mesmo defeito e foi
corrigido junto: o runtime passou a construir `Value::Variant` a partir do
schema de variantes.

## Decisão

**Variantes são valores de primeira classe, e o runtime materializa os dois
casos:**

- variante **sem payload**: o nome nu é o valor (`Active` → `Value::Variant`
  com payload vazio), sem passar por chamada.
- variante **com payload**: o nome nu é uma **closure construtora** de aridade
  igual ao número de campos, do tipo `fn(payload) -> Enum`. Chamá-la produz o
  `Value::Variant` com os argumentos como payload.

É a opção (a) da auditoria. Foi escolhida por ser **pequena e local**: um
variante novo de `Callable` (`Callable::Constructor`), um ramo em
`eval_ident`, um em `call_function` e um em `call_env`. Não há currying,
aplicação parcial, nem sistema de closures novo — a aridade é fixa e a chamada
exige exatamente os argumentos.

## Consequência

- Sema e runtime concordam: `let ctor = Circle`, `print(ctor(1.0))` e
  `apply(Circle)` funcionam, e a mensagem `expected `fn(Float) -> Shape`,
  found `fn(Float) -> Shape`` (que aparecia por um bug de resolução de tipo
  aninhado, corrigido nesta sprint) deixa de existir.
- Igualdade e `Display` de um valor construído pela closure são idênticos aos da
  construção direta, porque ambos produzem o mesmo `Value::Variant`.
- Um variant com payload usado sem chamada **não** é mais um erro de runtime:
  vira um valor de função. Se o programa tentar usá-lo onde um `Shape` é
  esperado, o erro é de tipo, na sema, como para qualquer outra função.
- `Callable::Constructor` não captura escopo (`call_env` devolve o global), o
  que é correto: o construtor não referencia nada do local de uso.
- Cobertura: `runs_a_unit_only_enum`,
  `runs_unit_variants_as_values_and_in_collections`,
  `a_payload_variant_is_a_first_class_constructor`,
  `a_constructor_can_be_passed_to_a_higher_order_function`,
  `a_constructor_equality_and_display_match_the_direct_form`.
