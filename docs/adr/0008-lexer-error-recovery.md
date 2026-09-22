# ADR 0008 — Recuperação de erro no lexer: tokens de melhor esforço

- **Status:** aceita
- **Data:** M1.1

## Contexto

A revisão do M1 mostrou que o lexer, ao encontrar um erro, emitia **somente** o
diagnóstico e nenhum token para o construto. Isso obriga quem consome a stream a
tratar "não houve token" como caso normal, e faz `orv tokens` pular linhas que o
leitor espera ver. Também havia três comportamentos indefinidos: pilha de
delimitadores com fechamento desalinhado, `\` dentro de uma interpolação, e
cascata de `E0002` numa interpolação não fechada.

## Decisões

### A. Erro léxico emite um token de melhor esforço

Além do diagnóstico, o lexer produz o token mais próximo do que foi lido:

| Erro | Token emitido |
|---|---|
| `E0002` / `E0004` / `E0006` | `Str(parts)` com as partes **já lidas** |
| `E0005` | `Int(0)` ou `Float(0.0)` |

Consequência: a stream nunca "perde" uma posição, `orv tokens` mostra a linha
problemática, e o parser pode reportar o erro **uma vez** (léxico) em vez de
tropecar na ausência de token. O valor de placeholder (`0`) é irrelevante porque
o diagnóstico já invalidou o programa; qualquer milestone que consuma tokens
deve checar `diagnostics` antes.

### B. Fechamento desalinhado desempilha até o abridor correspondente

Ao ver `)`, `]` ou `}`, se o abridor correspondente existir **em qualquer
posição** da pilha, tudo acima dele é desempilhado; se não existir, apenas o
token é emitido (nada é removido). Sem isso, `f(1 }` deixava o `(` preso na
pilha e o `Newline` seguinte era suprimido indevidamente.

Teste de referência: `fn g() { f(1 }\nlet b = 2` emite `Newline` antes de `let`.

### C. `\` dentro de `{...}` de interpolação é `E0006`

Dentro de uma interpolação, `\` não é escape: o texto é re-lexado pelo parser, e
uma string aninhada se escreve **crua** — `"{f("a")}"`. Um `\` ali é `E0006`
com help "escreva strings aninhadas sem escape: {f("a")}". O scanner **continua
até o `}` final**, para não perder o fim da interpolação.

Isso substitui o tratamento anterior, em que `\` consumia o próximo caractere
(solução que só funcionava por acidente para `\"`).

### D. Interpolação não fechada gera **um** `E0002`

Quando uma interpolação não fecha, o erro reportado é o mais **externo**
(`E0002` na string), e o scanner consome até o fim da linha/arquivo, sem emitir
um segundo diagnóstico pelo interior. Antes havia dois `E0002` para a mesma
causa (`"abc` + `def"`), o que enganava a leitura.

## Consequência

- `orv tokens` passa a mostrar uma linha por token mesmo em arquivos inválidos.
- O parser (M2) pode confiar que a stream cobre o arquivo; a validação de
  correção é responsabilidade do vetor de `diagnostics`.
- Um erro léxico nunca "esconde" o resto do arquivo atrás de uma cascata de
  diagnósticos idênticos.
- Os goldens `tokens_err_*` pinam o comportamento, então mudar essas regras é
  uma edição visível.

## Emenda (M2, §0): o contrato do pipeline na presença de erro léxico

Esta emenda **normatiza** o que o M2 deve fazer com tokens de melhor esforço.
Ela é a razão de A–D existirem; sem ela, os placeholders seriam uma armadilha.

1. **O parser roda mesmo com diagnósticos léxicos.** Os tokens de melhor esforço
   existem para que o parser possa continuar e reportar erros sintáticos
   adicionais, em vez de parar no primeiro caractere ruim.
2. **Diagnósticos léxicos vêm primeiro**, na ordem do vetor de diagnóstico, e
   antes de qualquer diagnóstico do parser. A causa raiz é o léxico.
3. **Suprimir o diagnóstico do parser cujo span primário intersecta o de um
   diagnóstico léxico.** Um `Int(0)`/`Float(0.0)`/`Str` parcial não é uma
   construção que o autor escreveu, então cobrar sintaxe sobre ela é ruído: o
   erro léxico já explicou o trecho. A interseção é sobre `[start, end)` do span
   primário, não igualdade.
4. **Não fazer sub-parse de `StrPart::Expr.src` cujo span contenha diagnóstico
   léxico.** O token é de melhor esforço: o texto pode estar truncado ou conter
   a barra invertida que gerou o `E0006`. Re-lexar esse `src` produziria erros
   sobre uma string que não existe no arquivo.
5. **Nunca avançar para sema/run se houver qualquer erro.** Basta um
   diagnóstico de severidade `Error` — léxico ou sintático — para que `orv check`
   não rode a análise semântica e `orv run` não execute. Um `Int(0)` de
   placeholder jamais chega ao interpretador.

Consequência para o M2: o parser precisa saber quais spans têm diagnóstico
léxico. O caminho mais simples é consultar o vetor de diagnósticos que `lex`
devolve (buscando interseção por span) antes de decidir se reporta e se faz
sub-parse; não é preciso flag novo no `Token`.
