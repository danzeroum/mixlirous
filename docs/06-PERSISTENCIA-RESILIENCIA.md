# 06 — Persistência, Fila e Resiliência

## 1. Estratégia dual

Um trait, dois adapters, escolha em runtime por variável de ambiente.

```
DATABASE_URL definida?
  ├─ não  → SQLite em ~/.mixlirous/data.db (modo WAL)   [padrão]
  └─ sim  → PostgreSQL com RLS
```

Nenhuma linha de domínio sabe qual está ativo.

| | SQLite (WAL) | PostgreSQL |
| --- | --- | --- |
| Público | laptop, VPS pequena, CI | multiusuário, SaaS |
| Setup | nenhum | `docker compose up postgres` |
| Escritores simultâneos | 1 (WAL permite leituras paralelas) | N |
| Fila | transação `IMMEDIATE` | `FOR UPDATE SKIP LOCKED` |
| Isolamento de tenant | filtro no adapter | RLS no banco |
| Séries temporais | tabela comum + índice | TimescaleDB (opcional) |
| Limite prático | ~10 GB / ~50k jobs | horizontal |

### PRAGMAs obrigatórios no SQLite

```sql
PRAGMA journal_mode = WAL;        -- leitura concorrente com escrita
PRAGMA synchronous = NORMAL;      -- compromisso velocidade/durabilidade
PRAGMA foreign_keys = ON;         -- desligado por padrão no SQLite (!)
PRAGMA busy_timeout = 5000;       -- espera 5 s em vez de falhar na hora
PRAGMA cache_size = -16000;       -- 16 MB
```

Sem `WAL`, a API gravando um job e o worker lendo a fila geram `SQLITE_BUSY`
imediato. Sem `foreign_keys = ON`, as FKs do schema são decorativas.

---

## 2. Schema

SQL escrito para funcionar nos dois bancos com o mínimo de divergência. Onde
divergem, há dois arquivos: `migrations/sqlite/` e `migrations/postgres/`.

