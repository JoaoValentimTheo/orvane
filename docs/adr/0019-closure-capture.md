# ADR 0019 — Captura de closures: snapshot de escopos com células compartilhadas

- **Status:** aceita
- **Data:** sprint `fix-0.1.1`
- **Relacionada:** §5.3 (closures capturam `let mut`), ADR 0012 (recorte do alpha)

## Contexto

`Env` era uma única cadeia (`Rc<RefCell<Vec<Scope>>>`) mutada com `push`/`pop`.
Uma lambda capturava `self.env.clone()` — um clone do `Rc`, ou seja **a mesma
cadeia**. Quando a função que definia a lambda retornava, o `pop` do chamador
removia o escopo do frame (os parâmetros) daquela cadeia compartilhada, e a
lambda passava a não enxergar mais as capturas.

Sintoma (classe sema↔runtime, a mesma das variantes no ADR 0017): a sema
aceitava

```
fn adder(n: Int) -> fn(Int) -> Int {
    let f: fn(Int) -> Int = (x) => x + n
    return f
}
```

e o runtime falhava com `R0010: undefined name 'n'` ao chamar `adder(10)(5)`.
Idem para lambda aninhada `a => (b) => a + b`.

## Decisão

Separar **a lista de escopos** da **armazenagem dos bindings**:

- `Env` passa a possuir sua própria lista: `Rc<RefCell<Vec<Rc<Scope>>>>`. Assim,
  `push`/`pop` em um `Env` não afetam outro.
- `Scope` é `Rc`-compartilhado e guarda os slots em
  `RefCell<HashMap<Rc<str>, Slot>>`.
- `Slot` é uma célula `Rc<RefCell<..>>`, compartilhada entre o escopo que a
  criou e qualquer closure que o capture.
- `Env::snapshot()` devolve um `Env` novo com a **mesma lista de escopos**
  (handles `Rc` clonados). A lambda captura `self.env.snapshot()`; `call_env`
  devolve um snapshot fresco por chamada, de modo que o frame de parâmetros da
  chamada não vaza para o valor da closure nem para outra chamada.

Consequência de semântica: `let mut` capturado continua **por referência**
(células compartilhadas) e as capturas sobrevivem ao `pop` do frame que definiu
a closure.

## Consequência

- As três construções passam a rodar: lambda retornada, lambda aninhada, e
  mutação de `let mut` externo observada depois por closure retornada.
- `child()` (que devolvia um `Env::new()` vazio e nunca era usada) foi removida;
  `snapshot()` é a operação correta.
- Custo: um `Rc<RefCell>` por binding (antes um `Slot` inline). O caminho de
  leitura continua O(escopos) e determinístico.
- Regressão: `a_returned_lambda_keeps_its_captured_parameters`,
  `a_nested_lambda_closes_over_the_outer_lambda_parameter`,
  `a_lambda_returned_from_a_function_still_sees_later_outer_mutation`,
  golden `matrix_m28_lambda_from_frame`.
