# ADR 0001 — Spans são offsets de byte, renderizados como byte spans

- **Status:** aceita
- **Data:** M0

## Contexto

`docs/SPEC.md` §3.4 fixa `Span { file: FileId, start: u32, end: u32 }` sem dizer
se `start`/`end` são offsets de `char` ou de `byte`. O `ariadne` 0.6 aceita
`IndexType::Byte` e `IndexType::Char`; a escolha precisa ser única porque
`SourceFile::line_col` deriva a coluna usada no formato golden
`CODE:linha:coluna: mensagem` (§8.1).

## Decisão

`Span.start`/`Span.end` são **offsets de byte** em `[start, end)`.

- `ariadne` é configurado com `IndexType::Byte`.
- `SourceMap::line_col` converte byte → (linha, coluna) contando `char`s desde o
  início da linha, de modo que a coluna reportada é em **caracteres** (o que o
  usuário vê), mesmo o span sendo em bytes.
- O lexer (M1) só produz spans em fronteiras de `char`.

## Consequência

- `SourceFile::text()` pode ser fatiado diretamente por um span sem risco de
  panic em UTF-8, o que simplifica lexer, parser e recuperação de erro.
- A coluna em `.err` é estável para arquivos com acentos (ver teste
  `line_col_counts_chars_not_bytes`).
- Se algum dia precisarmos de colunas em bytes, é só mudar `line_col`, não a
  representação do span.
