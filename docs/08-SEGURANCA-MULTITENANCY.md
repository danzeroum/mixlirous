# 08 — Segurança e Multi-tenancy

## 1. Princípio

> `tenant_id` é imposto na fronteira (JWT) e verificado em cada camada. Nunca
> vem do cliente, nunca é inferido, nunca é omitido de uma query.

O MVP roda single-user no laptop — mas o `tenant_id` existe desde a primeira
migração. Adicionar isolamento depois exige reescrever toda camada de acesso a
dados; deixar pronto agora custa uma coluna e um middleware.

```
Browser → JWT → API Gateway → Rust → Banco (RLS)
                   │            │       │
                   └── prefixo S3 ──────┴── audit_event
```

---

## 2. Camada 1 — Autenticação

JWT assinado (RS256 em SaaS; HS256 com segredo local no modo single-user).
`tenant_id` sai exclusivamente das claims.

```rust
pub struct TenantContext {
    pub user_id: Uuid,
    pub tenant_id: Uuid,
    pub roles: Vec<Role>,
    pub plan: PlanTier,
}
```

Extraído por `FromRequestParts` e injetado em todo handler. **Um handler sem
`TenantContext` no assinatura é bloqueado em code review.**

### Modo local

No primeiro boot, o binário gera um par de chaves e um usuário/tenant padrão,
persistidos em `~/.mixlirous/`. O frontend obtém o token via
`GET /api/v1/auth/local-session` (só aceita conexão de `127.0.0.1`).

Um caminho de código só, local e SaaS. Sem `if is_local` espalhado.

---

## 3. Camada 2 — Banco de dados

### PostgreSQL: RLS

```sql
ALTER TABLE jobs ENABLE ROW LEVEL SECURITY;

CREATE POLICY jobs_tenant_isolation ON jobs
  USING      (tenant_id = current_setting('app.tenant_id', true)::uuid)
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);
```

Repetir para `tracks`, `nodes`, `proposals`, `projects`, `audit_events`,
`job_events`, `feature_flags`.

O usuário da aplicação **não** pode ser superusuário nem dono das tabelas —
ambos ignoram RLS silenciosamente. Criar `mixlirous_app` com `NOBYPASSRLS`.

Escopo por transação:

```rust
pub async fn with_tenant_scope<T, F>(pool: &PgPool, tenant_id: Uuid, f: F)
    -> Result<T> where F: FnOnce(&mut Transaction) -> BoxFuture<Result<T>>
{
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT set_config('app.tenant_id', $1, true)")  // true = local à tx
        .bind(tenant_id.to_string()).execute(&mut *tx).await?;
    let out = f(&mut tx).await?;
    tx.commit().await?;
    Ok(out)
}
```

O `true` em `set_config` limita o escopo à transação — não vaza para a próxima
requisição que reusar a conexão do pool. Errar esse parâmetro é a forma clássica
de furar RLS com pool de conexões.

### SQLite: filtro no adapter

Sem RLS. O `SqliteRepo` guarda o `tenant_id` do contexto e **toda** query inclui
`WHERE tenant_id = ?`. Para garantir, o adapter não expõe método sem tenant:

```rust
impl SqliteRepo {
    pub fn scoped(&self, tenant_id: Uuid) -> ScopedRepo<'_> { /* ... */ }
}
// Só ScopedRepo implementa AudioRepo. Não existe caminho sem escopo.
```

### Regra de resposta

Recurso de outro tenant retorna **404**, não 403. Um 403 confirma que o ID
existe — é vazamento de informação.

---

## 4. Camada 3 — Storage

Prefixo por tenant, aplicado no adapter e não montável pelo chamador:

```
tenant-{tenant_id}/project-{project_id}/{raw|processed|artifacts}/{id}.wav
```

```rust
pub struct ScopedStorage { inner: Arc<dyn Storage>, prefix: String }

impl ScopedStorage {
    fn key(&self, rel: &str) -> Result<String> {
        if rel.contains("..") || rel.starts_with('/') {
            return Err(StorageError::InvalidKey);   // path traversal
        }
        Ok(format!("{}/{}", self.prefix, rel))
    }
}
```

URLs assinadas (presigned) com validade de 15 min para upload e 60 min para
download. No modo local, o endpoint de download valida o JWT e o prefixo antes
de servir o arquivo.

---

