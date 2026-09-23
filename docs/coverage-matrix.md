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

## Features 0.1.2 (interpolação e mutação de campo)

Sprint `feat-0.1.2-interpolation-and-mutation`. Duas features; cada linha roda e
produz a saída esperada (ou o diagnóstico esperado).

| Construção | Onde é exercitada | Resultado | Golden |
|---|---|---|---|
| Interpolação `{expr}` de cada tipo de Value (Int, Float, Str, Bool, enum s/ payload, enum c/ payload, `data`, List, Map, none) | `interpolation_of_every_value_kind_matches_str` | ok; `"{x}"` == `str(x)` | `matrix_m31_interpolation` |
| Interpolação concatenando com o texto literal | `interpolation_concatenates_with_the_literal_text` | ok | `matrix_m31_interpolation` |
| Interpolação aninhada `"{f("{x}")}"` | `interpolation_can_nest` | ok (o lexer já aceitava; agora avalia) | — |
| Interpolação vê o escopo onde aparece (incl. bloco interno) | `an_interpolation_sees_the_scope_around_it` | ok | — |
| Nome fora de escopo dentro de `{}` | `an_undefined_name_inside_an_interpolation_reports_e0201` | `E0201`, span dentro da interpolação | — |
| Erro de runtime dentro de `{}` (`"{1/0}"`) | `a_runtime_error_inside_an_interpolation_has_the_inner_span` | `R0001` com span interno | — |
| Sub-parse de `{...}` com conteúdo hostil nunca panica | `interpolation_subparse_never_panics` (proptest) | ok | — |
| Mutação de campo em `let mut u` | `mutating_a_data_field_is_visible_on_the_binding` | ok | `matrix_m32_field_mutation` |
| Semântica de valor: `let b = a`; mutar `a2` não muda `b` | `data_has_value_semantics_on_assignment` | ok | `matrix_m32_field_mutation` |
| Escrita em campo de `data` imutável | `assigning_to_a_field_requires_a_mutable_binding` | `E0230` | — |
| Escrita em campo opcional / receptor aninhado | `assigning_to_an_optional_field_is_rejected`, `assigning_to_a_nested_field_is_rejected` | `E0231` | — |
| `Value::Data` clonado é cópia independente | `cloning_a_data_makes_an_independent_copy` | ok | — |

### Combinações cruzadas com o que já existia

| Combinação (cruza 0.1.2 com feature anterior) | Onde é exercitada | Resultado | Golden |
|---|---|---|---|
| Interpolar um campo de `data` **depois** de mutá-lo | `interpolating_a_mutated_field_shows_the_new_value` | ok | `matrix_m32_field_mutation` |
| Mutar campo dentro de um loop com `break` | `a_field_can_be_mutated_inside_a_loop` | ok | `matrix_m31`/`matrix_m32` |
| Interpolar dentro de lambda que captura a variável mutada | `a_data_value_can_be_captured_and_mutated_by_a_closure_binding` + `matrix_m32` | ok | `matrix_m32_field_mutation` |
| Interpolação de um campo mutado dentro de closure | `matrix_m31_interpolation`/`matrix_m32` | ok | ambos |
| Interpolar `data` cujo campo veio de `for`/`if` | `matrix_m31_interpolation` | ok | `matrix_m31_interpolation` |

## Auditoria `fix-0.1.3` (superfície nova da 0.1.2)

Sprint `fix-0.1.3`. Cada item da varredura dirigida, com o resultado.

| Item | Construção | Onde é exercitada | Resultado |
|---|---|---|---|
| (a) | Interpolação aninhada até milhares de níveis | `deeply_nested_interpolation_reports_e0104_instead_of_overflowing`, `nested_interpolation_never_overflows` (proptest), `nested_interpolation_never_overflows` no fuzz-bytes | **era bug P0**: `Parser::new_nested` resetava `depth` a 0, então 3000 níveis estouravam a pilha nativa (SIGABRT). Corrigido: o sub-parse **compartilha** o orçamento de profundidade (ADR 0013) e reporta `E0104`. |
| (a) | Interpolação aninhada moderada (20 níveis) aceita | `moderately_nested_interpolation_is_fine` | ok |
| (b) | N interpolações sequenciais, custo linear | `many_sequential_interpolations_scale_linearly` | verificado, sem bug (50000 partes em ~0,12 s) |
| (c) | Igualdade estrutural de dois `data` independentes | `equality_of_separately_built_data_is_structural_not_identity` | verificado, sem bug (compara conteúdo, não `Rc`) |
| (c) | Mutar e desmutar um campo mantém a igualdade | idem | verificado, sem bug |
| (d) | Lambda como valor em campo de `data` conduzindo mutação (ADR 0017) | `a_lambda_in_a_data_field_can_drive_a_field_mutation` | ok |
| (d) | Mutação + interpolação dentro de lambda chamada (ADR 0021) | `mutation_and_interpolation_happen_inside_a_lambda` | ok |
| (d) | Campo de `data` que é `enum` unitário, mutado e comparado (ADR 0020) | `a_data_field_holding_a_unit_enum_can_be_mutated_and_compared` | ok |
| (e) | Falha dependente de dado dentro de `{}` (índice OOB em loop) | `a_data_dependent_failure_inside_an_interpolation_keeps_its_code` | `R0003`, span interno |
| (e) | Falha dentro de `match` dentro de `{}` | `a_failure_inside_a_match_inside_an_interpolation_keeps_its_code` | `R0001`, span interno |
| (e) | Chave de mapa ausente dentro de `{}` | `a_missing_map_key_inside_an_interpolation_reports_r0003` | `R0003`, span interno |

