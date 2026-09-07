# Adendo Pareto — Plano de Produção Mixlirous

> Complementa o "Plano de integração completo para produção" com correções verificadas contra o código atual (`main`, commit `244063f5a4`, 2026-08-20) e uma sequência de 80/20 para execução solo.

## 1. Correções ao plano original (validadas no código)

O plano de 10 fases está correto na direção, mas alguns itens listados como "trabalho a fazer" já estão parcial ou totalmente resolvidos. Executar esses itens de novo seria desperdício.

| # | Item do plano original | Estado real verificado | Fonte |
|---|---|---|---|
| 1 | "Migrar segredos do `production.yaml`" | **Já feito.** `url: "${DATABASE_URL}"`, comentário explícito "injetado via secrets manager; nunca hardcoded aqui". A doc `docs/08` é que está desatualizada. | `config/production.yaml` |
| 2 | "Corrigir exposição 0.0.0.0 no compose (#28)" | **Já corrigido no código.** `docker-compose.yml` documenta a remoção da publicação 0.0.0.0; serviços conversam só pela rede interna `mixlirous`. A issue #28 segue aberta — falta apenas confirmar em VPS real e fechar. | `docker-compose.yml` |
| 3 | "Endpoint de diagnóstico como risco de segurança" | **Já é fail-closed.** Não registrado sem `MIXLIROUS_DEV_SLICE=1`; ignorado sob `CONFIG_ENV=production` com log de erro. Risco residual: sem auth de aplicação própria, depende do `auth_basic` do proxy — documentar isso como requisito de deploy, não como bug de código. | `main.rs`, `.env.example`, `docs/18` |
| 4 | "Implementar `POST /jobs/:id/cancel`" | **A rota já existe** no router (`jobs::cancel_job`). O trabalho real é implementar a lógica — hoje é placeholder (CHANGELOG C6). Menor esforço do que criar do zero. | `crates/audio_api/src/routes/mod.rs`, `CHANGELOG.md` |
| 5 | "HITL de propostas pendente" (citado no README) | **Falso — já resolvido no código.** `App.tsx` deriva a proposta pendente do evento `agent.proposal`; overlay tem Aprovar/Recusar/Aprovar-com-ajuste. O README ainda traz esse aviso desatualizado. | `ui/src/App.tsx` |
| 6 | `retry` do job | **Confirmado ausente**, com data no próprio contrato: nota "(2026-08-20): endpoint retry ainda não implementado no router". Não é suposição — é status oficial do projeto. | `docs/03-CONTRATOS-API.md` |
| 7 | (não previsto no plano) `save_job` persistia só config/blocks | **Divergência achada no Lote 2**: os dois adapters descartavam `mode`, `user_prompt` e `track_id` — todo job criado via `POST /jobs` chegava ao worker sem track e falhava com "no track_id/object_key associated with job". As colunas SQLite já existiam (migration 002); faltava populá-las. Corrigido com `JobMeta` atômico em `AudioRepo::save_job`. | `crates/audio_core/src/ports/repo_trait.rs`, `crates/audio_api/src/adapters/*` |
| 8 | (não previsto no plano) `GET /auth/local-session` cria tenant NOVO a cada chamada | **Divergência achada na integração dos Lotes 2+3**: dois callers independentes acionavam a rota (bootstrap do App e `ensureSseSession`), e com o StrictMode do React o upload/job podiam ficar no tenant de um token enquanto o cookie de SSE ficava no de outro — download do artifact dava 404. Mitigado no frontend com `ensureLocalSession()` singleton (um token/tenant por navegador, reutilizado nos reloads). **Pendente**: renovação automática após expiração do token (30 dias) — hoje exige limpar o localStorage. | `ui/src/lib/authHeaders.ts`, `crates/audio_api/src/routes/auth.rs` |
| 9 | (não previsto no plano) HITL explicável sem dados de confiança/risco | **Achado no Lote de design**: o payload de `agent.proposal` publicado pelo worker não inclui `confidence`/`risk`/`impact`/`at_sec`. O overlay do plano de design renderiza esses campos QUANDO existem (nunca simula). Enriquecer o payload é trabalho de backend ligado ao item B5 (ProposalStore). | `crates/audio_api/src/worker.rs`, `ui/src/components/ProposalOverlay.tsx` |

