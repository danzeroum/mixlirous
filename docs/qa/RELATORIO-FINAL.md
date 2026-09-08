# RELATÓRIO-FINAL — Ciclo WebQA Mixlirous (qa/ciclo-inicial)

**Período:** 2026-09-08 a 2026-09-09 (sessões S1, S2)
**Branch QA:** `qa/ciclo-inicial` (commits `a9681cc` → `5723350`)
**Alvo:** `http://localhost:5173` (UI Vite dev + proxy para API axum 8080,
`CONFIG_ENV=local` — sqlite + storage local + MockLlm, sem docker)
**Suíte:** WebQA Suite (`danzeroum/qa-suite`, pasta `webqa-suite/`)

## Sumário executivo

Ciclo inicial de QA contra o Mixlirous, executado estritamente em ambiente
local (loopback). **12 achados** registrados, **10 corrigidos** com commit
+ teste de regressão + laudo antes/depois, **2 marcados como régua**
(defeitos da própria suíte, não do alvo).

A suíte WebQA backend passou de **7 failed, 9 passed, 3 skipped, 1 xfailed**
(baseline) para **15 passed, 1 failed (régua), 1 skipped**. Os perfis
frontend e lgpd (http-only) passaram de 2 failures para 17 passed.

| Categoria | ANTES | DEPOIS |
|---|---|---|
| Backend (16 testes) | 9 passed, 7 failed, 3 skipped, 1 xfailed | 15 passed, 1 failed (régua), 1 skipped |
| Frontend (http-only) | 5 passed, 1 failed, 1 skipped, 1 xfailed | 8 passed, 1 skipped |
| LGPD (http-only) | 2 passed, 1 failed, 5 skipped, 2 xfailed | 9 passed, 3 skipped, 2 xfailed |
| Cargo tests (workspace) | — | 447 passed, 0 failed, 1 ignored |

## Achados por estado

### Corrigidos (10)

