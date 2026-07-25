# 03 — Contratos de API (REST + SSE)

> **Fonte da verdade compartilhada entre backend e frontend.**
> Mudança neste documento exige PR próprio, aviso no canal do time e atualização
> simultânea dos tipos em `crates/audio_api/src/routes/` e `ui/src/types/api.ts`.

---

## 1. Convenções

| Item | Regra |
| --- | --- |
| Base URL | `/api/v1` — versão no path, sem exceção |
| Formato | JSON, `Content-Type: application/json` |
| Nomes de campo | `snake_case`, sufixo de unidade obrigatório (`_ms`, `_db`, `_sec`, `_hz`) |
| Identificadores | UUID v4 em string |
| Datas | ISO 8601 com timezone (`2026-07-24T18:30:00Z`) |
| Paginação | cursor: `?limit=50&cursor=<opaco>` → resposta traz `next_cursor` |
| Autenticação | `Authorization: Bearer <JWT>` em tudo, exceto `/healthz` e `/readyz` |
| Rastreamento | cliente envia `traceparent` (W3C); servidor devolve no response |
| Idempotência | `POST` de criação aceita `Idempotency-Key: <uuid>` |
| Erros | `application/problem+json` (RFC 7807) |

### JWT — claims esperadas

```json
{
  "sub": "user_uuid",
  "tenant_id": "tenant_uuid",
  "roles": ["owner"],
  "plan": "free",
  "iat": 1753380000,
  "exp": 1753466400
}
```

`tenant_id` do token é a **única** fonte de verdade. Nenhum endpoint aceita
`tenant_id` no corpo ou na query.

> **Modo local (single-user):** o servidor emite um JWT de sessão local no
> primeiro boot e persiste em `~/.mixlirous/session.json`. O frontend em modo
> local pega esse token de `GET /api/v1/auth/local-session`. Isso mantém um único
> caminho de código para local e SaaS.

---

## 2. Envelope de parâmetro (conceito central)

Todo parâmetro que a IA pode preencher **nunca** é um valor solto:

```json
{
  "crossfade_ms": { "value": 1500, "source": "LLM_INFERRED", "confidence": 0.88 },
  "block_size_beats": { "value": 8, "source": "USER_DEFINED" }
}
```

| Campo | Tipo | Regra |
| --- | --- | --- |
| `value` | number \| string \| bool \| enum | O valor efetivo |
| `source` | `"LLM_INFERRED"` \| `"USER_DEFINED"` \| `"DEFAULT"` | Precedência |
| `confidence` | number 0–1, opcional | Só presente quando `LLM_INFERRED` |

**Regra de precedência (implementada no backend, refletida na UI):** um campo com
`source: "USER_DEFINED"` nunca é sobrescrito pelo agente, mesmo que ele rode de
novo. A UI mostra isso com um ícone de cadeado.

---

## 3. Endpoints

### 3.1 Saúde e sistema

| Método | Path | Descrição |
| --- | --- | --- |
| `GET` | `/healthz` | Liveness. `200 {"status":"ok"}`. Sem auth. |
| `GET` | `/readyz` | Readiness: banco e storage acessíveis. Sem auth. |
| `GET` | `/metrics` | Prometheus text format. Sem auth (bind interno). |
| `GET` | `/api/v1/system/info` | Versão, backend de banco, provedor LLM, nº de cores. |
| `GET` | `/api/v1/system/resources` | Estado dos workers e da fila. |
| `POST` | `/api/v1/system/scale` | Ajusta número de workers. |

**`GET /api/v1/system/info`**

```json
{
  "version": "0.1.0",
  "database_backend": "sqlite",
  "llm_provider": "deepseek",
  "llm_model": "deepseek-v4-flash",
  "data_egress": true,
  "cpu_cores": 8
}
```

`data_egress` é `true` quando o provedor ativo é externo — prompt e
metadados da faixa saem da máquina, nunca o áudio
(`08-SEGURANCA-MULTITENANCY.md` §8). `false` só para provedor local
(Ollama). É a fonte que a tela de consentimento (§3.8) lê para nomear o
provedor antes da primeira execução em modo assistido.

**`GET /api/v1/system/resources`**

```json
{
  "cpu_cores": 8,
  "cpu_usage_pct": 41.2,
  "memory_used_mb": 2840,
  "workers": { "active": 4, "max_allowed": 7, "target": 4 },
  "queue": { "queued": 3, "running": 4, "awaiting_approval": 1 },
  "autopilot": {
    "enabled": false,
    "last_action": {
      "at": "2026-07-24T18:22:11Z",
      "from": 2, "to": 4, "reason": "cpu_high"
    }
  },
  "docker_available": true
}
```

