# 03 — Adendo R2: adições de contrato vindas do desenho

**Adendo, não reescrita.** Lista apenas os deltas em relação ao
`03-CONTRATOS-API.md`. Cada item é aplicável isoladamente.

**Origem:** revisão do protótipo (`Mixlirous.dc.html`, 13 telas) e do handoff de
frontend, seção 11. As 11 lacunas listadas lá são adições de contrato, não
pedidos de funcionalidade — a UI já está desenhada contra dados que a API não
expõe.

**Data:** 24/07/2026

---

## 0. Errata ao `docs/16` — as curvas de fade e de crossfade são tipos diferentes

**Esta correção é a mais importante do adendo, e o erro era meu.**

O `docs/16` T2.1 manda remover `Logarithmic` e `Exponential` do enum `FadeCurve`
porque as duas estão quebradas na implementação. A remoção está certa **para
crossfade** e **errada para fade de entrada e saída** — e o motivo é conceitual,
não de implementação.

| | **Crossfade** | **Fade de entrada/saída** |
|---|---|---|
| O que acontece | Dois sinais somam durante a sobreposição | Um sinal vai de/para o silêncio |
| A pergunta | Como a **soma** se comporta | Como a **percepção de volume** se comporta |
| Vocabulário correto | potência constante · ganho constante | linear · logarítmica · exponencial |

Um fade não tem segundo sinal, então "potência constante" não significa nada
ali. E rampa linear de amplitude num fade **soa errada**: a percepção de volume é
aproximadamente logarítmica, então um fade linear parece cair rápido no começo e
se arrastar no fim. É por isso que todo software de áudio oferece curva de fade —
o conceito é legítimo. O que está quebrado é a *implementação* (`.ln() / 1.0f32.ln()`
divide por zero), não a ideia.

O kit fundiu os dois casos num `FadeCurve` só. Meu `docs/16` herdou a fusão e
"corrigiu" apagando as variantes.

**Correção: dois tipos distintos.**

```rust
/// Crossfade: dois sinais somando.
pub enum CrossfadeCurve {
    /// gain_a + gain_b = 1. Material correlacionado.
    ConstantGain,
    /// gain_a² + gain_b² = 1. Padrão — blocos de origens diferentes.
    ConstantPower,
}

/// Fade de entrada/saída: um sinal de/para o silêncio.
pub enum FadeCurve {
    /// Rampa linear de amplitude.
    Linear,
    /// Linear em dB. Padrão — é a que soa uniforme.
    Logarithmic,
    /// Cai devagar e depois rápido.
    Exponential,
}
```

**Ajustes decorrentes no `docs/16`:**

- T2.1 passa a valer só para `CrossfadeCurve`.
- **Reimplementar** `FadeCurve::Logarithmic` e `Exponential` corretamente, com as
  propriedades do Bloco 1 aplicadas às duas: nenhuma amostra não finita, e
  monotonicidade (`fade_out` nunca sobe, `fade_in` nunca desce). A propriedade de
  monotonicidade é nova e pega o bug do exponencial que nunca desvanecia.
- O protótipo já usa a nomenclatura certa nos dois lugares — `crossfade` com
  potência/ganho constante, `fade_in`/`fade_out` com linear/logarítmica/exponencial,
  padrão logarítmica. **O desenho estava certo e a documentação errada.**

---

## 1. `warnings[]` — um array servindo a todos os avisos

Descoberta do desenho: a UI precisa de *"emenda brusca aos 0:18"*. O `docs/16`
T3.3 precisa de *"não foi possível atingir os dois alvos"*. **É o mesmo
mecanismo.** Dois sistemas paralelos divergiriam.

Adicionar a `GET /jobs/:id`:

```json
{
  "warnings": [
    {
      "code": "abrupt_splice",
      "severity": "warning",
      "at_sec": 18.42,
      "message_ptbr": "Emenda brusca aos 0:18.",
      "hint_ptbr": "Aumentar a transição para 1,8 s costuma resolver.",
      "related_node_id": "nd_7f3a"
    },
    {
      "code": "loudness_target_conflict",
      "severity": "warning",
      "at_sec": null,
      "message_ptbr": "Saiu em −16,8 LUFS. O teto de pico de −1 dBTP foi respeitado.",
      "hint_ptbr": "Material muito dinâmico para −14 LUFS sem limitação audível.",
      "measured": { "lufs": -16.8, "target_lufs": -14.0, "true_peak_db": -1.0 }
    }
  ]
}
```

