# 10 — Testes e Qualidade

## 1. Onde concentrar o esforço

Não distribuir cobertura por igual. Quatro áreas concentram o risco real:

| Pilar | Risco que elimina | Peso do esforço |
| --- | --- | --- |
| **1. Validação de contrato** | Alucinação do LLM chega ao áudio | 25% |
| **2. Atomicidade + recovery** | Queda de máquina perde trabalho | 25% |
| **3. Invariantes de DSP** | Áudio com estalo ou clipping publicado | 30% |
| **4. Idempotência da fila** | Processamento duplicado ou perdido | 10% |
| Interface e E2E | Fricção de uso | 10% |

Os 30% no DSP são a diferença deste projeto para um CRUD: um bug de contrato
gera erro visível; um bug de DSP gera um arquivo que soa mal e é publicado.

---

## 2. Pirâmide

```
       ┌──────────────────────────────────────┐
       │  E2E + Caos  — Playwright, SIGKILL   │  ~15 cenários
       ├──────────────────────────────────────┤
       │  UI  — Vitest + Testing Library      │  ~50 testes
       ├──────────────────────────────────────┤
       │  Integração  — testcontainers, mocks │  ~120 testes
       ├──────────────────────────────────────┤
       │  Unitário  — cargo test, proptest    │  ~400 testes
       └──────────────────────────────────────┘
```

---

## 3. Nível 1 — Unitário (Rust puro, sem I/O)

Ferramentas: `cargo test`, `proptest`, `cargo-fuzz`, `criterion`.

### Testes baseados em propriedade (DSP)

O tipo de teste mais valioso aqui. Em vez de checar um caso, checa uma
propriedade contra milhares de entradas geradas.

```rust
proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn compressor_nunca_aumenta_o_pico(
        input in prop::collection::vec(-1.0f32..=1.0, 1000..48_000)
    ) {
        let params = CompressorParams { makeup_gain_db: 0.0, ..Default::default() };
        let out = apply_compression(&input, &params);
        let p_in  = input.iter().fold(0.0f32, |m, &x| m.max(x.abs()));
        let p_out = out.iter().fold(0.0f32, |m, &x| m.max(x.abs()));
        prop_assert!(p_out <= p_in + 1e-6);
    }
}
```

### Invariantes obrigatórios

| # | Invariante | Módulo |
| --- | --- | --- |
| I1 | Compressor com makeup ≤ 0 nunca aumenta o pico | `mastering/compressor` |
| I2 | Limiter nunca deixa amostra acima do teto | `mastering/limiter` |
| I3 | Crossfade preserva duração: `len = a + b − L` | `stitching/crossfade` |
| I4 | Crossfade não introduz descontinuidade > 1,5× do máximo interno | `stitching/crossfade` |
| I5 | `snap_to_zero_crossing` retorna índice dentro da janela e do buffer | `stitching/zero_cross` |
| I6 | Grade de batidas estritamente crescente | `analysis/beat_tracking` |
| I7 | Blocos não se sobrepõem | `domain/block` |
| I8 | Knapsack respeita `target ± tolerance` ou retorna `Err` | `selection/knapsack` |
| I9 | Knapsack é determinístico (mesma entrada → mesma saída) | `selection/knapsack` |
| I10 | `fingerprint.distance(x, x) == 0` e é simétrica | `domain/fingerprint` |
| I11 | Após normalização, `|lufs − alvo| ≤ 0,5 LU` | `mastering/lufs` |
| I12 | Time-stretch entrega duração dentro de ±20 ms | `mastering/stretch` |
| I13 | `RMS(seno amplitude 1) ≈ 0,7071` | `analysis/rms` |
| I14 | Newtype rejeita valor fora do limite na desserialização | `domain/*` |

O I4 é o teste anti-estalo. Definição operacional:

```rust
fn max_delta(x: &[f32]) -> f32 {
    x.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0, f32::max)
}
// na região da emenda, max_delta <= 1.5 * max(max_delta(a), max_delta(b))
```

