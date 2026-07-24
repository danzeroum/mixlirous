# 07 — Observabilidade e Telemetria

## 1. Objetivo prático

Quando um usuário disser "o render travou por 15 minutos", a resposta precisa
sair de um `trace_id` colado no Grafana — não de `grep` em log.

Três sinais, um contexto:

```
Métricas  → o QUE está errado (fila cresceu, latência subiu)
Traces    → ONDE está errado (o span do LLM levou 8 s)
Logs      → POR QUE está errado (provider timeout, retry, sucesso)
```

Todos correlacionados por `trace_id`.

---

## 2. Instrumentação em Rust

```toml
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
tracing-opentelemetry = "0.28"   # versão pareada com opentelemetry 0.27
opentelemetry = "0.27"
opentelemetry_sdk = { version = "0.27", features = ["rt-tokio"] }
opentelemetry-otlp = { version = "0.27", features = ["grpc-tonic"] }
metrics = "0.24"
metrics-exporter-prometheus = "0.16"
```

> As versões do kit (`opentelemetry = "0.12"`) são de 2022 e incompatíveis entre
> si. Pareamento correto é obrigatório — ver `14-AUDITORIA-KIT.md`.

### Padrão de span

```rust
#[instrument(
    name = "job.render",
    skip(self, pcm),
    fields(
        job.id       = %job.id,
        tenant.id    = %job.tenant_id,
        track.id     = %job.track_id,
        job.mode     = ?job.mode,
        dsp.stage    = tracing::field::Empty,
        audio.duration_sec = pcm.len() as f32 / sr as f32,
    )
)]
pub async fn render(&self, job: &RemixJob, pcm: Array1<f32>) -> Result<Artifact> {
    Span::current().record("dsp.stage", "decoding");
    // ...
}
```

Spans obrigatórios (um por fronteira significativa):

```
http.request
 └── job.create
      └── agent.run
           ├── llm.call            (atributos: modelo, tokens, tentativa)
           ├── validation.check
           └── tool.execute
      └── job.render
           ├── dsp.decode
           ├── dsp.analyze
           ├── dsp.select
           ├── dsp.stitch
           ├── dsp.master
           └── storage.put
```

### Propagação para threads Rayon

O `tracing` não propaga span automaticamente para outra thread. Capturar e
entrar explicitamente:

```rust
let span = Span::current();
let result = tokio::task::spawn_blocking(move || {
    let _g = span.enter();
    heavy_dsp()
}).await?;
```

Sem isso, todo o tempo de DSP some do trace — que é justamente o que se quer
medir.

---

## 3. Propagação de contexto ponta a ponta

| Salto | Mecanismo |
| --- | --- |
| Browser → API | header `traceparent` (W3C) |
| API → resposta | header `traceparent` devolvido, para a UI exibir |
| API → SSE | `traceparent` como query param no `EventSource` (não dá para pôr header) |
| API → fila | coluna `jobs.trace_id` |
| Worker → LLM | header `traceparent` na chamada HTTP |
| Worker → storage | atributo no span |

No frontend, instrumentar com `@opentelemetry/api` +
`@opentelemetry/instrumentation-fetch`. O `trace_id` fica visível na UI em
telas de erro e no painel de detalhes do job — copiável em um clique.

---

## 4. Métricas

### Negócio

| Métrica | Tipo | Labels |
| --- | --- | --- |
| `mixlirous_jobs_total` | counter | `status`, `mode`, `tenant` |
| `mixlirous_job_duration_seconds` | histogram | `mode` |
| `mixlirous_queue_depth` | gauge | `status` |
| `mixlirous_workers_active` | gauge | — |
| `mixlirous_proposals_total` | counter | `decision` (approved/rejected/expired) |
| `mixlirous_param_overrides_total` | counter | `parameter` |

> `proposals_total{decision}` e `param_overrides_total{parameter}` são as
> métricas de **produto** mais valiosas do sistema: dizem quais sugestões da IA
> as pessoas rejeitam e quais valores elas sempre corrigem à mão. É o dado que
> guia o ajuste dos prompts.

### LLM

| Métrica | Tipo | Labels |
| --- | --- | --- |
| `mixlirous_llm_calls_total` | counter | `provider`, `model`, `outcome` |
| `mixlirous_llm_duration_seconds` | histogram | `provider`, `model` |
| `mixlirous_llm_tokens_total` | counter | `direction` (prompt/completion) |
| `mixlirous_llm_validation_failures_total` | counter | `tool`, `field` |
| `mixlirous_agent_tools_used` | histogram | — (distribuição do budget) |