```sql
-- ─────────────────────────── tenancy ───────────────────────────
CREATE TABLE tenants (
    id           TEXT PRIMARY KEY,          -- UUID  (PG: UUID)
    name         TEXT NOT NULL,
    plan         TEXT NOT NULL DEFAULT 'free',
    created_at   TEXT NOT NULL              -- PG: TIMESTAMPTZ
);

CREATE TABLE users (
    id           TEXT PRIMARY KEY,
    tenant_id    TEXT NOT NULL REFERENCES tenants(id),
    email        TEXT NOT NULL,
    role         TEXT NOT NULL DEFAULT 'member',
    created_at   TEXT NOT NULL,
    UNIQUE (tenant_id, email)
);

CREATE TABLE projects (
    id           TEXT PRIMARY KEY,
    tenant_id    TEXT NOT NULL REFERENCES tenants(id),
    name         TEXT NOT NULL,
    created_at   TEXT NOT NULL
);

-- ─────────────────────────── áudio ─────────────────────────────
CREATE TABLE tracks (
    id             TEXT PRIMARY KEY,
    tenant_id      TEXT NOT NULL REFERENCES tenants(id),
    project_id     TEXT REFERENCES projects(id),
    object_key     TEXT NOT NULL,
    display_name   TEXT NOT NULL,
    status         TEXT NOT NULL,           -- uploaded|analyzing|ready|failed
    duration_sec   REAL,
    sample_rate    INTEGER,
    channels       INTEGER,
    sha256         TEXT,
    analysis       TEXT,                    -- JSON (PG: JSONB)
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL
);
CREATE INDEX idx_tracks_tenant   ON tracks(tenant_id, created_at DESC);
CREATE INDEX idx_tracks_status   ON tracks(status);

-- ─────────────────────────── jobs / fila ───────────────────────
CREATE TABLE jobs (
    id                TEXT PRIMARY KEY,
    tenant_id         TEXT NOT NULL REFERENCES tenants(id),
    user_id           TEXT REFERENCES users(id),
    track_id          TEXT NOT NULL REFERENCES tracks(id),
    status            TEXT NOT NULL,        -- ver máquina de estados
    mode              TEXT NOT NULL,        -- manual | assisted
    priority          INTEGER NOT NULL DEFAULT 0,
    attempt           INTEGER NOT NULL DEFAULT 0,
    max_attempts      INTEGER NOT NULL DEFAULT 3,

    user_prompt       TEXT,
    prompt_id         TEXT,
    prompt_version    TEXT,
    llm_model         TEXT,
    version_frozen    INTEGER NOT NULL DEFAULT 0,   -- PG: BOOLEAN

    graph             TEXT NOT NULL,        -- JSON
    pipeline_config   TEXT NOT NULL,        -- JSON
    agent_run         TEXT,                 -- JSON (thoughts, tool calls)

    progress_pct      INTEGER NOT NULL DEFAULT 0,
    stage             TEXT,

    worker_id         TEXT,
    started_at        TEXT,
    heartbeat_at      TEXT,                 -- detecção de worker morto
    completed_at      TEXT,

    artifact_key      TEXT,
    artifact_sha256   TEXT,
    artifact_bytes    INTEGER,
    fingerprint       TEXT,                 -- JSON
    similarity_score  REAL,

    error_code        TEXT,
    error_detail      TEXT,
    warnings          TEXT,                 -- JSON array

    trace_id          TEXT,
    idempotency_key   TEXT,
    created_at        TEXT NOT NULL,
    updated_at        TEXT NOT NULL,
    UNIQUE (tenant_id, idempotency_key)
);
CREATE INDEX idx_jobs_queue   ON jobs(status, priority DESC, created_at ASC);
CREATE INDEX idx_jobs_tenant  ON jobs(tenant_id, created_at DESC);
CREATE INDEX idx_jobs_stalled ON jobs(status, heartbeat_at);

-- ─────────────────────────── grafo / nós ───────────────────────
CREATE TABLE nodes (
    id           TEXT PRIMARY KEY,
    job_id       TEXT NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    tenant_id    TEXT NOT NULL,
    node_key     TEXT NOT NULL,             -- "n4" no grafo do cliente
    type         TEXT NOT NULL,
    tool         TEXT,
    status       TEXT NOT NULL,             -- idle|proposed|queued|running|completed|failed|rejected
    parameters   TEXT NOT NULL,             -- JSON com envelope {value, source}
    position     TEXT NOT NULL,             -- JSON {x, y}
    error        TEXT,
    updated_at   TEXT NOT NULL,
    UNIQUE (job_id, node_key)
);

-- ─────────────────────────── propostas (HITL) ──────────────────
CREATE TABLE proposals (
    id                    TEXT PRIMARY KEY,
    job_id                TEXT NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    tenant_id             TEXT NOT NULL,
    tool                  TEXT NOT NULL,
    reason                TEXT NOT NULL,
    parameters_suggestion TEXT NOT NULL,    -- JSON
    position_hint         TEXT,             -- JSON
    status                TEXT NOT NULL,    -- pending|approved|rejected|expired
    decided_by            TEXT,
    decided_reason        TEXT,
    created_node_id       TEXT REFERENCES nodes(id),
    expires_at            TEXT NOT NULL,
    created_at            TEXT NOT NULL,
    decided_at            TEXT
);
CREATE INDEX idx_proposals_pending ON proposals(status, expires_at);

-- ─────────────────────────── eventos / auditoria ───────────────
CREATE TABLE job_events (              -- buffer de replay do SSE
    id         INTEGER PRIMARY KEY AUTOINCREMENT,   -- PG: BIGSERIAL
    job_id     TEXT NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    seq        INTEGER NOT NULL,
    event      TEXT NOT NULL,
    payload    TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE (job_id, seq)
);

CREATE TABLE audit_events (            -- imutável, nunca deletado
    id            TEXT PRIMARY KEY,
    tenant_id     TEXT NOT NULL,
    user_id       TEXT,
    actor_type    TEXT NOT NULL,       -- USER | LLM | SYSTEM
    actor_detail  TEXT,                -- JSON: modelo, versão do prompt, worker
    action        TEXT NOT NULL,
    resource_type TEXT,
    resource_id   TEXT,
    before        TEXT,                -- JSON
    after         TEXT,                -- JSON
    metadata      TEXT,                -- JSON: ip, user agent, trace_id
    occurred_at   TEXT NOT NULL
);
CREATE INDEX idx_audit_tenant ON audit_events(tenant_id, occurred_at DESC);

-- ─────────────────────────── MLOps ─────────────────────────────
CREATE TABLE golden_masters (
    id            TEXT PRIMARY KEY,
    label         TEXT NOT NULL,       -- "bossa_nova × tiktok_aggressive_v2"
    prompt_id     TEXT NOT NULL,
    llm_model     TEXT NOT NULL,
    fixture_key   TEXT NOT NULL,
    artifact_key  TEXT NOT NULL,
    fingerprint   TEXT NOT NULL,       -- JSON
    created_at    TEXT NOT NULL
);

CREATE TABLE feature_flags (
    tenant_id  TEXT NOT NULL,
    key        TEXT NOT NULL,
    value      TEXT NOT NULL,          -- JSON
    updated_at TEXT NOT NULL,
    PRIMARY KEY (tenant_id, key)
);
```

### Ações obrigatórias que **sempre** geram `audit_event`

