# ADR 0009 — Quebras de linha e `\`: LF, CRLF e CR são equivalentes

- **Status:** aceita
- **Data:** M1.2

## Contexto

`\r` isolado é espaço e `\r\n` é uma quebra (§5.1), mas o tratamento do `\`
antes de uma quebra estava incompleto:

- em strings normais, `\`+LF já deixava o LF para o scanner (M1.1), mas `\`+CR
  **consumia** o CR, então CRLF e LF divergiam na parte literal;
- dentro de uma interpolação e de uma string aninhada, `\` consumia a quebra
  inteira, então `"{f("a\<LF>b")}"` saía com **exit 0 e nenhum diagnóstico**,
  enquanto a mesma entrada com CRLF produzia `E0002`/`E0006`.

Ou seja: a mesma causa (barra invertida seguida de quebra) tinha diagnósticos
diferentes conforme o fim de linha — e o caso LF passava em silêncio.

## Decisão

Uma barra invertida **nunca consome uma quebra de linha**, seja em string
normal, em string aninhada ou em interpolação. A quebra fica para o scanner
externo, que a trata como fim de linha.

Consequências diretas, todas testadas:

- `"a\<LF>"`, `"a\<CRLF>"` e `"a\<CR>"` produzem o **mesmo** `E0002`
  (string não terminada) e o mesmo estado de tokens.
- `"{f("a\<LF>b")}"` e a variante CRLF produzem os mesmos `E0002` e `E0006`,
  em vez de o LF passar silenciosamente.
- Nenhuma expressão de interpolação contém `\n` ou `\r` no `src`: a quebra
  nunca é dobrada para dentro do texto da expressão.
- Um `\r` isolado dentro de string, de interpolação ou de string aninhada é
  tratado como quebra (encerra o construto), para que LF e CRLF concordem.

Invariante de propriedade: trocar todo `\n` por `\r\n` **não** altera a
sequência de kinds de token (comparada via `dump`, sem spans) nem a sequência de
códigos de diagnóstico.

## Consequência

- Um arquivo LF e um arquivo CRLF com o mesmo programa têm o mesmo
  comportamento léxico e o mesmo conjunto de diagnósticos.
- O `E0006` de `\` em interpolação passa a ser **um por interpolação** (e não um
  por barra), porque a mensagem é a mesma e repeti-la por `\"a\"` era ruído; a
  primeira barra é o span reportado.
- Diferente do ADR 0007 §3 (comentário de bloco), aqui CR **não** é espaço:
  dentro de um construto de string uma quebra é sempre significativa, e tratar
  CR como espaço deixaria um `\`+CR+LF "engolir" meio fim de linha.
- Cobertura: `backslash_before_a_line_break_inside_interpolation_is_reported`,
  `line_break_after_backslash_inside_interpolation_is_not_swallowed`,
  `lf_and_crlf_agree_inside_a_nested_string`,
  `backslash_inside_interpolation_is_e0006`, e a invariante
  `kind_sequence` no proptest `lex_invariants_hold`.