**Catálogo inicial de códigos:**

| Código | Origem | Quando |
|---|---|---|
| `abrupt_splice` | emenda | Descontinuidade acima do limiar de I4.1 |
| `splice_power_dip` | emenda | Queda de RMS acima de 1 dB na emenda (I4.2) |
| `loudness_target_conflict` | masterização | Alvo de LUFS e teto de pico incompatíveis (`docs/16` §4 passo 6) |
| `duration_target_unreachable` | montagem | Duração alvo exige esticamento fora de ±10% |
| `limiting_heavy` | masterização | Redução de ganho acima do limiar de transparência |
| `insufficient_material` | seleção | Blocos aprovados não somam a duração alvo |
| `tool_unavailable_skipped` | agente | Ferramenta proposta indisponível na máquina |

**Regras:**

1. `severity` é `info` ou `warning`. **Aviso nunca é erro** — não muda o estado do
   job, não impede o download. O handoff já trata assim, e está certo: tratar
   aviso como erro gera alarme falso e o usuário aprende a ignorar.
2. `at_sec` é `null` quando o aviso é do artefato inteiro.
3. Todo aviso é emitido também por SSE, evento novo `job.warning`, para aparecer
   antes de o render terminar.
4. `hint_ptbr` é obrigatório. Aviso sem caminho de saída é ruído.

---

## 2. Artefato renderizado — os quatro campos que faltam

Sem estes, a tela de resultado é ilustração. O diagnóstico do designer está
exato.

### 2.1 `GET /jobs/:id/artifact/peaks?resolution=1024`

Idêntico a `GET /tracks/:id/peaks`, para o artefato. Hoje só a faixa original tem.

```json
{ "resolution": 1024, "duration_sec": 30.4, "peaks": [[-0.82, 0.79], ...] }
```

### 2.2 `splice_markers[]` em `GET /jobs/:id`

```json
{
  "artifact": {
    "splice_markers": [
      { "at_sec": 6.12, "crossfade_ms": 1000, "curve": "constant_power",
        "source_block_ids": ["blk_02", "blk_07"], "warning_code": null },
      { "at_sec": 18.42, "crossfade_ms": 250, "curve": "constant_power",
        "source_block_ids": ["blk_07", "blk_11"], "warning_code": "abrupt_splice" }
    ]
  }
}
```

`warning_code` amarra o marcador ao aviso, o que permite a UI distinguir emenda
normal de emenda com problema — como o protótipo já faz.

> **Adição do desenho que entra na especificação:** o controle **"Solo da
> emenda"** no transporte. Isolar só a região da emenda é a forma mais rápida de
> um músico julgar se o crossfade funcionou. É a contraparte audível do
> invariante I4.2, que hoje só existe como teste automático. Não exige campo
> novo — deriva de `at_sec` e `crossfade_ms`.

### 2.3 Matriz de espectro para o diff A/B

```
GET /jobs/:id/spectrum?which=original|render&bands=64&frames=256
```

```json
{ "which": "render", "bands": 64, "frames": 256,
  "freq_hz": [20, 25, 31, ...], "time_sec": [0.0, 0.119, ...],
  "db": [[-62.1, -48.3, ...], ...], "db_floor": -90.0 }
```

Duas chamadas para o A/B. **Escala idêntica nas duas** — o protótipo já garante
isso na renderização, e o contrato precisa garantir na origem: mesma `db_floor`,
mesmas bordas de banda.

### 2.4 `confidence` na proposta

Hoje só existe no envelope de parâmetro. Adicionar ao payload de
`agent.proposal`:

```json
{ "proposal_id": "prop_8c1d", "tool": "dynamic_eq", "confidence": 0.92, ... }
```

---

## 3. Replanejar — endpoint e regras

