# DECISÕES — registro de decisões técnicas e itens aguardando humano

Formato: D-NNN. Decisões reversíveis ficam aqui com contexto e alternativas.
Itens sem resposta objetiva que dependem de dono de produto vão para a
seção **aguardando-humano** (estado explícito, não resolvidos pelo agente).

## D-001 — Árvore ativa do binário: `crates/audio_api`

**Contexto:** o repo tem `audio_api/` (raiz) e `crates/audio_api/`
duplicados; só a segunda está no workspace (`Cargo.toml` → members
`crates/*`). **Decisão:** todo ciclo (build, testes, análise de rotas)
usa `crates/audio_api`. A árvore raiz é legado não-buildável via
workspace. **Não** removemos a árvore legada neste ciclo (fora do escopo
QA; risco de quebrar referências de docs/scripts que ainda a citam) —
fica no BACKLOG como pendência de limpeza para decisão do dono.

## D-002 — Topologia do alvo local: UI (vite:5173) com proxy para API (8080)

**Contexto:** a suíte usa UM `target_url` e espera HTML na raiz; a API
não serve UI (Dockerfile não embute frontend; produção usa nginx —
docs/18). O e2e do próprio projeto usa `http://localhost:5173` com proxy
`/api` → 8080. **Decisão:** alvo da suíte = `http://localhost:5173`;
API sobe nativa com `CONFIG_ENV=local`. Justificativa: é a topologia
documentada e testada pelo próprio repo; inventar serving estático na
API seria mudar o produto para agradar a régua.

## D-003 — Build da API em debug (não release) para o ciclo local

**Contexto:** perfis de latência da suíte (p50/p95/p99) medem respostas
HTTP; build debug de handlers axum tem overhead desprezível para esse
perfil (o custo debug pesa em DSP, não em roteamento). Release em
2 CPUs custa dezenas de minutos. **Decisão:** debug; se o perfil de
performance reprovar por margem compatível com build debug, re-executar
em release antes de classificar o achado. Registramos o caveat nos
laudos.

## aguardando-humano

- **QA-0021 (aceito)**: `test_erro_500_na_api_nao_vaza_detalhe_tecnico`
  falha no vite preview porque o proxy Node.js gera HTML "500 Internal
  Server Error" quando a API responde 500. Não é detalhe técnico da
  Mixlirous (a API axum devolve `application/problem+json` via QA-0006).
  Em produção (nginx), `proxy_intercept_errors off` deixa o axum
  responder problem+json. Aceito como limitação do dev server.
  Justificativa completa em laudos/qa-0019-0021-antes-depois.txt.