**`POST /api/v1/system/scale`**

```json
// request
{ "workers": 6 }
// ou
{ "autopilot": true }
```

```json
// 200
{ "workers": { "active": 4, "target": 6 }, "autopilot": false }
```

Erros: `409 docker_unavailable` (socket do Docker inacessível),
`422 limit_exceeded` (acima de `cpu_cores - 1`).

---

### 3.2 Upload e faixas

**`POST /api/v1/uploads/presign`**

```json
// request
{ "filename": "jam_04.wav", "size_bytes": 62914560, "content_type": "audio/wav" }
```

```json
// 200
{
  "object_key": "tenant-a7c1/project-01/raw/9f2b....wav",
  "upload_url": "http://localhost:9000/audio-pipeline/...",
  "method": "PUT",
  "headers": { "Content-Type": "audio/wav" },
  "expires_at": "2026-07-24T19:30:00Z"
}
```

No modo local com storage em disco, `upload_url` aponta para
`PUT /api/v1/uploads/{object_key}` do próprio servidor. O frontend não precisa
saber a diferença.

**`POST /api/v1/tracks`** — registra a faixa e enfileira a análise

```json
// request
{ "object_key": "tenant-a7c1/...wav", "display_name": "Jam 04 — ensaio março",
  "project_id": "uuid" }
```

```json
// 202
{ "track_id": "uuid", "status": "analyzing",
  "stream_url": "/api/v1/tracks/{track_id}/events" }
```

**`GET /api/v1/tracks/:track_id`**

```json
{
  "track_id": "uuid",
  "display_name": "Jam 04 — ensaio março",
  "status": "ready",
  "duration_sec": 743.2,
  "sample_rate": 44100,
  "channels": 2,
  "analysis": {
    "bpm": 128.4,
    "bpm_confidence": 0.91,
    "beat_count": 1587,
    "strong_beat_count": 318,
    "energy_profile": {
      "rms_mean": 0.184, "rms_std": 0.062,
      "peak_db": -0.8, "dynamic_range_db": 14.2
    },
    "sections": [
      { "label": "intro",  "start_sec": 0.0,   "end_sec": 22.4 },
      { "label": "A",      "start_sec": 22.4,  "end_sec": 96.1 },
      { "label": "chorus", "start_sec": 96.1,  "end_sec": 148.7, "repeat_of": null },
      { "label": "chorus", "start_sec": 302.5, "end_sec": 355.0, "repeat_of": 2 }
    ]
  },
  "waveform_peaks_url": "/api/v1/tracks/{id}/peaks?resolution=1024"
}
```

**`GET /api/v1/tracks/:track_id/peaks?resolution=1024`**

Array de picos min/max normalizados, para desenhar a waveform sem baixar o WAV.

```json
{ "resolution": 1024, "peaks": [[-0.42, 0.51], [-0.38, 0.47], ...] }
```

**`GET /api/v1/tracks`** — lista paginada.
**`DELETE /api/v1/tracks/:track_id`** — remove faixa e artefatos derivados.

---

### 3.3 Trabalhos (jobs)

**`POST /api/v1/jobs`** — cria e enfileira um remix