### DSP

| Métrica | Tipo | Labels |
| --- | --- | --- |
| `mixlirous_dsp_stage_duration_seconds` | histogram | `stage` |
| `mixlirous_dsp_audio_seconds_processed_total` | counter | — |
| `mixlirous_dsp_warnings_total` | counter | `kind` (harsh_splice, heavy_gain...) |

### Infra

`mixlirous_db_query_duration_seconds{operation}` ·
`mixlirous_storage_operation_duration_seconds{operation}` ·
`mixlirous_sse_connections_active` · `mixlirous_recovery_jobs_total{outcome}`

Exposição: `GET /metrics` no formato texto do Prometheus.

---

## 5. Logs

JSON estruturado, sempre com `trace_id` e `tenant_id`.

```json
{"timestamp":"2026-07-24T18:30:12.482Z","level":"WARN",
 "target":"audio_agent::llm","trace_id":"4bf92f...","span_id":"00f0...",
 "tenant_id":"a7c1...","job_id":"9f2b...",
 "message":"llm provider timed out","provider":"openai","model":"gpt-4o",
 "attempt":1,"timeout_ms":30000}
```

Regras:

- Nunca logar o conteúdo do prompt do usuário em nível INFO (pode conter material
  criativo não publicado). Logar hash + tamanho. O texto completo vai só para o
  `audit_event`, que tem controle de acesso.
- Nunca logar credenciais, chaves de API ou tokens JWT — nem truncados.
- Nível padrão `INFO`; `DEBUG` para `mixlirous=debug` via `RUST_LOG`.
- Sem log dentro de loop de amostra de áudio. Nunca.

---

## 6. Alertas

| Alerta | Condição | Severidade | Ação |
| --- | --- | --- | --- |
| `LLMHighLatency` | p99 de `llm_duration_seconds` > 5 s por 5 min | warning | Verificar provedor |
| `LLMErrorRate` | erros / total > 5% por 5 min | critical | Ativar fallback manual |
| `QueueBacklog` | `queue_depth{status="queued"}` > 50 por 5 min | warning | Escalar workers |
| `StalledJobs` | job em `running` sem heartbeat há 2 min | critical | Worker morto |
| `RenderDurationRegression` | p95 > 2× baseline por 15 min | warning | Regressão de performance |
| `ValidationFailureSpike` | `llm_validation_failures` > 20% das tool calls | warning | Prompt degradado |
| `StorageErrors` | qualquer erro de escrita | critical | Bloquear novos jobs |
| `RecoveryLostJobs` | `recovery_jobs_total{outcome="lost"}` > 0 | critical | Investigar disco |

No modo local não há Alertmanager. Os mesmos limiares viram avisos na própria UI
(banner) — mesma lógica, saída diferente.

---

## 7. Bundle local de observabilidade

`docker-compose.observability.yml`: Grafana (porta **3001**), Tempo, Loki,
Prometheus, Grafana Agent. Sobe com um comando, dashboards já provisionados:

```bash
docker compose -f docker-compose.observability.yml up -d
# → http://localhost:3001
```

Dashboards versionados em `grafana/dashboards/`:

1. **Visão geral** — jobs por estado, duração p50/p95/p99, fila, workers
2. **Agente** — latência do LLM, custo, budget usado, falhas de validação
3. **DSP** — duração por etapa, segundos de áudio processados, warnings
4. **Produto** — taxa de aprovação de propostas, parâmetros mais sobrescritos

Sem o bundle (padrão no laptop), o binário exporta traces para `stdout` em modo
compacto e mantém `/metrics` ativo. O sistema **nunca** exige coletor para
funcionar.

---

## 8. Investigação forense — o fluxo esperado

```
Usuário: "o render de ontem à noite travou"
   │
   ├─ 1. Pede o trace_id (visível na UI, um clique para copiar)
   ├─ 2. Cola no Grafana → Tempo
   ├─ 3. Vê a árvore de spans:
   │       job.render 14,2 s
   │        ├─ agent.run 9,1 s
   │        │   └─ llm.call 8,3 s  ← gargalo
   │        │        ├─ erro: provider timeout
   │        │        └─ retry: sucesso em 5,1 s
   │        └─ dsp.* 4,8 s
   ├─ 4. Clica no span → logs correlacionados
   └─ 5. Conclusão em < 2 min, com evidência
```

Se esse fluxo não funcionar de ponta a ponta em staging na Sprint 4, a
instrumentação está incompleta — é o critério de aceite da entrega.
