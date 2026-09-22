# ADR 0016 — Limites do runtime: profundidade de chamada e tipo de erro

- **Status:** aceita
- **Data:** M4 (alpha)

## Contexto

O interpretador é um tree-walker recursive-descent sobre a AST. Duas
consequências precisam de decisão explícita, porque a regra do projeto é
"nunca dá panic para entrada arbitrária" — inclusive um `SIGABRT` por estouro de
pilha, que é abort e não um erro recuperável.

1. Recursão do programa: `fn boom(n) { boom(n + 1) }` consome frames Rust muito
   mais rápido que frames Orvane (cada chamada avalia argumentos, o callee, o
   corpo e cada expressão dentro). Com limite alto demais, o estouro de pilha
   acontece **antes** de o guarda disparar.
2. Controle de fluxo: `break`, `continue` e `return` precisam escapar de uma
   expressão (por exemplo de um `if` dentro de um `for`) cujo tipo de retorno é
   `Result<Value, Failure>`.

## Decisão

### Limite de profundidade

`MAX_CALL_DEPTH = 48` chamadas Orvane aninhadas. Ao atingi-lo, a chamada
devolve [`FailureKind::Overflow`](crate::failure::FailureKind) com a mensagem
"call depth exceeded" (`R0004`), em vez de estourar a pilha.

O número foi escolhido **medindo**: 256 (o valor de §5.4.2 para intents) estoura
a pilha padrão de uma thread de teste de 2 MiB antes do guarda disparar. 48
dispara com folga e é generoso para qualquer programa real do alpha.

### Controle de fluxo

`Control` (`Value`/`Return`/`Break`/`Continue`) é o resultado de um statement. Um
bloco em posição de expressão que levanta controle guarda-o em
`Interpreter::pending`, e o statement que avaliou aquela expressão o relê e
propaga. Isso mantém um único tipo de retorno (`Result`) sem canais paralelos e
sem sentinelas de string.

## Consequência

- Recursão infinita é `R0004` com exit 1, coberto por
  `infinite_recursion_is_a_failure_not_a_crash`.
- `break`/`continue` funcionam em qualquer posição de bloco, coberto por
  `runs_break_and_continue`.
- Um `return` dentro de um `if` dentro de um `for` (comum) funciona: o valor
  viaja em `Control::Return` até `call_function`, que o converte no valor da
  função.
- O limite é de **implementação**, não da linguagem: um programa que o atinja é
  recusado por precaução. Aumentar o limite exige aumentar a pilha da thread
  principal (`orv run`) ou trocar o interpretador por um iterativo — não remover
  o guarda.
- `FailureKind::Flow` existe para o caso residual de controle cruzando uma
  fronteira que não tem como devolvê-lo; `call_function` o converte num
  diagnóstico em vez de vazar a sentinela.
