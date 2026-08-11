# Plano Final de Desenvolvimento — mixlirous

> **Versão final validada** — 10/08/2026
> 
> Incorpora: (a) trait `AudioRepo` já existe, estender não criar; (b) ValidationLayer/limits/prompt_loader implementados; (c) stitching e mastering parcialmente prontos; (d) `update_context` trivial como bloqueador crítico do ReAct; (e) frontend ~40% não 90%; (f) I15 com cobertura parcial em `stitching/crossfade`; (g) colisão de numeração "R" entre documentos.

---

## 0. Diagnóstico — Estado Real

| Componente | Estado | O que falta |
|---|---|---|
| `audio_core` DSP | Crossfade (constant_power/gain), fades (linear/log/exp), LUFS, limiter, time_stretch, zero-crossing **existem**. Compressor, EQ, knapsack, chroma/seções, MFCC real **não existem** | Compressor, seleção, fingerprint |
| `audio_agent` | ValidationLayer (420 linhas), limits.rs (836 linhas), prompt_loader (38 linhas), tools.rs **existem**. ReAct kernel: 3 `unimplemented!()` + `update_context` trivial | Loop ReAct, LlmProvider, execução de tools |
| `audio_api` | health, system, tools, prompts, consent **funcionam**. Jobs é placeholder. SSE só emite `stream.ready` | Fila, worker, eventos reais, rotas de upload/track/artifact |
| `AudioRepo` trait | **Existe** com save_job, get_job, list_jobs, save_fingerprint, transition_job, list_audit_records, get_consent, save_consent. Só `InMemoryRepo` implementa | claim_next_job, heartbeat, fail_and_retry + adapter SQLite/Postgres |
| `ui/` | Componentes casca (RemixCanvas, ProposalOverlay, useParamStream, graphStore) sem integração real | Upload, conexão API, lógica de nós, tipos TS |

---

## 1. Sprint 1 — Persistência, Fila e API

### 1.1 Estender `AudioRepo` (trait já existe)

**Arquivo:** `crates/audio_core/src/ports/repo_trait.rs`

Adicionar ao trait:
```rust
async fn claim_next_job(&self, worker_id: Uuid) -> Result<Option<JobRecord>, RepoError>;
async fn heartbeat(&self, job_id: Uuid, worker_id: Uuid) -> Result<(), RepoError>;
async fn fail_and_retry(&self, job_id: Uuid, max_attempts: u8) -> Result<(), RepoError>;
```

### 1.2 Implementar no `InMemoryRepo`

**Arquivo:** `crates/audio_api/src/adapters/repo_memory.rs`

### 1.3 Adapter SQLite (WAL, PRAGMAs)

**Novo arquivo:** `crates/audio_api/src/adapters/repo_sqlite.rs`

### 1.4 Testes de concorrência

| # | Cenário | Assertiva |
|---|---|---|
| P1 | 10 workers concorrentes reivindicam fila | Cada job vai para exatamente 1 worker |
| P2 | Idem, em SQLite | Sem `SQLITE_BUSY` não tratado |
| P6 | Job sem heartbeat por 2 min | Volta para `queued` |

### 1.5 Rotas de produto

| Método | Path | Descrição |
|---|---|---|
| `POST` | `/uploads/presign` | Gerar URL de upload |
| `POST` | `/tracks` | Registrar faixa |
| `GET` | `/tracks/{id}` | Dados da faixa |
| `GET` | `/tracks/{id}/peaks` | Picos da waveform |
| `GET` | `/jobs/{id}/artifact` | URL de download |
| `POST` | `/jobs/{id}/retry` | Reexecução |

---

## 2. Sprint 2 — DSP: Compressor, Seleção, Fingerprint

### Pré-existente (não criar)
- crossfade_buffers, apply_fade_in/out, brickwall_limiter, apply_lufs_gain, time_stretch, find_zero_crossing, onset_strength, build_beat_blocks

### Trabalho

