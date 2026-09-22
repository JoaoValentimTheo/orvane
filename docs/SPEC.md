# ORVANE — Especificação de Implementação v0.1

> **Documento para agente de código (DeepSeek Flash).**
> Objetivo: implementar do zero, em Rust, a linguagem **Orvane** — orientada a intenção, com interoperabilidade bidirecional com Python.
> Idioma deste documento: PT-BR. Idioma do código, identificadores, keywords, comentários e mensagens de erro: **inglês**.

---

## 0. Como usar este documento

### 0.1 Para o humano
- O documento é **fatiável**. Em cada sessão com o agente, forneça: **§0.2 + §1 + §3 + o milestone atual (§12)** e apenas as seções técnicas citadas por ele.
- Uma sessão = **um milestone**. Não peça dois de uma vez.
- Cole o *Prompt de sessão* (§14) no início de cada sessão.
- Salve as seções §0.2 e §3 no repositório como `AGENTS.md` (o agente deve relê-lo sempre).

### 0.2 Contrato do agente (regras invioláveis)
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

---

## 1. Visão

### 1.1 Nome e identidade
- **Nome:** Orvane (`orv` na CLI, extensão `.orv`, módulo Python `orvane`).
- **Origem do nome:** palavra cunhada; "vane" evoca a biruta/cata-vento — o instrumento que *aponta a direção*, como a intenção aponta o objetivo do programa.
- **Tagline:** *"Diga o que precisa acontecer. Orvane escolhe como."*

### 1.2 O que significa "orientada a intenção" (definição OPERACIONAL)
Não é marketing. Em Orvane, isso significa exatamente:

1. O programador declara **intents**: *o quê* se quer (assinatura + pré-condições `given` + garantias `ensure`).
2. O programador (ou bibliotecas) declaram **strategies** (`how`): *maneiras* possíveis de cumprir a intent, cada uma com guarda (`when`), prioridade (`priority`) e nome (`via`).
3. Ao chamar uma intent, o **planner do runtime** (determinístico, **sem LLM**) seleciona a estratégia aplicável de maior prioridade, executa, **verifica as garantias**, e em caso de falha **cai para a próxima estratégia**.
4. Toda decisão é **explicável**: `orv run --explain` imprime a árvore de tentativas.
5. Além disso (inspirado no Kof): **menos cerimônia** — `data` declara tipo + construtor + igualdade + display + conversão para/de dict automaticamente.

### 1.3 A "sacada": interop com Python
- **Orvane → Python:** `use py numpy as np` importa qualquer pacote Python instalado. Chamadas são dinâmicas (`Py`), e o **contrato da intent (`ensure`) valida o que volta do Python** — Python vira "tipado na fronteira".
- **Python → Orvane:** `import orvane; m = orvane.load("x.orv"); m.shipping_cost(order)` (wheel via maturin).
- Um mesmo arquivo `.orv` funciona nos dois sentidos.

### 1.4 Não-objetivos da v0.1 (NÃO IMPLEMENTAR)
LSP · macros · criptografia · gerenciador de pacotes próprio (use `pip`/`cargo`) · compilação nativa/JVM/JS · concorrência/async · traits/interfaces · generics definidos pelo usuário (só M10) · GC com detecção de ciclos · FFI com C · otimizações · syntax freeze.

### 1.5 Orçamento de escopo da v0.1
- Máx. **8 subcomandos** de CLI.
- Máx. **5 módulos** de stdlib nativa (o resto vem do Python).
- Sintaxe **NÃO é congelada** antes da 0.5 (só após exemplos reais de pelo menos 3 pessoas).

---

## 2. Lições do projeto anterior (aura-lang) — o que NÃO repetir

> Observações feitas a partir do README público de `aura-lang`. Não foi feita auditoria do código.

| Sintoma no aura-lang | Regra em Orvane |
|---|---|
| Linguagem "transpila para Python" — o Python define a semântica | Orvane tem **interpretador próprio** (semântica dele). Python é *host de bibliotecas*, não *definidor da linguagem* |
| Ideia central era "um jeito de escrever cada coisa"; nada no núcleo era diferente de uma linguagem comum | O **núcleo é a intent/strategy/planner** (M6). Sem ele o projeto não existe |
| Syntax freeze na `0.2.0a1`, sem usuários | Sem freeze até 0.5 |
| 17 subcomandos, 18 módulos de stdlib, LSP, macros, crypto pós-quântica em Python puro marcada "não segura" | Orçamento de escopo §1.5; sem crypto; sem LSP |
| 3.000+ testes, mas sobre superfície enorme e instável | Poucos recursos, **cada um com golden tests** |
| Tudo em Python (transpilador + runtime) | Rust; núcleo sem dependência de Python (`orv-py` isolado) |

**Regra de ouro:** primeiro um núcleo pequeno e *impressionante* (M0–M6), depois interop (M7–M8), depois ferramentas.

---

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

---

## 4. Visão geral da linguagem (exemplos alvo)

### 4.1 Hello
```orv
fn main() {
    print("Olá, Orvane!")
}
```

### 4.2 Dados sem cerimônia
```orv
data User {
    name: Str,
    age: Int,
    email: Str? = none,
}

fn main() {
    let u = User(name: "Mel", age: 30)
    print(u)                       // User(name: "Mel", age: 30, email: none)
    print(u == User("Mel", 30))    // true
}
```

### 4.3 Intents e strategies (o núcleo)
```orv
data Order { id: Int, total: Float, country: Str }

intent shipping_cost(o: Order) -> Float
    given o.total >= 0.0
    ensure result >= 0.0

how shipping_cost(o) via free_shipping priority 20 when o.total >= 200.0 {
    return 0.0
}

how shipping_cost(o) via domestic priority 10 when o.country == "BR" {
    return 19.9
}

how shipping_cost(o) via international {
    return 89.0
}

fn main() {
    print(shipping_cost(Order(1, 250.0, "BR")))   // 0.0    (free_shipping)
    print(shipping_cost(Order(2, 50.0, "BR")))    // 19.9   (domestic)
    print(shipping_cost(Order(3, 50.0, "US")))    // 89.0   (international)
}
```