### Fuzzing

Alvos: `decode_to_pcm` (bytes arbitrários), `validate_tool_call` (JSON
arbitrário), `load_prompt_file` (YAML arbitrário). Nenhum pode entrar em pânico.
Rodam no CI noturno, 15 min cada.

### Benchmarks

`criterion` nas etapas de DSP, com baseline versionada. CI falha se p95 piorar
mais de 20%.

---

## 4. Nível 2 — Integração

Ferramentas: `sqlx::test` / `testcontainers`, `wiremock`, `tempfile`.

### Persistência e fila

| # | Cenário | Assertiva |
| --- | --- | --- |
| P1 | 10 workers concorrentes reivindicam a fila | cada job vai para exatamente 1 worker |
| P2 | Idem, em SQLite | idem, sem `SQLITE_BUSY` não tratado |
| P3 | Transição inválida (`completed` → `running`) | rejeitada |
| P4 | Query sem escopo de tenant | não compila (`ScopedRepo`) ou retorna 0 linhas (RLS) |
| P5 | `Idempotency-Key` repetida | retorna o mesmo `job_id`, não cria outro |
| P6 | Job sem heartbeat por 2 min | volta para `queued` |
| P7 | Migração roda em banco vazio e em banco existente | idempotente |

### Recovery (injeção de falha)

| # | Cenário | Estado final esperado |
| --- | --- | --- |
| R1 | `SIGKILL` durante escrita; só `.tmp` existe | `queued`, `.tmp` removido |
| R2 | `SIGKILL` após `rename`, antes do `UPDATE` | `completed` |
| R3 | Arquivo existe, hash não bate | `queued`, arquivo removido |
| R4 | Artefato ausente e `attempt >= max` | `failed(artifact_lost)` |
| R5 | Proposta expirada no crash | `expired` → job `queued` |
| R6 | Recovery interrompido no meio; roda de novo | mesmo resultado (idempotente) |
| R7 | Dois processos sobem juntos | só um faz recovery (lock) |

R2 é o teste mais importante do conjunto: prova que o usuário não perde trabalho
concluído por causa de um desligamento.

### Agente (LLM mockado)

Cenários A1–A10 de `05-AGENTE-IA-HITL.md` §9. Todos com `wiremock` ou `MockLlm`.

### Storage

Mesma bateria contra os três adapters (`local_fs`, `minio` via testcontainer,
`s3` via mock): escrita, leitura, ausência, prefixo de tenant, path traversal
rejeitado.

---

## 5. Nível 3 — Interface

Ferramentas: `vitest`, `@testing-library/react`, `msw`.

| # | Cenário | Assertiva |
| --- | --- | --- |
| U1 | SSE `agent.thought` com `delta` | texto acumula sem piscar |
| U2 | SSE `node.parameters` em campo travado | valor do usuário permanece |
| U3 | Clique em "Aprovar" | `POST .../approve` uma única vez |
| U4 | Duplo clique rápido em "Aprovar" | uma requisição só (botão desabilita) |
| U5 | `proposal.expired` recebido | overlay fecha, toast aparece |
| U6 | Reconexão de SSE | envia `Last-Event-ID`, sem eventos duplicados |
| U7 | Slider fora do limite | bloqueado no cliente, com dica do limite |
| U8 | `job.failed` | estado de erro com `trace_id` copiável |
| U9 | Aresta inválida no canvas | recusada com destaque na aresta |
| U10 | F5 durante proposta pendente | overlay reabre com tempo restante |

U10 e U6 são os que costumam quebrar. Não pular.

---

## 6. Nível 4 — E2E e caos

Ferramenta: Playwright. Matriz: `{sqlite, postgres} × {local, docker}`.

