# Changelog

Todos os mudanças notáveis deste projeto serão documentados neste arquivo.
Formato baseado em [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/),
versionamento [SemVer](https://semver.org/lang/pt-BR/).

## [Unreleased] — Sistema de cor da marca

### Adicionado

- **Tokens da marca em `@theme`** (`ui/src/index.css`, fonte canônica
  `docs/design/tokens/theme.css`) — oito famílias semânticas: `surface`
  (base neutra-quente, inclui o degrau 850 que a escala padrão do Tailwind v4
  não tem), `ink` (texto), `action` (teal do CTA da landing), `ai` (violeta do
  poster), `manual` (azul-ciano do holograma), `warn` (âmbar do case), `danger`
  (vermelho H=357°, deslocado do coral) e `brand` (coral, exclusivo de marca —
  R2). Oito famílias. Amostragem e justificativa em `docs/design/00-PALETA-E-ORIGEM.md`.
- **Constantes para Canvas/React Flow** (`ui/src/lib/theme.ts`) — `theme` +
  `canvasColors`; Waveform e RemixCanvas importam daqui (fim dos 4 hexes em
  .tsx; o 5.º, do anel de foco no index.css, virou `var(--color-ai-400)`).
- **Guardas automatizadas** — teste de sincronia CSS↔TS
  (`theme.spec.ts`: token a token, falha se um lado divergir ou sumir;
  também protege o bloco de acessibilidade do #59), teste de contraste
  calculado (`themeContrast.spec.ts`: fórmula WCAG 2.1 recalcula todos os
  pares aprovados de `01-TOKENS.md`, inclui o known-fail action-600+branco
  4.49 que motiva R5) e `npm run lint:colors`
  (`scripts/lint-colors.mjs`: falha com classe de família nativa ou hex
  literal fora de index.css e lib/theme.ts).

### Alterado

- **Migração completa das 343 ocorrências de classe de cor** (61 combinações
  em 12 arquivos) conforme `docs/design/02-MAPA-MIGRACAO.md` — incluindo as
  seis classes que o mapa não cobria (linhas adicionadas: `bg-red-800`,
  `text-red-100`, `bg-transparent`, `border-purple-500/700/800` por degrau
  justificado) e dois refinamentos por proeminência de seleção
  (`border-purple-500`→`ai-400`, `border-blue-500`→`manual-400`).
- **Correção de defeito** (`ui/src/App.tsx`): `bg-gray-850` não existe na
  escala do Tailwind v4 e não gerava CSS — sidebar virou `bg-surface-850`.
  Única mudança de aparência por correção de defeito, não por troca de paleta.
- **Semântica R3**: item ativo da navegação global saiu da família `ai`
  (proibida para seleção genérica) para neutro `surface-700` + `ink-100`.
- Texto sobre botões: `text-white` preservado só dentro de botão preenchido
  (R5 — pares calculados com branco; 16 usos); os demais 31 viraram
  `text-ink-100`. Secundários usam `surface-700` + `ink-100` (R5).

## [Unreleased] — Plano de design centrado no usuário (etapa única vertical)

### Adicionado

- **Política de privacidade auditável** (`GET /api/v1/system/privacy-policy`,
  `crates/audio_api/src/routes/system.rs`) — provider/model ativos,
  `audio_sent_to_provider` (false, com contraprova estrutural: o contexto do
  LLM é só metadados numéricos via `worker.rs::agent_context_for_track`),
  `prompt_sent_to_provider`, `analysis_metadata_sent_to_provider`,
  `retention_policy`, `training_opt_out` e `region` descrevendo o que a
  instalação faz de fato. Testes unit + HTTP de integração.
- **Revogação real de consentimento** (`DELETE /api/v1/tenants/me/consent`,
  `crates/audio_api/src/routes/tenants.rs` + `AudioRepo::revoke_consent`
  nos adapters InMemory/SQLite) — idempotente, escopada por tenant,
  com registro de data/ator/tenant/provider em log estruturado;
  trocar para modo manual NÃO revoga (a UI explica). Testes handler +
  HTTP ponta a ponta.
- **Transparência estéreo→mono (Fase A do épico estéreo)** —
  `worker.rs::aviso_downmix_mono` publica `job.warning` `mono_downmix`
  com metadados de canais (`source_channels`/`analysis_channels`/
  `processing_channels`/`output_channels`/`channel_policy=
  downmix_arithmetic_mean`) para arquivos com >1 canal;
  `GET /tracks/{id}/peaks` expõe `channels`/`sample_rate` do decode real;
  UI exibe o aviso no wizard (análise), no preview (Player) e na
  timeline (JobTimeline mostra `job.warning` em texto); helper
  `avisoCanais` em `ui/src/lib/wavPeaks.ts` com testes Vitest.
- **E2E honestos de Atividade** (`ui/e2e/full-flow.spec.ts`) — estado
  vazio explicável em contexto novo (não mais "pode estar vazio" num
  teste de histórico) + fluxo com job real (espera determinística via
  GET /jobs, localização por ID, status em TEXTO e ação contextual real:
  Abrir preview / Tentar de novo / Cancelar) + consentimento com
  revogação real via UI.
- **Navegação `Visão geral | Biblioteca | Novo remix | Atividade |
  Espaço de trabalho`** (`ui/src/App.tsx` + `ui/src/views/`) — o fluxo guiado por
  intenção é o caminho principal (view default); o canvas vira modo
  avançado (Espaço de trabalho), compatível com a experiência anterior.
  "Projetos" virou **Visão geral** com rótulo "Projeto atual do seu
  espaço" — o backend não tem domínio Project persistido e a UI não
  deve vender gestão de projetos inexistente (backlog no adendo §1.11).
  Biblioteca lista faixas com "usar no remix"; Atividade lista jobs com
  estado em TEXTO + ações reais (cancelar — C6; tentar de novo — C12,
  cria novo job_id).
- **Wizard "Novo remix"** (`ui/src/views/NovoRemixView.tsx`) — 6 passos
  numerados: upload → análise (waveform real dos picos do backend,
  Lote 2/C9) → objetivo (presets em linguagem musical + consentimento de
  IA) → receita (canvas projetando o `PipelineConfig`, Lote 3) → render
  (timeline com aria-live, cancelável) → preview/exportação.
- **HITL explicável** (`ui/src/components/ProposalOverlay.tsx`) — mostra
  alteração, razão, trecho, confiança, risco e impacto QUANDO o backend
  os envia (nunca simula; divergência no adendo §1 item 9). Ações:
  aprovar, ajustar, recusar, pedir alternativa (replan) e fazer
  manualmente (recusa + troca para modo manual). Foco inicial no botão
  primário, Escape recusa, `role=dialog`/`aria-modal`, expiração neutra.
- **Preview A/B com waveform e manifesto** (`ui/src/components/Player.tsx`,
  `ui/src/lib/wavPeaks.ts`, `ui/src/components/Waveform.tsx`) — waveforms
  remix e original lado a lado com agulha sincronizada; parser WAV no
  browser (chunks reais, PCM 8/16/24/32 int + float32, mesma semântica de
  `compute_peaks`); métricas técnicas (duração, sample rate, canais, pico
  dBFS) e manifesto de exportação com SHA-256 verificado.
- **Transparência IA/LGPD** (`ui/src/components/PrivacyPanel.tsx`) —
  provedor/modelo do assistente (GET /system/info), o que vai para a IA
  com base na política auditável (ver Modificado) e consentimento
  separado para o modo assistido (GET/POST/DELETE
  /tenants/me/consent) exigido pelo wizard.
- **Acessibilidade base** (`ui/src/index.css` + componentes) — foco
  visível (violeta AA sobre o tema escuro), `prefers-reduced-motion`,
  `aria-current` na navegação, `aria-live` no status do job e na timeline,
  radiogroup no modo, estados vazios explicáveis.

### Corrigido

- **Corrida de `CONFIG_ENV` nos testes de local-session (CI Linux do PR
  #59)** — dois testes do mesmo binário mutavam a env global do processo
  (`std::env::set_var`) em paralelo enquanto `get_local_session` lia a
  env por request; 404 não-determinístico. Correção de raiz: o modo é
  capturado no boot em `AppConfig.config_env` e lido do estado da app
  (`routes/auth.rs`); `main.rs` usa a mesma fonte única; testes passam o
  modo via config, sem `set_var`. Comportamento fail-closed segue
  testado (404 fora de "local").
- **Comunicação de privacidade não enganosa** — a frase "o áudio NÃO é
  enviado" deixou de ser hardcoded no `PrivacyPanel`: a afirmação só
  aparece com base em `audio_sent_to_provider=false` da política
  auditável servida pelo backend (testada); sem política, o texto é
  condicional ("confira os dados enviados ao provedor configurado nesta
  instalação").
- **Bootstrap do App** — sessão local é garantida ANTES dos GETs
  autenticados; `systemInfo`/consent/política podiam 401 por dispararem
  antes do token e ficarem null para sempre (botão "Concordo" travado).
- **Rate limiter configurável** (`features.rate_limit_per_minute`,
  default 60) — o browser excede 60 req/min numa sessão e o presign do
  E2E recebia 429; `config/local.yaml` usa 6000 com o limiter ATIVO.
  Nunca desligar em produção (docs/08 §8).
- E2E "Abrir preview" usa locator por heading (strict mode: o texto do
  job aparece em 2 elementos quando o player já está montado).

### Modificado

- `ui/src/components/UploadPanel.tsx` — modos `soUpload`/`soObjetivo`
  (um só conjunto de `data-testid` por tela), `trackIdExterno` para o
  wizard, radiogroup de modo, aria-live no status, dica de recuperação
  no erro de upload.
- `ui/src/hooks/useApi.ts` + `ui/src/types/api.ts` — getTrackPeaks,
  cancelJob, retryJob, replanProposal, getConsent/postConsent/
  revokeConsent, getPrivacyPolicy, listTracks; types `PeaksResponse`
  (channels/sample_rate), `ConsentInfo` e `PrivacyPolicy`.

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
  fallback para o `defaultPipelineConfig()` explícito (opção prevista no
  próprio contrato de `graphToPipelineConfig`): enquanto a paleta do
  Lote 1 (PR #55) não estiver no build, o canvas vazio se comporta como
  antes do Lote 3 — job sempre criável, sem dead end na UI.
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
- **Suítes de propriedade DSP realinhadas ao limiter pós-#37** —
  `dc_offset.rs` deixa de afirmar média zero para o `brickwall_limiter`
  (a propriedade valia por linearidade de ganho UNIFORME; com o ganho
  por amostra do #37 ela deixa de valer por construção — DC residual
  medido ~7e-4, inaudível) e `thd.rs` documenta o novo regime (teto de
  0,1% em vez de piso numérico; THD medido ~1.6e-6 escalando). Garantias
  reais do limiter seguem com cobertura dedicada em
  `dsp::mastering::limiter`. docs/17.1 §3.1 e §7 atualizados em consonância.
- **`ui/e2e/full-flow.spec.ts`** — bootstrap de sessão determinístico:
  fulfill de `GET /auth/local-session` com JWT assinado em-processo
  (HS256, segredo de dev do modo local) + cookie de SSE pela rota REAL
  `POST /auth/sse-session` (Lote 2); auto-pula com motivo explícito em
  backend sem a rota.
- **`ui/package-lock.json`** — ressincronizado com o `package.json`
  (entrada de `@tailwindcss/vite` trouxe platform packages ausentes do
  lock; `npm ci` — gate do Frontend CI — quebrava).

## [Unreleased] — Lote 2 do plano Pareto (UX de job essencial)

### Adicionado

- **C9: `GET /api/v1/tracks/{id}/peaks` real** — o handler lê o objeto do
  storage, decodifica via `decode_to_pcm` em `spawn_blocking` e reduz para
  `resolution` buckets [min, max] (`tracks.rs::compute_peaks`, com testes
  de propriedade: bucket desigual, NaN ignorado, entrada vazia). A
  waveform deixa de ser um array vazio.
- **`GET /api/v1/tracks/{id}/raw`** — stream do áudio ORIGINAL por
  track_id, tenant-scoped. Complemento do item 2 do Lote 2: o player A/B
  não precisa mais de upload manual do arquivo original.
- **C6: `POST /api/v1/jobs/{id}/cancel` real** — `AudioRepo::cancel_job`
  (novo método do trait) faz transição validada `Queued/Processing →
  Cancelled` + registro de auditoria `JOB_CANCELLED` atomicamente (nos
  dois adapters; SQLite sob transação). Estado terminal → 409
  `job_not_editable`. O handler publica `job.cancelled` no hub SSE; o
  worker reconfere o estado ao terminar a execução e descarta o
  resultado de job cancelado (nem `completed`, nem requeue).
- **C12 (parcial): `POST /api/v1/jobs/{id}/retry`** — requeue simples
  conforme contrato docs/03 §3.3: só em `failed`, cria **novo** `job_id`
  reusando receita (`config`), `track_id`, modo e prompt. O job original
  permanece `failed` como histórico.
- **Issue #33: autenticação do SSE via cookie de sessão same-origin** —
  `AuthContext` aceita, como fallback ao header `Authorization`, o cookie
  `mixlirous_session` (`HttpOnly`, `SameSite=Lax`, `Path=/api/v1`). Duas
  rotas novas em `routes/auth.rs`: `POST /auth/sse-session` (emite o
  cookie para quem já tem Bearer; TTL 1h) e `GET /auth/local-session`
  (modo local single-user do contrato §1; fail-closed fora de
  `CONFIG_ENV=local`). `useSSE` garante a sessão antes do handshake.
  Sem WebSocket, como o adendo recomenda.
- **Testes** — `crates/audio_api/tests/routes_lote2.rs` (9 testes HTTP via
  `tower::oneshot`: peaks real, peaks sem áudio, raw por tenant, cancel
  com auditoria + 409, retry feliz + 409, handshake SSE por cookie,
  local-session no local e fail-closed em produção); testes de
  cancelamento e de meta no `repo_memory`; regressão do `JobMode`.

### Corrigido

- **Gap de integração do `save_job`** (divergência documentada no adendo
  Pareto §1, item 7): os adapters descartavam `mode`, `user_prompt` e
  `track_id` — todo job criado via `POST /jobs` chegava ao worker sem
  track e falhava com "no track_id/object_key associated with job". As
  colunas SQLite já existiam (migration 002); faltava populá-las. Novo
  parâmetro `JobMeta` em `AudioRepo::save_job`, atômico com o registro.
- **Worker cancel-aware** — `job.cancelled` não é sobrescrito por
  `completed`/`failed`, e job cancelado não volta para a fila via
  `fail_and_retry`.

### Modificado

- `ui/src/components/Player.tsx` — lado "original" do A/B liga ao
  `track_id` do job (`GET /tracks/{id}/raw`); upload manual vira fallback.
- `ui/src/hooks/useSSE.ts` — `ensureSseSession()` antes do handshake.
- `ui/src/App.tsx` — `trackId` repassado ao Player.
- `.dev/module-status.yaml` — audio_api 70→80, ui 80→85.
- `docs/03-CONTRATOS-API.md` — nota de status do `retry` atualizada.
- `docs/ADENDO-PARETO-PRODUCAO.md` — divergência do `save_job` registrada
  na tabela da seção 1 (item 7).

## [Unreleased] — Lote 1 do plano Pareto (docs, CI e paleta de ferramentas)

### Corrigido

- **README/docs sincronizados com o estado real** — removidas as
  afirmações de que "9 endpoints REST documentados mas não implementados"
  e de pendências já resolvidas; `docs/08-SEGURANCA-MULTITENANCY.md` e
  `docs/14-AUDITORIA-KIT.md` alinhados ao código.
- **CI incondicional em todo PR (issue #5)** — `ci-rust.yml` e
  `ci-frontend.yml` sem filtro de `paths:`; um PR que só toca docs/UI
  não escapa do gate Rust. Branches dos triggers de push reparadas
  (`branches: [main]`).
- **Redes dos Docker Compose** — serviços referenciam a rede declarada
  `mixlirous` (`docker-compose.yml`, `docker-compose.observability.yml`,
  `docker-compose.ingress.yml`).

### Adicionado

- **ToolPalette (`ui/src/components/ToolPalette.tsx`)** — paleta
  alimentada por `GET /api/v1/tools`; ghost tools (`available: false`,
  ex. compression, dynamic_eq) desabilitadas com motivo no tooltip via
  `ui/src/lib/toolFilter.ts` (testes em `toolFilter.spec.ts`).
- **`graphStore.addToolNode`** — adiciona nó de efeito por ferramenta
  (um nó por ferramenta, posição em grade); `NodeData.tool?` preservado.
- `.dev/module-status.yaml` — ui 80→85.

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
