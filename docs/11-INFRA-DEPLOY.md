# 11 — Infraestrutura, Empacotamento e Deploy

## 1. Três modos de execução, um binário

| Modo | Público | Banco | Storage | Workers |
| --- | --- | --- | --- | --- |
| **Local** (padrão) | músico solo | SQLite WAL | disco | threads no processo |
| **VPS** | banda | Postgres em Docker | MinIO ou disco | containers via Docker API |
| **SaaS** (futuro) | multi-tenant | Postgres gerenciado | S3 | pods com KEDA |

A mudança entre modos é **configuração**, não build. O mesmo binário atende os
três. Isso é o que impede o projeto de bifurcar em duas bases de código.

---

## 2. Modo local — a experiência que define o produto

Meta: da instalação ao primeiro render em menos de 10 minutos, sem terminal
além de um comando.

```bash
./mixlirous
```

O que acontece no primeiro boot:

```
1. cria ~/.mixlirous/ (config.toml, data.db, storage/, session.json)
2. roda migrações
3. detecta núcleos → sugere número de workers
4. detecta se Docker existe (habilita o botão de escala)
5. detecta se Ollama está rodando em :11434 (habilita LLM local)
6. roda o recovery loop
7. sobe API em :8080 e serve a UI compilada na mesma porta
8. abre o navegador
```

Frontend embutido no binário via `rust-embed` — sem `npm install` para o usuário
final. Em desenvolvimento, o Vite roda separado com proxy para `:8080`.

### Distribuição

| Plataforma | Formato |
| --- | --- |
| macOS (Apple Silicon e Intel) | `.dmg` assinado e notarizado |
| Windows | `.msi` |
| Linux | `.AppImage` + `.deb` |
| Qualquer | `docker run` |

Builds cruzados no CI com `cargo-dist`. Assinatura de código é chato mas
necessário: um `.app` não assinado no macOS assusta o usuário e mata a adoção.

---

## 3. Docker

### Imagem do backend (multi-stage)

```dockerfile
FROM rust:1.83 AS builder
WORKDIR /app
COPY . .
RUN cargo build --release --bin audio_api

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/audio_api /usr/local/bin/mixlirous
RUN useradd -u 65532 -m app
USER 65532
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/mixlirous"]
```

> **Nota sobre `musl` + distroless:** documentos anteriores propunham
> `x86_64-unknown-linux-musl` com imagem distroless. É defensável, mas o
> alocador padrão do musl tem desempenho notoriamente ruim em cargas
> multithread com alocação intensa — exatamente o perfil de DSP com Rayon. Se
> for por esse caminho, use `jemalloc` ou `mimalloc` explicitamente. Para o
> MVP, `debian-slim` com glibc é mais simples e mais rápido; distroless entra
> quando houver requisito de segurança que justifique. Ver ADR-0007.

### Compose de desenvolvimento

Três arquivos, escolhidos pela necessidade:

| Arquivo | Sobe |
| --- | --- |
| `docker-compose.local.yml` | nada além do necessário (só a API) |
| `docker-compose.yml` | Postgres + MinIO + API |
| `docker-compose.observability.yml` | Grafana + Tempo + Loki + Prometheus |

**Correção obrigatória do kit:** o Grafana está na porta 3000, que conflita com
o dev server. Grafana passa para **3001**.

---

## 4. Botão de escala local

O usuário percebe que uma faixa de cada vez está lento. Ele aumenta o número de
processadores num slider.

```
┌──────────────────────────────────────────────┐
│  Processadores                               │
│                                              │
│  ●━━━━━━━━━━━○───────────────  4 de 7        │
│                                              │
│  Sua máquina tem 8 núcleos. Cada processador  │
│  usa cerca de 1 núcleo.                      │
│                                              │
│  ○ Piloto automático                         │
└──────────────────────────────────────────────┘
```

### Implementação

`POST /api/v1/system/scale { workers: 6 }` → crate **`bollard`** conversa com o
socket do Docker e sobe containers da própria imagem com `ROLE=worker`.

Requisitos:

- Container principal precisa do socket montado:
  `-v /var/run/docker.sock:/var/run/docker.sock`
- Sem Docker disponível → o slider ajusta **threads do pool interno**, não
  containers. Funciona no laptop sem Docker, com teto menor.
- Limites: `MIN_WORKERS = 1`, `MAX_WORKERS = num_cpus − 1`
- Cada worker novo recebe `DATABASE_URL` e credenciais de storage por env
- Containers nomeados `mixlirous-worker-{uuid}` e rotulados
  `app=mixlirous,role=worker` — para o recovery encontrar órfãos no boot

### Piloto automático (adiável)

