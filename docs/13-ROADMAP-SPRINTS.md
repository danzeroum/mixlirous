# 13 — Roadmap e Execução

## 1. Premissas e leitura honesta do prazo

O escopo da V1 (canvas DAG + ReAct + SSE + persistência dual + resiliência +
MLOps + observabilidade) é substancial.

| Time | Prazo realista |
| --- | --- |
| 2 devs experientes (1 BE Rust, 1 FE) + designer meio período | 5 sprints × 1 semana |
| 1 dev full-stack experiente em Rust | 9–10 semanas |
| 1 dev aprendendo Rust | 14+ semanas — considerar backend em outra linguagem |

O plano abaixo assume o primeiro cenário. Se for o segundo, some as sprints
sequencialmente; a ordem não muda, porque as dependências são reais.

**Sprint 0 não é opcional.** O kit atual não compila. Tentar construir por cima
sem consertar a base gera uma semana de depuração confusa.

---

## 2. Ordem das sprints — e por que essa ordem

```
S0  Fundação           ← faz o kit compilar; sem isso nada anda
S1  Contratos + fila   ← todo o resto depende do contrato e da persistência
S2  Motor DSP          ← é o produto; precisa estar sólido antes da IA
S3  Agente + interface ← só faz sentido com o motor funcionando
S4  Resiliência + MLOps ← endurece o que já funciona
```

A tentação é começar pela interface, porque é visível. Não funciona aqui: sem o
motor, a interface só tem dados falsos para mostrar, e todo o trabalho de estado
(progresso, erro, recuperação) precisa ser refeito quando o backend chega.

---

## Sprint 0 — Fundação (3–5 dias)

**Objetivo:** `cargo build --workspace` e `npm run build` verdes, CI passando.

| # | Tarefa | Quem | Referência |
| --- | --- | --- | --- |
| 0.1 | Corrigir erros de compilação do Rust (~12) | BE | `14-AUDITORIA-KIT.md` §2 |
| 0.2 | Corrigir versões de crates (OTel, symphonia, realfft, minio) | BE | §2.2 |
| 0.3 | Trocar `minio` + `aws-sdk-s3` por `object_store` | BE | ADR-0006 |
| 0.4 | Mover testes de integração para dentro das crates | BE | §2.3 |
| 0.5 | Corrigir `ui/` (pacote `@xyflow/react`, imports, store) | FE | §3 |
| 0.6 | Grafana 3000 → 3001; separar compose de observabilidade | BE | §4 |
| 0.7 | Corrigir CI (matriz de SO, linter em Python) | BE | §5 |
| 0.8 | Remover segredos de `production.yaml` | BE | §4 |
| 0.9 | `rustfmt.toml`, `clippy.toml`, `.editorconfig`, `prettier` | BE+FE | — |
| 0.10 | Setup do GitHub (labels, milestones, templates, proteção) | Todos | §8 |
| 0.11 | Kickoff: leitura conjunta de `01-GLOSSARIO` e `03-CONTRATOS-API` | Todos | — |

**Aceite:** CI verde nos três SOs · `docker compose up` sem conflito de porta ·
`cargo run --bin audio_api` responde em `/healthz` · `npm run dev` abre a UI.

**Em paralelo (design):** começar os fluxos das 3 jornadas críticas.

---

## Sprint 1 — Contratos e persistência

**Objetivo:** o esqueleto de dados e a API existem e são confiáveis.

| # | Tarefa | Quem |
| --- | --- | --- |
| 1.1 | Newtypes com limites (`CrossfadeMs`, `CompressionRatio`, …) + testes | BE |
| 1.2 | `Parameter<T>` com `source` e regra de precedência | BE |
| 1.3 | `JobStatus` + máquina de estados com transições validadas | BE |
| 1.4 | Migrações SQLite e Postgres (schema completo do `06`) | BE |
| 1.5 | `AudioRepo` + adapter SQLite (WAL, PRAGMAs) | BE |
| 1.6 | Adapter Postgres + RLS + `with_tenant_scope` | BE |
| 1.7 | Fila: `claim_next_job` nos dois bancos + heartbeat | BE |
| 1.8 | Teste de concorrência: 10 workers, 1 job cada | BE |
| 1.9 | `Storage` trait + adapters local e MinIO | BE |
| 1.10 | Rotas: jobs (CRUD), tracks, tools, healthz, readyz, metrics | BE |
| 1.11 | Middleware: JWT, `TenantContext`, sessão local | BE |
| 1.12 | Hub SSE + endpoint de eventos + replay por `Last-Event-ID` | BE |
| 1.13 | Erros RFC 7807 + catálogo completo | BE |
| 1.14 | `ts-rs`: exportar tipos para `ui/src/types/` + verificação no CI | BE |
| 1.15 | Cliente de API + hook SSE no frontend (contra backend real) | FE |

