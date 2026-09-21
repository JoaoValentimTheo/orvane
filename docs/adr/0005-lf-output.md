# ADR 0005 — `orv version` escreve LF, independente do SO

- **Status:** aceita
- **Data:** M0.1

## Contexto

`println!` termina a linha com o separador da plataforma: `\r\n` no Windows. As
expectativas golden são versionadas com `\n` (e `.gitattributes` marca
`tests/golden/**` como `-text` para garantir isso). O harness normaliza CRLF da
saída capturada (ADR 0003, emenda), mas essa normalização **mascararia** um `CR`
emitido de fato pelo binário: o teste passaria enquanto `orv version` produzisse
bytes diferentes por SO.

## Decisão

- A CLI não usa `println!` para saída golden: `print_lf` escreve os bytes e um
  `\n` explícito em stdout.
- Um teste de integração afirma sobre os **bytes crus** do stdout de
  `orv version`: nenhum `\r`, terminação em exatamente um `\n`. Ele roda em
  todos os SOs, então uma regressão no Windows falha em CI.
- Se a escrita em stdout falhar, a CLI retorna exit code `2` (ambiente), sem
  `panic!` — coerente com SPEC §0.2 item 5 e §10.

## Consequência

- A saída de `orv version` é byte-idêntica em Linux, macOS e Windows, o que
  torna o golden realmente portável em vez de dependente de normalização.
- A normalização do harness permanece (é necessária para stderr de ferramentas
  de terceiros, e para o `Usage:` do clap), mas deixa de ser o único mecanismo
  de defesa, e o teste de bytes crus impede que ela esconda o problema.
- A regra vale para toda saída que vire golden daqui pra frente: **quem escreve
  LF é o binário**, não o harness.