### 4.4 Fallback + Python na fronteira
```orv
use py requests

intent fetch_rate(currency: Str) -> Float
    ensure result > 0.0

how fetch_rate(currency) via api priority 10 {
    let r = requests.get("https://api.example.com/rate/" + currency, timeout: 3)
    return r.json()["rate"] as Float     // `as Float` valida na fronteira Py→Orvane
}

how fetch_rate(currency) via cached_default {
    return 5.0
}
```
Se a chamada HTTP levantar exceção Python, ou devolver algo que não seja `Float > 0`, o planner cai para `cached_default`. `orv run --explain` mostra ambas as tentativas.

### 4.5 Python chamando Orvane
```python
import orvane
m = orvane.load("shipping.orv")
print(m.shipping_cost({"id": 1, "total": 250.0, "country": "BR"}))  # 0.0
```

---

## 5. Especificação da linguagem

### 5.1 Léxico
- **Encoding:** UTF-8. BOM ignorado.
- **Comentários:** `// linha` e `/* bloco aninhável */`.
- **Identificadores:** `[A-Za-z_][A-Za-z0-9_]*` (ASCII na v0.1; Unicode = ADR futura).
- **Keywords (reservadas):**
  `fn intent how given ensure when via priority data enum let mut if else match for in while return break continue use as try fail true false none and or not pub test`
- **`py` é contextual (NÃO reservada):** `py` é keyword **apenas logo após `use`** (`use py math as m`). Em qualquer outro lugar é um identificador comum, pois o módulo `py` está disponível sem `use` e expõe `py.eval` / `py.exec` (§6.2). Consequência para o lexer: `py` é emitido como `Ident("py")`; o parser decide se é o marcador de import Python pelo contexto. Um programa pode portanto declarar `let py = 1` ou `fn py() {}` sem erro léxico.
- **Literais:**
  - Int: `123`, `1_000`, `0xFF`, `0b1010` (tipo `Int` = `i64`).
  - Float: `1.5`, `2e10`, `1_0.5`.
  - Str: `"..."` com escapes `\n \t \r \\ \" \{ \}` e **interpolação** `"Olá {name}"` / `"{a + b}"` (use `{{` e `}}` para chaves literais).
  - Bool: `true`, `false`. Nulo: `none`.
- **Operadores/pontuação:**
  `+ - * / % == != < <= > >= = += -= *= /= => -> . , : ; ( ) [ ] { } #{ .. ..= ? ?. ??`
- **Newline:** o lexer emite `Newline` (múltiplos colapsados). Regras:
  1. O lexer mantém uma **pilha de delimitadores**. `Newline` é suprimido se e somente se o delimitador aberto mais interno for `(`, `[` ou `#{`. Um `{` de bloco empilha um contexto em que `Newline` é significativo, mesmo dentro de `(`.
  2. O parser ignora `Newline` após: operador binário, `,`, `(`, `[`, `{`, `=>`, `->`, `=`.
  3. Em nível de statement, `Newline` (ou `;`) termina o statement.
- **Interpolação de string:** o lexer emite **um único token** `Str(parts)` onde `parts: Vec<StrPart>` e `StrPart = Lit(String) | Expr { src: String, span: Span }`. O parser faz sub-parse de cada `Expr.src` com `span` deslocado para diagnósticos corretos.

### 5.2 Gramática (EBNF v0.1)
```ebnf
program      = { NEWLINE | item } ;
item         = use_decl | data_decl | enum_decl | fn_decl
             | intent_decl | how_decl | test_decl ;

use_decl     = "use" [ "py" ] path [ "as" IDENT ] ;
path         = IDENT { "." IDENT } ;

data_decl    = [ "pub" ] "data" IDENT "{" [ field { "," field } [ "," ] ] "}" ;
field        = IDENT ":" type [ "=" expr ] ;

enum_decl    = [ "pub" ] "enum" IDENT "{" variant { "," variant } [ "," ] "}" ;
variant      = IDENT [ "(" type { "," type } ")" ] ;

fn_decl      = [ "pub" ] "fn" IDENT "(" [ params ] ")" [ "->" type ] block ;
params       = param { "," param } [ "," ] ;
param        = IDENT ":" type [ "=" expr ] ;

intent_decl  = [ "pub" ] "intent" IDENT "(" [ params ] ")" "->" type
               { intent_clause } [ block ] ;
intent_clause= ( "given" | "ensure" ) expr ;

how_decl     = "how" IDENT "(" [ IDENT { "," IDENT } ] ")" { how_mod } block ;
how_mod      = "via" IDENT | "priority" INT | "when" expr ;

test_decl    = "test" STRING block ;

type         = base_type [ "?" ] ;
base_type    = "Py"
             | IDENT [ "<" type { "," type } ">" ]
             | "(" ")"                              (* Unit *)
             | "(" type "," type { "," type } ")"   (* tuple, ≥ 2 *)
             | "fn" "(" [ type { "," type } ] ")" "->" type ;

block        = "{" { NEWLINE | stmt } "}" ;
stmt         = let_stmt | assign_stmt | while_stmt | for_stmt
             | "return" [ expr ] | "break" | "continue"
             | "fail" expr | expr ;
let_stmt     = "let" [ "mut" ] IDENT [ ":" type ] "=" expr ;
assign_stmt  = lvalue ( "=" | "+=" | "-=" | "*=" | "/=" ) expr ;
lvalue       = IDENT { "." IDENT | "[" expr "]" } ;
while_stmt   = "while" expr block ;
for_stmt     = "for" IDENT "in" expr block ;

expr         = lambda | binary ;
lambda       = ( IDENT | "(" [ IDENT { "," IDENT } ] ")" ) "=>" ( expr | block ) ;
binary       = (* Pratt, ver tabela de precedência §5.2.1 *) ;
postfix      = primary { call | index | field | "?." IDENT } ;
call         = "(" [ arg { "," arg } [ "," ] ] ")" ;
arg          = [ IDENT ":" ] expr ;                 (* nomeado: `timeout: 3` *)
index        = "[" expr "]" ;
field        = "." IDENT ;
primary      = INT | FLOAT | STRING | "true" | "false" | "none"
             | IDENT | "(" expr ")" | tuple | list | map
             | if_expr | match_expr | try_expr ;
tuple        = "(" expr "," [ expr { "," expr } ] ")" ;
list         = "[" [ expr { "," expr } [ "," ] ] "]" ;
map          = "#{" [ map_entry { "," map_entry } [ "," ] ] "}" ;
map_entry    = expr ":" expr ;
if_expr      = "if" expr block { "else" "if" expr block } [ "else" block ] ;
match_expr   = "match" expr "{" match_arm { match_arm } "}" ;
match_arm    = pattern [ "if" expr ] "=>" ( expr | block ) [ "," ] ;
pattern      = "_" | literal | IDENT
             | IDENT "(" [ pattern { "," pattern } ] ")" ;   (* variante/data posicional *)
try_expr     = "try" expr ;                          (* → Result<T, Failure> *)
```
**Decisões de projeto para evitar ambiguidade:**
- **Não existe literal de `data` com chaves.** Construção é por chamada: `User(name: "x", age: 3)` ou `User("x", 3)`. Isso elimina o conflito `expr {` × `block`.
- Literal de mapa usa `#{ }`; `{ }` é sempre bloco.
- `(a, b) => ...` é lambda se, ao ver `(`, o scan até o `)` correspondente encontra `=>` logo em seguida (lookahead de tokens).
- `if` e `match` são **expressões** (têm valor).

