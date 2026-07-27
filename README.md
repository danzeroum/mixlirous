# Mixlirous

Motor de remixagem algorítmica de áudio guiado por IA. Descreva a intenção em
linguagem natural; o sistema traduz em parâmetros determinísticos de DSP, corta
a faixa em blocos alinhados às batidas, remonta e masteriza.

> **Status:** pré-alpha, em desenvolvimento ativo. O workspace Rust
> (`crates/`) compila, testa e passa `clippy -- -D warnings`; CI verde em
> `main` (5 workflows). O motor de DSP tem testes acústicos reais sobre
> fixtures sintéticas, e os bugs que eles encontram ficam abertos como
> issues, não escondidos — ver o [rastreador de
> issues](https://github.com/danzeroum/mixlirous/issues). A UI (`ui/`) tem
> canvas, overlay de proposta e hook de SSE já implementados, mas ainda sem
> fluxo de upload/criação de job — não há como chegar a um remix de ponta a
> ponta pela interface hoje. `docs/14-AUDITORIA-KIT.md` é o registro
> histórico da auditoria do kit original (Sprint 0), não o estado atual.

---

## Ideia central

```
"versão de 30s pra Reels, agressiva, focada nas viradas de bateria"
                            │
                            ▼
              Agente LLM (loop ReAct, budget = 5 ferramentas)
                            │
                            ▼  JSON tipado, validado no Rust
        { target_duration: 30s, block_size_beats: 4,
          compression: { ratio: 4.0, threshold_db: -14.5 },
          crossfade_ms: 1500 }
                            │
                            ▼
            Motor DSP em Rust — determinístico, testável
                            │
                            ▼
                     WAV masterizado (-14 LUFS)
```

A IA **nunca** manipula buffers de áudio. Ela preenche um contrato. Se o
contrato violar os limites, a desserialização falha e o nó é marcado com erro —
não existe caminho em que uma alucinação chegue ao arquivo final.

---

## Arquitetura em uma tela

```
┌───────────────────────────────────────────────────────────────────┐
│  APRESENTAÇÃO — React + React Flow + Zustand                      │
│  Canvas DAG · Overlay de proposta (HITL) · Painel de raciocínio    │
└──────────────┬──────────────────────────────────┬─────────────────┘
               │ REST (comandos)                  │ SSE (telemetria)
┌──────────────▼──────────────────────────────────▼─────────────────┐
│  API — Axum + Tokio                                               │
│  Auth JWT · Escopo de tenant · Sanitização de prompt · OTel        │
└──────────────┬────────────────────────────────────────────────────┘
               │
┌──────────────▼────────────────────────────────────────────────────┐
│  AGENTE — audio_agent                                             │
│  Loop ReAct (budget) → Validation Layer (serde + bounds)           │
└──────────────┬────────────────────────────────────────────────────┘
               │
┌──────────────▼────────────────────────────────────────────────────┐
│  DOMÍNIO + DSP — audio_core (Rayon)                               │
│  Onset · RMS · Chroma · Knapsack · Crossfade · LUFS                │
└──────────────┬────────────────────────────────────────────────────┘
               │
┌──────────────▼────────────────────────────────────────────────────┐
│  INFRA — SQLite (WAL) │ PostgreSQL (RLS) · disco local │ S3/MinIO  │
└───────────────────────────────────────────────────────────────────┘
```

Documentação completa: [`docs/02-ARQUITETURA.md`](docs/02-ARQUITETURA.md)

---

## Stack

| Camada | Escolha |
| --- | --- |
| Backend | Rust — Axum, Tokio (I/O), Rayon (DSP) |
| DSP | `symphonia`, `ndarray`, `realfft`, `rubato`, `ebur128` |
| Frontend | React 18, `@xyflow/react`, Zustand, Vite, TypeScript |
| Tempo real | Server-Sent Events (`EventSource` nativo) |
| Persistência | SQLite WAL (local) · PostgreSQL + RLS (produção) |
| Storage | disco local · MinIO · S3 (via `object_store`) |
| Observabilidade | OpenTelemetry → Prometheus + Tempo + Loki + Grafana |

---

## Quickstart (modo local, sem dependências externas)

Requisitos: Rust stable, Node 20+, Docker (opcional).

```bash
# Backend — SQLite embutido, storage em disco, sem Docker
cargo build --workspace
CONFIG_ENV=local cargo run --bin audio_api
# → http://localhost:8080

# Frontend
cd ui && npm install && npm run dev
# → http://localhost:5173
```

### Modo completo (Postgres + MinIO + observabilidade)

Três composes, escolhidos pela necessidade (`docs/11-INFRA-DEPLOY.md` §3):

```bash
# API + Postgres + MinIO
docker compose up -d

# Some observabilidade (Grafana + Tempo + Loki + Prometheus) por cima
docker compose -f docker-compose.yml -f docker-compose.observability.yml up -d
```

Para rodar só a API sem nenhuma dependência externa via Docker, use
`docker-compose.local.yml` no lugar de `docker-compose.yml`.

| Serviço | URL |
| --- | --- |
| API | <http://localhost:8080> |
| UI (Vite) | <http://localhost:5173> |
| Grafana | <http://localhost:3001> (admin/admin) |
| MinIO Console | <http://localhost:9001> |
| Tempo (traces) | <http://localhost:3200> |

> Grafana usa a porta **3001**, não 3000, para não conflitar com o dev server.

---

## Estrutura do repositório

```
mixlirous/
├── crates/
│   ├── audio_core/         Domínio + DSP (biblioteca, zero I/O de rede)
│   │   ├── domain/         BeatBlock, EnergyProfile, PipelineConfig, Fingerprint
│   │   ├── dsp/            analysis · stitching · mastering
│   │   ├── ports/          Traits: AudioRepo, AudioAnalyzer, AudioMixer, Storage
│   │   ├── examples/       fatia_vertical — pipeline ponta a ponta sobre fixture sintética
│   │   └── tests/          Testes acústicos e o harness dirigido pelo manifesto de fixtures
│   ├── audio_agent/        Loop ReAct, registry de ferramentas, Validation Layer
│   └── audio_api/          Axum: rotas, SSE, middleware, config
├── ui/                     React Flow canvas, overlay de proposta, hook de SSE — sem fluxo de upload/job ainda
├── prompts/                Prompts versionados como código (.prompt)
├── config/                 default.yaml · local.yaml · production.yaml
├── fixtures/               Áudio sintético gerado por scripts/generate_fixtures.py (gitignored, não comitado)
├── scripts/                Geração de fixtures, lint de prompt, checagem de links de doc
├── backlog/                issues.csv + import-issues.sh (usados uma vez, no handoff inicial)
├── observability/          Config de Grafana/Tempo/Loki/Prometheus
├── .github/workflows/      ci-rust, ci-frontend, docker-build, docs-check, prompt-lint
└── docs/                   Arquitetura, contratos, design brief, roadmap
```

---

## Documentação

| Documento | Para quem |
| --- | --- |
| [Visão e escopo](docs/00-VISAO-ESCOPO.md) | Todos |
| [Glossário](docs/01-GLOSSARIO.md) | Todos |
| [Arquitetura](docs/02-ARQUITETURA.md) | Dev |
| [**Contratos de API + SSE**](docs/03-CONTRATOS-API.md) | Dev BE + FE |
| [Adendo R2 aos contratos](docs/03-ADENDO-R2-CONTRATOS.md) | Dev BE + FE |
| [Domínio e DSP](docs/04-DOMINIO-DSP.md) | Dev BE |
| [Agente IA e HITL](docs/05-AGENTE-IA-HITL.md) | Dev BE + FE |
| [Persistência e resiliência](docs/06-PERSISTENCIA-RESILIENCIA.md) | Dev BE |
| [Observabilidade](docs/07-OBSERVABILIDADE.md) | Dev BE |
| [Segurança e multi-tenancy](docs/08-SEGURANCA-MULTITENANCY.md) | Dev BE |
| [MLOps e regressão acústica](docs/09-MLOPS-GOLDEN-MASTER.md) | Dev BE |
| [Testes e qualidade](docs/10-TESTES-QUALIDADE.md) | Dev |
| [Infra e deploy](docs/11-INFRA-DEPLOY.md) | Dev BE |
| [**Design brief**](docs/12-DESIGN-BRIEF.md) | Design + FE |
| [Roadmap e sprints](docs/13-ROADMAP-SPRINTS.md) | Todos |
| [Auditoria do kit](docs/14-AUDITORIA-KIT.md) — histórico, Sprint 0 | Dev |
| [Correções de DSP](docs/16-CORRECOES-DSP.md) | Dev BE |
| [**Guia de testes**](docs/17-GUIA-DE-TESTES.md) | Dev |
| [Testes acústicos](docs/17.1-TESTES-ACUSTICOS.md) | Dev BE |
| [ADRs](docs/adr/README.md) | Dev |

---

## Contribuindo

Ver [CONTRIBUTING.md](CONTRIBUTING.md). Resumo: branch a partir de `main`,
Conventional Commits, PR com checklist verde, `cargo clippy -- -D warnings` e
`cargo fmt --check` obrigatórios.

## Licença

MIT OR Apache-2.0, sua escolha — o mesmo padrão dual que a maior parte das
dependências do projeto (`serde`, `ndarray`, `symphonia`, `tokio`, etc.) já
usa. Ver [`LICENSE-MIT`](LICENSE-MIT) e [`LICENSE-APACHE`](LICENSE-APACHE).