```
POST /jobs/:id/proposals/:proposal_id/replan
```

```json
{ "reason_ptbr": "gosto da ideia, mas 250 Hz está muito largo" }
```

```json
{ "status": "replanning", "budget_remaining": 2, "supersedes": "prop_8c1d" }
```

**Regras invioláveis** — sem elas o endpoint quebra o loop do agente:

1. **Replanejar consome orçamento.** É um passo do ReAct como qualquer outro. Com
   `budget_remaining == 0`, a API responde **409 `budget_exhausted`** e a UI
   oferece só aprovar, recusar ou ajustar.
2. **A proposta substituída é encerrada** e registrada como `replanned`, não como
   `rejected`. São intenções diferentes e o histórico precisa distingui-las.
3. **A alternativa não pode repetir a sugestão substituída.** A regra existente
   de não reinsistir em proposta recusada vale igual aqui — a tupla
   (ferramenta, parâmetros, posição) entra na lista de bloqueio da sessão.
4. **`reason_ptbr` é opcional mas vai ao agente** quando presente. É o que
   diferencia replanejar de "tente outra coisa qualquer".
5. **O TTL reinicia** na proposta nova, com os mesmos 120 s.
6. **Uma decisão por proposta.** Segunda chamada devolve
   **409 `proposal_already_decided`**, como aprovar e recusar já fazem.

**Ajustar valor não passa por aqui.** Ajustar antes de aprovar é
`POST .../approve` com `parameters` no corpo — o campo já existe no contrato. É o
caminho mais importante dos quatro, porque é onde a trava manual vive dentro do
fluxo de proposta: o valor aprovado com ajuste entra como `USER_DEFINED`, não
como `LLM_INFERRED`.

---

## 4. Limites que hoje não existem no contrato

O protótipo expõe faixas que o modelo de domínio não impõe. Enquanto o limite
viver só na UI, o agente pode propor fora dele e o validador aceita.

| Parâmetro | Hoje no domínio | Precisa ser |
|---|---|---|
| `dynamic_eq.bands` | `Vec<EqBand>` sem teto | **1 a 8 bandas** (`max_items`) |
| `stem_separation.model` | `String` livre | Enum **derivado do binário detectado**, não lista fixa |
| `stem_separation.stems` | `Vec<String>` livre | Enum: `drums`, `bass`, `vocals`, `other` — 1 a 4 |
| `dynamic_eq.bands[].type_filter` | `String` livre | Enum: `peak`, `shelf`, `highpass`, `lowpass` |

Sobre `stem_separation.model`: a ADR-0010 determina detecção de binário externo.
A lista de modelos precisa vir do que o binário suporta e ser refletida em
`GET /tools`, não fixada no código — senão o contrato promete modelo que a
máquina não tem.

### `knee_db` — decisão pendente

O protótipo tem um controle **"Joelho: 0 a 12 dB, padrão 6,0 dB"**.
`CompressionParams` **não tem esse campo**. Uma das duas:

- **(a)** Adicionar `knee_db` ao domínio, ao validador, à tabela canônica e ao
  registry. Joelho suave é o que separa compressão musical de compressão que
  soa apertada — tem mérito real.
- **(b)** Remover o controle do protótipo.

**Recomendo (a)**, mas é decisão do dono do produto e precisa entrar na tabela
canônica de `docs/05` §3 **antes** de o dev implementar a tela. Um controle na UI
sem parâmetro no domínio é a divergência que o `CONTRIBUTING.md` chama de bug
mais caro do sistema — aqui detectada antes de existir, que é o momento certo.

---

## 5. Endpoints de leitura que faltam

| Endpoint | Tela | Observação |
|---|---|---|
| `GET /audit-events?actor=&cursor=&limit=` | Registro de atividade | O evento já é gravado (`docs/06`). Falta só ler. `trace_id` por linha. |
| `GET /tenants` | Trocador de espaços | Lista os tenants do usuário. A troca **reemite o token** com outro `tenant_id` — nunca aceita `tenant_id` do cliente. Ver §6. |
| `PATCH /projects/:id/settings` | Configurações | `version_freeze`, `llm_temperature`, `llm_timeout_sec`, `max_tools`. Hoje só existe por job. |
| `GET /projects/:id/golden-master` | Configurações | Última execução: data, similaridade, resultado. |
| `GET /system/resources` → `workers[]` | Recursos | `{ id, status, job_id, started_at }`. Hoje só contadores. |
| `POST /auth/logout` | Sessão | Invalidação do token no servidor. |