```json
{
  "track_id": "uuid",
  "mode": "assisted",
  "user_prompt": "versão de 30s pra Reels, agressiva, foco nas viradas de bateria",
  "prompt_id": "tiktok_aggressive_v2",
  "graph": {
    "nodes": [
      { "id": "n1", "type": "source",     "position": { "x": 0,   "y": 120 } },
      { "id": "n2", "type": "analysis",   "position": { "x": 200, "y": 120 } },
      { "id": "n3", "type": "agent",      "position": { "x": 400, "y": 120 } },
      { "id": "n4", "type": "processor",  "position": { "x": 600, "y": 120 },
        "tool": "crossfade",
        "parameters": {
          "duration_ms": { "value": 1200, "source": "USER_DEFINED" },
          "curve":       { "value": "logarithmic", "source": "DEFAULT" }
        } },
      { "id": "n5", "type": "mastering",  "position": { "x": 800, "y": 120 } },
      { "id": "n6", "type": "output",     "position": { "x": 1000,"y": 120 } }
    ],
    "edges": [
      { "id": "e1", "source": "n1", "target": "n2" },
      { "id": "e2", "source": "n2", "target": "n3" },
      { "id": "e3", "source": "n3", "target": "n4" },
      { "id": "e4", "source": "n4", "target": "n5" },
      { "id": "e5", "source": "n5", "target": "n6" }
    ]
  },
  "pipeline_config": {
    "target_duration_sec": { "value": 30, "source": "USER_DEFINED" },
    "duration_tolerance_sec": 2,
    "selection": {
      "block_size_beats":            { "value": 8,   "source": "DEFAULT" },
      "min_strong_beat_percentile":  { "value": 0.8, "source": "DEFAULT" },
      "preserve_intro_ms":           { "value": 0,   "source": "USER_DEFINED" },
      "preserve_outro_ms":           { "value": 3000,"source": "DEFAULT" }
    },
    "crossfade": {
      "enabled":     { "value": true, "source": "DEFAULT" },
      "duration_ms": { "value": 1200, "source": "USER_DEFINED" },
      "curve":       { "value": "logarithmic", "source": "DEFAULT" }
    },
    "mastering": {
      "lufs_target":       { "value": -14.0, "source": "DEFAULT" },
      "true_peak_db":      { "value": -1.0,  "source": "DEFAULT" },
      "compression_ratio": { "value": 4.0,   "source": "LLM_INFERRED" },
      "enable_limiting":   { "value": true,  "source": "DEFAULT" }
    },
    "output": { "sample_rate": 44100, "channels": 2, "bit_depth": 24, "codec": "wav" }
  },
  "version_freeze": {
    "enabled": true,
    "prompt_version": "tiktok_aggressive_v2@2.0",
    "llm_model": "openai/gpt-4o"
  }
}
```

Campos obrigatórios: `track_id`, `mode`. Tudo mais tem padrão do servidor.

| `mode` | Comportamento |
| --- | --- |
| `manual` | Ignora o agente. Usa exatamente o `pipeline_config` enviado. |
| `assisted` | Agente preenche os campos com `source != USER_DEFINED`. |

```json
// 202 Accepted
{
  "job_id": "uuid",
  "status": "queued",
  "stream_url": "/api/v1/jobs/{job_id}/events",
  "created_at": "2026-07-24T18:30:00Z",
  "trace_id": "4bf92f3577b34da6a3ce929d0e0e4736"
}
```

**`GET /api/v1/jobs?status=running&track_id=&limit=50&cursor=`**

```json
{
  "items": [
    { "job_id": "uuid", "track_id": "uuid", "status": "completed",
      "progress_pct": 100, "duration_sec": 30.4,
      "similarity_score": 0.983,
      "created_at": "...", "completed_at": "..." }
  ],
  "next_cursor": "eyJ0IjoiMjAy..."
}
```

**`GET /api/v1/jobs/:job_id`** — job completo, incluindo grafo resolvido,
`pipeline_config` final, histórico de tool calls e artefato.

```json
{
  "job_id": "uuid",
  "status": "completed",
  "mode": "assisted",
  "progress_pct": 100,
  "user_prompt": "...",
  "graph": { "nodes": [ { "id": "n4", "status": "completed", "...": "..." } ], "edges": [] },
  "pipeline_config": { "...": "resolvido, com source por campo" },
  "agent_run": {
    "tool_budget": 5,
    "tools_used": 3,
    "steps": [
      { "step": 1, "thought": "A faixa está em 128 BPM...",
        "tool": "compression",
        "parameters": { "ratio": 4.0, "threshold_db": -14.5,
                        "attack_ms": 30, "release_ms": 250 },
        "result": "ok", "duration_ms": 1840 }
    ]
  },
  "artifact": {
    "object_key": "tenant-a7c1/project-01/processed/{job_id}.wav",
    "sha256": "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08",
    "size_bytes": 5292000,
    "duration_sec": 30.4,
    "lufs": -14.1,
    "true_peak_db": -1.0,
    "fingerprint": { "mfcc": [ ], "spectral_centroid": 2418.7, "rms_energy": 0.191 }
  },
  "warnings": [
    {
      "code": "loudness_target_conflict",
      "severity": "warning",
      "at_sec": null,
      "message_ptbr": "Saiu em −16,8 LUFS. O teto de pico de −1 dBTP foi respeitado.",
      "hint_ptbr": "Material muito dinâmico para −14 LUFS sem limitação audível.",
      "measured": { "lufs": -16.8, "target_lufs": -14.0, "true_peak_db": -1.0 }
    }
  ],
  "error": null,
  "trace_id": "4bf92f...",
  "created_at": "...", "updated_at": "...", "completed_at": "..."
}
```

