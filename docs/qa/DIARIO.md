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
