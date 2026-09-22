# AGENTS.md — Orvane

> Cópia literal de **§0.2** e **§3** de `docs/SPEC.md`. O agente deve reler este
> arquivo no início de cada sessão (§0.1). Se este arquivo e o `SPEC.md`
> divergirem, o `SPEC.md` vence e este arquivo deve ser corrigido.

## 0.2 Contrato do agente (regras invioláveis)

1. **Não invente sintaxe ou semântica.** Se algo não está neste documento, escolha o padrão mais simples, registre em `docs/adr/NNNN-titulo.md` (contexto, decisão, consequência) e siga em frente.
2. **Escopo é orçamento.** Só implemente o milestone atual. Não antecipe features futuras. Se sentir vontade de "já deixar preparado", não faça.
3. **Antes de dizer "pronto"**, rode e cole a saída de:
   `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
4. **Uma feature só é "Implementada" se tiver teste golden que a exercita.** Nada de status otimista no README.
5. **Sem `unsafe`** (exceto se o PyO3 exigir e o próprio PyO3 já encapsular — nesse caso, nenhum `unsafe` escrito por você). Sem `unwrap()`/`expect()` fora de testes. Sem `panic!` para erros de usuário: sempre `Diagnostic`.
6. **Dependências**: apenas as da lista em §3.3. Qualquer outra exige ADR aprovada pelo humano.
7. **Commits pequenos**, um por sub-tarefa, mensagem no formato `M4: implement while loops`.
8. **Nunca reescreva arquivos inteiros** sem necessidade; edite cirurgicamente. Nunca apague testes para fazê-los passar.
9. Ao terminar: liste (a) o que foi feito, (b) o que ficou de fora, (c) decisões tomadas (ADRs), (d) próximo milestone.
10. Se um teste do milestone anterior quebrar, **conserte antes de continuar**.
11. **Merge em `main` é sempre humano.** Você pode abrir PR contra `main` a qualquer momento com gate verde, mas nunca clica merge nela — a proteção de branch (§3.5, adicionar referência) impede isso tecnicamente também, mas a regra vale independente da proteção estar ativa no momento. Merge em branches de integração/rascunho que não sejam `main` (ex.: `fix-0.1.1`, branches de milestone) continua permitido com gate 100% verde, sem precisar de autorização por item, exceto onde um prompt disser o contrário.
12. **STOP CONDITION é regra permanente, não algo a repetir por prompt.** Quando uma auditoria ou varredura encontrar mais bugs da mesma classe do que o limite combinado na tarefa (ou, na ausência de limite explícito, mais de 3), pare de corrigir, escreva o relatório do que já foi encontrado e aguarde decisão — mesmo que a correção pareça pequena. "Documentar como pendência e mesclar assim mesmo" não satisfaz esta regra.
13. **Todo relatório final deve ser verificável, não só afirmado.** Qualquer frase do tipo "não mesclei em main", "gate verde" ou "N testes passam" vem acompanhada do comando literal usado para confirmar (ex.: `git merge-base --is-ancestor <branch> origin/main`, a saída de `cargo test`, o link do run de CI) — não a alegação sozinha.

## 3. Restrições técnicas

### 3.1 Toolchain
- Rust **stable**, edição **2024** (Rust ≥ 1.85). Fixar em `rust-toolchain.toml`.
- Python alvo: **3.10 – 3.13**. Testar em CI com matriz.
- SO: Linux, macOS, Windows (CI: pelo menos Linux + Windows).

### 3.2 Arquitetura de crates (workspace)
```
orvane/
├─ AGENTS.md                 # cópia de §0.2 + §3
├─ Cargo.toml                # [workspace]
├─ rust-toolchain.toml
├─ docs/
│  ├─ SPEC.md                # este documento
│  ├─ grammar.ebnf           # gramática (extraída de §5.2)
│  ├─ errors.md              # códigos de diagnóstico (§8)
│  ├─ STATUS.md              # tabela de features x testes
│  └─ adr/                   # decisões
├─ crates/
│  ├─ orv-syntax/            # span, source map, diagnostics, lexer, ast, parser
│  ├─ orv-sema/              # resolução de nomes, tipos, checagem de intents
│  ├─ orv-runtime/           # Value, interpretador, planner, trait Host
│  ├─ orv-py/                # implementação de Host com PyO3 (única crate que depende de pyo3, junto com orv-pymod)
│  ├─ orv-cli/               # binário `orv`
│  └─ orv-pymod/             # extension module Python `orvane` (maturin)
├─ tests/
│  ├─ golden/                # <nome>.orv + <nome>.out (stdout esperado) + <nome>.err (diagnóstico esperado)
│  └─ python/                # pytest para o módulo orvane
└─ examples/
```
**Regra de dependência (unidirecional):**
`orv-syntax ← orv-sema ← orv-runtime ← orv-py ← (orv-cli, orv-pymod)`.
`orv-syntax`, `orv-sema` e `orv-runtime` **não podem** depender de `pyo3`.

### 3.3 Dependências permitidas
| Crate | Uso |
|---|---|
| `thiserror` | erros internos |
| `ariadne` | renderizar diagnósticos |
| `clap` (derive) | CLI |
| `rustyline` | REPL (M9) |
| `insta` (dev) | snapshots/golden |
| `proptest` (dev) | property tests no lexer/parser |
| `serde`, `serde_json` | dump de AST/trace (`--json`) |
| `pyo3` | **somente** em `orv-py` e `orv-pymod` |
| `maturin` | build do wheel (ferramenta, não dependência) |

> Ao adicionar, use `cargo add <crate>` (para pegar a versão atual) — **não** copie números de versão de memória.
> PyO3: use a API atual (`Bound<'py, T>`). Não use APIs marcadas *deprecated*. A forma de obter o GIL mudou entre versões (`Python::with_gil` → `Python::attach` nas versões recentes): **siga a documentação (docs.rs) da versão fixada no `Cargo.lock`**.

### 3.4 Convenções de código
- Módulos ≤ ~400 linhas; funções ≤ ~60 linhas. Se passar, divida.
- Cada `enum` de AST/Token/Erro em arquivo próprio quando crescer.
- `Span { file: FileId, start: u32, end: u32 }` em **todo** nó de AST.
- Erros de usuário sempre `Diagnostic { code, message, span, labels, help }` (§8). Nunca `String` solta.
- Determinismo: nenhuma iteração de `HashMap` afeta saída visível (use `IndexMap`-like via `Vec` ou ordene). Proibido depender de ordem de hash.
- Testes de unidade ao lado do código; testes de ponta a ponta em `tests/golden`.

### 3.5 Proteção do branch `main`
`main` exige, de forma **não contornável por administradores**
(`enforce_admins = true`, o ponto mais importante — sem ele, uma conta com
direitos de admin mescla direto e a proteção vira teatro):

- Pull Request obrigatório antes de merge;
- no mínimo **1 aprovação** (`required_approving_review_count = 1`);
- revisões antigas descartadas quando novos commits chegam
  (`dismiss_stale_reviews = true`);
- os checks de `.github/workflows/ci.yml` verdes antes do merge, com os nomes
  exatos dos jobs: `rustfmt`, `clippy + test (ubuntu-latest)`,
  `clippy + test (windows-latest)`.

Isso materializa §0.2 item 11 (merge em `main` é sempre humano). A regra vale
mesmo que a proteção seja desligada temporariamente.