---

## 6. Aviso de segurança sobre o trocador de espaços

O trocador de espaços é a única adição deste adendo com superfície de segurança.

**A troca de tenant é reemissão de token no servidor.** O cliente pede
`POST /auth/switch-tenant { tenant_id }`; o servidor **verifica que o usuário
pertence àquele tenant** e emite token novo. Em nenhum ponto o `tenant_id` do
corpo da requisição é usado como escopo de dados.

Isto conecta com o bug corrigido no PR #4: a cadeia de confiança do `tenant_id`
começa e termina nos claims do token. Um trocador de espaços mal implementado é
a forma mais direta de furar o isolamento que acabamos de consertar.

---

## 7. Consentimento de privacidade — requisito, não tela

O `docs/08` exige e a ADR-0009 determina: antes do **primeiro uso em modo
assistido**, o usuário vê o provedor ativo nomeado e o que sai da máquina.

Não é opcional e não é só interface:

```
GET  /system/info          → llm_provider, llm_model, data_egress: true|false
GET  /tenants/me/consent   → { assisted_mode_accepted_at, provider_at_accept }
POST /tenants/me/consent   → { accepted: true, provider: "deepseek" }
```

**O provedor no aceite é registrado.** Se o provedor mudar, o consentimento é
pedido de novo — consentir com IA local não é consentir com serviço externo.

Com DeepSeek como padrão, há transferência internacional de dados e a LGPD pede
declaração. Custo baixo, prioridade alta, e é o tipo de item que não dá para
adicionar depois do lançamento.

---

## 8. Ordem de implementação

Alinhada com o backlog do handoff:

**S1 — destrava o desenho**

1. `confidence` na proposta (§2.4)
2. Ajustar-ao-aprovar já existe no contrato — só precisa da UI
3. `POST .../replan` com as 6 regras (§3)
4. `warnings[]` + evento `job.warning` (§1) — só `loudness_target_conflict` e
   `duration_target_unreachable`, que vêm do `docs/16`
5. Consentimento nomeando o provedor (§7)
6. Limites de `dynamic_eq.bands` e enums (§4)
7. Decisão sobre `knee_db` (§4)

**S2 — a tela de resultado deixa de mentir**

8. `peaks` do artefato (§2.1)
9. `splice_markers[]` + códigos de emenda em `warnings[]` (§2.2)
10. `GET /jobs/:id/spectrum` (§2.3)
11. `GET /audit-events` (§5)

**S3 — P3, não P1**

12. `GET /tenants` + troca de espaço (§5, §6)
13. `PATCH /projects/:id/settings` (§5)
14. `golden-master` por projeto (§5)
15. `workers[]` detalhado (§5)

Os itens 12 a 15 servem a persona de estúdio. As telas estão desenhadas; o
compromisso de escopo não existe. São as primeiras a sair se apertar.

---

## 9. A tela `FERR` virou a fonte legível da tabela de parâmetros

Observação de processo, e importa.

A tela **Ferramentas e parâmetros** do protótipo é hoje a apresentação mais
legível dos ~20 controles com faixa e padrão que existe no projeto — mais legível
que a tabela canônica de `docs/05` §3. Isso significa que **as pessoas vão tratá-la
como a verdade**, mesmo que a arquitetura diga que os limites vêm de `GET /tools`.

Duas providências:

1. **Reconciliar os números** da tela com a tabela canônica, item por item, antes
   de a tela virar código. Foi o que revelou o `knee_db` e o teto de bandas.
2. A tela precisa **carregar os limites de `GET /tools` em tempo de execução**, como
   o próprio protótipo já declara. Nenhum número fixado no cliente. Aí a tela
   deixa de poder divergir por construção — o que é melhor que combinar que ela
   não divirja.
