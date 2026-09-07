# Changelog

Todos os mudanças notáveis deste projeto serão documentados neste arquivo.
Formato baseado em [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/),
versionamento [SemVer](https://semver.org/lang/pt-BR/).

## [Unreleased] — Lote 3 do plano Pareto (canvas executável + qualidade sonora)

### Adicionado

- **Item 1 (canvas executável): `ui/src/lib/graphToPipeline.ts`** — o
  grafo montado no canvas (`graphStore`) é serializado para o
  `PipelineConfig` que o backend executa; `App.tsx` deixa de enviar
  `defaultPipelineConfig()` fixo. Mapeamento: crossfade →
  `crossfade.enabled/max_duration_ms`; lufs_normalization →
  `mastering.enable_limiting/lufs_target`; time_stretch/fades
  registrados como unmapped; ghost tools (compression, dynamic_eq,
  stem_separation) **nunca** serializadas — regra do plano. Grafo
  cíclico → erro `invalid_graph` antes de gastar um job; grafo vazio →
  erro explícito pedindo ferramenta da paleta.
- **Testes de contrato do grafo** — `graphToPipeline.spec.ts` (11
  testes, golden canônico) espelhado em
  `contract_ts_rust.rs::grafo_canonico_do_canvas_desserializa_no_rust`
  e `grafo_sem_crossfade_desabilita_crossfade_no_rust`: se um campo
  mudar de um lado, um dos dois testes quebra primeiro.
- **Item 4: `ui/e2e/full-flow.spec.ts` + `playwright.config.ts`** — 1
  spec Playwright do fluxo feliz completo (upload → criação de job →
  aprovação de proposta HITL quando existir → download do artefato
  validando RIFF/audio-wav via cookie de sessão). Auto-pula sem a
  stack de pé. `data-testid` adicionados aos controles do fluxo.
- **Sessão local na app** — `App.tsx` faz o bootstrap
  (`GET /auth/local-session` → token no localStorage + cookie);
  `useApi.fetchJson` e `UploadPanel` mandam o Bearer (`authHeaders()`).

### Corrigido

- **Item 2 (#37): limiter de pico real** — `brickwall_limiter` troca a
  escala uniforme do buffer inteiro (que desfazia o ganho de LUFS em
  material percussivo: −17 LU medidos na issue) por ganho por amostra
  com lookahead (mínimo deslizante O(n), deque monótono) e release
  exponencial. Garantia de teto provada no comentário e testada:
  pico ≤ teto em todos os casos; regressão #37
  (`limiter_preserva_loudness_em_material_percussivo`: |final−alvo| ≤
  1,5 LU onde a versão antiga ficava a ~7 LU); cauda recuperada pós
  transiente; NaN não silencia o buffer.
- **Pipeline masterização**: cadeia passa `sample_rate` ao limiter e
  confere o loudness FINAL — emite aviso `loudness_target_conflict`
  (docs/03-ADENDO-R2 §1) quando o alvo não é alcançável com o teto
  (>2 LU de distância).
- **Item 3 (#27): limiar de onset híbrido local** — complemento do fix
  parcial de f400fad (que era global: p75 do onset inteiro). Falhava em
  crescendo (batidas da parte baixa somem sob o p75 global) e em
  material denso (p75 ≈ pico). Agora: p75 da janela local (~2 s,
  centrada) + 10% do range local (p95−p75), piso absoluto 1e-4. Testes:
  crescendo, denso, ruído de fundo e regressão do f400fad.

### Modificado

- **worker.rs**: `on_proposal_created` publica `agent.proposal` no hub
  SSE — é o que a UI espera para abrir o overlay (a decisão continua
  automática; pausar no ProposalStore é o item B5, fora dos lotes).
- `.dev/module-status.yaml` — ui 80→88 (canvas executável + E2E).

## [Unreleased] — 2026-08-20

### Adicionado

- **B4: Rota `GET /api/v1/jobs/{id}/artifact`** — endpoint de download do WAV
  remixado. Em modo local (storage em disco), faz stream direto com
  `Content-Type: audio/wav` + `Content-Disposition: attachment`. Só
  disponível em `status=completed`; outro estado devolve
  `409 job_not_editable`. Handler em
  `crates/audio_api/src/routes/jobs.rs::download_artifact`.
- **B2: `ui/src/hooks/useSSE.ts`** — hook que substitui `useParamStream`.
  Registra `addEventListener` para cada evento nomeado conhecido
  (`agent.thought`, `job.state`, `agent.proposal`, `job.completed`, etc.)
  em vez de só `onmessage`. Não chama mais `source.close()` em
  `onerror` — deixa o auto-reconnect nativo do EventSource funcionar.
- **B3: `defaultPipelineConfig()` em `ui/src/types/api.ts`** — helper que
  gera um `PipelineConfig` que desserializa corretamente no Rust (mirror
  exato de `PipelineConfig::default()`).
- **C2: `ui/src/components/Player.tsx`** — player comparativo A/B
  (Design Brief §Tela 7). Reproduz remix vs original (este último via
  upload manual por enquanto), com toggle A/B mantendo a posição.
- **T1: `crates/audio_api/tests/e2e.rs`** — teste E2E do fluxo
  `save_job` → storage → hub.publish(job.completed) → subscriber recebe
  evento com `download_url` correto. Usa `tempfile::TempDir` para
  isolar o storage.
- **T2: `crates/audio_core/tests/contract_ts_rust.rs`** — teste de
  contrato que valida sincronia TS↔Rust para `PipelineConfig`. Quando
  `ts-rs` for integrado (item B3 do relatório, próxima iteração), este
  teste pode ser substituído por `cargo test export_bindings`.
- **T3: `ui/src/hooks/__tests__/useSSE.spec.ts`** — teste Vitest que
  valida cobertura do catálogo de eventos SSE conhecidos.
- **D4: este `CHANGELOG.md`** — novo arquivo, ausente antes.
- **`audio_api` como bin + lib crate** — adicionado `src/lib.rs` e
  seções `[lib]` + `[[bin]]` em `Cargo.toml`. Permite que integration
  tests em `tests/` importem módulos internos (`use audio_api::worker::Worker`).

### Modificado

- **B1: `crates/audio_api/src/worker.rs`** — adicionada função
  `apply_recipe_to_config(recipe, &mut PipelineConfig)` que traduz cada
  `AudioToolDef` da receita do ReAct em overrides sobre o
  `PipelineConfig` base (CompressionRatio, CrossfadeMs/Curve, LufsTarget).
  O worker não descarta mais a receita — ela é serializada para JSON,
  passada pelo `spawn_blocking`, e aplicada antes de chamar
  `DefaultRemixPipeline::run`. Erros do agente (item C4) viram
  `agent.error` SSE estruturado + warning + fallback para config manual.
- **B4: `crates/audio_api/src/worker.rs`** — o `download_url` publicado
  no evento `job.completed` agora aponta para `/api/v1/jobs/{id}/artifact`
  (rota REST) em vez de `/api/v1/artifacts/{key}` (path interno de
  storage que não existia no router).
- **M7: `crates/audio_api/src/worker.rs`** — `job.warning` agora segue o
  schema completo do contrato (`job_id`, `code`, `severity`, `at_sec`,
  `message_ptbr`, `hint_ptbr`, `measured`) em vez de só `{"message": ...}`.
- **B3: `ui/src/types/api.ts`** — totalmente reescrito para alinhar com
  structs Rust reais. `PipelineConfig.crossfade` agora tem `enabled`,
  `max_duration_ms`, `curve` (não `duration_ms`). `MasteringConfig` tem
  `enable_limiting`. Adicionado `TuningConfig` completo com `mode`,
  `max_global_cents`, `min_confidence`. Adicionado `ApiError` (RFC 7807)
  + `ApiRequestError` para a UI mapear 422 → inputs.
- **B3: `ui/src/App.tsx`** — agora usa `useSSE` em vez de
  `useParamStream`. Usa `defaultPipelineConfig()` (não shape hardcoded).
  Toggle de modo `manual`/`assisted` exposto ao usuário (não mais
  hardcoded `'manual'`). Adiciona `Player` quando recebe `job.completed`.
  Mostra `api.error.fieldErrors()` em vermelho abaixo do botão (item C1).
- **B3: `ui/src/components/UploadPanel.tsx`** — aceita `mode` + `onModeChange`
  como props. Botões "Manual" / "Assistido" selecionáveis. Usa `useCallback`
  para os handlers.
- **C3: `ui/src/components/ProposalOverlay.tsx`** — overlay agora editável.
  Cada parâmetro da sugestão renderiza um input (number/string/checkbox/
  JSON textarea para arrays). Botão muda label para "Aprovar com ajuste"
  quando o usuário mexeu. TTL countdown via `useEffect` (não `useState`).
- **C1: `ui/src/hooks/useApi.ts`** — `fetchJson` agora parseia erros como
  RFC 7807 (`application/problem+json`) e materializa `ApiRequestError`.
  `approveProposal` aceita `ApproveRequestBody` com `parameters` (item C3).
  `rejectProposal` aceita `reason`. `createJob` recebe `(trackId, mode,
  prompt, pipelineConfig?)` em vez de `JobRequest` cru.
- **`crates/audio_agent/src/react_kernel.rs`** — `ReActOutput` agora
  derive `Serialize` + `Deserialize` (antes não tinha; o worker precisava
  serializar a receita para passar pelo `spawn_blocking`).
- **`crates/audio_api/src/config/mod.rs`** — adicionado `#[derive(Default)]`
  em `AppConfig`, `DatabaseConfig`, `StorageConfig`, `AudioConfig`,
  `LlmConfig`, `ObservabilityConfig` para permitir construção em testes
  via `AppConfig::default()`. Campos sem `#[serde(default)]` antes agora
  têm — não quebra parse de YAML existente.
- **`crates/audio_api/src/main.rs`** — módulos movidos para `lib.rs`
  (`mod adapters; mod config; ...` viraram `use audio_api::{adapters,
  config, ...}`). Necessário para integration tests terem acesso.
- **`README.md`** — removida a afirmação incorreta de que "o loop ReAct é
  `unimplemented!()`" (item C1 do relatório de análise). Status agora
  reflete que o loop está implementado, é chamado pelo worker, e a
  receita é aplicada ao pipeline.

### Deprecated

- **`ui/src/hooks/useParamStream.ts`** — virou re-export de `useSSE`.
  Imports antigos continuam funcionando por compat. Remover na próxima
  iteração após migrar todos os callers.

### Removido

Nenhum arquivo foi removido nesta iteração. Dead code identificado no
relatório (`crates/audio_api/src/sse/route.rs`, `ProposalHandlers::store`
field) foi preservado para reduzir churn — limpeza fica para a próxima
iteração, marcada como `#technical-debt` no relatório.

### Pendências conhecidas (não resolvidas nesta iteração)

Estes itens do mapa de ação não foram fechados — ver
`analise-arquitetural-mixlirous.md` para detalhes:

- **B5:** `ProposalStore` nunca populado pelo worker
  (`HubCallbacks::on_proposal_created` é `{}`). HITL real pendente.
- **B7:** Replay SSE via `Last-Event-ID` não implementado (broadcast de
  fan-out ≠ ring buffer de replay).
- **C4 do relatório:** `GET /api/v1/jobs/{id}` ainda retorna `JobSummary`
  (4 campos) em vez do `JobResponse` completo documentado.
- **C5:** `list_jobs` ignora cursor.
- **C6:** `cancel_job` é placeholder.
- **C7:** SSE event `agent.tool_call` publicado como `agent.tool` (sem `_call`).
- **C9:** `get_track_peaks` retorna array sempre vazio.
- **C12:** 9 endpoints REST documentados mas não implementados no router
  (retry, system/resources, system/scale, tenants/me, tracks DELETE,
  tracks/{id}/events SSE, PATCH nodes parameters, DELETE nodes parameters).
- **M1:** `MockLlm` é o provider ativo em produção.
- **M2:** System prompt hardcoded em `react_kernel.rs::build_llm_request`
  (não lê de `prompts/*.prompt`).

### Critérios de aceite verificados

- ✅ `cargo build --workspace` — verde
- ✅ `cargo test -p audio_api --tests` — 4 e2e tests passing
- ✅ `cargo test -p audio_core --test contract_ts_rust` — 4 contract tests passing
- ✅ `cargo test -p audio_agent` — 71 unit tests passing
- ✅ `cargo test -p audio_core --lib` — 199 unit tests passing
- ✅ `cd ui && npx tsc --noEmit` — limpo
- ✅ `cd ui && npx eslint .` — limpo
- ✅ `cd ui && npx vitest run` — 5 tests passing
- ⚠️ `cargo test --workspace --no-fail-fast` — 2 testes falham em
  `audio_core/tests/aliasing.rs` por falta de fixtures (gitignored,
  gerados por `scripts/generate_fixtures.py`). Falha pré-existente,
  não relacionada a esta iteração.
