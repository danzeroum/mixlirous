# 05 — Agente de IA e Human-in-the-Loop

## 1. Princípio de governança

> A IA **preenche um formulário**. Ela não executa nada.

Três consequências que o código precisa refletir:

1. Toda saída do LLM passa pela `ValidationLayer` antes de existir como tipo do
   domínio. Alucinação vira erro de desserialização, não bug de áudio.
2. O agente tem **orçamento finito** de ferramentas. Sem isso, ele entra em loop
   de raciocínio e consome tokens indefinidamente.
3. O agente **não altera a topologia** do grafo sozinho. Se precisa de uma
   ferramenta que o usuário não desenhou, ele **propõe** e espera decisão.

---

## 2. Loop ReAct

```
┌──────────────────────────────────────────────────────────┐
│  entrada: user_prompt + contexto da faixa + grafo atual  │
└───────────────────────────┬──────────────────────────────┘
                            ▼
                  ┌───────────────────┐
              ┌──►│  RACIOCINAR       │  → SSE agent.thought (streaming)
              │   └─────────┬─────────┘
              │             ▼
              │   ┌───────────────────┐
              │   │  ESCOLHER TOOL    │  → SSE agent.tool_call
              │   └─────────┬─────────┘
              │             ▼
              │   ┌───────────────────┐   falha
              │   │  VALIDAR (Rust)   │──────────┐
              │   └─────────┬─────────┘          │
              │             │ ok                 ▼
              │             ▼            observação de erro
              │   ┌───────────────────┐   volta ao loop
              │   │ ferramenta existe │   (não conta budget)
              │   │ no grafo?         │
              │   └────┬─────────┬────┘
              │    sim │         │ não
              │        ▼         ▼
              │   ┌────────┐  ┌──────────────────┐
              │   │EXECUTAR│  │ PROPOR + PAUSAR  │ → SSE agent.proposal
              │   └────┬───┘  └────────┬─────────┘
              │        │               │ decisão humana
              │        ▼               ▼
              │   ┌────────────────────────┐
              └───┤ OBSERVAR + budget−−     │ → SSE agent.tool_result
                  └───────────┬────────────┘
                              │ budget == 0 ou agente sinaliza fim
                              ▼
                  ┌────────────────────────┐
                  │  CONSOLIDAR RECEITA    │ → SSE node.parameters
                  └────────────────────────┘
```

### Parâmetros do loop

| Parâmetro | Padrão | Config |
| --- | --- | --- |
| `max_tools` (budget) | 5 | `llm.max_tools` |
| Timeout por chamada LLM | 30 s | `llm.timeout_sec` |
| Tentativas por chamada | 3, backoff 500 ms exponencial | `llm.retry_policy` |
| Falhas de validação sem consumir budget | 2 | `llm.max_validation_retries` |
| Temperatura | 0,3 (dev) / 0,2 (prod) | `llm.temperature` |

**Por que falha de validação não consome budget (com teto de 2):** quando o
modelo erra o formato, dar a ele a chance de corrigir com a mensagem de erro como
observação eleva bastante a taxa de sucesso. Mas o teto impede loop de correção
infinito.

### Fim do loop

O loop termina quando: (a) budget zerado — força consolidação; (b) o modelo
responde sem `tool_call`; (c) erro irrecuperável (provedor fora, timeout total).

No caso (c), o job **não falha**: cai para o `pipeline_config` do usuário (ou
padrões) e emite `agent.error` com `will_replan: false` e um aviso na UI
("assistente indisponível — usando configuração manual"). Um LLM fora do ar não
deve impedir alguém de renderizar áudio.

---

## 3. Registry de ferramentas — TABELA CANÔNICA DE LIMITES