`warnings[]` é um único array servindo a todos os avisos não-bloqueantes do
job — a UI trata aviso de emenda e aviso de masterização pelo mesmo mecanismo.
`severity` é `info` ou `warning`; **aviso nunca muda o estado do job nem
impede o download**. `at_sec` é `null` quando o aviso é do artefato inteiro.
`hint_ptbr` é obrigatório — aviso sem caminho de saída é ruído. Todo aviso
também é emitido por SSE (`job.warning`, §5), para aparecer antes de o render
terminar.

**Hoje o array sempre vem vazio.** Os dois códigos ativos nesta versão
(`loudness_target_conflict`, `duration_target_unreachable`) dependem da
cadeia de masterização de `docs/16-CORRECOES-DSP` (T3.3), que ainda não
executa — `DefaultMixer` é um placeholder (ver `docs/03-ADENDO-R2-CONTRATOS.md`
§4.1). O campo existe no contrato porque a tela de resultado precisa ser
escrita contra ele agora; o valor populado chega junto do motor DSP.

**`POST /api/v1/jobs/:job_id/cancel`** → `200` com job em `cancelled`.
Só válido em `queued`, `running`, `awaiting_approval`.

**`POST /api/v1/jobs/:job_id/retry`** → `202` com **novo** `job_id`.
Só válido em `failed`. Reusa a mesma receita e o mesmo `track_id`.

**`GET /api/v1/jobs/:job_id/artifact`** → `302` para URL assinada, ou
`?redirect=false` para receber `{ "download_url": "...", "expires_at": "..." }`.

---

### 3.4 Parâmetros de nó (a trava manual)

**`PATCH /api/v1/jobs/:job_id/nodes/:node_id/parameters`**

```json
// request — o cliente NÃO envia "source"; o servidor força USER_DEFINED
{ "duration_ms": 1500, "curve": "linear" }
```

```json
// 200
{
  "node_id": "n4",
  "parameters": {
    "duration_ms": { "value": 1500, "source": "USER_DEFINED" },
    "curve":       { "value": "linear", "source": "USER_DEFINED" }
  }
}
```

Erros relevantes:

- `422 parameter_out_of_bounds` — valor fora dos limites (ver `05-AGENTE-IA-HITL.md` §3)
- `409 job_not_editable` — job já em `completed`/`failed`
- `404 unknown_parameter` — chave não existe para o tipo da ferramenta

**`DELETE /api/v1/jobs/:job_id/nodes/:node_id/parameters/:key`** — destrava o
campo (volta a `LLM_INFERRED` ou `DEFAULT`).

---

### 3.5 Propostas (Human-in-the-Loop)

**`GET /api/v1/jobs/:job_id/proposals`** — propostas pendentes (útil no
reload). Cada item tem a mesma forma do payload de `agent.proposal` (§5),
`confidence` incluso — um reload não pode reconstruir um overlay mais pobre
que o que o SSE já tinha mostrado.

**`POST /api/v1/jobs/:job_id/proposals/:proposal_id/approve`**

```json
// request (opcional: ajustar parâmetros na aprovação)
{ "parameters": { "stems": ["drums", "other"] } }
```

```json
// 200
{
  "proposal_id": "uuid",
  "status": "approved",
  "created_node": {
    "id": "n7", "type": "processor", "tool": "stem_separation",
    "status": "queued",
    "position": { "x": 500, "y": 260 },
    "parameters": { "stems": { "value": ["drums","other"], "source": "USER_DEFINED" } }
  }
}
```

**`POST /api/v1/jobs/:job_id/proposals/:proposal_id/reject`**

```json
// request
{ "reason": "não quero separar stems" }   // opcional, vai como observação ao agente
```

```json
// 200
{ "proposal_id": "uuid", "status": "rejected", "agent_will_replan": true }
```

Erros: `409 proposal_expired` (TTL de 120 s vencido), `409 proposal_already_decided`.

**`POST /api/v1/jobs/:job_id/proposals/:proposal_id/replan`** — pede uma
alternativa à mesma proposta, sem decidir aprovar ou recusar ainda. Distinto
de ajustar valor: ajuste é `approve` com `parameters` no corpo (o campo já
existe acima); replanejar troca a sugestão inteira, não só um número.

```json
// request
{ "reason_ptbr": "gosto da ideia, mas 250 Hz está muito largo" }
```

```json
// 200
{ "status": "replanning", "budget_remaining": 2, "supersedes": "prop_8c1d" }
```

Regras:

1. **Replanejar consome orçamento** — é um passo do ReAct como qualquer
   outro. Com `budget_remaining == 0`, `409 budget_exhausted`; a UI passa a
   oferecer só aprovar, recusar ou ajustar.