#### 5.2.1 Precedência (menor → maior)
| Nível | Operadores | Assoc. |
|---|---|---|
| 1 | `??` | direita |
| 2 | `or` | esquerda |
| 3 | `and` | esquerda |
| 4 | `not` (prefixo) | — |
| 5 | `== != < <= > >=` | não-associativo |
| 6 | `..` `..=` | não-associativo |
| 7 | `+ -` | esquerda |
| 8 | `* / %` | esquerda |
| 9 | `as` (cast: `expr as Type`) | esquerda |
| 10 | `-` unário | — |
| 11 | postfix: chamada, `[]`, `.`, `?.` | esquerda |

### 5.3 Sistema de tipos
- **Primitivos:** `Int` (i64), `Float` (f64), `Bool`, `Str`, `()` (Unit).
- **Compostos embutidos:** `List<T>`, `Map<K, V>` (K ∈ {Int, Str, Bool}), tuplas `(A, B)`, `T?` (opcional), `Result<T, Failure>`, `fn(A) -> B`.
- **Definidos pelo usuário:** `data`, `enum`.
- **`Py`:** tipo dinâmico do host Python (§6).
- **Sem `Any`.** Sem herança. Sem null implícito (só `T?`).
- **Inferência:** local e bidirecional. `let x = 1` infere; parâmetros de `fn`/`intent` **anotados obrigatoriamente**; retorno de `fn` sem anotação = `()`. Lambda infere parâmetros do **tipo esperado**; se não houver contexto → `E0310`.
- **Conversões:** só explícitas com `as`. `Int as Float` sempre ok. `Py as T` é **checado em runtime** (§6.3). Qualquer outro `as` inválido → `E0311`.
- **Opcionais:** `x ?? default`, `x?.campo`, `match x { none => ..., v => ... }`. Usar `T?` onde `T` é esperado → `E0305`.
- **Igualdade:** estrutural para `data`/`enum`/listas/mapas/tuplas. `Py == Py` delega ao `==` do Python. `Float` usa `==` IEEE (sem tolerância).
- **Truthiness:** inexistente. `if` exige `Bool`.
- **`data` gera automaticamente:** construtor (posicional e nomeado, com defaults), `==`, `Display` (`print`), `to_map() -> Map<Str, Py>` e `T.from_map(m)` (ver §6.4).
- **Variáveis:** `let` imutável; `let mut` mutável. Atribuir a imutável → `E0230`.
- **Inteiros:** overflow, divisão por zero e `%` por zero geram `Failure` (não wrap, não panic).

### 5.4 Semântica das intents (NÚCLEO — ler com atenção)

#### 5.4.1 Declaração
```orv
intent NAME(params) -> T
    given  <bool expr>       // pré-condição (zero ou mais)
    ensure <bool expr>       // pós-condição; `result` = valor retornado (zero ou mais)
    [ { corpo default } ]    // opcional: vira strategy `default` de prioridade 0
```
```orv
how NAME(p1, p2) via <label> priority <int> when <bool expr> { corpo }
```
- `via` opcional (default: `strategy_<n>`); `priority` opcional (default `0`); `when` opcional (default: sempre aplicável).
- Os nomes de parâmetros no `how` **devem coincidir em número** com a intent; **tipos vêm da intent** (por isso não são repetidos).
- `given`, `ensure` e `when` **devem ser puras** na v0.1: proibido chamar `intent` dentro delas (`E0410`); efeitos colaterais dentro delas são comportamento indefinido documentado (não checado).
- Uma `intent` sem nenhum `how` **e** sem corpo default → `E0401`.

