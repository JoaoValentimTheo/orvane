# ADR 0020 — Nomes de variante são únicos entre enums

- **Status:** aceita
- **Data:** sprint `fix-0.1.1`
- **Relacionada:** ADR 0015 (resolução de `IDENT` em padrões), ADR 0017 (variantes)

## Contexto

A resolução de nomes de variante é **global**: um `IDENT` como `V` é uma variante
se **algum** `enum` declarado tem uma variante `V` (ADR 0015). Nada impedia dois
enums de declararem o mesmo nome:

```orv
enum A { V(Int) }
enum B { V(Str) }
```

Com isso, `V` tinha dois donos possíveis. A sema (`find_variant`) iterava
`user_types`, um `HashMap`, e escolhia por **ordem de hash** — não determinístico
e proibido por §3.4. O runtime (`variant_schemas`) registrava por nome e a
**última** declaração vencia. Resultado: a sema dizia `V: fn(Str) -> B` e o
runtime registrava `V: fn(Int) -> A` (ou vice-versa, dependendo do hash), então

```orv
let b = V("hello")
```

passava no `check` e falhava no `run` com `E0301 expected Int, found Str` — a
classe sema↔runtime (mesma de ADR 0017/0019) mais uma violação de determinismo.

## Decisão

**O nome de uma variante é único entre todos os enums do programa.** Uma segunda
declaração do mesmo nome é `E0202` (definição duplicada), com a mensagem
apontando o enum que já o declarou.

É a única regra simples que mantém o modelo do ADR 0015 (a resolução decide
usando "o conjunto de variantes declaradas") coerente: se o conjunto tem um nome
com dois donos, ele não é mais um conjunto de nomes.

## Consequências

- `find_variant` e a tabela do runtime passam a ter **no máximo um** dono por
  nome; ambos concordam e deixa de haver dependência de ordem de hash.
- Programas que declaravam variantes homônimas em enums diferentes param de
  compilar — é a mudança certa para uma alpha: o comportamento anterior era
  inconsistente entre `check` e `run`.
- Nomes distintos em enums distintos continuam funcionando (teste
  `distinct_variant_names_in_distinct_enums_are_fine`).
- Regressão: `a_variant_name_shared_by_two_enums_reports_e0202` (falha antes do
  fix com `left: []`).
- A qualificação `Enum::Variant` continua fora do recorte (§5.2 não a tem); se
  entrar no futuro, esta restrição pode ser relaxada.