**Aceite:** criar job via `curl` → aparece no banco → SSE emite `job.state` ·
teste de concorrência verde em SQLite e Postgres · tipos TS gerados e commitados
· `03-CONTRATOS-API.md` reflete o código.

**Design (paralelo):** entregáveis da Fase 1 (fluxos, anatomia do nó, overlay,
tokens).

---

## Sprint 2 — Motor DSP

**Objetivo:** um WAV de entrada vira um WAV de saída, com qualidade auditável.

| # | Tarefa | Quem |
| --- | --- | --- |
| 2.1 | `decode_to_pcm` com symphonia + validação de magic bytes | BE |
| 2.2 | Onset strength com flux espectral (upgrade do RMS) | BE |
| 2.3 | Beat tracking com programação dinâmica + confiança de BPM | BE |
| 2.4 | Filtro de batidas fortes (percentil com interpolação) | BE |
| 2.5 | Construção de blocos + energia + centroide | BE |
| 2.6 | Chroma + matriz de similaridade + detecção de seções | BE |
| 2.7 | Knapsack (exato ≤500 blocos, guloso acima) + restrições | BE |
| 2.8 | Modo contínuo (melhor janela) | BE |
| 2.9 | Zero-crossing + curvas de fade + crossfade | BE |
| 2.10 | `AudioStitchingPolicy` (diferença de dBFS → duração + warning) | BE |
| 2.11 | Compressor com detector RMS e envelope | BE |
| 2.12 | Limiter com lookahead | BE |
| 2.13 | Normalização LUFS com `ebur128` | BE |
| 2.14 | Time-stretch com `rubato` (fator limitado) | BE |
| 2.15 | Escrita atômica (`.tmp` → fsync → rename → fsync dir) | BE |
| 2.16 | Loop do worker: consome fila, executa, persiste, emite SSE | BE |
| 2.17 | Invariantes I1–I13 com `proptest` | BE |
| 2.18 | Benchmarks `criterion` + baseline no CI | BE |
| 2.19 | Endpoint de picos da waveform | BE |

**Aceite:** `POST /jobs` em modo `manual` produz WAV de 30 s ± 2 s a partir de
uma faixa de 5 min · zero estalos (invariante I4 verde) · zero clipping (I1, I2)
· LUFS dentro de ±0,5 do alvo · pipeline completo em < 20 s no hardware de
referência · **validação humana: 5 renders ouvidos e aprovados**.

O último item não é formalidade. Todos os invariantes podem passar e o resultado
soar ruim porque a heurística de seleção escolhe blocos que não combinam. Isso só
o ouvido pega.

**Frontend (paralelo):** canvas com React Flow, tipos de nó, store Zustand,
painel de propriedades com controles genéricos.

---

## Sprint 3 — Agente e interface

**Objetivo:** o prompt em linguagem natural vira um render, com o usuário no
controle.

| # | Tarefa | Quem |
| --- | --- | --- |
| 3.1 | Trait `LlmProvider` + adapters OpenAI e Ollama | BE |
| 3.2 | Loader de `.prompt` + templating com `minijinja` | BE |
| 3.3 | Loop ReAct com budget, retry e consolidação | BE |
| 3.4 | `ValidationLayer`: limites + regras cruzadas R1–R7 | BE |
| 3.5 | Erro de validação como observação estruturada para o modelo | BE |
| 3.6 | `prompt_guard` (anti-injection) | BE |
| 3.7 | Ciclo de propostas: criar, TTL, aprovar, rejeitar, expirar | BE |
| 3.8 | Replanejamento após rejeição | BE |
| 3.9 | Streaming de raciocínio via SSE (com `delta`) | BE |
| 3.10 | Fallback quando o LLM está indisponível | BE |
| 3.11 | Cenários A1–A10 com `MockLlm` | BE |
| 3.12 | Painel de raciocínio com streaming estável | FE |
| 3.13 | Overlay de proposta com todas as variações | FE |
| 3.14 | Sliders que se preenchem via SSE, respeitando travas | FE |
| 3.15 | Trava manual (`PATCH` de parâmetro) com feedback | FE |
| 3.16 | Tela de resultado: player, waveform, marcadores, A/B | FE |
| 3.17 | Biblioteca de faixas + upload com presign | FE |
| 3.18 | Testes de UI U1–U10 | FE |