#### 5.4.2 Algoritmo de resolução (implementar EXATAMENTE assim)
```
resolve(intent, args):
  1. Vincular params.
  2. Para cada `given` em ordem: se avaliar false → falha IMEDIATA
     Failure{kind:"IntentPrecondition"} (culpa do chamador, SEM fallback).
  3. candidatos = [ s ∈ strategies(intent) | s.when ausente OU avalia true ]
     (avaliar `when` na ordem de declaração)
  4. ordenar candidatos por priority DESC, desempate por ordem de declaração (ordenação ESTÁVEL).
  5. Se candidatos vazio → Failure{kind:"IntentNoStrategy"}.
  6. Para cada s em candidatos:
       registrar tentativa no Trace
       r = executar corpo de s
       se r levantou Failure (fail, erro de runtime, exceção Python):
            trace.attempt(s, Failed(r)); continuar
       vincular `result = r`
       se TODOS os `ensure` avaliam true:
            trace.attempt(s, Ok); retornar r
       senão:
            trace.attempt(s, EnsureViolated(idx do ensure)); continuar
  7. Failure{kind:"IntentUnsatisfied", attempts: trace}
```
- Um `return` dentro do corpo do `how` devolve o valor da strategy. Corpo sem `return` devolve o valor da última expressão (como qualquer bloco).
- Recursão de intent (uma intent chamando a si mesma) é permitida; limite de profundidade padrão **256** → `Failure{kind:"StackOverflow"}`.
- **Efeitos colaterais:** strategies que falham no meio podem ter deixado efeitos. Na v0.1 isso é **responsabilidade do programador** (documentar em `docs/intents.md`: "strategies devem ser idempotentes ou revertíveis").

#### 5.4.3 Trace e `--explain`
```rust
pub struct Trace { pub intent: String, pub args_repr: Vec<String>, pub attempts: Vec<Attempt> }
pub struct Attempt { pub via: String, pub priority: i64, pub outcome: Outcome }
pub enum Outcome { Ok, Failed(String), EnsureViolated { index: usize, expr_src: String }, SkippedWhenFalse }
```
Saída de `orv run --explain` (formato **fixo**, coberto por golden):
```
intent fetch_rate("USD")
  ├─ via api (priority 10)            ✗ failed: PyError ConnectionError: ...
  └─ via cached_default (priority 0)  ✓ ok  → 5.0
```
Estratégias filtradas por `when=false` aparecem só com `--explain=all` como `– skipped (when false)`.

#### 5.4.4 Checagens de sema (intents)
| Código | Condição |
|---|---|
| E0401 | intent sem strategies e sem corpo |
| E0402 | `how` referencia intent inexistente |
| E0403 | número de parâmetros do `how` ≠ intent |
| E0404 | `given`/`ensure`/`when` não é `Bool` |
| E0405 | `via` duplicado na mesma intent |
| E0406 | `result` usado fora de `ensure` |
| E0410 | chamada de intent dentro de `given`/`ensure`/`when` |
| E0411 | corpo do `how` retorna tipo ≠ tipo da intent |

### 5.5 Erros em tempo de execução
- **Um único mecanismo:** `Failure { kind: Str, message: Str, trace: List<Str> }`.
- Fontes: `fail "msg"`, erros de runtime (div/0, overflow, índice fora), `ensure`/`given` violados, exceções Python (§6.5).
- `try expr` avalia `expr` e devolve `Result<T, Failure>` (`Ok(v)` / `Err(f)`), nunca propaga.
- Failure não capturada em `main` → diagnóstico de runtime com pilha de chamadas e **exit code 1**.
- `Result` tem: `is_ok()`, `is_err()`, `unwrap_or(d)`, `map(f)`. O operador `?` **não** existe na v0.1 (ADR futura).

### 5.6 Módulos
- v0.1: **um arquivo = um módulo.** `use util` (busca `util.orv` na pasta do arquivo atual), `use util.math as m`.
- Somente itens `pub` são importáveis. Ciclos de import → `E0250`.
- `use py <mod>` importa módulo Python (§6.1). `use py numpy.linalg as la` permitido.
- Prelude implícito: `print`, `len`, `range`, `str`, `int`, `float`, `Ok`, `Err`, `Failure`, `assert`.

### 5.7 Testes embutidos
```orv
test "shipping é gratuito acima de 200" {
    assert shipping_cost(Order(1, 250.0, "BR")) == 0.0
}
```
`assert expr` (função do prelude) falha com `Failure{kind:"AssertionFailed"}` mostrando a expressão-fonte. `orv test` roda todos os `test` do arquivo/diretório e reporta `N passed, M failed`.

---

## 6. Interop com Python (a "sacada")

### 6.1 Importar
```orv
use py math
use py numpy as np
use py os.path as osp
```
- Import é **preguiçoso**: acontece na primeira execução do `use`, não na sintaxe. Módulo ausente → `Failure{kind:"PyImportError"}` com `help: "pip install <pkg>"`.
- O nome introduzido tem tipo `Py`.

### 6.2 Operações dinâmicas sobre `Py`
Todas retornam `Py`:
- atributo: `np.pi`; chamada: `np.array([1,2,3])`; chamada com kwargs: `requests.get(url, timeout: 3)`;
- índice: `df["col"]`; iteração: `for x in py_iterable { ... }`;
- operadores `+ - * / % == != < <= > >=` entre `Py` e (`Py` | literal Orvane convertido) delegam ao Python.
- `if py_value {}` **não** compila (`E0312`) — use `py_value as Bool`.
- Argumentos Orvane passados ao Python são convertidos (§6.4).
- Existem também: `py.eval("expr") -> Py` e `py.exec("codigo") -> ()` (módulo especial `py`, disponível sem `use`).

### 6.3 Cast checado `Py as T` (a fronteira tipada)
`expr as T` onde `expr: Py`:
1. Converte o objeto Python para `T` conforme a tabela §6.4.
2. Se impossível → `Failure{kind:"PyCastError", message:"expected Float, got <tipo python>"}` (capturável, dispara fallback de intent).
3. Para `data D`: aceita `dict` com as chaves dos campos **ou** objeto com atributos homônimos; valida recursivamente os tipos dos campos.
4. `as T?`: `None` → `none`.