> **Fonte única de verdade, e agora literalmente uma só.** A tabela abaixo é
> **gerada** a partir de `audio_agent::limits::tool_registry()` — a mesma
> struct que alimenta `GET /api/v1/tools` e que `validator.rs` valida contra
> (com teste cruzado). Não edite as linhas dentro dos marcadores à mão: rode
> `cargo test -p audio_agent test_docs_05_table_matches_registry -- --nocapture`
> (o teste imprime o valor esperado quando falha) e cole o resultado. Editar
> aqui sem tocar `limits.rs` é exatamente a divergência que este mecanismo
> existe para impedir.

<!-- BEGIN GENERATED TOOLS TABLE (ver crates/audio_agent/src/limits.rs::render_markdown_table) -->
| Ferramenta | Disponível | Parâmetro | Tipo | Mín | Máx | Padrão | Unidade/Enum |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `compression` | não (not_implemented) | `ratio` | float | 1 | 10 | 2.0 | :1 |
|  |  | `threshold_db` | float | -60 | 0 | -18.0 | dB |
|  |  | `attack_ms` | integer | 0 | 500 | 30.0 | ms |
|  |  | `release_ms` | integer | 10 | 5000 | 250.0 | ms |
|  |  | `makeup_gain_db` | float | -12 | 12 | 0.0 | dB |
|  |  | `knee_db` | float | 0 | 12 | 6.0 | dB |
| `dynamic_eq` | não (not_implemented) | `bands[].freq_hz` | float | 20 | 20000 | — | Hz |
|  |  | `bands[].gain_db` | float | -24 | 24 | 0.0 | dB |
|  |  | `bands[].q` | float | 0.1 | 10 | 0.7 | — |
|  |  | `bands[].type_filter` | enum | — | — | "peak" | peak \| shelf \| highpass \| lowpass |
|  |  | `bands` | array | 1 | 8 | — | — |
| `crossfade` | sim | `duration_ms` | integer | 0 | 3000 | 1000.0 | ms |
|  |  | `curve` | enum | — | — | "logarithmic" | linear \| logarithmic \| exponential |
| `fade_in` | sim | `duration_ms` | integer | 0 | 10000 | 1000.0 | ms |
|  |  | `curve` | enum | — | — | "logarithmic" | linear \| logarithmic \| exponential |
| `fade_out` | sim | `duration_ms` | integer | 0 | 10000 | 1000.0 | ms |
|  |  | `curve` | enum | — | — | "logarithmic" | linear \| logarithmic \| exponential |
| `time_stretch` | sim | `factor` | float | 0.9 | 1.1 | 1.0 | × |
| `lufs_normalization` | sim | `target_lufs` | float | -30 | -6 | -14.0 | LUFS |
|  |  | `max_true_peak_db` | float | -6 | 0 | -1.0 | dBTP |
| `stem_separation` | não (not_implemented) | `model` | enum | — | — | "htdemucs" | htdemucs \| htdemucs_ft |
|  |  | `stems` | array_enum | 1 | 4 | ["drums","other"] | drums \| bass \| vocals \| other |
<!-- END GENERATED TOOLS TABLE -->

**`curve` de `crossfade` está com o vocabulário errado hoje** —
`linear`/`logarithmic`/`exponential` é o modelo de fade de entrada/saída, não
de crossfade (dois sinais somando pedem potência-constante/ganho-constante,
não uma curva de percepção de volume). Ver adendo R2 §0 — a correção
(`CrossfadeCurve` distinto de `FadeCurve`) é trabalho do pacote `docs/16`,
atrás de T0.0/T0.1; a tabela acima reflete o registry **como ele é hoje**,
não como vai ficar.

**`stem_separation.model` é lista fixa** (`htdemucs` \| `htdemucs_ft`) —
deveria vir do binário externo detectado (ADR-0010), não do código. Prioridade
baixa enquanto a ferramenta estiver `available: false`, mas não pode passar da
Sprint 3.