`PROMPT_SUBMITTED` · `TOOL_CALL_ATTEMPT` · `TOOL_CALL_DENIED` ·
`PARAM_OVERRIDE` · `PROPOSAL_CREATED` · `PROPOSAL_DECIDED` · `JOB_STARTED` ·
`JOB_COMPLETED` · `JOB_FAILED` · `WORKER_SCALE_ACTION` · `RECOVERY_ACTION` ·
`VERSION_FREEZE_CHANGED` · `MALICIOUS_PROMPT_BLOCKED`

---

## 3. Fila no banco (sem broker no MVP)

### PostgreSQL

```sql
UPDATE jobs
   SET status = 'running',
       worker_id = $1,
       started_at = now(),
       heartbeat_at = now(),
       attempt = attempt + 1
 WHERE id = (
   SELECT id FROM jobs
    WHERE status = 'queued'
    ORDER BY priority DESC, created_at ASC
    FOR UPDATE SKIP LOCKED
    LIMIT 1
 )
RETURNING *;
```

`SKIP LOCKED` é o que permite N workers puxarem da mesma fila sem deadlock e sem
que dois peguem o mesmo job.

### SQLite

Não tem `SKIP LOCKED`. O equivalente seguro:

```sql
BEGIN IMMEDIATE;                    -- pega o lock de escrita já na abertura
UPDATE jobs
   SET status='running', worker_id=?1, started_at=?2,
       heartbeat_at=?2, attempt=attempt+1
 WHERE id = (SELECT id FROM jobs
              WHERE status='queued'
              ORDER BY priority DESC, created_at ASC
              LIMIT 1);
COMMIT;
```

`BEGIN IMMEDIATE` (não `BEGIN`) é essencial: sem ele, dois workers leem o mesmo
`SELECT` e um recebe `SQLITE_BUSY` no `COMMIT`, o que é tratável, mas produz
retrabalho. Com `IMMEDIATE`, a serialização acontece na entrada.

### Heartbeat e detecção de worker morto

O worker atualiza `heartbeat_at` a cada 15 s durante o processamento. Uma rotina
periódica (a cada 60 s) devolve à fila:

```sql
UPDATE jobs
   SET status = 'queued', worker_id = NULL
 WHERE status = 'running'
   AND heartbeat_at < (now - 120 seconds)
   AND attempt < max_attempts;
```

Jobs que estouram `max_attempts` vão para `failed` com `error_code = 'stalled'`.

### Quando migrar para broker

Sinais concretos: fila com mais de ~50 mil linhas ativas, ou latência de
reivindicação acima de 100 ms, ou necessidade de fan-out para múltiplos
consumidores distintos. Antes disso, Postgres como fila é mais simples e mais
confiável do que operar RabbitMQ. Ver ADR-0005.

### Forma do repositório: operações compostas, não transação genérica

`AudioRepo` expõe operações nomeadas — `claim_job`, `transition_job`,
`heartbeat`, `fail_and_retry` — e cada uma é atômica por construção do lado do
adapter (uma transação SQL por baixo). **Não existe `begin_transaction()` nem
um tipo `Transaction` exposto pelo trait.** A justificativa não é estilo: sem
um segundo adapter real (SQLite) para validar contra, uma abstração genérica de
transação vira chute sobre uma API que nenhum dos dois bancos obriga a ter a
mesma forma. Adicionar uma operação nova é modelar o que ela precisa fazer
atomicamente e nomear isso — não abrir uma transação genérica no meio do
código de negócio.

---

## 4. Escrita atômica de artefato

A regra: o arquivo final **nunca** aparece no destino em estado parcial.

```rust
pub fn atomic_write(path: &Path, data: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension(format!("tmp.{}", Uuid::new_v4()));

    {
        let mut f = File::create(&tmp)?;
        f.write_all(data)?;
        f.flush()?;
        f.sync_all()?;                 // 1. dados no disco, não no cache do SO
    }

    fs::rename(&tmp, path)?;           // 2. rename é atômico no mesmo FS

    let dir = File::open(path.parent().unwrap())?;
    dir.sync_all()?;                   // 3. o próprio rename precisa persistir

    Ok(())
}
```

Os três passos são necessários. Sem o passo 1, o `rename` pode apontar para um
arquivo vazio após queda de energia. Sem o passo 3, o `rename` pode não ter sido
gravado no diretório — o arquivo "some" no boot seguinte. É exatamente o cenário
de laptop que fecha a tampa e hiberna mal.

Ordem de persistência ao concluir um job:

```
1. escreve WAV (atômico)
2. calcula SHA-256 do arquivo já em disco (não do buffer em memória)
3. UPDATE jobs SET status='completed', artifact_sha256=...
4. emite SSE job.completed
```

