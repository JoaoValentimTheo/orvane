# Matriz de cobertura — §4.1, §4.2 e Apêndice A

Varredura dirigida da sprint `fix/0.1.0-stabilization` (§7 do roteiro): para cada
construção de sintaxe dos exemplos-alvo, o critério é **roda e produz a saída
esperada**, não apenas "parseia" ou "type-checa".

Cada linha vira (ou já era) um golden com `// orv: run %f`. A coluna "golden"
aponta o arquivo em `tests/golden/`. `examples/` tem os programas idiomáticos.

## §4.1 Hello e §4.2 Dados

| Construção | Onde é exercitada | Resultado | Golden |
|---|---|---|---|
| `fn main` + `print("...")` | `examples/hello.orv:2` | ok | `matrix_m01_hello`, `example_hello` |
| `let` com inferência (Int/Float/Str/Bool/none) | `matrix_m02_let_infer` | ok | `matrix_m02_let_infer` |
| `let` com anotação de tipo | `matrix_m03_let_annot` | ok | `matrix_m03_let_annot` |
| `let mut` + `=`/`+=`/`-=`/`*=`/`/=` | `matrix_m04_let_mut` | ok | `matrix_m04_let_mut` |
| `data` com campos tipados | `examples/data.orv:3` | ok | `example_data`, `matrix_m18_data` |
| `data` com construtor nomeado | `examples/data.orv:11` | ok | `example_data` |
| `data` com construtor posicional | `matrix_m18_data` | ok | `matrix_m18_data` |
| `data` com default (`Str? = none`) | `examples/data.orv:6` | ok | `example_data` |
| `Display` de `data` | `examples/data.orv:12` | ok | `example_data` |
| Igualdade estrutural de `data` | `examples/data.orv:16` | ok | `example_data` |
| Campo de `data` (leitura) | `matrix_m18_data` | ok | `matrix_m18_data` |

## Apêndice A

| Construção | Onde é exercitada | Resultado | Golden |
|---|---|---|---|
| A1: `for` + `1..=15` + `if/else if` encadeado | `examples/fizzbuzz.orv` | ok | `run_fizzbuzz`, `example_fizzbuzz` |
| A2: `enum` com payload + `match` | `examples/shapes.orv` | ok | `run_shape`, `example_shapes` |
| A3: `use py` + `intent`/`how` | — | **fora do alpha** (ADR 0012) | — |
| A4: `intent` + `given` | — | **fora do alpha** (ADR 0012) | — |

## Linguagem do recorte (varredura ampliada)

| Construção | Onde é exercitada | Resultado | Golden |
|---|---|---|---|
| Aritmética com precedência e parênteses | `matrix_m05_arith` | ok | `matrix_m05_arith` |
| Comparação e lógica (`and`/`or`/`not`, curto-circuito) | `matrix_m06_compare_logic` | ok | `matrix_m06_compare_logic` |
| Literais de string e escapes | `matrix_m07_string` | ok | `matrix_m07_string` |
| Interpolação `{expr}` | `matrix_m07_string` | **imprime literal** (ADR 0018) | `matrix_m07_string` |
| `if` como expressão, `else if`, `if` sem `else` | `matrix_m08_if` | ok | `matrix_m08_if` |
| `while` | `matrix_m09_while` | ok | `matrix_m09_while` |
| `for` com `..`, `..=` e `range` | `matrix_m10_for_range` | ok | `matrix_m10_for_range` |
| `break`/`continue` dentro de `if` dentro de laço | `matrix_m11_break_continue` | ok | `matrix_m11_break_continue` |
| Lista: literal, vazia, `len`, índice, negativo, igualdade, escrita | `matrix_m12_list` | ok | `matrix_m12_list` |
| Mapa: literal, vazio, `len`, chave, escrita | `matrix_m13_map` | ok | `matrix_m13_map` |
| `fn` com retorno, sem retorno, `return` antecipado | `matrix_m14_fn` | ok | `matrix_m14_fn` |
| Lambda com anotação, como argumento, corpo de bloco | `matrix_m15_lambda` | ok | `matrix_m15_lambda` |
| Closure capturando `let mut` | `matrix_m16_closure` | ok | `matrix_m16_closure` |
| Recursão (`fib`) | `matrix_m17_recursion`, `examples/fib.orv` | ok | `matrix_m17_recursion`, `example_fib` |
| `enum` com payload + `match` com binding | `matrix_m19_enum_match` | ok | `matrix_m19_enum_match` |
| `match` com literal, guarda `if` e `_` | `matrix_m20_match_guard_wild` | ok | `matrix_m20_match_guard_wild` |
| Opcional `T?` com `??` | `matrix_m21_optional` | ok | `matrix_m21_optional` |
| `as` cast (`Int as Float`) | `matrix_m22_cast` | ok | `matrix_m22_cast` |
| Tuplas: literal, `Display`, igualdade | `matrix_m23_tuple` | ok | `matrix_m23_tuple` |
| Prelude: `str`/`int`/`float`/`len`/`assert`/`Ok`/`Err` | `matrix_m24_prelude` | ok | `matrix_m24_prelude` |
| **Enum unitário como campo de `data` e em `match`** | `matrix_m25_match_unit_variant_in_data` | **corrigido nesta sprint** | `matrix_m25_...` |
| **Enum unitário em lista, iterado, em `match`** | `matrix_m26_enum_in_list`, `examples/enum_flags.orv` | **corrigido nesta sprint** | `matrix_m26_...`, `example_enum_flags` |
| **Construtor de variante como valor** | `matrix_m27_closure_constructor` | **corrigido nesta sprint** (ADR 0017) | `matrix_m27_...` |

