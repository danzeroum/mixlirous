# Diário de sessões — ciclo WebQA (qa/ciclo-inicial)

Formato: uma entrada por sessão de trabalho, sempre no fim do arquivo.
Nada é reescrito; correções viram entradas novas.

---

## 2026-09-08 (S1) — Re-execução após perda do sandbox

**Incidente de continuidade (registrado com transparência):** um ciclo
anterior de WebQA chegou a 16 commits na branch `qa/ciclo-inicial`, mas o
push falhou (401 — token sem permissão/revogado) e o sandbox do agente foi
**resetado** antes de um segundo envio. Os commits existiam apenas no
sandbox e foram perdidos. Verificação independente no GitHub confirmou:
branch `qa/ciclo-inicial` inexistente, `main` intacta em `d45e74e`,
`docs/qa/` ausente. **Lição registrada: push cedo, push sempre** — a partir
desta sessão, a branch é empurrada assim que o primeiro commit existe.

**Decisão:** re-executar o ciclo **do zero e empiricamente**. Os achados
do ciclo perdido servem apenas como *checklist mental de suspeitas*; nenhum
achado entra em `ACHADOS.md` sem reprodução própria nesta execução.

**Fase 0 — leitura estrutural (feita antes de qualquer teste):**
- Workspace Cargo declara `crates/{audio_core, audio_agent, audio_api}` —
  o `audio_api/` na raiz é árvore legada (ambiguidade resolvida; ver
  `DECISOES.md` D-001).
- Perfil local (`config/local.yaml`): sqlite + storage `local` + rate limit
  6000/min → stack sem docker/postgres/minio. `CONFIG_ENV=local`.
- Rotas mapeadas direto de `crates/audio_api/src/routes/mod.rs` (ver
  `mapa-rotas.yaml`): `/healthz`, `/readyz`, `/metrics` na raiz; API sob
  `/api/v1` (jobs, prompts, tools, tenants, system, proposals, auth,
  uploads, tracks); `dev_router` **só** com `MIXLIROUS_DEV_SLICE=1` e
  `CONFIG_ENV != production` (não ativado neste ciclo).
- UI: SPA React/Vite **sem react-router** — 5 views por estado (projetos,
  biblioteca, novo-remix, atividade, workspace). Dev server 5173 com proxy
  `/api` → 8080. Playwright e2e existente (`ui/e2e/full-flow.spec.ts`)
  auto-pula sem stack.
- Observação inicial (a confirmar empiricamente): `api_router()` não aplica
  `DefaultBodyLimit` (o default do axum é 2 MB) — o comentário sobre
  "faixa real em WAV passa de 50 MB" está no `dev_router`, que tem limite
  próprio. Suspeita de 413 em upload real (a suíte/fluxo e2e dirá).
- Ambiente do agente: **sem docker** (binário nativo), 4 GB RAM, 2 CPUs,
  ~9 GB disco livre. Builds em debug para ergonomia; perfis de latência
  avaliados com esse caveat (ver `DECISOES.md` quando aplicável).

**Instalações em curso:** rustup (stable, minimal), venv Python da suíte
(nota régua: `requirements.txt` da suíte tem linha corrompida
`httpxttp2]>=0.27` — instalado `httpx[http2]` manualmente sem tocar na
suíte; será aberto achado `regua`), Playwright 1.56.0 + Chromium, `npm
install` da UI.

**Próximo:** subir API (CONFIG_ENV=local) + UI (vite), smoke manual
(`/healthz`, home 5173), primeiro perfil `backend` como linha de base.

---

## 2026-09-08 (S1, continuação) — Stack de pé e baseline backend

**Stack:** API sobe nativa (`CONFIG_ENV=local`, sqlite, storage local,
porta 8080; build debug 2m34s) + UI vite dev 5173 com proxy `/api`.
Incidente de ambiente: `npm install` ceifado no meio deixou o binário
nativo do `lightningcss-linux-x64-gnu` **truncado em 80 KB** (SIGBUS ao
carregar — vite 8 usa lightningcss/rolldown). Reinstalação do pacote
resolveu. Registrado como risco operacional do sandbox (processos
background são ceifados; scripts de instalação agora rodam em
foreground).

**Baseline perfil `backend`** (laudos/backend-antes.txt):
**7 failed, 9 passed, 3 skipped, 1 xfailed** —
f413-upload, sem-headers-de-segurança, sem-compressão, 404-fallback,
HSTS, https-loopback, x-content-type-options.

**Prova empírica do achado mais grave (QA-0001):** WAV de 5 MB via
`PUT /api/v1/uploads/{key}` → **413 direto na API** (default do axum =
2 MB; envio cortado em ~2,75 MB); via proxy vite → 502; 1,5 MB → 204.
Produção: nginx `client_max_body_size 100M` deixa passar — o axum
rejeita. Upload real quebrado em produção.

**Armadilhas empíricas registradas:** `/healthz` na origem dev devolve
**200 fake (HTML do fallback SPA)** — o check de saúde "passa" pelo
motivo errado; `/api/*` 404 tem corpo vazio (sem problem+json do
contrato docs/03); `traceparent` não é ecoado (contrato docs/03).

