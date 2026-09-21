# ADR 0002 — `orv version` com a flag de versão do clap desativada

- **Status:** aceita
- **Data:** M0

## Contexto

§10 lista `orv version` como subcomando. O `clap` 4, com `version`, também
registra `-V`/`--version`, que seria um **nono** subcomando disfarçado de flag.
§1.5 limita a CLI a no máximo 8 subcomandos, e o gate do M0 pede explicitamente
`orv version` imprimindo a versão.

## Decisão

- `#[command(disable_version_flag = true)]`: `--version`/`-V` **não** existem.
- `orv version` é o único caminho para obter a versão.
- Formato de saída fixado em uma linha: `orv <versão> (orvane <versão>)`, com a
  mesma versão do workspace nos dois campos (crate CLI e crate `orv-pymod` são
  versionados juntos).
- A linha é coberta por golden (`tests/golden/version.out`), não só por teste
  unitário, para travar o texto exato.

## Consequência

- Contagem de subcomandos sob controle: `version` é o 1º de 8.
- `orv --version` falha com exit code 2 (erro de uso), o que é aceitável e
  testado.
- Quando o projeto tiver 8 subcomandos, nenhuma flag de versão precisará ser
  removida.