## 2. Causa raiz do desalinhamento

O README carrega um bloco desatualizado: *"Ainda pendentes: HITL de propostas (store nunca populado), replay SSE via Last-Event-ID, e 9 endpoints REST documentados mas não implementados"*. Esse texto já induziu uma análise externa a concluir erroneamente que o Mixlirous está em pré-alpha sem fluxo de upload. Corrigir essa única fonte evita repetição do erro por qualquer pessoa (ou agente) que audite o projeto depois.

## 3. Sequência Pareto (80/20)

Ordenada por razão impacto/esforço, para execução solo com agentes de IA.

| # | Ação | Esforço | Impacto |
|---|---|---|---|
| 1 | Reescrever o bloco "Ainda pendentes" do README; sincronizar `docs/08`, `docs/14` e `RELATORIO-AUDITORIA.md` com o estado real | horas | Elimina a causa raiz de diagnósticos errados sobre o projeto |
| 2 | Fechar/atualizar issues já resolvidas no código (#28 após teste em VPS, #34) e anotar status real nas P0 de DSP | minutos–horas | Backlog deixa de misturar dívida quitada com bloqueador ativo |
| 3 | CI rodando incondicionalmente em todo PR (#5) | horas | Pré-requisito de tudo mais — sem isso, nenhum gate de qualidade é confiável |
| 4 | UI respeitar `available: false` do `GET /tools` (esconder/desabilitar ghost tools com motivo) | horas | Backend já envia o campo certo; evita o usuário pedir compressão/EQ inexistentes |
| 5 | Implementar `get_track_peaks` real (C9) | 1–2 dias | Desbloqueia waveform — a feature de UX mais visível de todo o plano |
| 6 | Player A/B: ligar lado "original" ao `track_id` do job em vez de upload manual | horas | Remove a costura mais estranha do fluxo atual |
| 7 | Implementar a lógica de `cancel_job` (rota já existe, é placeholder) | 1 dia | Menor esforço possível para uma operação crítica de UX de job |
| 8 | SSE auth via cookie de sessão same-origin atrás do Nginx (#33) | 1–2 dias | Resolve o bloqueador de deploy público sem reescrever para WebSocket |
| 9 | Implementar `retry` de job (requeue simples) | 1–2 dias | Fecha o par cancel/retry que todo usuário de ferramenta de processamento espera |
| 10 | Grafo → `pipeline_config`: serializar `graphStore` no `POST /jobs` | 2–4 dias | Transforma o canvas de decorativo em funcional — o gap estrutural mais importante |
| 11 | Corrigir LUFS vs. limiter (#37, P0) e onset em áudio real (#27, P0) | 2–4 dias | Sem isso, qualidade sonora do resultado final é o gargalo real, não a UI |
| 12 | 1 spec Playwright cobrindo upload → job → aprovar → download | 1–2 dias | Infraestrutura já instalada; cobre o fluxo inteiro com esforço mínimo |

## 4. Cortes recomendados (adiar sem culpa)

- Stems, reverb, delay, pitch shift — dependem de binário externo; core de fade/crossfade/loudness bem feito já é produto viável.
- WebSocket para SSE — over-engineering; cookie/token de curta duração resolve a #33 com um décimo do esforço.
- Escuta cega com múltiplos avaliadores — substituir por métricas objetivas (energia na emenda, true peak, delta de LUFS) até o beta.
- SLO público (99,5%) — medir primeiro, prometer depois.
- Multitenancy/billing completo — quotas simples por usuário bastam enquanto houver poucos usuários.

## 5. Cronograma resultante

- **Semana 1:** itens 1–4 (higiene documental e de segurança, quase gratuitos).
- **Semanas 2–3:** itens 5–9 (UX de job e operações essenciais).
- **Semanas 3–4:** itens 10–12 (canvas executável, qualidade sonora crítica, E2E mínimo).

Esse recorte de ~4 semanas entrega uma alpha honesta e testável, deixando o plano de 10 fases original como mapa de médio prazo para o beta e a versão 1.0.