2. A proposta substituída é encerrada como **`replanned`**, não `rejected` —
   são intenções diferentes e o histórico precisa distingui-las.
3. A alternativa não pode repetir a sugestão substituída: a tupla
   (ferramenta, parâmetros, posição) entra na lista de bloqueio da sessão,
   igual à regra já existente para proposta recusada.
4. `reason_ptbr` é opcional, mas vai ao agente quando presente — é o que
   diferencia replanejar de "tente outra coisa qualquer".
5. O TTL reinicia na proposta nova, com os mesmos 120 s.
6. **Uma decisão por proposta.** Segunda chamada devolve
   `409 proposal_already_decided`, como `approve`/`reject` já fazem.

Erros: `409 proposal_expired`, `409 proposal_already_decided`,
`409 budget_exhausted`.

---

### 3.6 Prompts

| Método | Path | Descrição |
| --- | --- | --- |
| `GET` | `/api/v1/prompts?tags=genre:pop` | Lista do catálogo |
| `GET` | `/api/v1/prompts/:prompt_id` | Spec completa (parâmetros, enums, constraints) |

```json
// GET /api/v1/prompts/tiktok_aggressive_v2
{
  "id": "tiktok_aggressive_v2",
  "name": "TikTok Agressivo",
  "version": "2.0",
  "status": "stable",
  "description": "Versão agressiva para TikTok focada em transientes de bateria.",
  "tags": ["genre:pop", "use:compression", "tier:pro"],
  "parameters": [
    { "name": "tone", "type": "string", "default": "agressiva",
      "enum": ["agressiva", "suave", "energética"],
      "label_ptbr": "Tom" },
    { "name": "platform", "type": "string", "default": "TikTok",
      "enum": ["TikTok", "Instagram", "Radio"], "label_ptbr": "Plataforma" }
  ],
  "tool_sequence": ["stem_separation", "compression", "dynamic_eq", "crossfade"],
  "constraints": ["compression.ratio <= 6.0", "crossfade_ms <= 3000"]
}
```

O campo `parameters` alimenta a UI diretamente: a tela de "receitas" é gerada a
partir dessa spec, sem hardcode no React.

---

### 3.7 Ferramentas e limites

**`GET /api/v1/tools`** — registry disponível para o tenant, **com os limites**.

```json
{
  "tools": [
    {
      "name": "crossfade",
      "label_ptbr": "Transição",
      "category": "stitching",
      "available": true,
      "parameters": [
        { "name": "duration_ms", "type": "integer", "min": 0, "max": 3000,
          "default": 1000, "unit": "ms", "label_ptbr": "Duração da transição" },
        { "name": "curve", "type": "enum",
          "enum": ["linear", "logarithmic", "exponential"],
          "default": "logarithmic", "label_ptbr": "Curva" }
      ]
    },
    {
      "name": "stem_separation",
      "label_ptbr": "Separação de stems",
      "category": "analysis",
      "available": false,
      "unavailable_reason": "requires_plan_pro"
    }
  ]
}
```

> **Isso elimina duplicação de limites.** O frontend não hardcoda `max: 3000`;
> ele lê daqui e configura o slider. Se o backend mudar o limite, a UI acompanha.

---

### 3.8 Tenant e quota

**`GET /api/v1/tenants/me`**

```json
{ "tenant_id": "uuid", "name": "Electric Wolves", "plan": "free",
  "members": 4, "created_at": "..." }
```

**`GET /api/v1/tenants/me/quota`**

```json
{
  "jobs": { "used": 42, "limit": 1000, "period": "month",
            "resets_at": "2026-08-01T00:00:00Z" },
  "storage": { "used_gb": 1.5, "limit_gb": 10.0 },
  "llm_tokens": { "used": 184920, "limit": 2000000 }
}
```

**`GET /api/v1/tenants/me/consent`** — consentimento de modo assistido
(`08-SEGURANCA-MULTITENANCY.md` §8, ADR-0009). Campos `null` quando o tenant
nunca aceitou.

```json
{ "assisted_mode_accepted_at": "2026-07-24T18:22:11Z", "provider_at_accept": "deepseek" }
```

**`POST /api/v1/tenants/me/consent`** — o cliente confirma o provedor que viu
em `GET /system/info` e aceita. `provider` no corpo é **verificado** contra o
provedor ativo no servidor, nunca gravado diretamente — se o provedor mudou
entre a tela mostrar e o aceite chegar, o servidor recusa em vez de gravar
consentimento para o provedor errado. **Se o provedor mudar depois de um
consentimento válido, o consentimento anterior fica obsoleto e a UI pede de
novo** — trocar de provedor muda o que sai da máquina, então o aceite antigo
não cobre o provedor novo.