**`block_selection` e `target_duration` não aparecem acima** — são campos de
`pipeline_config` (ver `docs/03-CONTRATOS-API.md`), não entradas de
`tool_registry()`. Não são ferramentas que o LLM invoca por tool-calling; são
parte da configuração do job. Fora do escopo deste mecanismo de geração até
que (se) ganharem sua própria representação no registry.

### Regras cruzadas (não expressáveis como min/max)

Validadas na `ValidationLayer` **após** a desserialização:

| # | Regra | Mensagem de erro |
| --- | --- | --- |
| R1 | `attack_ms <= release_ms` | "ataque não pode ser maior que o release" |
| R2 | `ratio >= 8.0` **e** `threshold_db > -10.0` → recusa | "compressão destrutiva: ratio alto com threshold raso" |
| R3 | `preserve_intro_ms + preserve_outro_ms < target_sec * 1000` | "intro e final preservados não cabem na duração alvo" |
| R4 | bandas de EQ com `freq_hz` duplicada (±5%) | "bandas de EQ sobrepostas" |
| R5 | `lufs_normalization` só pode aparecer **uma vez** por pipeline | "normalização duplicada" |
| R6 | `crossfade.duration_ms` ≤ duração do menor bloco / 2 | "transição maior que o bloco" |
| R7 | ferramenta fora do plano do tenant | "ferramenta indisponível no seu plano" |

> **Conflito resolvido:** documentos anteriores citaram tanto 3000 ms quanto
> 5000 ms como teto de crossfade. O valor canônico é **3000 ms**, alinhado ao
> `config/default.yaml` (`crossfade_max_ms: 3000`) e ao `validator.rs` do kit.
> O teto absoluto do tipo (`CrossfadeMs::MAX`) é 3000; qualquer flexibilização
> futura passa por config, nunca por hardcode.

### Erro de validação → observação para o agente

Quando a validação falha, a mensagem devolvida ao modelo é **estruturada e
acionável**, não uma stack trace:

```json
{
  "observation_type": "VALIDATION_ERROR",
  "tool": "crossfade",
  "field": "duration_ms",
  "received": 50000,
  "constraint": { "min": 0, "max": 3000 },
  "hint": "Reduza para no máximo 3000 ms ou remova a transição."
}
```

O `hint` importa: sem ele, o modelo tende a repetir o mesmo erro.

---

## 4. Propostas — Ciclo de vida (HITL)

Este é o fluxo mais delicado do produto. Frontend e backend precisam concordar
milimetricamente.

### Máquina de estados

```
                    agente precisa de ferramenta
                    ausente no grafo
                              │
                              ▼
                     ┌────────────────┐
                     │   pending      │  TTL = 120 s
                     └───┬────┬───┬───┘
             aprova      │    │   │      expira
          ┌──────────────┘    │   └──────────────┐
          │            rejeita│                  │
          ▼                   ▼                  ▼
   ┌────────────┐      ┌────────────┐     ┌────────────┐
   │  approved  │      │  rejected  │     │  expired   │
   └─────┬──────┘      └─────┬──────┘     └─────┬──────┘
         │                   │                  │
         ▼                   └────────┬─────────┘
   nó materializado                   ▼
   status = queued            observação ao agente
   job volta a running        → REPLANEJA
                              job volta a running
```

### Regras invioláveis

| # | Regra |
| --- | --- |
| P1 | Nenhum nó é materializado sem `proposal_id` aprovado. Verificado no `match` do handler. |
| P2 | Nó em estado `proposed` **nunca** é processado pelo motor DSP. |
| P3 | TTL de 120 s. Expiração é tratada como rejeição implícita (agente replaneja). |
| P4 | Rejeição **não** falha o job. Vira observação e o agente tenta outra estratégia. |
| P5 | Decisão é idempotente: segundo POST no mesmo `proposal_id` retorna `409 proposal_already_decided`, não duplica. |
| P6 | Toda proposta, aprovação e rejeição vira `audit_event` imutável. |
| P7 | Enquanto há proposta pendente, o worker **libera a thread** (job em `awaiting_approval`). |
| P8 | `auto_approve` existe como flag de tenant, **desligado por padrão**. |