**Achados abertos:** QA-0001..QA-0007 (correções), QA-0008/QA-0009
(régua). Ordem de ataque: QA-0001 (alta, quebra funcional) →
QA-0005/QA-0006 (contrato de API) → QA-0002/0003/0004/0007 (vite +
docs/18).

---

## 2026-09-08 (S1, continuação 2) — QA-0010: mojibake sistêmico nos fontes

**Descoberta:** 50 arquivos `.rs` com comentários/mensagens corrompidos
(mojibake — UTF-8 lido como cp850/latin-1 e regravado; travessões como
`ÔÇö`, acentos como `├ó`). Difícil de ler, bloqueava edição limpa, e
mensagens de erro exibíveis saíam ilegíveis.

**Correção:** script `repara-mojibake-v3.py` (fora do repo, em
`scripts/` do agente) — reversão por token com validação UTF-8 gulosa;
52 arquivos consertados; resíduo zero conferido por varredura de
assinaturas. `cargo check` + **`cargo test --workspace`: 430 testes,
0 falhas** (fixtures de áudio gerados localmente conforme design —
`scripts/generate_fixtures.py`; WAVs são gitignored, só o manifest é
versionado).

**Nota de honestidade:** os 2 testes de `aliasing` falham ANTES da
geração de fixtures (passo manual documentado na própria mensagem do
teste) — não é regressão; com fixtures geradas, verdes.

**Incidente operacional:** disco chegou a 99% (target/ debug 5,8 GB) —
mesma causa-morte do ciclo anterior. Mitigado: incremental limado,
chromium redundantes (builds 1200/1234, não usados pela suíte 1.56)
removidos, caches limpos. O agente deletou por acidente o clone local
do qa-suite (linha de `rm` longa demais) — re-clonado intacto do
GitHub; a suíte nunca foi modificada (lei nº 3 preservada).

---

## 2026-09-09 (S2) — Retomada: QA-0005 (echo traceparent W3C) e QA-0006 (problem+json RFC 7807)

**Contexto da retomada:** o usuário pediu continuação do ciclo. Estado no
GitHub em `qa/ciclo-inicial` (commit `56b2c84` — fix QA-0001, upload).
Cycle lost do sandbox anterior: QA-0005 e QA-0006 foram reescritos do zero
seguindo os achados em `ACHADOS.md` (extractor existe, eco não; 404
`/api/*` com corpo vazio).

**Implementação QA-0005 — eco de `traceparent` (W3C Trace Context):**
- Novo `crates/audio_api/src/middleware/trace.rs`: middleware `from_fn`
  `echo_traceparent` que lê `traceparent` da request, valida formato W3C
  (`version-trace_id-parent_id-flags`, 32+16 hex lowercase, não-zero),
  ecoa se válido ou gera novo via RNG (16 bytes trace_id + 8 bytes
  parent_id, flags `01` sampled). Insere extensão `TraceIdInResponse` no
  request para handlers/fallbacks lerem e incluírem no `problem+json`.
- Aplicado em `main.rs` como **camada mais externa** (depois do
  `rate_limit`) — vê TODAS as respostas, incluindo 429 e 404 do
  fallback. No `api_router` também aplicado para testabilidade isolada.
- 11 testes unitários em `middleware::trace::tests` + 4 testes de
  integração `qa0005_*` em `tests/qa_contracts.rs`.

**Implementação QA-0006 — `application/problem+json` (RFC 7807):**
- Novo `crates/audio_api/src/problem.rs`: struct `Problem` com campos
  RFC 7807 (`type`, `title`, `status`, `detail`, `instance`) + extensões
  do catálogo docs/03 §4 (`code`, `trace_id`, `errors[]`). Helper
  `not_found(uri, trace_id)` constrói resposta 404 com
  `Content-Type: application/problem+json` e body JSON não-vazio.
- Fallback `api_fallback_problem` no `api_router` (rotas /api/v1/* não
  mapeadas) e `app_fallback_problem` no app (rotas /api/* fora de
  /api/v1, ex.: `/api/webqa-nao-existe` que a suíte usa). Ambos usam
  `OriginalUri` para preservar o path completo — `nest("/api/v1", ...)`
  stripa o prefixo da URI que chega ao `api_router`.
- Títulos estáveis por `code` (catálogo `docs/03 §4`) em `title_for_code`
  — a UI pode apresentar o `title` mesmo sem conhecer o `code`.
- 6 testes unitários em `problem::tests` + 6 testes de integração
  `qa0006_*` em `tests/qa_contracts.rs`.

**Validação:**
- `cargo test --workspace`: **447 testes, 0 falhas, 1 ignored** (lib
  207 + audio_api lib 142 + audio_agent 71 + aliasing 2 + routes_lote2
  14 + qa_contracts 10).
- Fixtures de áudio geradas (`scripts/generate_fixtures.py`) — passo
  manual documentado, sem ele os 2 testes de aliasing falham por
  arquivo não encontrado (não regressão).
- `rand = "0.9"` adicionado ao workspace (novo no `Cargo.toml`).

**Pendência de validação empírica:** a suíte WebQA precisa rodar contra
API+UI para confirmar que `test_correlacao_de_requisicoes` deixa de
xfail e `test_erro_de_api_e_estruturado` deixa de skip. Vai rodar no
próximo passo do ciclo.

**Push imediato após commit** (regra 3 internalizada — o ciclo anterior
se perdeu por não seguir isto).
