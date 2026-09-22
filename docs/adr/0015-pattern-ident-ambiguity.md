# ADR 0015 — Ambiguidade de `IDENT` em padrões de `match`

- **Status:** aceita
- **Data:** M3 (alpha)

## Contexto

§5.2 define:

```ebnf
pattern = "_" | literal | IDENT
        | IDENT "(" [ pattern { "," pattern } ] ")" ;
```

Um `IDENT` **sozinho** é ambíguo: pode ser um padrão de variante de `enum` com
payload vazio (`Circle`) ou uma ligação (`x`, `n`). A EBNF não distingue, e §5.3
também é silenciosa. É o mesmo problema que Rust resolve por resolução de nomes.

## Decisão

**A resolução de nomes decide, e um nome sem variante correspondente liga.**

Ordem, ao ver `PatternKind::Bind(name)`:

1. se `name` é um **variant** de algum `enum` declarado e o payload é vazio →
   padrão de variante, checado contra o tipo do scrutinee;
2. se é um variant **com payload** → `E0302` (faltaram os campos);
3. senão → liga o valor (`x => ...`).

Consequência: um variant **escrito errado** (`B` onde `E` só tem `A`) **liga**
em vez de ser reportado, e portanto um arm a mais é silencioso aqui. Essa ligação
errada é pega na **exaustividade** (`E0320`, M5) e no uso do valor ligado, não
pelo padrão.

## Por que não as alternativas

- **Case-sensitivity** (`Circle` é variante, `x` liga) resolveria o caso comum,
  mas é regra nova: §5.2 não restringe identificadores por capitalização, e
  introduzir isso agora proibiria `let Circle = 1` (legal hoje) e faria um
  variant `circle` no `enum` deixar de ser reconhecido. Inventar sintaxe/semântica
  é o que §0.2 regra 1 proíbe.
- **Prefixar variantes** (`E::Circle`) seria mudança de gramática, não decisão de
  sema.

A regra escolhida não inventa nada: usa a única informação que a spec dá (o
conjunto de variantes declaradas) e mantém o "senão liga" que a gramática já
implica para `IDENT`.

## Consequência

- `match e { A => 1 }` com `enum E { A }` funciona e é exaustivo.
- `match e { B => 1 }` compila com `B` ligando; o erro aparece como match não
  exaustivo faltando `A` (M5) — mensagem menos direta que "não existe variante
  `B`", mas é a consequência inevitável da gramática de §5.2.
- `match n { A => 1 }` onde `n: Int` e `A` é variante → `E0301` (não é enum),
  porque nesse caso o nome **é** um variant e o scrutinee não é enum.
- Cobertura: `a_unit_variant_pattern_is_accepted`,
  `an_unknown_variant_name_binds_instead`,
  `variant_pattern_against_a_non_enum_reports_e0301`,
  `variant_pattern_field_count_reports_e0302`.
- Se o M5 quiser melhorar a mensagem, o caminho é detectar, na exaustividade, um
  arm que liga um nome nunca usado cujo prefixo case com um variant declarado —
  decisão daquele milestone, não desta.
