# 09 — MLOps e Regressão Acústica

## 1. O problema que isso resolve

Um músico aprova um remix hoje. Amanhã o provedor atualiza o modelo, ou alguém
ajusta uma vírgula no prompt, e a mesma receita produz um som diferente. Ele
reabre o projeto e não reconhece o próprio trabalho.

Em ferramenta criativa isso é fatal para a confiança. Produtores tratam plugins
como instrumentos: precisam soar igual sempre.

Quatro mecanismos, do mais barato ao mais forte:

```
1. Prompt como código      → mudança passa por PR e revisão
2. Golden Master acústico  → CI mede o som, não só o JSON
3. Canary por tenant       → rollout gradual com rollback automático
4. Version freeze          → usuário congela modelo + prompt no projeto
```

---

## 2. Prompt como código

Detalhes do formato em `05-AGENTE-IA-HITL.md` §5. As regras de operação:

| Regra | Detalhe |
| --- | --- |
| Vive no git | `prompts/*.prompt` + `prompts/catalog.json` |
| Nunca no binário | carregado do disco em runtime (permite ajuste sem recompilar) |
| Versão no `id` | mudança incompatível vira `id` novo, não sobrescreve |
| Linter no CI | schema, enums, ferramentas existentes, constraints parseáveis |
| Compatibilidade | remover parâmetro ou apertar enum = *breaking*; exige `id` novo |
| Altera prompt → roda Golden Master | obrigatório antes do merge |

### Linter — o que verifica

```bash
python scripts/prompt_linter.py prompts/catalog.json
```

1. `id` e `version` presentes; `id` único no catálogo
2. `default` de cada parâmetro pertence ao `enum` declarado
3. Toda ferramenta em `tool_sequence` existe no registry
4. Cada `constraint` é parseável e usa campo real (`compression.ratio`, não
   `compression.rate`)
5. Constraint não é mais permissiva que o limite do tipo em Rust
6. `model_hint` está em `model_registry.json` com status ≠ `deprecated`

O item 5 é sutil e importante: um prompt não pode declarar
`crossfade_ms <= 5000` se o tipo `CrossfadeMs` limita em 3000. O linter pega
essa divergência antes de virar erro em runtime.

---

## 3. Golden Master acústico

### Conceito

Para cada par (fixture de áudio × prompt), guardamos um WAV de referência
aprovado por ouvido humano. O CI regenera e compara **acusticamente**.

```
fixtures/
├── stems/
│   ├── bossa_nova_120bpm.wav      30 s, tempo estável, harmonia rica
│   ├── punk_180bpm.wav            30 s, transientes agressivos
│   ├── ambient_no_beat.wav        30 s, sem grade clara (caso limite)
│   └── jam_tempo_drift.wav        60 s, tempo instável (caso limite)
└── golden/
    ├── bossa_nova__tiktok_aggressive_v2.wav
    ├── bossa_nova__tiktok_aggressive_v2.fingerprint.json
    └── ...
```

Fixtures precisam ser **áudio original, livre de direitos** — gravado pelo time
ou de biblioteca CC0. Nunca música comercial, nem em teste.

### Comparação

```rust
#[test]
fn golden_master_bossa_nova_tiktok() {
    let input  = load_fixture("stems/bossa_nova_120bpm.wav");
    let output = render_with(
        PromptRef::pinned("tiktok_aggressive_v2@2.0"),
        LlmProvider::ollama_seeded("llama3.1:8b", 42),
        &input,
    );

    let golden = load_fingerprint("golden/bossa_nova__tiktok_aggressive_v2.fingerprint.json");
    let actual = AudioFingerprint::from_pcm(&output, 44_100);

    let d = golden.distance(&actual);
    assert!(d < 0.15, "deriva sonora detectada: distância = {d:.3}");
}
```

### Limiares

| Distância | Interpretação | Ação no CI |
| --- | --- | --- |
| < 0,05 | Indistinguível | passa |
| 0,05 – 0,15 | Diferença sutil | passa com aviso no PR |
| 0,15 – 0,35 | Mudança audível | **falha** — exige aprovação humana |
| > 0,35 | Som diferente | **falha** — bloqueia merge |

Quando a mudança é intencional (melhoria real), o autor roda
`cargo test --features update-golden` e o novo WAV entra no PR — revisado por
alguém que **ouviu** os dois arquivos. O PR template pede a confirmação
explícita: "ouvi ambos e a mudança é desejada".

### Determinismo dos testes

O LLM é a fonte de não-determinismo. Duas estratégias:

1. **`MockLlm`** com respostas gravadas — para a maioria dos testes. Rápido, sem
   rede, 100% determinístico. Testa o DSP e o pipeline.
2. **Ollama com `seed` fixo** — para os testes de Golden Master de verdade.
   Mesmo prompt + mesmo seed = mesma sequência de tool calls.

Não usar provedor externo em CI: custo, flakiness e dependência de rede.

---

## 4. Fingerprint — o que medir

