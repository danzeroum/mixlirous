# 14 — Auditoria do Kit Existente

> Estado real do `remix-ai-kit.zip` (94 arquivos), verificado arquivo por
> arquivo. **Nem o Rust nem o frontend compilam hoje.** Isso é normal para um
> esqueleto gerado — mas precisa estar explícito antes de alguém tentar
> construir por cima.
>
> Todas as correções abaixo compõem a **Sprint 0**.

---

## 1. Panorama

| Área | Estado | Aproveitável |
| --- | --- | --- |
| Estrutura de diretórios | ✅ Boa | Manter como está |
| Modelo de domínio (`domain/`) | ✅ Bom | Manter, estender |
| `beat_tracking.rs` | 🟡 Melhor peça do kit | Corrigir imports, evoluir |
| `dsp/stitching/*` | 🟡 Esqueleto razoável | Revisar e testar |
| `dsp/mastering/*` | 🟡 Parcial | Falta compressor |
| `fingerprint.rs` | 🔴 MFCC é `vec![0.0; 13]` | Reimplementar |
| `ports/*` | 🟡 Traits certos | Ajustar assinaturas |
| `audio_agent/*` | 🔴 Tudo `unimplemented!()` | Reescrever a lógica |
| `audio_api/*` | 🔴 Não compila | Refazer com contrato de `03` |
| `ui/*` | 🔴 Não compila | Refazer com base no design |
| `docker-compose.yml` | 🔴 Referencia arquivos ausentes | Corrigir |
| CI | 🔴 Quebra na matriz de SO | Corrigir |
| `prompts/*` | ✅ Bons | Manter |
| `config/*` | 🟡 Segredo hardcoded | Corrigir |

**Conclusão:** o valor do kit está na **estrutura e no domínio**, não no código
executável. Estimativa da Sprint 0: 3–5 dias.

---

## 2. Erros de compilação — Rust

### 2.1 Erros diretos

| # | Arquivo | Problema | Correção |
| --- | --- | --- | --- |
| C1 | `audio_core/src/main.rs` | String não fechada: `"...use from audio_api.` termina com crase | Fechar a string. Melhor: **remover o `main.rs`** — é crate de biblioteca |
| C2 | `audio_agent/src/main.rs` | Mesmo caso (aqui a string está ok, mas o `main` é desnecessário) | Remover |
| C3 | `audio_api/src/routes/prompts.rs:32` | Caractere solto `n` antes de `Err(...)` | Remover |
| C4 | `audio_api/src/routes/prompts.rs` | `Path` usado sem importar | `use axum::extract::Path;` |
| C5 | `audio_api/src/routes/prompts.rs` | `PromptSpec` retornado em `Json<>` sem `Serialize` | Derivar `Serialize` |
| C6 | `audio_api/src/routes/jobs.rs` | `StatusCode` usado sem importar | `use axum::http::StatusCode;` |
| C7 | `audio_api/src/routes/jobs.rs` | `JobRecord` em `Json<>` sem `Serialize` | Derivar |
| C8 | `audio_api/src/routes/jobs.rs` | `list_jobs` recebe `Path<Uuid>` mas a rota `/jobs` não tem parâmetro | `tenant_id` vem do `TenantContext`, não do path |
| C9 | `audio_api/src/routes/tenants.rs` | `use axum::{Json, Path, Extension}` — `Path` não está em `axum::` | `use axum::extract::Path;` |
| C10 | `audio_api/src/routes/sse.rs` | `Sse::new(rx)` com `mpsc::Receiver<String>` | `Sse` exige `Stream<Item = Result<Event, E>>` → `ReceiverStream` + `map` |
| C11 | `audio_api/src/routes/sse.rs` | `Arc<dyn audio_agent::ReActOrchestrator>` — é `struct`, não `trait` | `Arc<ReActOrchestrator>` |
| C12 | `audio_api/src/middleware/auth.rs` | `http::request::Parts` sem `use`; `headers` e `jsonwebtoken` ausentes do `Cargo.toml` | Importar e adicionar deps |
| C13 | `audio_agent/src/validator.rs` | `AudioToolDef::Eq` **não existe** (a variante é `DynamicEq`) | Corrigir para `DynamicEq` |
| C14 | `audio_agent/src/validator.rs` | `params.gain_db` não existe em `DynamicEqParams` (o campo está em `bands[]`) | Iterar sobre as bandas |
| C15 | `audio_core/src/domain/block.rs` | Macro `s![]` sem `use ndarray::s;` | Importar |
| C16 | `audio_core/src/domain/block.rs` | `RealFftPlanner` sem `use`; API usada incorretamente (`process` exige buffer de saída complexo) | Reescrever o cálculo de centroide |
| C17 | `audio_core/src/domain/block.rs` | `calculate_rms(&Array1)` recebendo `ArrayView1` do `slice` | Aceitar `&[f32]` ou `ArrayView1` |
| C18 | `audio_core/src/dsp/analysis/beat_tracking.rs` | Macro `s![]` sem import | Importar |
| C19 | `beat_tracking.rs` + `analyzer_trait.rs` | `BeatDetectionParams` não é exportado em `domain/mod.rs` | Adicionar ao `pub use` |
| C20 | `audio_core/Cargo.toml` | Features `sqlite`/`postgres` referenciam `deadpool-postgres` e `tokio-postgres`, que **não são dependências desta crate** | Remover as features daqui (persistência não pertence ao core) |
| C21 | `Cargo.toml` (workspace) | `[workspace.dependencies]` declarado mas nenhuma crate usa `workspace = true` | Usar herança de workspace |