```
a cada 10 s:
  se cpu > 75% e fila > workers × 3   → sobe (limitado a MAX)
  se cpu < 35% e fila < workers       → desce (limitado a MIN)
  cooldown de 5 min entre ações
  só age se |alvo − atual| >= 2       (anti-oscilação)
```

Toda ação vira `audit_event` e aparece no histórico da UI. Automação invisível
gera desconfiança; automação narrada gera confiança.

---

## 5. VPS

Alvo: 2–4 vCPU, 8 GB RAM, 100 GB SSD.

```bash
git clone https://github.com/danzeroum/mixlirous.git && cd mixlirous
cp .env.example .env && $EDITOR .env      # senhas e chave do LLM
docker compose up -d
```

TLS e roteamento por domínio ficam num nginx compartilhado que já roda no VPS,
fora deste repositório — não Caddy nem Traefik, como esta seção dizia antes.
Runbook de publicação: [`18-DEPLOY-PUBLICO-NGINX.md`](18-DEPLOY-PUBLICO-NGINX.md).
Backup diário do Postgres (`pg_dump`) e do diretório de storage.

A UI precisa avisar quando o disco passar de 80% — em VPS pequena, WAV enche
disco rápido e "disco cheio" no meio de um lote é o pior modo de falhar.

---

## 6. SaaS (desenho, não implementação)

Quando houver demanda:

```
Cloudflare → ALB → API (2+ réplicas, stateless)
                     ├── RDS Postgres (Multi-AZ)
                     ├── S3 + CloudFront
                     └── Workers (spot instances, KEDA por profundidade de fila)
```

Ordem de introdução, conforme o gargalo aparece:

1. Réplicas de API atrás de load balancer (stateless, exceto o hub SSE)
2. Postgres gerenciado
3. S3 no lugar de MinIO
4. Workers em Auto Scaling / KEDA
5. Broker de mensagem — **só** quando a fila em Postgres saturar

> **Cuidado com SSE atrás de load balancer:** o hub de broadcast é in-memory por
> processo. Com N réplicas, o cliente pode conectar em uma réplica que não é a
> dona do job. Soluções: sticky session por `job_id`, ou Redis pub/sub como
> backplane. Decidir antes da segunda réplica, não depois.

---

## 7. CI/CD

```
push / PR
  ├─ lint + testes (ver 10-TESTES-QUALIDADE §8)
  ├─ build de imagem Docker (só em main)
  ├─ build de binários (macOS, Windows, Linux) via cargo-dist
  └─ tag v* → release no GitHub com os artefatos anexados
```

Deploy em VPS: `docker compose pull && docker compose up -d` via SSH ou webhook.
GitOps (ArgoCD) só faz sentido quando existir cluster — não antes.

---

## 8. Configuração

Precedência: **flags de CLI > variáveis de ambiente > arquivo YAML > padrões**.

```
config/
├── default.yaml       padrões seguros, versionado
├── local.yaml         modo laptop (SQLite, disco, sem rate limit)
└── production.yaml    modo VPS/SaaS — SEM segredos, só ${VAR}
```

```yaml
# production.yaml — correto
database:
  url: "${DATABASE_URL}"
llm:
  api_key: "${LLM_API_KEY}"
```

O `production.yaml` do kit tem `postgres://remix:prod@postgres:5432/...`
hardcoded. Corrigir na Sprint 0.

### Variáveis principais

| Variável | Padrão | Efeito |
| --- | --- | --- |
| `CONFIG_ENV` | `local` | Qual YAML carregar |
| `DATABASE_URL` | — | Definida = Postgres; ausente = SQLite |
| `STORAGE_TYPE` | `local` | `local` \| `minio` \| `s3` |
| `LLM_PROVIDER` | `ollama` | `openai` \| `anthropic` \| `ollama` |
| `LLM_API_KEY` | — | Obrigatória para provedor externo |
| `MIXLIROUS_WORKERS` | `cpus−1` | Workers iniciais |
| `MIXLIROUS_DATA_DIR` | `~/.mixlirous` | Raiz de dados |
| `RUST_LOG` | `mixlirous=info` | Verbosidade |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | — | Definida = exporta traces |

## 9. Backup e recuperação de desastre

| Ativo | Estratégia | RPO |
| --- | --- | --- |
| Banco | `pg_dump` diário (ou cópia do `.db` no SQLite) | 24 h |
| Áudio original | Já é do usuário; versionamento no bucket | 0 |
| Áudio processado | Regenerável a partir da receita | ∞ |
| Prompts e Golden Masters | Git | 0 |

O ponto importante: **o áudio processado é descartável**. Com a faixa original,
a receita e as versões congeladas, qualquer render é reproduzível. Isso reduz o
custo de backup em ordens de grandeza e é uma consequência direta do version
freeze.