## Assimetrias sema↔runtime (item 9)

Percorri cada branch de `check_expr`/`ident_type`/`check_call`/`check_assignable_target`
em `orv-sema/src/check.rs` e procurei o branch espelhado em
`orv-runtime/src/interpreter.rs`. O método: 30 programas-sonda, comparando
`orv check` com `orv run` para cada um (os 27 acima mais 10 de statement, em
`/tmp/audit2` durante a auditoria; o essencial está nos goldens).

| Ponto na sema | Espelho no runtime | Situação |
|---|---|---|
| `Ident` local/param | `eval_ident` → `env` | ok |
| `Ident` de `fn` | `eval_ident` → `functions` | ok |
| `Ident` de variante sem payload | `eval_ident` → `variant_schema` | **era ausente** → corrigido (item 1) |
| `Ident` de variante com payload (tipo `Ty::Fn`) | `eval_ident` → closure construtora | **era ausente** → corrigido (item 2, ADR 0017) |
| `Literal` | `literal_value` | ok |
| `Paren` | `eval` | ok |
| `Block` como expressão | `eval_block_as_expr` | ok |
| `Field` (leitura) | `field` | ok |
| `OptionalField` (leitura) | `field` + `None` | ok |
| `Call` de `fn`/closure/builtin | `call_function` | ok |
| `Call` de `data` (nomeado/posicional) | `try_construct` | ok |
| `Call` de variante com payload | `try_variant` | ok |
| `Index` em lista/mapa/string | `index` | ok |
| `Unary` (`-`, `not`) | `unary` | ok |
| `Binary` (aritmética/comparação/lógica/`??`/range) | `eval_binary`/`binary_op` | ok |
| `Cast` | `cast` | ok |
| `Tuple` | `eval` | ok |
| `List` | `eval` | ok |
| `Map` | `eval_map` | ok |
| `If` | `eval` | ok |
| `Match` | `eval_match` | ok |
| `Try` | `eval` | ok |
| `Fail` (expressão) | rejeitado pela sema (`E0313`) | ok |
| `Lambda` | `eval` → `Closure::lambda` | ok |
| `Assign` a `Ident` | `env.assign` | ok |
| `Assign` a `Index` | `assign_index` | ok |
| `Assign` a `Field` | ausente no runtime | **divergia** → sema agora recusa (`E0231`, ADR 0018) |
| `Assign` a `OptionalField` | ausente no runtime | **divergia** → sema agora recusa (`E0231`) |
| Tipo aninhado (`fn(..) -> Enum`, `List<Enum>`, `Map<_, Enum>`) | `resolve_user_type` não recursava | **era bug** → corrigido (mesma sprint) |

**Resultado:** as assimetrias encontradas foram 3, todas corrigidas nesta sprint
(as duas de variante e a de `Assign` a campo). Nenhuma ficou como "apenas
documentação". O item 9 pedia para relatar também as que virassem documentação:
não houve nenhuma.

Como garantia permanente, `crates/orv-runtime/tests/sema_runtime_agreement.rs`
gera programas **consistentes** (nomes de campo e variante reaproveitados de
fato) e afirma que sema-aceito ⇒ runtime-não-falha-por-desconhecimento. O teste
foi verificado contra o bug do item 1: reintroduzindo-o, 2 casos falham.