### 2.2 Dependências

Trate **toda** a lista de versões como não verificada. Suspeitas concretas:

| Dependência | Problema | Ação |
| --- | --- | --- |
| `symphonia-decoder = "0.4"` | **Crate não existe.** Decoders são features do `symphonia` | Usar `symphonia = { version = "…", features = ["mp3","wav","flac","aac","isomp4"] }` |
| `symphonia = "0.4"` | Versão antiga | Verificar a atual em crates.io |
| `realfft = "0.3"` | Muito antiga; a API mudou bastante | Atualizar e ajustar as chamadas |
| `opentelemetry = "0.12"` + `tracing-opentelemetry = "0.12"` | **Pareamento inválido.** As duas famílias têm numeração independente | Ver `07-OBSERVABILIDADE.md` §2 |
| `tokio-postgres = "0.8"` | Versão provavelmente inexistente (a linha é 0.7.x) | Verificar |
| `prometheus-exporter = "0.13"` | Nome/versão duvidosos | Preferir `metrics` + `metrics-exporter-prometheus` |
| `minio = "0.10"` **e** `aws-sdk-s3 = "1.0"` | Duas SDKs pesadas para o mesmo trabalho | Substituir por `object_store` (ADR-0006) |
| `rubato`, `ebur128`, `ndarray` | Verificar versões atuais | Pinar |
| `clap` em `audio_core` | Biblioteca não deve depender de CLI | Remover |

**Procedimento da Sprint 0:** `cargo add` de cada dependência (que sempre pega a
versão atual), depois `cargo build`, corrigindo as APIs que mudaram. Não tentar
adivinhar versões manualmente.

### 2.3 Estrutura

| # | Problema | Correção |
| --- | --- | --- |
| S1 | `tests/golden_master_tests.rs` está na raiz, mas a raiz do workspace não tem `[package]` — o Cargo não compila esse diretório | Mover para `crates/audio_core/tests/` |
| S2 | `include_bytes!("fixtures/golden_master_v1.wav")` aponta para arquivo inexistente | Criar fixtures ou usar `#[ignore]` até existirem |
| S3 | `audio_core/src/lib.rs` faz `pub use domain::*; pub use dsp::*; pub use ports::*;` — colisões e API pública imprevisível | Reexportar seletivamente |
| S4 | `mixer_trait.rs` define `pub enum Error` do core dentro de um arquivo de trait de mixer | Mover para `error.rs` |
| S5 | `ports/repo_trait_sqlite.rs` coloca implementação junto do trait | Mover para `adapters/` (na crate de infra, não no core) |
| S6 | `Box<dyn std::error::Error>` nos retornos do `AudioRepo` — impede `Send + Sync` e casamento de erro | Usar `thiserror` com enum próprio |