```json
// request
{ "accepted": true, "provider": "deepseek" }
```

```json
// 200 — mesma forma do GET acima
{ "assisted_mode_accepted_at": "2026-07-24T18:22:11Z", "provider_at_accept": "deepseek" }
```

Erros: `422 consent_not_accepted` (`accepted: false`), `409 provider_mismatch`
(o `provider` do corpo não bate com o provedor ativo agora — refazer
`GET /system/info` e reenviar).

---

## 4. Modelo de erro (RFC 7807)

```json
{
  "type": "https://mixlirous.dev/errors/parameter_out_of_bounds",
  "title": "Parâmetro fora dos limites permitidos",
  "status": 422,
  "code": "parameter_out_of_bounds",
  "detail": "crossfade.duration_ms deve estar entre 0 e 3000; recebido 50000",
  "instance": "/api/v1/jobs/9f2b.../nodes/n4/parameters",
  "trace_id": "4bf92f3577b34da6a3ce929d0e0e4736",
  "errors": [
    { "field": "duration_ms", "code": "max", "max": 3000, "received": 50000 }
  ]
}
```

### Catálogo de códigos

| HTTP | `code` | Quando | Ação da UI |
| --- | --- | --- | --- |
| 400 | `malformed_request` | JSON inválido | Bug — logar |
| 401 | `unauthenticated` | Token ausente/expirado | Renovar sessão |
| 403 | `forbidden` | Recurso de outro tenant | Tela de erro genérica |
| 404 | `not_found` | ID inexistente **ou de outro tenant** | "Não encontrado" |
| 409 | `job_not_editable` | Edição em job finalizado | Desabilitar controles |
| 409 | `proposal_expired` | TTL vencido | Remover overlay, toast informativo |
| 409 | `proposal_already_decided` | Duplo clique | Ignorar silenciosamente |
| 409 | `provider_mismatch` | Consentimento com provedor que não é mais o ativo | Refazer `GET /system/info`, pedir aceite de novo |
| 422 | `consent_not_accepted` | `POST .../consent` com `accepted: false` | Não habilitar modo assistido |
| 409 | `docker_unavailable` | Escala sem Docker | Explicar e sugerir instalar |
| 413 | `file_too_large` | Upload acima do limite | Mensagem com limite |
| 415 | `unsupported_media_type` | Formato de áudio não suportado | Listar formatos aceitos |
| 422 | `parameter_out_of_bounds` | Valor fora do limite | Destacar campo com o limite |
| 422 | `invalid_graph` | Grafo cíclico ou tipos incompatíveis | Destacar aresta culpada |
| 422 | `malicious_prompt` | Padrão de injection detectado | "Não consegui interpretar" |
| 422 | `limit_exceeded` | Workers acima do permitido | Mostrar máximo |
| 429 | `rate_limited` | Excesso de requisições | `Retry-After` |
| 429 | `quota_exceeded` | Quota do plano | Tela de upgrade |
| 500 | `internal_error` | Bug | "Algo deu errado" + `trace_id` visível |
| 502 | `llm_unavailable` | Provedor LLM fora | Oferecer modo manual |
| 503 | `storage_unavailable` | S3/disco inacessível | Bloquear novos jobs |

> **`trace_id` sempre visível ao usuário nos erros 5xx.** É o que transforma um
> ticket de suporte de horas em minutos.

> **404 em vez de 403 para recurso de outro tenant.** Vazar a existência de um ID
> é vazamento de informação. Ver `08-SEGURANCA-MULTITENANCY.md`.

---

## 5. Catálogo de eventos SSE

**Endpoint:** `GET /api/v1/jobs/:job_id/events`
**Também:** `GET /api/v1/tracks/:track_id/events` (só `track.*` e `job.progress`)

### Formato do frame

```
id: 42
event: agent.thought
data: {"job_id":"...","node_id":"n3","text":"A faixa está em 128 BPM..."}

```

Regras:

- Todo evento tem `id` monotônico por job. Reconexão usa `Last-Event-ID`.
- O servidor mantém um buffer dos últimos **200 eventos** por job para replay.
- `retry: 3000` no início do stream.
- Heartbeat comentado (`: ping`) a cada **15 s** para atravessar proxies.
- Todo `data` tem `job_id` e, quando aplicável, `node_id` e `seq`.