Se a máquina cair entre 1 e 3, o recovery encontra o arquivo válido e completa o
job. Se cair entre 2 e 3, idem. Nunca há estado em que o banco diz "completo" e
o arquivo não existe.

---

## 5. Recovery loop no boot

Roda **antes** de a API aceitar requisições. Idempotente: seguro rodar várias
vezes, inclusive se cair no meio.

```
1. ADQUIRIR LOCK
   PostgreSQL : pg_advisory_lock(hash('mixlirous_recovery'))
   SQLite     : arquivo .mixlirous/recovery.lock com PID + flock

2. LISTAR JOBS EM VOO
   SELECT * FROM jobs WHERE status IN ('running','awaiting_approval')

3. PARA CADA JOB EM 'running':
   3.1 artefato existe no storage?
       não → attempt < max ? status='queued' : status='failed'(artifact_lost)
       sim → 3.2
   3.2 SHA-256 do arquivo == artifact_sha256 registrado?
       (se o hash não foi registrado, recalcula e valida o cabeçalho WAV)
       sim → status='completed'   ← o usuário nem percebe a queda
       não → remove arquivo; attempt<max ? 'queued' : 'failed'(corrupt_artifact)

4. PARA CADA JOB EM 'awaiting_approval':
   proposta expirada → status='queued' (reprocessa; o agente replaneja)
   proposta viva     → mantém; o TTL continua correndo

5. LIMPEZA
   - propostas com expires_at vencido → status='expired'
   - arquivos *.tmp.* com mtime > 1 h → remove
   - uploads multipart órfãos > 24 h → aborta (só S3)
   - job_events de jobs terminais com mais de 7 dias → remove

6. RELATÓRIO
   - grava audit_event RECOVERY_ACTION
   - guarda o relatório em memória para o primeiro cliente SSE
     receber recovery.report

7. LIBERAR LOCK → sobe o servidor HTTP
```

O storage é a **autoridade primária**; o banco é a autoridade de transição.
Quando os dois discordam, um arquivo íntegro em disco vence um registro
desatualizado no banco.

### Cenários e resultados

| Situação no crash | Resultado após boot | O que o usuário vê |
| --- | --- | --- |
| DSP concluído, WAV íntegro, banco desatualizado | `completed` | Nada — job aparece pronto |
| DSP no meio, só `.tmp` no disco | `queued`, `.tmp` removido | "Retomando 1 trabalho" |
| WAV existe mas hash não bate | `queued`, arquivo removido | idem |
| `max_attempts` esgotado | `failed` | Nó em erro + botão "Tentar de novo" |
| Proposta pendente há 3 min | `expired` → `queued` | Overlay sumiu; agente replaneja |
| Proposta pendente há 20 s | mantida | Overlay reaparece com tempo restante |

---

## 6. Migrações

Ferramenta: **`sqlx::migrate!`** (embutido no binário) ou `refinery`. O
requisito: migração roda sozinha no boot, sem CLI externo — o usuário do laptop
não vai rodar `diesel migration run`.

```
migrations/
├── sqlite/
│   ├── 0001_initial.sql
│   └── 0002_add_heartbeat.sql
└── postgres/
    ├── 0001_initial.sql
    ├── 0002_add_heartbeat.sql
    └── 0003_rls_policies.sql
```

Regras: migrações são *forward-only*; nunca editar uma já publicada; toda
migração testada nos dois bancos no CI.

---

## 7. Migração SQLite → PostgreSQL

Comando do próprio binário, não script externo:

```bash
mixlirous migrate-db \
  --from sqlite://~/.mixlirous/data.db \
  --to postgres://user:pass@host/mixlirous
```

Faz: lê tabela por tabela, converte tipos (`TEXT` de data → `TIMESTAMPTZ`,
`TEXT` de JSON → `JSONB`, `INTEGER` de bool → `BOOLEAN`), insere em lote em
transação, valida contagem por tabela ao final. Os arquivos de storage não são
tocados — só as chaves são revalidadas.

---

## 8. Retenção e limpeza

| Dado | Retenção padrão | Config |
| --- | --- | --- |
| WAV original (`raw/`) | indefinido (é do usuário) | — |
| WAV processado | 90 dias | `retention.processed_days` |
| `job_events` (replay SSE) | 7 dias após job terminal | `retention.events_days` |
| `audit_events` | 365 dias | `retention.audit_days` |
| Jobs `failed` | 30 dias | `retention.failed_jobs_days` |
| Arquivos `.tmp` | 1 hora | fixo |

A rotina de limpeza roda a cada 6 h e no boot. No modo local, avisa antes de
remover qualquer WAV: `"Vou remover 12 renders com mais de 90 dias. Continuar?"`
— apagar áudio de alguém sem avisar é imperdoável em ferramenta criativa.