### 6.4 Tabela de conversão
| Orvane | → Python | Python → | Orvane (via `as`) |
|---|---|---|---|
| `Int` | `int` | `int` | `Int` (fora de i64 → `PyCastError`) |
| `Float` | `float` | `float`, `int` | `Float` |
| `Bool` | `bool` | `bool` | `Bool` |
| `Str` | `str` | `str` | `Str` |
| `none` | `None` | `None` | `T?` = `none` |
| `List<T>` | `list` | `list`, `tuple` | `List<T>` (elementos convertidos) |
| `Map<K,V>` | `dict` | `dict` | `Map<K,V>` |
| tupla | `tuple` | `tuple` | tupla (aridade checada) |
| `data D` | `dict` (v0.1; ADR futura para classe) | `dict` / objeto | `D` |
| `enum` | `dict {"variant": str, "values": list}` | idem | enum |
| `fn(...)` | callable Python | callable | (não convertível: `PyCastError`) |
| `Py` | o próprio objeto | — | — |

`Py as Float` de um `numpy.float64`: aceitar via protocolo `__float__`. `Py as Int` via `__index__`. Documentar essas duas exceções.

### 6.5 Exceções
- Qualquer exceção Python vira `Failure{ kind: "PyError", message: "<TipoExc>: <msg>", trace: [linhas do traceback Python] }`.
- Nunca vazar traceback bruto do Rust/PyO3 ao usuário.
- `KeyboardInterrupt` interrompe o programa (exit 130).

### 6.6 Ambiente Python
Ordem de descoberta: (1) `ORV_PYTHON`; (2) `VIRTUAL_ENV`/`CONDA_PREFIX`; (3) `python3`/`python` no `PATH`.
`orv doctor` imprime: caminho, versão, `sys.path` resumido, e se `pip` está disponível.
Versão < 3.10 → erro claro e exit 2.

### 6.7 Threads/GIL
v0.1 é **single-thread**. O runtime mantém o GIL durante toda a execução. Nenhum recurso de concorrência.

### 6.8 Orvane chamado de Python (`orv-pymod`)
```python
import orvane
m = orvane.load("shipping.orv")        # compila+checa; levanta orvane.OrvaneError com diagnósticos
m.shipping_cost({"id":1,"total":250.0,"country":"BR"})   # intent OU fn → resolve/chama
m.intents                               # ['shipping_cost', ...]
m.explain("shipping_cost", {...})       # devolve o Trace como dict (mesmo formato de --json)
orvane.run("x.orv")                     # executa main()
```
- Argumentos Python → Orvane pela tabela inversa de §6.4 usando **os tipos da assinatura** (`data D` aceita `dict`).
- `Failure` → exceção `orvane.OrvaneFailure` (com `.kind`, `.message`, `.attempts`).
- Build: `maturin` (modo `extension-module`); considerar `abi3` (ADR).

### 6.9 Isolamento e testabilidade
`orv-runtime` define:
```rust
pub trait Host {
    fn import(&mut self, module: &str) -> Result<HostObj, Failure>;
    fn get_attr(&mut self, o: &HostObj, name: &str) -> Result<Value, Failure>;
    fn call(&mut self, o: &HostObj, args: &[Value], kwargs: &[(String, Value)]) -> Result<Value, Failure>;
    fn get_item(&mut self, o: &HostObj, key: &Value) -> Result<Value, Failure>;
    fn binary_op(&mut self, op: BinOp, l: &Value, r: &Value) -> Result<Value, Failure>;
    fn iterate(&mut self, o: &HostObj) -> Result<Vec<Value>, Failure>;
    fn cast(&mut self, o: &HostObj, ty: &Type) -> Result<Value, Failure>;
    fn to_host(&mut self, v: &Value) -> Result<HostObj, Failure>;
}
```
`HostObj` é um wrapper opaco (`Rc<dyn Any>`). Implementações: `NoHost` (todo método devolve `Failure{kind:"PyUnavailable"}`), `MockHost` (para testes sem Python), `PyHost` (em `orv-py`).

---

## 7. Arquitetura do compilador/runtime

```
fonte .orv
  → Lexer            (Vec<Token>, Diagnostics)
  → Parser           (AST não-tipado, Diagnostics; com recuperação de erro por statement)
  → Sema.resolve     (tabela de símbolos, imports)
  → Sema.typeck      (AST tipado: cada Expr recebe TypeId)
  → Sema.intents     (checagens §5.4.4; monta IntentTable)
  → Runtime.interp   (tree-walking sobre AST tipado; Planner; Host)
```
- **Interpretador tree-walking** primeiro (M4). Bytecode VM só no M11 e apenas se houver necessidade medida.
- **Value:**
```rust
pub enum Value {
    Unit, Bool(bool), Int(i64), Float(f64), Str(Rc<str>),
    List(Rc<RefCell<Vec<Value>>>), Map(Rc<RefCell<MapImpl>>), Tuple(Rc<[Value]>),
    Data { ty: DataId, fields: Rc<[Value]> },
    Enum { ty: EnumId, variant: u32, payload: Rc<[Value]> },
    Opt(Option<Box<Value>>),
    Func(Rc<Closure>),
    Host(HostObj),
}
```
- **Memória:** `Rc` sem detecção de ciclos (ciclos vazam; documentar). Sem `unsafe`.
- **Ambiente:** pilha de frames com slots resolvidos em sema (índice, não nome).
- **Limites de segurança:** profundidade de chamada 256 (configurável), `--max-steps N` opcional para testes.

---

## 8. Diagnósticos

### 8.1 Formato
```rust
pub struct Diagnostic {
    pub code: &'static str,        // ex.: "E0301"
    pub severity: Severity,        // Error | Warning
    pub message: String,
    pub primary: Span,
    pub labels: Vec<(Span, String)>,
    pub help: Option<String>,
}
```
Renderização com `ariadne`. Formato de teste (`.err` golden): `CODE:linha:coluna: mensagem` (uma por linha), para ser estável.