| # | Cenário | Assertiva |
| --- | --- | --- |
| E1 | Fluxo completo: upload → prompt → render → download | WAV baixado, duração ± tolerância |
| E2 | `SIGKILL` no backend durante render, reinicia | banner de recuperação, job resolvido |
| E3 | Provedor LLM em timeout (WireMock) | job completa em modo manual com aviso |
| E4 | Proposta aprovada | nó novo aparece no canvas e é processado |
| E5 | Proposta rejeitada | agente replaneja; job completa |
| E6 | 50 jobs em lote | fila esvazia; nenhum perdido; nenhum duplicado |
| E7 | Slider travado + reexecução do agente | valor travado preservado |
| E8 | Version freeze ativo, prompt atualizado | render usa a versão congelada |
| E9 | Disco cheio durante escrita | `failed` com erro claro, sem corromper nada |
| E10 | Dois navegadores no mesmo job | ambos recebem os eventos |

---

## 7. Golden Master acústico

Ver `09-MLOPS-GOLDEN-MASTER.md`. Roda no CI a cada mudança em `prompts/`,
`crates/audio_core/src/dsp/` ou `crates/audio_agent/`.

Anexar os WAVs gerados como artifact do build, para o revisor ouvir sem
reproduzir localmente.

---

## 8. Pipeline de CI

```
Estágio 1  Lint e formato        cargo fmt --check · clippy -D warnings · eslint     ~2 min
Estágio 2  Unitário Rust         cargo test --lib · doctests                        ~5 min
Estágio 3  Property + fuzz curto proptest (1000 casos) · fuzz 60 s por alvo         ~5 min
Estágio 4  Integração            testcontainers (Postgres, MinIO) · wiremock       ~10 min
Estágio 5  UI                    vitest + msw                                       ~3 min
Estágio 6  Golden Master         Ollama seeded, se prompts/ ou dsp/ mudaram        ~8 min
Estágio 7  E2E                   Playwright, matriz 4×, paralelo                   ~15 min
Noturno    Fuzz longo · bench · E2E completo · cargo-audit · gitleaks              ~60 min
```

Matriz de SO: `ubuntu-latest`, `macos-14`, `windows-latest` para os estágios 1–3
(o núcleo DSP precisa rodar nos três — músicos usam os três). Estágios 4–7 só em
Linux.

> O CI do kit roda `apt-get install` em toda a matriz, o que quebra em macOS e
> Windows. Ver `14-AUDITORIA-KIT.md`.

---

## 9. Definição de Pronto (DoD)

Uma tarefa só fecha quando:

- [ ] Código compila sem warning (`clippy -D warnings`)
- [ ] `cargo fmt` / `prettier` aplicados
- [ ] Testes unitários da lógica nova
- [ ] Se toca DSP: invariante de propriedade adicionado
- [ ] Se toca contrato: `03-CONTRATOS-API.md` atualizado no mesmo PR
- [ ] Se toca contrato: tipos TS regenerados (`cargo test export_bindings`)
- [ ] Se toca limite de parâmetro: tabela canônica de `05-AGENTE-IA-HITL.md` §3 atualizada
- [ ] Se toca prompt: linter passa + Golden Master verde
- [ ] Se toca UI: estados de carregando, vazio e erro tratados
- [ ] Span de tracing na fronteira nova
- [ ] Erro novo registrado no catálogo de erros
- [ ] Ação sensível gera `audit_event`
- [ ] Sem `unwrap()` fora de teste e bootstrap
- [ ] Checklist de segurança (`08` §10) revisto quando aplicável

## 10. Metas de cobertura

| Módulo | Meta | Justificativa |
| --- | --- | --- |
| `audio_core::dsp` | ≥ 85% | É o produto |
| `audio_core::domain` | ≥ 90% | Validação de limites |
| `audio_agent::validator` | 100% | Barreira anti-alucinação |
| `audio_api::routes` | ≥ 70% | Muito código de transporte |
| `ui/` | ≥ 60% | E2E cobre o resto |

Cobertura é indicador, não meta em si. Um módulo com 95% e sem invariante de
propriedade está pior testado que um com 70% e o invariante certo.