### Tabela de eventos

| `event` | Payload | Efeito esperado na UI |
| --- | --- | --- |
| `stream.ready` | `{ job_id, resumed_from }` | Marca conexão ativa |
| `job.state` | `{ job_id, status, previous_status }` | Atualiza badge do job |
| `job.progress` | `{ job_id, stage, progress_pct, eta_sec? }` | Barra de progresso |
| `agent.step_started` | `{ node_id, step, budget_left }` | "Passo 2 de 5" |
| `agent.thought` | `{ node_id, step, text, delta? }` | Painel de raciocínio (streaming) |
| `agent.tool_call` | `{ node_id, step, tool, parameters }` | Destaca ferramenta escolhida |
| `agent.tool_result` | `{ node_id, step, tool, status, duration_ms, summary }` | Nó → `completed` |
| `agent.error` | `{ node_id, step, code, detail, will_replan }` | Nó → `failed`, aviso |
| `agent.proposal` | ver abaixo | Abre overlay de consentimento |
| `agent.finished` | `{ node_id, tools_used, budget_left }` | Fecha painel de raciocínio |
| `proposal.expired` | `{ proposal_id }` | Fecha overlay, toast |
| `proposal.decided` | `{ proposal_id, decision, node_id? }` | Sincroniza outras abas |
| `job.warning` | ver abaixo | Insere em `warnings[]` sem esperar o job terminar |
| `node.state` | `{ node_id, status, error? }` | Cor/borda do nó |
| `node.parameters` | `{ node_id, parameters }` | Preenche sliders (respeitando travas) |
| `node.created` | `{ node, edges }` | Anima novo nó no canvas |
| `job.completed` | `{ job_id, artifact, similarity_score? }` | Player + botão de download |
| `job.failed` | `{ job_id, code, detail, retryable }` | Estado de erro + "Tentar de novo" |
| `job.cancelled` | `{ job_id }` | Estado cancelado |
| `system.resources` | igual a `/system/resources` | Painel de recursos (a cada 5 s) |
| `recovery.report` | `{ recovered, requeued, lost, proposals_expired }` | Banner de recuperação |

### Payloads detalhados dos eventos críticos

**`agent.thought`** — suporta streaming token a token:

```json
{ "job_id": "uuid", "node_id": "n3", "step": 2,
  "text": "A faixa está em 128 BPM. Como o pedido enfatiza as viradas",
  "delta": " de bateria, vou priorizar blocos com onset alto.",
  "done": false }
```

O frontend concatena `delta` ao acumulado. Quando `done: true`, o campo `text`
contém o texto completo e canônico (usar esse para persistir na UI).

**`agent.proposal`**

```json
{
  "job_id": "uuid",
  "proposal_id": "uuid",
  "tool": "stem_separation",
  "tool_label_ptbr": "Separação de stems",
  "reason": "O pedido enfatiza as viradas de bateria. Separar os stems permite comprimir só a percussão, sem afetar o restante da mixagem.",
  "confidence": 0.92,
  "parameters_suggestion": {
    "model": "htdemucs",
    "stems": ["drums", "other"]
  },
  "position_hint": { "relation": "before", "node_id": "n4" },
  "expires_at": "2026-07-24T18:32:11Z",
  "expires_in_sec": 120
}
```

`confidence` (number 0–1) é a certeza do agente **na proposta como um todo** —
usar a ferramenta certa, não só os parâmetros certos. Distinto do `confidence`
por campo do envelope de parâmetro (§2), que mede certeza de um valor
individual depois que a proposta já foi aceita. A UI usa este para decidir
destaque visual da proposta (ex.: menor confiança pede revisão mais atenta
antes de aprovar), não para decidir se ela é oferecida — o agente já filtra
isso antes de propor.

**`job.warning`** — mesma forma de um item de `warnings[]` (§3.3), emitido
assim que o aviso existe, sem esperar o job terminar:

```json
{
  "job_id": "uuid",
  "code": "loudness_target_conflict",
  "severity": "warning",
  "at_sec": null,
  "message_ptbr": "Saiu em −16,8 LUFS. O teto de pico de −1 dBTP foi respeitado.",
  "hint_ptbr": "Material muito dinâmico para −14 LUFS sem limitação audível.",
  "measured": { "lufs": -16.8, "target_lufs": -14.0, "true_peak_db": -1.0 }
}
```

Aviso **nunca** é erro — não dispara `job.failed`, não muda `status`. É por
isso que é um evento à parte, não um campo de `job.state`.

**`node.parameters`**

