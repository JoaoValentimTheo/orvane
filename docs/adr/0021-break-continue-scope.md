# ADR 0021 — `break`/`continue` são confinados ao loop e à chamada

- **Status:** aceita
- **Data:** sprint `fix-0.1.1`
- **Relacionada:** §5.2 (controle de fluxo), ADR 0020

## Contexto

`break` e `continue` atravessavam fronteiras que não deveriam:

1. **Fora de loop** — `orv check` aceitava `fn main() { break }` e o runtime
   falhava com `R0010: break or continue outside a loop` (divergência
   sema↔runtime).
2. **Através de uma chamada** (o caso pior) —
   ```orv
   for i in 0..3 {
       print("before")
       let f: fn() -> () = () => { break }
       f()
       print("after")
   }
   print("end")
   ```
   imprimia só `before` / `end`: o `Control::Break` produzido dentro da lambda era
   **estacionado em `Interpreter::pending`** e relido pelo `step` do chamador, que
   então encerrava o `for` do chamador — silenciosamente, sem erro. O sema
   aceitava.

A causa comum: o sema não rastreava contexto de loop, e o runtime deixava o
controle vazar pela fronteira da chamada via `pending`.

## Decisão

**`break`/`continue` só são válidos lexicamente dentro de um `while`/`for` do
mesmo corpo de função ou lambda, e nunca atravessam uma chamada.**

- **Sema:** um contador `loop_depth` cresce ao entrar num `while`/`for` e é
  **resetado a 0 ao entrar num corpo de `fn` ou lambda** (nova fronteira de
  chamada). `break`/`continue` com `loop_depth == 0` é **`E0303`**
  ("`break` is only valid inside a loop").
- **Runtime (defesa em profundidade):** `call_function` salva o `pending` do
  chamador, zera o seu durante a chamada, e ao voltar restaura o do chamador. Se
  o chamado deixou controle estacionado — ou devolveu `Break`/`Continue` — isso
  não é do chamador: vira `Failure` ("`break` or `continue` cannot leave the
  function that contains it"). Um loop **dentro** da lambda consome o próprio
  `break` normalmente.

`E0303` já existe em `docs/errors.md` ("não é chamável/iterável/indexável"); o
código passou a cobrir também "controle de fluxo fora de contexto", que é a
mesma família "isto não é válido aqui". Não foi criado código novo.

## Consequências

- `fn main() { break }`, `continue` fora de loop, e `break` dentro de lambda
  passam a ser erro de **sema** (`E0303`), então `check` e `run` concordam.
- Um loop válido dentro de lambda continua funcionando
  (`a_loop_inside_a_lambda_is_fine`, golden `matrix_m30_loop_break_continue`).
- O runtime nunca mais encerra silenciosamente o loop do chamador por causa de um
  `break` de closure; mesmo que o sema seja contornado, há `Failure`.
- Regressão: `break_outside_a_loop_reports_e0303`,
  `continue_outside_a_loop_reports_e0303`,
  `a_break_in_a_lambda_inside_a_loop_reports_e0303`,
  `a_break_in_a_function_called_from_a_loop_reports_e0303` (sema) e
  `a_break_inside_a_lambda_cannot_leave_the_call`,
  `a_loop_inside_a_lambda_still_works` (runtime, via `run_failure`/`run`, que
  parseiam sem passar pela sema); golden `check_break_in_lambda`.