### 8.2 Faixas de código
| Faixa | Fase |
|---|---|
| E00xx | léxico (`E0001` caractere inválido, `E0002` string não terminada, `E0003` comentário não fechado, `E0004` escape inválido) |
| E01xx | parser (`E0101` token inesperado, `E0102` esperado X, `E0103` bloco não fechado) |
| E02xx | resolução (`E0201` nome indefinido, `E0202` duplicado, `E0230` atribuição a imutável, `E0250` ciclo de import, `E0251` item não `pub`) |
| E03xx | tipos (`E0301` tipos incompatíveis, `E0302` aridade, `E0305` opcional não tratado, `E0310` lambda sem contexto, `E0311` cast inválido, `E0312` truthiness) |
| E04xx | intents (§5.4.4) |
| E05xx | interop (`E0501` `use py` fora do topo, `E0502` `Py` onde tipo concreto é exigido sem `as`) |
| Rxxxx | runtime (`R0001` div/0, `R0002` overflow, `R0003` índice fora, `R0004` stack overflow, `R0010` intent insatisfeita, `R0020` erro Python) |

Todo código novo **deve** ser adicionado a `docs/errors.md` com exemplo mínimo.

---

## 9. Biblioteca padrão nativa (máx. 5 módulos)
| Módulo | Conteúdo |
|---|---|
| prelude | `print`, `len`, `range`, `str`, `int`, `float`, `assert`, `Ok`, `Err` |
| `std.text` | `split`, `join`, `trim`, `upper`, `lower`, `contains`, `replace`, `starts_with`, `ends_with` |
| `std.list` | métodos: `map`, `filter`, `reduce`, `any`, `all`, `sort`, `push`, `pop`, `contains`, `first`, `last` |
| `std.math` | `abs`, `min`, `max`, `sqrt`, `pow`, `floor`, `ceil` |
| `std.io` | `read_file`, `write_file`, `read_line`, `args` |

**JSON, HTTP, datas, regex, banco etc.: usar Python** (`use py json`, `use py requests`...). Esse é o ponto: **a stdlib é o PyPI**.

---

## 10. CLI (`orv`) — máx. 8 subcomandos
| Comando | Função |
|---|---|
| `orv run <f.orv> [--explain[=all]] [--json]` | executa `main()`; `--json` emite traces em JSON |
| `orv check <f.orv>` | lex+parse+sema, sem executar |
| `orv test [path]` | roda blocos `test` |
| `orv fmt <f.orv> [-w]` | formatador (M9) |
| `orv repl` | REPL (M9) |
| `orv doctor` | diagnóstico de ambiente Python |
| `orv ast <f.orv>` | debug: dump da AST (texto estável) |
| `orv version` | versão |

Exit codes: `0` ok · `1` erro de programa/diagnóstico · `2` erro de ambiente/uso · `130` interrupção.

---

## 11. Estratégia de testes
1. **Unitários** por crate (lexer: tokens; parser: AST dump; sema: diagnósticos; runtime: valores).
2. **Golden E2E** em `tests/golden/`: `nome.orv` + `nome.out` (stdout) e/ou `nome.err` (diagnósticos). Um harness único percorre a pasta. Atualização de snapshots só com `INSTA_UPDATE`/flag explícita — **nunca automática em CI**.
3. **Property tests** (`proptest`): lexer nunca dá panic para qualquer `String`; parser nunca dá panic; `fmt(fmt(x)) == fmt(x)` (M9).
4. **Conformance de intents:** pasta `tests/golden/intents/` com no mínimo os 12 casos do M6.
5. **Python (`tests/python/`)**: pytest sobre o wheel; gated por feature `python-tests` no Cargo e job próprio no CI (matriz 3.10–3.13).
6. **`docs/STATUS.md`**: tabela `feature → teste(s) golden`. Feature sem teste = "Planned".

---

## 12. Roadmap em milestones

> Cada milestone termina com um **Gate**: comandos que devem passar. Sem gate verde, não avance.
> Estimativas em linhas de Rust são orientativas (para o agente saber quando está fugindo do escopo).

### M0 — Bootstrap (~300 LOC)
**Entregar:** workspace com as 6 crates (vazias mas compilando), `rust-toolchain.toml`, CI (fmt/clippy/test em Linux+Windows), `AGENTS.md`, `docs/` esqueleto, `Span`/`SourceMap`/`Diagnostic` + renderização `ariadne`, harness golden (`tests/golden_runner.rs`) percorrendo a pasta.
**Gate:** `cargo test` roda o harness com 1 caso golden trivial; `orv version` imprime a versão.

### M1 — Lexer (~600 LOC)
**Entregar:** tokens de §5.1 incluindo `Newline`, strings com interpolação (`Str(parts)`), números com `_`/hex/bin, comentários aninhados, erros `E0001–E0004`.
**Testes:** ≥ 40 casos de unidade; proptest "nunca panic"; golden `tokens_*.orv`.
**Testes obrigatórios (supressão de `Newline`, §5.1 regra 1):**
1. lambda com bloco dentro de chamada — o `{` de bloco torna `Newline` significativo mesmo dentro de `(`:
   ```orv
   xs.map(x => {
       let y = 1
       y
   })
   ```
   `Newline` deve ser emitido após `let y = 1` e após `y`.
2. literal `#{ }` multilinha — chaves de mapa **suprimem** `Newline`:
   ```orv
   let m = #{
       "a": 1,
       "b": 2,
   }
   ```
   Nenhum `Newline` deve ser emitido dentro do `#{ }`, e o `Newline` após o `}` de fechamento deve ser emitido.
**Gate:** `orv tokens x.orv` (subcomando de debug, não conta no orçamento — remover no M12) dumpa tokens estáveis.

### M2 — Parser (~1.500 LOC)
**Entregar:** AST completa para §5.2 **exceto** `intent/how/test/use py` (só o esqueleto do enum de item). Pratt parser conforme §5.2.1. Recuperação de erro: ao errar, sincronizar no próximo `Newline`/`}` e continuar (múltiplos erros por arquivo). `orv ast` dumpa a AST.
**Testes:** golden AST para ≥ 25 programas; casos de precedência (`1 + 2 * 3 as Float`, `a ?? b or c`); lambda vs parênteses; erros `E0101–E0103`.
**Gate:** todos os exemplos de §4.1–§4.2 parseiam.

