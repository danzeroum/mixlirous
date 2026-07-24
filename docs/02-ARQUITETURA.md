# 02 — Arquitetura

## 1. Forma geral: monolito modular, pronto para partir

Um único binário Rust no MVP. As fronteiras internas são reais (crates
separadas, comunicação por traits), então extrair o worker para um processo
independente depois é mudança de `main.rs`, não reescrita.

```
                    ┌──────────────────────────────┐
                    │  ui/  (React + Vite)         │
                    └───────────┬──────────────────┘
                       REST ↕   │   ↓ SSE
                    ┌───────────▼──────────────────┐
                    │  audio_api   (binário)       │
                    │  ─ rotas, SSE, middleware    │
                    │  ─ config, DI, bootstrap     │
                    └───────┬──────────────┬───────┘
                            │              │
                ┌───────────▼────┐    ┌────▼─────────────┐
                │  audio_agent   │    │  worker (interno)│
                │  ReAct + valid.│    │  consome a fila  │
                └───────────┬────┘    └────┬─────────────┘
                            │              │
                    ┌───────▼──────────────▼───────┐
                    │  audio_core                  │
                    │  domain/ · dsp/ · ports/     │
                    └───────────┬──────────────────┘
                                │  (traits)
                ┌───────────────▼───────────────────┐
                │  adapters: sqlite · postgres      │
                │            local_fs · s3/minio    │
                │            openai · ollama        │
                └───────────────────────────────────┘
```

## 2. Regras de dependência (invioláveis)

```
audio_core   →  não depende de NADA do projeto
audio_agent  →  depende de audio_core
audio_api    →  depende de audio_core e audio_agent
adapters     →  implementam traits de audio_core::ports
```

Consequências práticas, verificadas em review:

| Regra | Como detectar violação |
| --- | --- |
| `audio_core` não importa `axum`, `reqwest`, `tokio-postgres` | `grep` no `Cargo.toml` da crate |
| `audio_core::dsp` não faz I/O de rede nem de disco | funções recebem `&Array1<f32>`, retornam dados |
| `audio_core::domain` não conhece SQL nem JSON de API | só `serde` para (de)serialização de valor |
| Nenhuma crate importa `audio_api` | `audio_api` é folha |
| Adapters não contêm regra de negócio | se tem `if` sobre domínio, está no lugar errado |

> **Por que isso importa aqui e não é dogma:** o motor DSP precisa ser testável
> com `proptest` gerando 10.000 buffers aleatórios. Se `dsp/` depender de Tokio
> ou de um pool de conexões, esse teste vira teste de integração lento e o
> projeto para de testar a parte que mais importa.

## 3. Papel de cada crate

### `audio_core` — domínio + DSP

Biblioteca pura. Sem `async`, sem rede, sem banco.

```
audio_core/src/
├── domain/
│   ├── beat.rs              BeatCandidate, BeatDetectionParams, OnsetStrength
│   ├── block.rs             BeatBlock, EnergyProfile, build_beat_blocks
│   ├── pipeline_config.rs   PipelineConfig e sub-structs (fonte da verdade)
│   ├── fingerprint.rs       AudioFingerprint + distance()
│   ├── recipe.rs         ▲  RemixRecipe, Parameter<T> { value, source }
│   └── job.rs            ▲  RemixJob, JobStatus (máquina de estados)
├── dsp/
│   ├── analysis/            fft, rms, chroma, beat_tracking, onset
│   ├── selection/        ▲  knapsack, strong_beat_filter, continuous_window
│   ├── stitching/           zero_cross, fades, crossfade
│   └── mastering/           lufs, limiter, compressor, stretch, default_mixer
└── ports/
    ├── analyzer_trait.rs    AudioAnalyzer
    ├── mixer_trait.rs       AudioMixer
    ├── repo_trait.rs        AudioRepo
    └── storage_trait.rs  ▲  Storage (async)
```

▲ = a criar (não existe no kit atual)

### `audio_agent` — orquestração cognitiva