### O que o agente recebe após rejeição

```json
{
  "observation_type": "PROPOSAL_REJECTED",
  "proposal_id": "uuid",
  "tool": "stem_separation",
  "user_reason": "não quero separar stems",
  "hint": "Alcance o objetivo usando apenas as ferramentas já presentes no grafo: compression, dynamic_eq, crossfade."
}
```

Comportamento esperado: o agente troca a estratégia (ex.: em vez de isolar a
bateria por stems, aplica EQ dinâmico realçando 60–120 Hz e 2–5 kHz). Isso
precisa aparecer no `agent.thought` — é o momento em que o produto parece
inteligente.

### Retomada após reload da página

O frontend, ao montar a tela de um job:

1. `GET /api/v1/jobs/:id` — estado completo.
2. `GET /api/v1/jobs/:id/proposals` — pendentes (reabre overlay se houver).
3. `GET /api/v1/jobs/:id/events` com `Last-Event-ID` do último recebido.

Sem esse trio, um F5 no meio de uma proposta perde a decisão do usuário.

---

## 5. Prompts como código

### Formato `.prompt` (YAML)

O kit já define o formato. Adições necessárias:

```yaml
version: 2.0
id: tiktok_aggressive_v2
author: remix-ai-team
status: stable                 # ▲ stable | canary | deprecated
model_hint: "openai/gpt-4o"    # ▲ modelo validado com este prompt
tags: [genre:pop, use:compression, tier:pro]

description: |
  Versão agressiva para TikTok focada em transientes de bateria.

system: |
  Você é um engenheiro de áudio mestre com 20 anos de experiência.
  Você NUNCA executa processamento; você apenas escolhe ferramentas e
  parâmetros. Responda sempre em JSON válido no formato de tool call.

user_template: |
  Quero uma versão {{tone}} para {{platform}},
  focada nas {{focus_element}}.

  Contexto da faixa:
  - BPM: {{bpm}}
  - Duração: {{duration_sec}}s
  - Energia média (RMS): {{rms_mean}}
  - Seções detectadas: {{sections}}

parameters:
  - name: tone
    type: string
    enum: ["agressiva", "suave", "energética"]
    default: "agressiva"
    label_ptbr: "Tom"          # ▲ usado pela UI

tool_sequence: [stem_separation, compression, dynamic_eq, crossfade]

constraints:
  - compression.ratio <= 6.0
  - crossfade_ms <= 3000
  - never use noise_reduction
  - rms_output_db >= -12.0
```

### Regras

1. Prompt vive em `prompts/*.prompt`, versionado no git. **Nunca** em `String`
   dentro do código Rust.
2. `id` inclui a versão (`tiktok_aggressive_v2`). Mudança incompatível = novo id.
3. `constraints` são **verificadas em runtime**, não apenas instruídas ao modelo.
   Instrução no prompt é sugestão; validação em Rust é lei.
4. O linter (`scripts/prompt_linter.py`) roda no CI e valida: schema, `default`
   dentro do `enum`, `tool_sequence` só com ferramentas registradas,
   `constraints` parseáveis.
5. Alterar um `.prompt` dispara os testes de Golden Master (ver `09-MLOPS`).

### Templating

Usar **`minijinja`** (mesma sintaxe do Jinja2, autor do `minijinja` é o mesmo do
ecossistema Python). Carregamento em runtime, do disco — permite ajustar prompt
sem recompilar o binário.

---

## 6. Abstração de provedor LLM

```rust
#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse, LlmError>;
    async fn stream(&self, req: LlmRequest)
        -> Result<BoxStream<'_, Result<LlmChunk, LlmError>>, LlmError>;
    fn model_id(&self) -> &str;
    fn supports_tools(&self) -> bool;
}
```