| # | Tarefa | Módulo |
|---|---|---|
| 1 | Compressor real (RMS detector, envelope, ratio, attack, release, makeup) | `mastering/compressor.rs` |
| 2 | Knapsack/seleção (intro/outro fixos, batida forte, ordem cronológica) | `selection/knapsack.rs` |
| 3 | Modo contínuo (melhor janela O(n)) | `selection/continuous.rs` |
| 4 | Chroma + detecção de seções (novelty curve) | `analysis/chroma.rs` |
| 5 | Normalização de features (centroid, rms, mfcc comparáveis) | `domain/fingerprint.rs` |
| 6 | MFCC real (banco de filtros mel + DCT-II) | `domain/fingerprint.rs` |
| 7 | Limiter com lookahead (5 ms) | `mastering/limiter.rs` |
| 8 | Invariantes I1–I14 com `proptest` (10.000 casos) | `tests/` |
| 9 | Estender I15 (finitude) para compressor e fades | `tests/` |
| 10 | Benchmarks `criterion` (threshold 20% p95) | `benches/` |

### Invariantes obrigatórios

| # | Invariante | Módulo |
|---|---|---|
| I1 | Compressor com makeup ≤ 0 nunca aumenta o pico | `mastering/compressor` |
| I2 | Limiter nunca deixa amostra acima do teto | `mastering/limiter` |
| I3 | Crossfade preserva duração: `len = a + b − L` | `stitching/crossfade` |
| I4 | Crossfade não introduz descontinuidade > 1,5× | `stitching/crossfade` |
| I5 | `snap_to_zero_crossing` retorna índice dentro da janela | `stitching/zero_cross` |
| I6 | Grade de batidas estritamente crescente | `analysis/beat_tracking` |
| I7 | Blocos não se sobrepõem | `domain/block` |
| I8 | Knapsack respeita `target ± tolerance` ou retorna `Err` | `selection/knapsack` |
| I9 | Knapsack é determinístico | `selection/knapsack` |
| I10 | `fingerprint.distance(x, x) == 0` e simétrica | `domain/fingerprint` |
| I11 | Após normalização, `|lufs − alvo| ≤ 0,5 LU` | `mastering/lufs` |
| I12 | Time-stretch entrega duração dentro de ±20 ms | `mastering/stretch` |
| I13 | `RMS(seno amplitude 1) ≈ 0,7071` | `analysis/rms` |
| I14 | Newtype rejeita valor fora do limite na desserialização | `domain/*` |
| I15 | Nenhuma amostra `NaN` ou infinita (estender para compressor/fades) | `tests/` |

---

## 3. Sprint 3 — Agente IA e Frontend

### 3.1 Pré-existente (não criar)
- `ValidationLayer` (420 linhas), `limits.rs` (836 linhas), `prompt_loader.rs` (38 linhas), `tools.rs`

### 3.2 Trabalho do Agente

| # | Tarefa | Arquivo |
|---|---|---|
| 1 | **Corrigir `update_context`:** acumular estado entre passos | `react_kernel.rs:73` |
| 2 | Trait `LlmProvider` | `llm/mod.rs` (novo) |
| 3 | Adapter Ollama (primeiro) | `llm/ollama.rs` |
| 4 | Templating com `minijinja` | `prompt_loader.rs` |
| 5 | `prompt_guard` (anti-injection) | `prompt_guard.rs` (novo) |
| 6 | Loop ReAct mínimo com `MockLlm` | `react_kernel.rs` |
| 7 | Adapter LLM real | `llm/openai.rs` |
| 8 | Streaming de `agent.thought` via SSE | `sse.rs` |
| 9 | Ciclo de propostas HITL (TTL 120s) | `routes/jobs.rs` |
| 10 | Fallback modo manual | `react_kernel.rs` |

### 3.3 Cenários A1–A10 (testes obrigatórios)

| # | Cenário | Resultado esperado |
|---|---|---|
| A1 | LLM devolve `threshold_db: 100` | `422`, nó com erro, agente replaneja |
| A2 | JSON malformado | Retry; após 2 falhas, `agent.error` |
| A3 | Ferramenta fora do plano | `UnauthorizedTool`, sem proposta |
| A4 | Budget esgota no passo 5 | Consolidação forçada |
| A5 | LLM em timeout | Fallback para config manual |
| A6 | Usuário rejeita proposta | Agente replaneja sem a ferramenta |
| A7 | Proposta expira (120 s) | Igual a A6 |
| A8 | Prompt com injection | `422 malicious_prompt`, `audit_event` |
| A9 | Campo `USER_DEFINED` travado | Agente sugere outro; sistema mantém |
| A10 | Regra cruzada violada | Recusa com mensagem específica |