**Aceite:** fluxo completo do E2E `E1` verde · aprovar proposta materializa o nó
· rejeitar faz o agente replanejar e concluir · valor travado sobrevive a nova
execução do agente · LLM fora do ar não impede render.

---

## Sprint 4 — Resiliência, observabilidade e MLOps

**Objetivo:** o sistema aguenta o mundo real e não muda de som sozinho.

| # | Tarefa | Quem |
| --- | --- | --- |
| 4.1 | Recovery loop completo com lock nos dois bancos | BE |
| 4.2 | Detecção de job travado por heartbeat | BE |
| 4.3 | Rotina de limpeza (tmp, eventos, retenção) | BE |
| 4.4 | Testes de injeção de falha R1–R7 | BE |
| 4.5 | OpenTelemetry: spans, propagação, Rayon | BE |
| 4.6 | Métricas Prometheus (negócio, LLM, DSP, infra) | BE |
| 4.7 | Bundle local de observabilidade + 4 dashboards | BE |
| 4.8 | Instrumentação do frontend + `trace_id` visível | FE |
| 4.9 | MFCC real (substituir o placeholder) | BE |
| 4.10 | Normalização das componentes da distância de fingerprint | BE |
| 4.11 | Fixtures de Golden Master (4 faixas originais) | BE |
| 4.12 | Testes de Golden Master no CI com Ollama seeded | BE |
| 4.13 | Linter de prompt completo (6 verificações) | BE |
| 4.14 | Version freeze: backend + UI | BE+FE |
| 4.15 | Botão de escala via `bollard` + fallback para threads | BE |
| 4.16 | Painel de recursos + banner de recuperação | FE |
| 4.17 | E2E de caos E2, E3, E6, E9 | Todos |
| 4.18 | `audit_events` em todas as ações sensíveis | BE |

**Aceite:** `SIGKILL` no meio de um render → reinício → job resolvido
corretamente · investigação forense por `trace_id` funciona ponta a ponta em
staging · Golden Master detecta uma mudança deliberada de prompt · lote de 50
jobs sem perda nem duplicação.

---

## Sprint 5 — Empacotamento e lançamento (opcional para uso interno)

| # | Tarefa |
| --- | --- |
| 5.1 | Frontend embutido no binário (`rust-embed`) |
| 5.2 | Primeiro boot: setup automático + abrir navegador |
| 5.3 | Builds cruzados com `cargo-dist`, assinatura macOS/Windows |
| 5.4 | Onboarding de primeiro uso |
| 5.5 | Documentação de usuário (instalação, primeiro remix, solução de problemas) |
| 5.6 | Aviso de privacidade sobre o que vai ao provedor LLM |
| 5.7 | Teste com 3 usuários reais das personas P1/P2 |

---

## 3. Marcos

| Marco | Sprint | Prova |
| --- | --- | --- |
| **M0 — Compila** | S0 | CI verde |
| **M1 — Dados fluem** | S1 | Job criado → fila → SSE |
| **M2 — Faz áudio** | S2 | WAV aprovado por ouvido humano |
| **M3 — Entende linguagem** | S3 | Prompt → render, com HITL |
| **M4 — Aguenta o mundo** | S4 | Sobrevive a `SIGKILL` e a troca de modelo |
| **M5 — Instalável** | S5 | Alguém de fora instala e usa sozinho |

---

## 4. Riscos