## 5. Camada 4 — Worker de áudio

O worker processa **arquivo enviado pelo usuário**. É a superfície de ataque mais
subestimada do sistema.

### Validação de entrada

1. **Magic bytes**, não extensão nem `Content-Type`:
   `RIFF....WAVE` · `ID3`/`0xFFFB` (MP3) · `fLaC` · `OggS`.
2. Limite de tamanho antes de decodificar (`audio.max_input_mb`, padrão 500).
3. Limite de duração após ler o cabeçalho (padrão 30 min).
4. Recusar sample rate absurdo (> 192 kHz) e contagem de canais > 8.

### Resistência a pânico

O decoder é código que processa entrada hostil. Requisitos:

- `cargo-fuzz` sobre `decode_to_pcm` no CI noturno.
- Decodificação roda em `spawn_blocking` com `catch_unwind`: um pânico falha o
  job, não derruba o processo.
- Timeout duro por job (`audio.max_processing_sec`, padrão 600) contra bomba de
  descompressão.

### Sandbox (pós-MVP)

`seccomp-bpf` + `chroot` + drop de UID são valiosos quando houver upload de
terceiros. Em Kubernetes, prefira `securityContext.seccompProfile` no manifesto
a chamar `unshare()` dentro do processo — o runtime já aplica AppArmor e o
autoisolamento em código costuma colidir com ele. Ver ADR-0008.

---

## 6. Camada 5 — Prompt injection

Ver `05-AGENTE-IA-HITL.md` §7 para os detalhes. Resumo das quatro barreiras:

1. Sanitização de entrada (padrões proibidos, limite de tamanho, Unicode).
2. Registry filtrado por plano — o modelo não sabe que existe ferramenta a mais.
3. Validação de saída — ferramenta verificada contra a lista permitida.
4. Contexto escopado — dados de outro tenant nunca entram no prompt.

---

## 7. Camada 6 — Auditoria

`audit_events` é imutável: sem `UPDATE`, sem `DELETE` (revogar a permissão no
Postgres). Toda ação sensível registra ator, antes, depois e `trace_id`.

Serve para três coisas concretas: investigar incidente, responder pedido de
titular (LGPD), e provar que uma decisão foi da IA ou do usuário.

---

## 8. LGPD / privacidade

| Requisito | Implementação |
| --- | --- |
| Minimização | Só coletamos e-mail e os arquivos que o usuário sobe |
| Finalidade | Áudio é usado exclusivamente para gerar o remix pedido |
| Portabilidade | `GET /api/v1/tenants/me/export` → ZIP com dados + arquivos |
| Eliminação | `DELETE /api/v1/tenants/me` → remove dados e objetos em até 30 dias |
| Transparência | Documento de quais dados vão ao provedor LLM |
| Segurança | TLS em trânsito; criptografia em repouso no bucket |

**Ponto sensível a comunicar com clareza:** no modo `assisted` com provedor
externo, o *prompt* e os *metadados* da faixa (BPM, duração, energia) são
enviados a terceiro. O **áudio nunca é enviado**. A UI precisa dizer isso na
primeira execução, com opção de trocar para LLM local. Para a persona P3
(estúdio profissional), essa frase é a diferença entre adotar e desinstalar.

---

## 9. Gestão de segredos

| Ambiente | Onde ficam |
| --- | --- |
| Local | `~/.mixlirous/config.toml`, permissão 0600 |
| VPS | arquivo `.env` fora do repositório, permissão 0600 |
| SaaS | secret manager (SSM / Vault), injetado como env |

Nunca em `config/*.yaml` versionado. O `production.yaml` do kit já tem
`postgres://remix:prod@...` hardcoded — corrigir para `${DATABASE_URL}`.

CI: `gitleaks` no pré-commit e no pipeline.

---

## 10. Checklist de segurança por PR

- [ ] Todo handler novo recebe `TenantContext`
- [ ] Toda query nova passa por `with_tenant_scope` / `ScopedRepo`
- [ ] Todo acesso a storage passa por `ScopedStorage`
- [ ] Nenhum segredo em código, config versionada ou log
- [ ] Entrada externa validada antes de qualquer parsing pesado
- [ ] Ação sensível gera `audit_event`
- [ ] Erro não vaza detalhe interno para o cliente (stack, SQL, path)
- [ ] Recurso de outro tenant retorna 404