### 3.4 Frontend

| # | Tarefa |
|---|---|
| 1 | Tela de upload de faixas |
| 2 | Conexão store ↔ API |
| 3 | Renderização dinâmica de nós |
| 4 | Tipos TS via `ts-rs` |
| 5 | Tela de resultado (player, waveform, A/B) |
| 6 | Integração ProposalOverlay com dados reais |
| 7 | Sliders via SSE respeitando travas |

---

## 4. Sprint 4 — Resiliência e Observabilidade

### Recovery (R1–R7)

| # | Cenário | Estado final |
|---|---|---|
| R1 | SIGKILL durante escrita; só `.tmp` existe | `queued`, `.tmp` removido |
| R2 | SIGKILL após rename, antes do UPDATE | `completed` |
| R3 | Arquivo existe, hash não bate | `queued`, arquivo removido |
| R4 | Artefato ausente e attempt >= max | `failed(artifact_lost)` |
| R5 | Proposta expirada no crash | `expired` → job `queued` |
| R6 | Recovery interrompido no meio | Idempotente (mesmo resultado) |
| R7 | Dois processos sobem juntos | Só um faz recovery (lock) |

### Observabilidade
- OTel spans com propagação ponta-a-ponta
- Métricas Prometheus (negócio, LLM, DSP, infra)
- 4 dashboards Grafana
- `trace_id` visível ao usuário

---

## 5. Sprint 5 — Empacotamento

- `rust-embed` para servir UI do binário
- Builds cruzados com `cargo-dist`
- Setup automático no primeiro boot
- Onboarding de primeiro uso

---

## Colisão de numeração "R" — Nota importante

Há colisão entre dois documentos:
- `docs/10-TESTES-QUALIDADE.md`: R1–R7 = cenários de **recuperação/infraestrutura**
- `docs/05-AGENTE-IA-HITL.md`: R2–R7 = **regras de validação de ferramentas**

**Recomendação:** Renomear regras do agente para `RV1–RV7` (Regras de Validação) antes de virarem título de PR ou teste.

---

## Regras cruzadas do agente (R1–R7 em docs/05)

| # | Regra | Mensagem |
|---|---|---|
| R1 | `attack_ms <= release_ms` | "ataque não pode ser maior que o release" |
| R2 | `ratio >= 8.0` + `threshold_db > -10.0` | "compressão destrutiva" |
| R3 | `preserve_intro + preserve_outro < target` | "intro/final não cabem na duração" |
| R4 | Bandas EQ duplicadas (±5%) | "bandas de EQ sobrepostas" |
| R5 | LUFS normalization só uma vez | "normalização duplicada" |
| R6 | crossfade <= menor bloco / 2 | "transição maior que o bloco" |
| R7 | Ferramenta fora do plano | "ferramenta indisponível no seu plano" |

---

## Dependências entre sprints

```
S1 (Persistência + Fila + API)
  │
  ├── S2 (DSP) — pode iniciar em paralelo após 1.1
  │
  └── S3 (Agente) — depende de S1 (fila) + S2 (DSP para execução de tools)
        │
        └── S4 (Resiliência) — depende de tudo funcionando
              │
              └── S5 (Empacotamento) — depende de tudo
```

---

## Definição de Pronto (DoD)

- [ ] `cargo clippy -- -D warnings` limpo
- [ ] `cargo fmt` / `prettier` aplicados
- [ ] Testes unitários da lógica nova
- [ ] Se toca DSP: invariante de propriedade adicionado
- [ ] Se toca contrato: `03-CONTRATOS-API.md` atualizado no mesmo PR
- [ ] Se toca contrato: tipos TS regenerados
- [ ] Se toca limite: tabela canônica de `05` §3 atualizada
- [ ] Sem `unwrap()` fora de teste e bootstrap
- [ ] Checklist de segurança (`08` §10) quando aplicável

---

## Metas de cobertura

| Módulo | Meta |
|---|---|
| `audio_core::dsp` | ≥ 85% |
| `audio_core::domain` | ≥ 90% |
| `audio_agent::validator` | 100% |
| `audio_api::routes` | ≥ 70% |
| `ui/` | ≥ 60% |