```json
{
  "job_id": "uuid", "node_id": "n4",
  "parameters": {
    "duration_ms": { "value": 1500, "source": "LLM_INFERRED", "confidence": 0.88 },
    "curve": { "value": "logarithmic", "source": "LLM_INFERRED", "confidence": 0.72 }
  }
}
```

A UI **não** aplica valor sobre campo com `source: "USER_DEFINED"` local. Se o
backend enviar (não deveria), o frontend ignora e loga um aviso.

**`job.progress`** — `stage` é enum fechado:

```
decoding · analyzing_beats · building_blocks · selecting_blocks
stitching · mastering · encoding · uploading
```

**`recovery.report`** — chega no primeiro `connect` após um boot com recuperação:

```json
{ "recovered": 2, "requeued": 1, "lost": 0, "proposals_expired": 5,
  "jobs": [ { "job_id": "uuid", "outcome": "completed" },
            { "job_id": "uuid", "outcome": "requeued" } ] }
```

---

## 6. Máquina de estados do job

```
                 ┌──────────┐
                 │  queued  │◄──────────────────┐
                 └────┬─────┘                   │ recovery / retry
                      │ worker reivindica       │
                 ┌────▼─────┐                   │
            ┌────│ running  │───────────────────┤
            │    └────┬─────┘                   │
   proposta │         │ conclui                 │
            │    ┌────▼──────────┐              │
   ┌────────▼────┴──┐            │              │
   │awaiting_approval│           │              │
   └────────┬────────┘           │              │
     aprova │ │ rejeita/expira   │              │
            │ └──────────────────┤              │
            └────────────────────┤              │
                                 │              │
        ┌────────────┬───────────┴──┬───────────┴──┐
        ▼            ▼              ▼              ▼
  ┌───────────┐ ┌────────┐  ┌───────────┐   ┌──────────┐
  │ completed │ │ failed │  │ cancelled │   │  (loop)  │
  └───────────┘ └────┬───┘  └───────────┘   └──────────┘
                     │ retry → novo job em queued
```

### Tabela de transições permitidas

| De | Para | Gatilho |
| --- | --- | --- |
| `queued` | `running` | worker reivindica |
| `queued` | `cancelled` | usuário cancela |
| `running` | `awaiting_approval` | agente emite proposta |
| `running` | `completed` | DSP + artefato persistido + hash validado |
| `running` | `failed` | erro irrecuperável ou budget de retry esgotado |
| `running` | `cancelled` | usuário cancela |
| `running` | `queued` | **recovery** encontrou job órfão com artefato inválido |
| `awaiting_approval` | `running` | aprovação, rejeição ou expiração |
| `awaiting_approval` | `cancelled` | usuário cancela |
| `failed` | — | terminal. `retry` cria job novo. |
| `completed` | — | terminal |
| `cancelled` | — | terminal |

Qualquer outra transição é bug e deve gerar `panic!` em debug / log de erro
crítico + `audit_event` em release.

---

## 7. Rate limiting

| Escopo | Limite | Header |
| --- | --- | --- |
| Global por tenant | 300 req/min | `X-RateLimit-Remaining` |
| `POST /jobs` | 60/min (free), 600/min (pro) | idem |
| SSE simultâneos | 10 streams por tenant | `429 rate_limited` |
| Upload | 20 GB/dia (free) | `429 quota_exceeded` |

No modo local, o rate limiting é desativado (`features.rate_limit: false`).

---

## 8. Contrato de tipos para o frontend

Gerar `ui/src/types/api.ts` a partir dos structs Rust, não à mão.

Recomendado: **`ts-rs`** — anota o struct em Rust e exporta TypeScript no
`cargo test`.

```rust
#[derive(Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../ui/src/types/")]
pub struct JobResponse { /* ... */ }
```

```bash
cargo test export_bindings   # regenera ui/src/types/*.ts
```

Adicionar ao CI: se `git diff --exit-code ui/src/types/` falhar, o PR quebra —
garantindo que o contrato nunca fica dessincronizado.

---

## 9. Checklist para alterar este contrato

- [ ] O campo tem sufixo de unidade?
- [ ] Se é numérico e a IA pode preencher, usa `Parameter<T>`?
- [ ] Se é conjunto fechado, é enum (não `String`)?
- [ ] Existe entrada no catálogo de erros para o caso de falha?
- [ ] Se gera evento, está na tabela SSE com payload documentado?
- [ ] Os tipos TS foram regenerados (`cargo test export_bindings`)?
- [ ] O designer sabe que um estado novo apareceu na UI?