### M3 — Sema v1: nomes e tipos (~1.800 LOC)
**Entregar:** resolução de nomes (escopos, shadowing, imports de arquivo — sem `py` ainda), typecheck de §5.3 para primitivos, `List`, `Map`, tuplas, `T?`, funções, lambdas com tipo esperado, `as`. Erros `E02xx`, `E03xx`.
**Testes:** ≥ 30 golden `.err` (um por código de erro) + ≥ 15 programas válidos.
**Gate:** `orv check` aceita programas válidos e rejeita cada caso com **o código certo**.

### M4 — Interpretador v1 (~1.500 LOC)
**Entregar:** `Value`, avaliação de expressões/statements, `fn`, closures, `while`/`for` (com `range` e `..`), `print`, prelude, `std.text/list/math` básicos, erros de runtime `R000x` com pilha de chamadas.
**Testes:** golden para `fib`, `fizzbuzz`, `primes`, `collatz`, ordenação, closures capturando `let mut`.
**Gate:** `orv run examples/fib.orv` imprime o esperado; overflow e div/0 dão `R0002`/`R0001` e exit 1.

### M5 — `data`, `enum`, `match` (~1.200 LOC)
**Entregar:** `data` com construtor posicional/nomeado/defaults, `==`, `Display`; `enum` com payload; `match` com guardas e checagem de **exaustividade** (`E0320` não exaustivo); opcionais (`??`, `?.`).
**Gate:** exemplo §4.2 roda com a saída indicada; `match` não exaustivo é rejeitado.

### M6 — INTENTS (o coração) (~1.500 LOC)
**Entregar:** `intent`, `how`, `given`, `ensure`, `result`, planner exatamente como §5.4.2, `Trace`, `--explain`, `--explain=all`, `--json`, `fail`, `try`, `Result`. Checagens E040x.
**Testes obrigatórios (12 casos mínimos em `tests/golden/intents/`):**
1. escolhe maior prioridade;
2. desempate por ordem de declaração;
3. `when` falso filtra;
4. `given` falso ⇒ `IntentPrecondition` sem fallback;
5. `fail` na primeira ⇒ cai para a segunda;
6. `ensure` violado ⇒ cai para a próxima;
7. todas falham ⇒ `IntentUnsatisfied` com trace completo;
8. intent com corpo default;
9. intent recursiva; estouro ⇒ `R0004`;
10. `--explain` (formato exato);
11. `--json` (schema estável);
12. exemplo §4.3 completo.
**Gate:** os 12 casos passam. **Este é o milestone que decide se o projeto tem identidade — não apresse.**

### M7 — Python → uso (`use py`) (~1.400 LOC)
**Entregar:** trait `Host`, `NoHost`, `MockHost`, `PyHost` (crate `orv-py`); `use py`, atributo/chamada/kwargs/índice/iteração/operadores sobre `Py`; `py.eval/exec`; `Py as T` (§6.3–6.4); exceções ⇒ `PyError`; `orv doctor`; descoberta de ambiente (§6.6). Lembrar que `py` é **contextual** (§5.1): o lexer sempre emite `Ident("py")` e o parser só o trata como import Python imediatamente após `use`.
**Testes:** com `MockHost` (sem Python) para a lógica; com Python real (feature `python-tests`) para `math`, `json`, `os.path`, e `numpy` **se instalado** (teste marcado opcional).
**Gate:** exemplo §4.4 roda com `requests` **mockado via `py.exec`** (não dependa de rede nos testes): a 1ª strategy falha por exceção, a 2ª responde; `--explain` mostra as duas.

### M8 — Python → Orvane (`orvane` wheel) (~800 LOC)
**Entregar:** crate `orv-pymod` (maturin), `orvane.load/run`, `m.<fn|intent>(...)`, `m.intents`, `m.explain`, conversão via assinatura, `OrvaneError`/`OrvaneFailure`.
**Testes:** `tests/python/test_*.py` (pytest).
**Gate:** `maturin develop && pytest tests/python` verde; exemplo §4.5 funciona.

### M9 — Ferramentas (~1.500 LOC)
**Entregar:** `orv fmt` (idempotente; preserva comentários), `orv test` + blocos `test`/`assert`, `orv repl` (estado persistente, `:type`, `:reset`, `:q`).
**Testes:** proptest `fmt(fmt(x)) == fmt(x)`; golden do formatador em ≥ 20 programas.
**Gate:** todos os `examples/` são fixos-de-ponto do formatador.

### M10 — Generics e módulos (~1.500 LOC)
**Entregar:** generics para `fn`/`data`/`enum` definidos pelo usuário (monomorfização **não**; verificação por substituição + apagamento em runtime), `use` multi-arquivo com `pub`, ciclos `E0250`.
**Gate:** exemplo com `data Box<T>`, `fn map_list<A,B>(...)` e duas unidades de compilação.

### M11 — (Opcional) Bytecode VM
Só se benchmarks (`examples/bench/`) mostrarem necessidade. ADR obrigatória antes de começar.

### M12 — Release 0.1.0
README com **status honesto** gerado de `docs/STATUS.md`; `docs/` completos; remover `orv tokens`; exemplos em `examples/`; checar disponibilidade do nome (§16); publicar wheel de teste (TestPyPI) e crate.

---

## 13. Definition of Done (global)
- [ ] Gate do milestone verde.
- [ ] `cargo fmt --check` · `cargo clippy --workspace --all-targets -- -D warnings` · `cargo test --workspace` limpos.
- [ ] Todo código de erro novo em `docs/errors.md`.
- [ ] `docs/STATUS.md` atualizado (feature → teste).
- [ ] Sem `TODO` sem issue/ADR associada.
- [ ] Sem `unwrap`/`expect`/`panic!` fora de testes.
- [ ] Nenhum arquivo > 600 linhas sem justificativa em ADR.
- [ ] Relatório final da sessão (feito / fora de escopo / ADRs / próximo).

---

## 14. Prompt de sessão (cole no início de cada sessão)