Adapters: `openai`, `anthropic`, `ollama`. Todos falam o formato de tool calling
compatível com OpenAI, então o adapter `ollama` reusa 90% do código — troca só a
base URL e o modelo.

**Não usar SDK oficial.** `reqwest` + structs `serde` diretos. Motivos: os SDKs
em Rust são não-oficiais e ficam atrás; o formato REST é estável; e trocar de
provedor vira mudança de config, não de dependência.

### Modo determinístico para testes

```rust
pub struct MockLlm { responses: HashMap<String, LlmResponse> }
```

Mais: `ollama` com `seed` fixo para testes de integração. Ver `10-TESTES`.

---

## 7. Defesa contra prompt injection

Camada 1 — **sanitização de entrada** (`prompt_guard.rs`):

```rust
const FORBIDDEN: &[&str] = &[
    r"(?i)\bsystem\s*:",
    r"(?i)ignore\s+(as\s+)?(the\s+|your\s+|previous\s+|todas?\s+)",
    r"(?i)\b(shell|bash|exec|eval)\b",
    r"(?i)\b(env|environment)\s*(var|variable)",
    r"(?i)\b(secret|api[_-]?key|token|password|senha)\b",
    r"(?i)\bfile\s*system\b",
    r"(?i)\b(docker|kubectl|sudo)\b",
];
```

Mais: limite de 4096 caracteres; recusa de caracteres de controle e de blocos
Unicode de direção (`U+202E` e afins, usados para esconder texto).

Camada 2 — **registry filtrado**: o agente só recebe o schema das ferramentas
que o tenant pode usar. Ele não sabe que `run_custom_script` existe.

Camada 3 — **validação de saída**: mesmo com JSON bem-formado, a ferramenta é
verificada contra a lista permitida antes de executar.

Camada 4 — **sem canal para dados de outro tenant**: o contexto montado para o
prompt vem de queries já escopadas. Não existe caminho pelo qual "mostre os
prompts dos outros usuários" retorne algo — a informação nunca entra no contexto.

Toda detecção gera `audit_event` com o prompt original (hash + texto) e retorna
`422 malicious_prompt` sem detalhar qual padrão disparou.

---

## 8. Modo assistido vs manual

| | `manual` | `assisted` |
| --- | --- | --- |
| Agente roda? | não | sim |
| Origem dos parâmetros | `USER_DEFINED` / `DEFAULT` | agente preenche o que não estiver travado |
| Custo de LLM | zero | 1 sessão |
| Propostas | não existem | possíveis |
| Uso típico | reprocessar lote com receita validada | primeira exploração |

O modo `manual` é o que torna viável processar 200 faixas: o usuário refina uma
receita no modo assistido, salva, e aplica em lote sem gastar tokens nem esperar
LLM.

---

## 9. Casos de teste obrigatórios do agente

| # | Cenário | Resultado esperado |
| --- | --- | --- |
| A1 | LLM devolve `threshold_db: 100` | `422`, nó marcado com erro, agente replaneja, worker vivo |
| A2 | LLM devolve JSON malformado | Retry com erro como observação; após 2 falhas, `agent.error` |
| A3 | LLM pede ferramenta fora do plano | `UnauthorizedTool`, sem proposta, agente informado |
| A4 | Budget esgota no passo 5 | Consolidação forçada, job completa |
| A5 | Provedor LLM em timeout | Fallback para config manual, job completa com aviso |
| A6 | Usuário rejeita proposta | Agente replaneja sem a ferramenta; job completa |
| A7 | Proposta expira (120 s) | Igual a A6 |
| A8 | Prompt com injection | `422 malicious_prompt`, `audit_event` registrado |
| A9 | Campo travado (`USER_DEFINED`) | Agente sugere outro valor; sistema mantém o do usuário |
| A10 | Regra cruzada R2 violada | Recusa com mensagem específica, não genérica |