Ver `04-DOMINIO-DSP.md` §9 para a implementação. Recapitulando as features e por
que cada uma está lá:

| Feature | Captura | Peso |
| --- | --- | --- |
| MFCC (13 coef.) | Timbre — o "caráter" do som | 2,0 |
| Centroide espectral | Brilho — agudo vs abafado | 1,5 |
| Energia RMS | Intensidade média | 1,0 |
| Contraste espectral (7 bandas) | Separação entre picos e vales | 1,0 |
| Peak/RMS ratio | Quanto foi comprimido | 1,0 |
| LUFS | Volume percebido final | 1,0 |

**Todas normalizadas antes de somar.** O bug já identificado no kit: somar Hz
com amplitude produz uma distância dominada pelo centroide. Corrigir na Sprint 4.

### O que a fingerprint não captura

Ela não detecta um estalo de 2 ms na emenda — o impacto nas médias é
desprezível. Estalos são pegos pelo invariante de continuidade em
`04-DOMINIO-DSP.md` §7.3, que é um teste separado e obrigatório.

---

## 5. Registry de modelos

```json
{
  "default": "openai/gpt-4o",
  "models": [
    { "model_id": "openai/gpt-4o", "status": "stable",
      "approved_for": ["pro", "enterprise"],
      "golden_scores": { "bossa_nova": 0.02, "punk": 0.04 },
      "approved_at": "2026-07-01T12:00:00Z" },
    { "model_id": "ollama/llama3.1-8b", "status": "stable",
      "approved_for": ["free", "pro"], "local_only": true,
      "golden_scores": { "bossa_nova": 0.09, "punk": 0.12 } },
    { "model_id": "openai/gpt-4o-mini", "status": "canary",
      "canary_traffic_pct": 5, "approved_for": ["free"] }
  ]
}
```

Trocar o modelo padrão exige: suíte de Golden Master verde, aprovação humana
registrada no PR, e entrada com `approved_at`.

---

## 6. Canary (pós-MVP)

Rollout gradual por hash de tenant: 5% → 25% → 50% → 100%, com janela de
observação entre etapas.

Rollback automático se, na coorte canary:

- distância de fingerprint p95 > 0,15, **ou**
- taxa de erro > 1%, **ou**
- p99 de latência do LLM > 2× a baseline

Antes de existir usuário pagante isso é infraestrutura sem uso. Fica desenhado,
implementado depois.

---

## 7. Version freeze

A funcionalidade mais valiosa para a persona P3.

```json
"version_freeze": {
  "enabled": true,
  "prompt_version": "tiktok_aggressive_v2@2.0",
  "llm_model": "openai/gpt-4o",
  "frozen_at": "2026-07-24T18:30:00Z"
}
```

Quando ativo:

1. Novos jobs do projeto usam exatamente essas versões.
2. Feature flags e canary **não se aplicam**.
3. Se a versão do prompt não existir mais no disco, o job falha com
   `frozen_version_unavailable` — em vez de silenciosamente usar outra.
4. O job salva a fingerprint do render e mostra a similaridade com o anterior.

Item 3 é uma decisão consciente: falhar alto é melhor que entregar som diferente
sem avisar. Consequência prática: **prompts nunca são deletados**, só marcados
como `deprecated`.

### Na interface

```
┌──────────────────────────────────────────────────┐
│ 🔒 Versões congeladas                            │
│                                                  │
│ Assistente   gpt-4o                              │
│ Receita      TikTok Agressivo v2.0               │
│ Congelado em 24/07/2026                          │
│                                                  │
│ Este projeto sempre vai renderizar com as mesmas  │
│ configurações, mesmo quando houver atualizações.  │
│                                        [Liberar]  │
└──────────────────────────────────────────────────┘
```

Ao liberar, avisar: "renders futuros podem soar diferente dos anteriores".

---

## 8. Deriva em produção

Além do CI, medir deriva no uso real: quando um job usa a mesma receita e a
mesma faixa de um job anterior, comparar as fingerprints e registrar
`similarity_score`.

Alerta se a distância média por (prompt × modelo) subir acima de 0,10 na janela
de 7 dias. É o sinal precoce de que um modelo mudou de comportamento sem aviso —
coisa que provedores fazem sem changelog.

---

## 9. Fluxo de mudança de prompt (o caminho completo)

```
1. Ajusta o .prompt em branch                        [dev]
2. Testa no playground / script descartável           [dev]
   → não recompila o Rust para isso
3. Abre PR
4. CI: linter de prompt                               [automático]
5. CI: Golden Master de todos os fixtures             [automático]
   → distância > 0,15 = falha, com o WAV como artifact do build
6. Revisor baixa os dois WAVs e ouve                  [humano]
7. Merge → o prompt novo é o padrão
8. Prompt antigo permanece no disco (deprecated)      [obrigatório]
9. Novo Golden Master versionado no repositório
```

O passo 6 não é opcional e não é automatizável. Distância euclidiana aproxima
percepção; ela não substitui um ouvido.