```
Você é o engenheiro responsável por implementar a linguagem Orvane em Rust.
Contexto obrigatório (leia integralmente antes de agir): AGENTS.md e docs/SPEC.md §1, §3 e o milestone <Mx> do §12,
mais as seções técnicas que ele cita.

Tarefa: implementar APENAS o milestone <Mx>.

Regras:
1) Plano primeiro: liste as sub-tarefas (máx. 10) e a ordem. Espere minha confirmação apenas se houver ambiguidade real; senão, siga.
2) Implemente sub-tarefa por sub-tarefa; faça commit após cada uma (`<Mx>: <verbo> <coisa>`).
3) Escreva os testes ANTES ou junto do código; nunca depois.
4) Não crie funcionalidades fora do milestone. Não invente sintaxe: se faltar decisão, escreva uma ADR curta e adote o padrão mais simples.
5) Ao final rode e cole a saída de:
   cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
6) Entregue o relatório: feito / fora de escopo / ADRs / riscos / próximo milestone.
```

**Prompt de correção (quando algo quebrar):**
```
O gate do milestone <Mx> falhou. Saída do erro abaixo.
Reproduza com um teste mínimo, conserte a causa raiz (não o sintoma), não altere/apague testes existentes para passar,
e mostre o diff e a nova saída do gate.
<cole a saída>
```

---

## 15. Riscos e decisões em aberto (ADRs a criar quando chegar a hora)
| # | Tema | Padrão da v0.1 |
|---|---|---|
| 1 | Idioma das keywords | Inglês. Localização PT-BR de mensagens de erro: pós-0.1 |
| 2 | `data` ↔ Python | `dict` (não classe). Revisar após uso real |
| 3 | Efeitos colaterais em fallback | Responsabilidade do programador; considerar marcador `effect`/rollback na 0.3 |
| 4 | `?` operador | Não existe; usar `try`/`match` |
| 5 | Ciclos de referência | Vazam; aceito na 0.1 |
| 6 | `abi3` no wheel | Decidir no M8 (menos wheels vs. menos performance) |
| 7 | Estratégias "aprendidas"/por custo dinâmico | **Fora do escopo**; planner é estático e determinístico |
| 8 | Unicode em identificadores | ASCII na 0.1 |
| 9 | Concorrência/async | Fora da 0.1; interação com GIL exige ADR própria |
| 10 | Licença | Decidir antes do M12 (sugestão: MIT OU Apache-2.0) |

---

## 16. Checklist de disponibilidade do nome (fazer antes do M12 — e ideal antes do M0)
Nome candidato: **Orvane**. Uma busca web rápida não encontrou linguagem de programação com esse nome, mas isso **não é garantia**. Verifique você mesmo:
- [ ] `cargo search orvane` e https://crates.io/crates/orvane
- [ ] https://pypi.org/project/orvane/ (e `pip index versions orvane`)
- [ ] `npm view orvane`
- [ ] Busca no GitHub: `orvane language`, `orvane in:name`
- [ ] https://pldb.io (busca por "Orvane")
- [ ] Domínios/handles (`orvane.dev`, `orvane.org`, `@orvanelang`)
- [ ] Marcas (INPI Brasil / USPTO) — se houver intenção comercial
- [ ] Reservar os nomes em PyPI/crates.io com um pacote vazio no M0

Se o nome estiver ocupado, reserva de nomes alternativos (também **não verificados**): `Tavrel`, `Ilvoro`.

---

## Apêndice A — Programas de referência (devem virar golden tests)

**A1. FizzBuzz**
```orv
fn main() {
    for i in 1..=15 {
        let out = if i % 15 == 0 { "FizzBuzz" }
                  else if i % 3 == 0 { "Fizz" }
                  else if i % 5 == 0 { "Buzz" }
                  else { str(i) }
        print(out)
    }
}
```

**A2. Enum + match**
```orv
enum Shape { Circle(Float), Rect(Float, Float) }

fn area(s: Shape) -> Float {
    match s {
        Circle(r)  => 3.14159 * r * r,
        Rect(w, h) => w * h,
    }
}

fn main() { print(area(Rect(2.0, 3.0))) }   // 6.0
```

**A3. Fallback com Python mockado (sem rede)**
```orv
use py json

intent parse_price(raw: Str) -> Float
    ensure result >= 0.0

how parse_price(raw) via json_path priority 10 {
    return json.loads(raw)["price"] as Float
}

how parse_price(raw) via plain_number priority 5 {
    return float(raw)
}

how parse_price(raw) via zero {
    return 0.0
}

fn main() {
    print(parse_price("{\"price\": 12.5}"))   // 12.5  (json_path)
    print(parse_price("7.25"))                // 7.25  (json_path falha → plain_number)
    print(parse_price("abc"))                 // 0.0   (as duas primeiras falham → zero)
}
```
`orv run --explain` para a 2ª chamada deve mostrar:
```
intent parse_price("7.25")
  ├─ via json_path (priority 10)     ✗ failed: PyError JSONDecodeError: ...
  └─ via plain_number (priority 5)   ✓ ok  → 7.25
```

**A4. Precondição sem fallback**
```orv
intent sqrt_safe(x: Float) -> Float
    given x >= 0.0
{ return std.math.sqrt(x) }

fn main() {
    print(try sqrt_safe(-1.0))   // Err(Failure(kind: "IntentPrecondition", ...))
}
```

---

## Apêndice B — Checklist rápida por sessão (para o humano)
1. Abri sessão nova com §0.2 + §1 + §3 + milestone atual?
2. O agente listou plano e sub-tarefas?
3. Os commits estão pequenos e com prefixo `Mx:`?
4. O gate rodou e a **saída completa foi colada**?
5. Há teste golden para cada feature nova?
6. `docs/STATUS.md` e `docs/errors.md` foram atualizados?
7. O agente adicionou dependência fora da lista? (se sim, recuse e peça ADR)
8. Rodei eu mesmo `cargo test` antes de aceitar?