---

## 3. Frontend (`ui/`)

| # | Problema | Correção |
| --- | --- | --- |
| F1 | Dependência `react-flow` — **o pacote correto é `reactflow` (v11) ou `@xyflow/react` (v12)** | Usar `@xyflow/react` |
| F2 | `graphStore.ts` faz `export default useStore`, mas `RemixCanvas` importa `{ useStore }` | Padronizar (recomendo export nomeado) |
| F3 | `useParamStream.ts` usa `useState` sem importar | Importar |
| F4 | Classes Tailwind em toda a UI, mas **Tailwind não está instalado nem configurado** | Instalar e configurar, ou trocar de abordagem |
| F5 | Não existe `index.html` — o Vite não sobe sem ele | Criar |
| F6 | Não existe `tsconfig.json` | Criar |
| F7 | `playwright` como dependência; o correto para testes é `@playwright/test` | Corrigir |
| F8 | `App.tsx` importa `useParamStream` sem usar | Remover ou usar |
| F9 | Vite na porta **3000**, mesma do Grafana no compose | Vite em 5173 (padrão) e Grafana em 3001 |
| F10 | `zustand/persist` guardando o grafo em `localStorage` | Fonte da verdade é o servidor; persistir só preferências de UI |
| F11 | `Proposal` tipado com `tool: any` | Tipar a partir de `ts-rs` |
| F12 | Nenhum tratamento de erro, carregando ou vazio | Ver `12-DESIGN-BRIEF.md` |

---

## 4. Infraestrutura e configuração

| # | Problema | Correção |
| --- | --- | --- |
| I1 | `docker-compose.yml` tem `build: .` mas **não existe Dockerfile** | Criar (ver `11-INFRA-DEPLOY.md` §3) |
| I2 | Compose monta `./migrations`, `./grafana/provisioning`, `./tempo`, `./loki`, `./agent.yaml` — **nenhum existe** | Criar ou remover os volumes |
| I3 | Grafana na porta 3000, conflitando com o dev server | Grafana → 3001 |
| I4 | Observabilidade no mesmo compose do banco — sobe 6 containers para um `cargo run` | Separar em `docker-compose.observability.yml` |
| I5 | `remix_api` roda `cargo run --release` dentro do container, com o workspace montado | Usar o binário compilado da imagem |
| I6 | `version: '3.8'` — obsoleto no Compose v2 | Remover a chave |
| I7 | `config/production.yaml` com `postgres://remix:prod@postgres:5432/...` | Trocar por `${DATABASE_URL}` |
| I8 | `config/default.yaml` com `access_key: minioadmin` versionado | Mover para `.env.example` |
| I9 | Não existe `config/local.yaml`, embora o compose use `CONFIG_ENV=local` | Criar |
| I10 | `.gitignore` não cobre `.mixlirous/`, `*.db`, `data/` | Adicionar |

---

## 5. CI

| # | Problema | Correção |
| --- | --- | --- |
| CI1 | `sudo apt-get install` roda na matriz inteira → **quebra em macOS e Windows** | `if: runner.os == 'Linux'` |
| CI2 | `cargo run --bin prompt_linter` — não existe esse bin; o linter é Python | Chamar `python scripts/prompt_linter.py` ou portar para Rust |
| CI3 | `actions-rs/toolchain` está sem manutenção | Usar `dtolnay/rust-toolchain@stable` |
| CI4 | Sem cache de `cargo` | `Swatinem/rust-cache@v2` |
| CI5 | Sem `clippy` nem `fmt` | Adicionar como estágio bloqueante |
| CI6 | Sem job de frontend | Adicionar lint, build e testes de UI |
| CI7 | Testes rodam `--release` (compilação lenta) | `--release` só para benchmarks |