```
audio_agent/src/
├── react_kernel.rs      Loop ReAct com budget
├── tools.rs             AudioToolDef (registry tipado)
├── validator.rs         ValidationLayer (limites de parâmetro)
├── prompt_loader.rs     Parser de .prompt + templating
├── prompt_guard.rs   ▲  Sanitização anti-injection
└── llm/              ▲  Trait LlmProvider + adapters openai/ollama/anthropic
```

O agente **não executa DSP**. Ele valida a `ToolCall` e emite a receita. Quem
executa é o worker, através dos traits de `audio_core::ports`.

### `audio_api` — transporte

```
audio_api/src/
├── main.rs              bootstrap, DI, recovery loop no boot
├── config/              carga de YAML + env
├── routes/              jobs, tracks, prompts, proposals, tenants, system, sse
├── middleware/          auth (JWT), tenant_scope, otel, rate_limit
├── sse/              ▲  Hub de broadcast por job_id
└── worker/           ▲  Loop de consumo da fila + Rayon pool
```

## 4. Modelo de concorrência

A regra que evita 80% dos problemas de performance neste tipo de sistema:

```
Tokio   →  tudo que ESPERA        (HTTP, LLM, banco, S3, SSE)
Rayon   →  tudo que CALCULA       (FFT, RMS, crossfade, masterização)
Ponte   →  tokio::task::spawn_blocking
```

```rust
// Padrão obrigatório na fronteira
let blocks = tokio::task::spawn_blocking(move || {
    // Aqui dentro: Rayon, ndarray, zero await.
    analyzer.build_blocks(&pcm, &beats, block_size, sample_rate)
}).await??;
```

Nunca chamar `.await` dentro de closure do Rayon; nunca chamar função de DSP
diretamente no runtime Tokio. Ambos travam o event loop e o sintoma é sutil (SSE
que engasga, health check que expira).

### Configuração do pool

```rust
let dsp_threads = (num_cpus::get().saturating_sub(1)).max(1); // 1 core p/ I/O
rayon::ThreadPoolBuilder::new().num_threads(dsp_threads).build_global()?;
```

## 5. Fluxo de execução — do prompt ao WAV

```
 1. POST /api/v1/jobs                     [Tokio]
      ├─ auth JWT → tenant_id
      ├─ sanitize_user_prompt()
      ├─ valida PipelineConfig (serde + bounds)
      ├─ INSERT job (status=queued) + emite trace_id
      └─ 202 Accepted { job_id, stream_url }

 2. GET /api/v1/jobs/:id/events            [Tokio, SSE]
      └─ assina o broadcast channel do job

 3. Worker reivindica o job                [Tokio]
      └─ claim_next_job() → status=running

 4. Loop ReAct (até 5 passos)               [Tokio]
      ├─ monta prompt (template + contexto)
      ├─ chama LlmProvider                  → SSE agent.thought (streaming)
      ├─ ValidationLayer.validate()         → falha = SSE agent.error + replan
      ├─ ferramenta ausente no grafo?       → SSE agent.proposal + PAUSA
      └─ consolida RemixRecipe              → SSE node.parameters

 5. Execução DSP                            [Rayon via spawn_blocking]
      ├─ decode (symphonia)                 → SSE job.progress
      ├─ onset + beat grid
      ├─ blocos + energia + knapsack
      ├─ stitching (zero-cross + crossfade)
      └─ masterização (compressor → limiter → LUFS)

 6. Persistência do artefato                [Tokio]
      ├─ escrita atômica: .tmp → fsync → rename → fsync(dir)
      ├─ SHA-256 → UPDATE job (status=completed, artifact_hash)
      └─ SSE job.completed { download_url }
```

O passo 4 pode **pausar** esperando decisão humana. O job fica em
`awaiting_approval` e o worker libera a thread. Isso não é detalhe: um worker
bloqueado esperando clique é um worker desperdiçado.

## 6. Comunicação com o frontend: por que SSE e não WebSocket