| Risco | Prob. | Impacto | Mitigação |
| --- | --- | --- | --- |
| Beat tracking ruim em material real (jam com tempo instável) | **Alta** | **Alto** | Testar cedo com áudio real; modo contínuo como alternativa; expor confiança de BPM |
| Qualidade sonora das emendas insatisfatória | Média | Alto | Invariante de continuidade + validação humana obrigatória na S2 |
| Curva de aprendizado de Rust | Média | Alto | Mob programming na S1; revisar toda fronteira `async`/Rayon |
| Separação de stems sem solução em Rust | **Alta** | Médio | ADR-0010 — adiar ou usar binário externo |
| Custo de LLM em lote de 200 faixas | Média | Médio | Modo `manual` para lote; Ollama local como padrão |
| Escopo cresce durante a execução | **Alta** | Alto | Lista de adiáveis em `00-VISAO-ESCOPO` §5; nada entra sem sair |
| Design atrasa e trava a S3 | Média | Médio | Entregáveis da Fase 1 antes do fim da S2 |
| Golden Master com fixture de música comercial | Baixa | Alto | Só áudio original ou CC0 — regra explícita |

---

## 5. Cadência

- **Diária:** 15 min, foco em bloqueios
- **Fim de sprint:** demo com áudio tocando de verdade, não slide
- **Revisão de contrato:** toda mudança em `03-CONTRATOS-API.md` é anunciada
- **Sessão de escuta:** a cada sprint a partir da S2, o time ouve 5 renders

A sessão de escuta é o ritual mais importante deste projeto. Métrica não diz se
está bom; ouvido diz.

---

## 6. O que fazer se atrasar

Ordem de corte, do primeiro ao último:

1. Piloto automático de escala → só slider manual
2. Separação de stems → remover do registry
3. Canary e feature flags por tenant → só version freeze
4. Detecção de seções (chroma) → só energia e onset
5. Modo contínuo → só colagem por blocos
6. Dashboards de observabilidade → métricas cruas bastam

**Não cortar, em hipótese alguma:** validação de limites, escrita atômica,
recovery loop, invariantes de DSP, ciclo de propostas. Cortar qualquer um desses
gera um produto que perde trabalho do usuário ou publica áudio ruim — e não há
como recuperar a confiança depois.

---

## 7. Setup do GitHub

### Branches

```
main       protegida; só merge via PR com CI verde e 1 aprovação
feat/*     funcionalidade
fix/*      correção
chore/*    infra, docs, dependências
```

### Commits (Conventional Commits)

```
feat(dsp): implementa crossfade com curva logarítmica
fix(api): corrige escopo de tenant em list_jobs
docs(contratos): adiciona evento agent.proposal
test(dsp): invariante de continuidade na emenda
chore(deps): atualiza opentelemetry para 0.27
```

### Labels

```
área:      area/dsp · area/agent · area/api · area/ui · area/infra · area/docs
tipo:      type/feat · type/fix · type/test · type/chore · type/spike
prioridade: prio/p0 · prio/p1 · prio/p2
pilar:     pillar/validation · pillar/atomicity · pillar/tracing · pillar/queue
estado:    status/blocked · status/needs-design · status/needs-review
```

As labels `pillar/*` marcam os quatro pilares de confiabilidade — permitem
filtrar rapidamente o que não pode ser cortado.

### Milestones

`S0 Fundação` · `S1 Contratos` · `S2 Motor DSP` · `S3 Agente e UI` ·
`S4 Resiliência` · `S5 Lançamento`

### Template de PR

```markdown
## O que muda

## Por quê

## Como testar

## Checklist
- [ ] `cargo clippy -- -D warnings` limpo
- [ ] `cargo fmt` / `prettier` aplicados
- [ ] Testes cobrem a lógica nova
- [ ] Se toca DSP: invariante adicionado
- [ ] Se toca contrato: `docs/03-CONTRATOS-API.md` atualizado + tipos TS gerados
- [ ] Se toca limite: tabela canônica de `docs/05` §3 atualizada
- [ ] Se toca prompt: linter verde + Golden Master ouvido
- [ ] Sem `unwrap()` fora de teste
- [ ] Checklist de segurança (`docs/08` §10) quando aplicável
```

### Backlog inicial

```bash
bash backlog/import-issues.sh    # cria labels, milestones e ~60 issues
```
