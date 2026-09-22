# ADR 0013 — Limite de profundidade no parser

- **Status:** aceita
- **Data:** M2 (alpha)

## Contexto

O parser é recursive-descent: cada nível de aninhamento é uma chamada de função.
A entrada é **arbitrária** (inclui arquivos gerados e fuzz), e `(((((...` com
dezenas de milhares de níveis estoura a pilha do processo. Um stack overflow é
abort, não um `panic` recuperável — não há como reportá-lo como diagnóstico nem
como "erro tratado".

A regra de qualidade do projeto é "nunca dá panic para entrada arbitrária", com
proptest de "nunca panica" em cada camada. Isso inclui não poder ser derrubado
por uma entrada.

## Decisão

O parser mantém um contador de profundidade (`Parser::depth`) incrementado em
`expr`, `primary`, `unary` e `statement`, com limite de **256** níveis
(`MAX_DEPTH`). Ao atingi-lo:

- reporta `E0104` ("expression nesting is too deep") com help citando o limite;
- devolve `None` para a sub-expressão, o que encerra aquela ramificação.

O limite é generoso para código real (um programa com 256 níveis de aninhamento
já é ilegível) e pequeno o bastante para caber com folga na pilha padrão.

## Consequência

- Entrada patológica vira um diagnóstico, não um crash. Coberto por
  `deeply_nested_input_reports_e0104_instead_of_overflowing` (1000 parênteses) e
  `deeply_nested_unary_reports_e0104` (1000 sinais de menos).
- `E0104` é um limite de **implementação**, não da linguagem: um programa que o
  atinja é rejeitado por precaução. Se algum dia um gerador real precisar de mais,
  o caminho é aumentar `MAX_DEPTH` conscientemente, ou trocar o parser por um
  iterativo — não remover a guarda.
- O proptest `parser_never_panics_on_fragments` reforça isso com fragmentos que
  podem compor aninhamento; sozinho ele não basta (gerar 256 níveis por acaso é
  improvável), por isso os testes diretos existem.