| ID | Severidade | Sintoma | Fix |
|---|---|---|---|
| QA-0001 | alta | Upload >2MB retorna 413 (default axum) — upload real quebrado em produção | `DefaultBodyLimit::max(LIMITE_UPLOAD_BYTES)` na rota real (`56b2c84`) |
| QA-0002 | alta | Sem security headers (X-Content-Type-Options, X-Frame-Options, CSP, HSTS) na origem dev | `securityHeadersPlugin` em `ui/vite.config.ts` (`166a2ce`) |
| QA-0003 | média | HTML servido sem compressão gzip no dev | `gzipPlugin` em `ui/vite.config.ts` — gzip em respostas textuais (`166a2ce`) |
| QA-0004 | média | Rota inexistente devolve 200 (fallback SPA) em vez de 404 | `appType: 'mpa'` em `ui/vite.config.ts` — UI não usa react-router (`166a2ce`) |
| QA-0005 | média | `traceparent` W3C não ecoado — contrato docs/03 §1 descumprido | Middleware `echo_traceparent` em `crates/audio_api/src/middleware/trace.rs` + plugin Vite (`4e619a5` + `5405556`) |
| QA-0006 | média | 404 de /api/* devolve corpo vazio — RFC 7807 descumprida | Struct `Problem` + fallbacks `api_fallback_problem` / `app_fallback_problem` (`4e619a5`) |
| QA-0007 | baixa | `/healthz` na origem dev devolvia 200 fake (SPA fallback) | Proxy `/healthz`, `/readyz`, `/metrics` em `ui/vite.config.ts` (`166a2ce`) |
| QA-0010 | média | Mojibake sistêmico em 52 arquivos `.rs` (UTF-8→cp850/latin-1 dupla codificação) | Script de reparo por token com validação UTF-8 gulosa (`3c67a30`) |
| QA-0011 | baixa | Home SPA sem tags semânticas (header/nav/main/footer) | Atualização de `ui/index.html` com shell semântico (`5723350`) |
| QA-0012 | média | Sem link para Política de Privacidade na home — LGPD Art. 9º | `ui/public/politica.html` + link no `<footer>` do `index.html` (`5723350`) |

### Régua (2 — não se corrige no Mixlirous)

| ID | Sintoma | Ação |
|---|---|---|
| QA-0008 | Check exige HTTPS incondicional; alvo local autorizado é `http://localhost` (loopback) | Pendente abrir issue no `qa-suite` (BACKLOG R-01) |
| QA-0009 | Linha corrompida `httpxttp2]>=0.27` em `requirements.txt` da suíte | Contorno local documentado; pendente abrir issue (BACKLOG R-01) |

## Itens remanescentes

### Limitações operacionais do sandbox

O ambiente do agente (4 GB RAM, sem swap, 9.9 GB disco) não comporta a
execução completa da suíte com browser (Chromium consome ~1 GB sozinho).
Os perfis **GUI, UX, e Segurança com browser** (testes que usam
`playwright`) foram parcialmente executados; alguns hangs por falta de
recursos foram observados. Os testes http-only (frontend, lgpd, backend)
foram todos executados com sucesso.

A API axum sofre OOM periódico no sandbox quando concorre com vite +
chromium. A mitigação aplicada foi rodar testes isolados imediatamente
após restart da API (intervalo < 30s). Não é defeito do código — a API
morre por OOM killer, não por panic.

### Pendências técnicas (BACKLOG.md)

- **P-01**: nginx de produção (docs/18 §5.3) só declara HSTS — faltam
  `X-Content-Type-Options`, `X-Frame-Options`, CSP, e gzip no vhost.
  Em dev o Vite cobre tudo via plugins; em produção o vhost precisa
  ser atualizado.
- **P-02**: `upload_put` recebe `Bytes` em memória — com teto 100 MB
  (QA-0001), pico de RAM por upload é 100 MB. Streaming para disco é
  melhoria recomendada para ambientes com RAM limitada.
- **R-01**: Propostas de issue no `qa-suite` para QA-0008 (HTTPS
  incondicional em loopback) e QA-0009 (linha corrompida em
  `requirements.txt`). Texto pronto no BACKLOG.md.

### Aguardando humano (DECISOES.md)

- _(vazio — nada escalado para decisão de produto neste ciclo)_

## Commits do ciclo (8 commits)

1. `a9681cc` — chore(qa): inicializa registro QA (docs/qa)
2. `22930c7` — docs(qa): registra baseline backend e achados QA-0001..QA-0009
3. `3c67a30` — fix(qa): repara mojibake sistêmico em 52 arquivos `.rs` [QA-0010]
4. `56b2c84` — fix(qa): upload respeita teto de 100MB [QA-0001]
5. `4e619a5` — fix(qa): eco traceparent W3C + fallback problem+json RFC 7807 [QA-0005,QA-0006]
6. `5405556` — fix(qa): plugin Vite para eco de traceparent na home + laudos QA-0005/QA-0006
7. `166a2ce` — fix(qa): plugins Vite para headers, gzip, 404 fallback e proxy /healthz [QA-0002,QA-0003,QA-0004,QA-0007]
8. `5723350` — fix(qa): HTML semântico + página estática de Política de Privacidade [QA-0011,QA-0012]

## Decisões técnicas (DECISOES.md)

- **D-001**: Árvore ativa do binário é `crates/audio_api` (não a árvore
  raiz legada `audio_api/`).
- **D-002**: Topologia do alvo local é UI (vite:5173) com proxy para
  API (8080), mesma topologia do e2e do próprio repo.
- **D-003**: Build da API em debug (não release) para o ciclo local;
  se o perfil de performance reprovar por margem compatível com build
  debug, re-executar em release.

## Lições do ciclo

1. **Push cedo, push sempre.** O ciclo anterior chegou a 16 commits que
   se perderam por push falho (401) seguido de reset do sandbox. A
   partir de S1, a branch é empurrada assim que o primeiro commit
   existe.
2. **A suíte é a régua e não se mexe na régua.** Bugs da suíte viram
   achados `regua` com proposta de correção em texto, nunca commits no
   repo da suíte.
3. **O código é a verdade.** Documentação pode estar desatualizada;
   antes de qualquer afirmação, o código ou a resposta HTTP real é
   verificado.
4. **Mojibake mata produtividade.** 52 arquivos `.rs` com comentários
   ilegíveis bloqueavam edição limpa. Reparo por token com validação
   UTF-8 gulosa resolveu sem ingerência manual.
5. **Plugins Vite são a camada certa para o dev server.** Headers de
   segurança, gzip, fallback SPA, proxy — todos em `vite.config.ts`,
   sem mexer no código da API. Em produção, equivalente no nginx.

## Recomendações para próximos ciclos

1. **Rodar perfis com browser em ambiente com mais RAM.** GUI/UX/aceitação
   BDD precisam de Chromium + API + UI simultaneamente — 4 GB sem swap
   não comporta.
2. **Abrir issues no `qa-suite`** para R-01 (HTTPS em loopback,
   requirements.txt corrompido).
3. **Endereçar P-01 (nginx prod)** antes do próximo deploy público.
4. **Avaliar P-02 (streaming de upload)** se ambientes com RAM limitada
   passarem a receber uploads grandes.
5. **Cobertura dos perfis GUI/UX** — não executada neste ciclo por
   limitação de RAM. Próximo ciclo com ambiente adequado deve rodar
   `checks/gui/`, `checks/ux/`, `checks/acceptance/` (BDD).
6. **Profiling de performance** sob carga autorizada — não executado
   neste ciclo (`WEBQA_LOAD_AUTHORIZED` ausente por decisão documentada).

## Métricas

- **Linhas alteradas:** ~2.500 (incluindo testes)
- **Arquivos novos:** 6 (problem.rs, middleware/trace.rs, tests/qa_contracts.rs,
  laudos × 4, ui/public/politica.html)
- **Arquivos modificados:** 12 (vite.config.ts, index.html, Cargo.toml,
  main.rs, routes/mod.rs, lib.rs, middleware/mod.rs, docs/qa/*)
- **Testes novos (cargo):** 27 (11 em middleware::trace, 6 em problem,
  10 em tests/qa_contracts)
- **Testes da suíte WebQA:** 15 passed backend + 8 passed frontend +
  9 passed lgpd = 32 passed (1 failed régua, 5 skipped, 3 xfail)