---

## 6. Aproveitamento de código

| Arquivo | Veredito |
| --- | --- |
| `domain/block.rs` | **Aproveitar** — modelo bom; corrigir imports e FFT |
| `domain/pipeline_config.rs` | **Aproveitar** — boa base; trocar campos por `Parameter<T>` |
| `domain/beat.rs` | **Aproveitar** |
| `dsp/analysis/beat_tracking.rs` | **Aproveitar** — a melhor peça; evoluir para flux espectral |
| `dsp/analysis/rms.rs` | **Aproveitar** |
| `dsp/stitching/*` | **Aproveitar com revisão** — precisa dos invariantes de teste |
| `dsp/mastering/lufs.rs`, `limiter.rs`, `stretch.rs` | **Aproveitar com revisão** |
| `domain/fingerprint.rs` | **Estrutura sim, implementação não** — MFCC é placeholder e a distância mistura escalas |
| `ports/*.rs` | **Aproveitar as assinaturas**, corrigir tipos de erro |
| `agent/tools.rs` | **Aproveitar** — registry bem modelado |
| `agent/validator.rs` | **Reescrever** — 2 erros de compilação e limites incompletos |
| `agent/react_kernel.rs` | **Reescrever** — esqueleto correto, tudo `unimplemented!()` |
| `agent/prompt_loader.rs` | **Aproveitar**, adicionar templating |
| `api/*` | **Reescrever** conforme `03-CONTRATOS-API.md` |
| `ui/*` | **Reescrever** conforme o design |
| `prompts/*` | **Aproveitar** |
| `scripts/prompt_linter.py` | **Aproveitar**, completar as verificações |

---

## 7. Divergências entre documentos (já resolvidas)

Ao consolidar o material, apareceram conflitos. Ficam registradas as decisões:

| Tema | Conflito | Decisão |
| --- | --- | --- |
| Limite de crossfade | 3000 ms (config e validator) vs 5000 ms (docs) | **3000 ms** — `05` §3 é canônico |
| Limite de compressão | ratio ≤ 10 (validator) vs ≤ 6:1 (prompt) | Tipo permite até 10; o prompt aperta para 6 na receita |
| Threshold | −60..0 (validator) vs −40..0 (docs) | **−60..0** |
| Broker de mensagem | RabbitMQ desde o início vs fila no banco | **Fila no banco** no MVP (ADR-0005) |
| Storage | `minio` + `aws-sdk-s3` vs abstração única | **`object_store`** (ADR-0006) |
| Imagem Docker | musl + distroless vs debian-slim | **debian-slim** no MVP (ADR-0007) |
| Porta do Grafana | 3000 (conflita com Vite) | **3001** |
| Porta do dev server | 3000 no `vite.config.ts` | **5173** (padrão do Vite) |
| Sandbox seccomp | no código Rust vs `securityContext` | **`securityContext`** quando houver K8s (ADR-0008) |

---

## 8. Ordem de execução sugerida da Sprint 0

```
dia 1  C1–C21: fazer o Rust compilar (comece por audio_core, depois agent, depois api)
dia 2  Dependências: cargo add de tudo, ajustar APIs que mudaram
dia 3  F1–F12: fazer o frontend subir com uma tela mínima
dia 4  I1–I10 e CI1–CI7: infra e pipeline verdes
dia 5  Setup do GitHub, kickoff, leitura conjunta dos contratos
```

**Aceite da Sprint 0:**

```bash
cargo build --workspace          # sem erro
cargo clippy -- -D warnings      # sem warning
cargo test --workspace           # verde (mesmo com poucos testes)
cd ui && npm ci && npm run build # sem erro
docker compose up -d             # sem conflito de porta
curl localhost:8080/healthz      # {"status":"ok"}
```