| Critério | SSE | WebSocket |
| --- | --- | --- |
| Direção necessária aqui | servidor → cliente (99% do tráfego) | bidirecional |
| Reconexão | nativa no browser (`retry` + `Last-Event-ID`) | manual (heartbeat, backoff) |
| Proxy / firewall | HTTP puro, atravessa tudo | `Upgrade` derrubado silenciosamente |
| Depuração | texto legível no DevTools Network | frames binários |
| Custo no Axum | `tokio::sync::broadcast` + stream | estado por socket |

As ações do usuário (aprovar proposta, travar slider, cancelar) são **comandos
pontuais** — HTTP `POST`/`PATCH` normal, idempotente e auditável. Não precisa de
canal persistente.

Reavaliar WebSocket quando: (a) edição colaborativa multiusuário no mesmo grafo,
(b) preview de reprodução com scrub em tempo real. Nenhum dos dois está no MVP.

## 7. Persistência: um trait, dois backends

```rust
pub trait AudioRepo: Send + Sync {
    fn claim_next_job(&self, worker_id: Uuid) -> Result<Option<JobRecord>>;
    fn transition(&self, job_id: Uuid, to: JobStatus) -> Result<()>;
    // ...
}
```

| | SQLite (WAL) | PostgreSQL (RLS) |
| --- | --- | --- |
| Quando | laptop, VPS pequena, testes | multiusuário, SaaS |
| Config | nenhuma (`.mixlirous/data.db`) | `DATABASE_URL` |
| Fila | `UPDATE ... WHERE id = (SELECT ... LIMIT 1)` em transação `IMMEDIATE` | `FOR UPDATE SKIP LOCKED` |
| Isolamento de tenant | filtro explícito no adapter | RLS no banco |
| Limite prático | ~10 GB / ~50k jobs | horizontal |

Detalhes e SQL completo em [`06-PERSISTENCIA-RESILIENCIA.md`](06-PERSISTENCIA-RESILIENCIA.md).

## 8. Storage: abstração única

Um trait `Storage` async com três adapters: `local_fs`, `minio`, `s3`.

Recomendação: usar a crate **`object_store`** (do projeto Arrow) em vez de somar
`minio` + `aws-sdk-s3`. Ela cobre local, S3, GCS e Azure com uma API só, é
mantida ativamente e reduz duas dependências pesadas a uma. Ver ADR-0006.

Estrutura de chaves — desde o dia 1, mesmo no modo local:

```
tenant-{tenant_id}/project-{project_id}/
├── raw/{track_id}.wav              imutável
├── processed/{job_id}.wav          output
└── artifacts/{job_id}.meta.json    fingerprint, receita, hashes
```

## 9. Camadas transversais

| Preocupação | Onde vive | Nunca vive |
| --- | --- | --- |
| Autenticação | `audio_api/middleware/auth.rs` | domínio |
| Escopo de tenant | middleware + adapter de repo | `dsp/` |
| Tracing / spans | `#[instrument]` em toda fronteira pública | dentro de loop de DSP |
| Configuração | `audio_api/config` → structs tipadas | `unwrap()` de env no meio do código |
| Feature flags | config + tabela `feature_flags` | `#[cfg]` de compilação |

## 10. Anti-padrões que serão recusados em review

1. **`unwrap()` / `expect()` em código de request.** Só em testes e no bootstrap.
2. **Bloquear o runtime.** Qualquer DSP fora de `spawn_blocking`.
3. **Parâmetro numérico "solto".** Todo valor que a IA pode preencher usa
   `Parameter<T> { value, source }`. Sem isso, a trava do usuário não existe.
4. **`String` para o que é enum.** Curva de fade, tipo de nó, status: enum.
5. **SQL sem escopo de tenant.** Ver `08-SEGURANCA-MULTITENANCY.md`.
6. **Escrita direta de arquivo final.** Sempre `.tmp` → `fsync` → `rename`.
7. **Log de texto livre em ponto quente.** Log estruturado com campos, ou nada.
8. **Regra de negócio no componente React.** O frontend desenha e envia comando;
   ele não decide se um crossfade de 4 s é válido.
