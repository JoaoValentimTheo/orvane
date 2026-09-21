# ADR 0003 — Harness golden próprio em vez de `insta` para E2E

- **Status:** aceita
- **Data:** M0

## Contexto

§11 item 2 descreve `tests/golden/`: `<nome>.orv` + `<nome>.out` (stdout) e/ou
`<nome>.err` (diagnósticos), percorrido por um **harness único**. `insta` está
na lista de dependências permitidas, mas §11 item 2 também proíbe atualização
automática de snapshots e no M0 ainda não existe lexer/parser/interpretador.

## Decisão

- O harness E2E é um teste de integração escrito à mão
  (`tests/golden_runner.rs`), sem `insta`: ele lê os arquivos `.out`/`.err`
  versionados no repositório e compara byte a byte, falhando com um diff.
- `insta` fica reservado para snapshots internos de crate (lexer/parser),
  quando fizer sentido no M1/M2.
- O runner tem dois modos de comparação, escolhidos pela presença dos arquivos:
  - **`.out`** → compara com o **stdout** do comando descrito no cabeçalho do
    `.orv`;
  - **`.err`** → compara com o stderr **normalizado** (só a ocorrência de
    diagnóstico `CODE:linha:coluna: mensagem`, sem o desenho do `ariadne`), que
    é o formato estável de §8.1.
- O cabeçalho do `.orv` declara o comando a executar, para que o mesmo runner
  sirva a `orv run` (M4), `orv check` (M3) e `orv test` (M9) sem reescrita.

## Consequência

- Zero dependência de snapshotting no caminho E2E: nada atualiza `.out`/`.err`
  sozinho em CI.
- Como o M0 não tem como executar `.orv` ainda, o único caso golden é o
  `version` (comando que já existe). A partir do M1 o mesmo runner passa a
  cobrir arquivos de verdade sem mudança de formato.
- O cabeçalho `// orv: <subcomando>` é uma convenção do harness; comentários
  `//` já são léxico válido (§5.1), então os arquivos não contêm sintaxe nova.