## Combinações cruzadas (sprint `fix-0.1.1`)

Cada linha cruza **duas** construções que isoladamente já tinham golden, mas cuja
junção não era exercitada. O critério continua "roda e produz a saída esperada".

| Combinação | Onde é exercitada | Resultado | Golden |
|---|---|---|---|
| Lambda como campo de `data`, chamada pelo campo | `matrix_m28_lambda_from_frame` | ok | — (coberto por `interpret.rs`) |
| Lambda retornada por `fn` que captura o parâmetro do frame | `matrix_m28_lambda_from_frame` | **corrigido nesta sprint** (ADR 0019) | `matrix_m28_...` |
| Lambda aninhada `a => (b) => a + b` | `matrix_m28_lambda_from_frame` | **corrigido nesta sprint** (ADR 0019) | `matrix_m28_...` |
| Closure captura `data`/`enum`/mapa e é chamada depois | `interpret.rs` (`a_lambda_returned_...`) | ok | — |
| Closure mutável como contador retornado (`make_counter`) | `interpret.rs` | ok (só após ADR 0019) | — |
| Tupla dentro de `data`, em lista e em `match` de variante | `matrix_m23_tuple` + probes | ok | `matrix_m23_tuple` |
| `enum` com payload de `data`, `match` com binding | probes `e09` | ok | — |
| `enum` como chave de mapa | probe `e06` | **era bug** → sema recusa (E0301) | — |
| `data`/`enum` como chave de mapa | probes `k_data` | **era bug** → sema recusa (E0301) | — |
| Mapa com chave `Int`/`Bool`, iterado com `for` | `matrix_m29_map_int_bool_keys` | **corrigido nesta sprint** | `matrix_m29_...` |
| `match` aninhado (enum dentro de enum) | probe `f01` | ok | — |
| Guarda encadeada com wildcard final | probe `f03` | ok | — |
| `match` como expressão retornando lambda | probe `e04` (E0310, sem contexto) | consistente | — |
| Campo opcional `List<Int>?` com `??` | probe `f05` | ok | — |
| Tipo de usuário aninhado em `fn(..)`, `List`, `Map` | `matrix_m18` + probes | corrigido na 0.1.0 | — |
| Índice aninhado (`xs[1][0]`, `m["a"][1]`) | probes `g03`, `g04` | ok | — |
| Recursão mútua (`is_even`/`is_odd`) | probe `f14` | ok | — |

**Nota sobre `t.0` e patterns de tupla:** a gramática de §5.2 só tem
`field = "." IDENT` e `pattern = "_" | literal | IDENT | IDENT "(" ... ")"`. Nem
acesso indexado a tupla nem pattern de tupla existem no recorte; o parser recusa
consistente (E0102) e isso é escopo, não bug.

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
| Chave de mapa fora de `{Int, Str, Bool}` (`enum`, `data`, `Float`) | `MapKey::from_value` só aceita os três | **divergia** → sema agora recusa (`E0301`, 0.1.1) |
| Lambda capturando parâmetro de frame que já retornou / lambda aninhada | `Env` compartilhava a cadeia e o `pop` removia a captura | **era bug** → corrigido (`Env::snapshot`, ADR 0019, 0.1.1) |

**Resultado:** as assimetrias encontradas foram 3, todas corrigidas nesta sprint
(as duas de variante e a de `Assign` a campo). Nenhuma ficou como "apenas
documentação". O item 9 pedia para relatar também as que virassem documentação:
não houve nenhuma.

**Na 0.1.1** a mesma varredura, dirigida a combinações cruzadas, achou mais 4
divergências da mesma classe (chave de mapa, captura de closure, nome de variante
duplicado e escopo de `break`/`continue`), todas corrigidas e cobertas por
regressão dirigida.

| Ponto na sema | Espelho no runtime | Situação |
|---|---|---|
| `Break`/`Continue` sem rastrear contexto de loop | runtime recusa com `R0010` | **era bug** → sema recusa (`E0303`, ADR 0021) |
| `Break` dentro de lambda | `Control::Break` atravessava a fronteira da chamada | **era bug** → sema recusa (`E0303`) e o runtime confina `pending` à chamada (ADR 0021) |



Como garantia permanente, `crates/orv-runtime/tests/sema_runtime_agreement.rs`
gera programas **consistentes** (nomes de campo e variante reaproveitados de
fato) e afirma que sema-aceito ⇒ runtime-não-falha-por-desconhecimento. O teste
foi verificado contra o bug do item 1: reintroduzindo-o, 2 casos falham.
