# Registro de Decisões Arquiteturais (ADRs)

Decisões já tomadas, com a alternativa descartada e o motivo. Servem para não
reabrir discussão fechada — e para reabrir com conhecimento quando o contexto
mudar.

Formato: **Contexto · Decisão · Consequências · Quando revisar**.

Status: `aceito` · `proposto` · `substituído`

---

## ADR-0001 — Rust para backend e DSP

**Status:** aceito

**Contexto.** O material original está em Python (`librosa`, `pydub`, `numpy`).
O sistema precisa processar 200+ faixas com paralelismo real, em máquina modesta.

**Decisão.** Backend e motor DSP em Rust; Python fica só em scripts auxiliares.

**Por quê.** O gargalo não é a FFT isolada — `librosa` já chama C por baixo. O
gargalo é orquestrar dezenas de faixas simultâneas: o GIL obriga a
`multiprocessing`, e cada processo carrega um interpretador e cópias de buffers
de áudio, que são grandes. Rust dá threads reais com custo de memória baixo,
liberação determinística (sem picos de GC no meio de um render) e ausência de
pausas imprevisíveis.

**Consequências.** Ciclo de desenvolvimento mais lento no início; ecossistema de
IA mais pobre (mitigado pelo ADR-0004); binário único simplifica distribuição
para o usuário final — o que é decisivo para a persona do laptop.

**Revisar se:** o time não conseguir produtividade em Rust após 3 sprints.

---

## ADR-0002 — SSE em vez de WebSocket

**Status:** aceito

**Contexto.** A UI precisa de raciocínio do agente, progresso e propostas em
tempo real.

**Decisão.** Server-Sent Events para servidor→cliente; REST para comandos.

**Por quê.** O tráfego é 99% unidirecional. `EventSource` traz reconexão e
retomada por `Last-Event-ID` de graça; WebSocket exigiria heartbeat, backoff e
biblioteca de cliente. SSE é HTTP puro e atravessa proxy sem `Upgrade`. As ações
do usuário são pontuais e se beneficiam de serem `POST` idempotentes e
auditáveis.

**Consequências.** Comando e telemetria em canais separados — precisa de
`Last-Event-ID` bem implementado. Com múltiplas réplicas de API, o hub in-memory
exige sticky session ou backplane (ver `11-INFRA-DEPLOY.md` §6).

**Revisar se:** houver edição colaborativa em tempo real ou controle de
reprodução com scrub.

---

## ADR-0003 — Persistência dual: SQLite e PostgreSQL

**Status:** aceito

**Contexto.** O MVP roda no laptop de um músico. A mesma base precisa virar SaaS.

**Decisão.** Um trait `AudioRepo` com dois adapters, escolha em runtime por
`DATABASE_URL`.

**Por quê.** Exigir Postgres no laptop mata a adoção da persona primária. Fazer
só SQLite trava o SaaS. O custo de manter dois adapters é baixo porque o SQL é
quase idêntico; a divergência real está na fila (`SKIP LOCKED` vs
`BEGIN IMMEDIATE`) e no isolamento (RLS vs filtro no adapter).

**Consequências.** Todo teste de integração roda duas vezes. Migrações em pastas
separadas. Ganho: zero fricção de instalação.

---

## ADR-0004 — Sem SDK de LLM; HTTP direto com abstração própria

**Status:** aceito

**Decisão.** Trait `LlmProvider` implementada com `reqwest` + `serde`, seguindo
o formato de tool calling compatível com OpenAI.

**Por quê.** Os SDKs em Rust são não-oficiais e ficam atrás dos provedores. O
formato REST é estável e quase universal (OpenAI, Anthropic, Ollama, vLLM). Com
a abstração própria, trocar de provedor é mudar base URL e modelo — não mudar
dependência. Prompts ficam em arquivo externo, então ajustá-los não recompila
nada.

**Consequências.** Manter o cliente HTTP na mão; recursos exclusivos de um
provedor exigem trabalho extra.

---

## ADR-0005 — Fila no banco, não em broker

**Status:** aceito

**Contexto.** O desenho inicial previa RabbitMQ com DLQ e prioridades.

**Decisão.** Fila em tabela do banco. `FOR UPDATE SKIP LOCKED` no Postgres,
transação `IMMEDIATE` no SQLite.

**Por quê.** RabbitMQ é mais um serviço para o usuário instalar, mais um ponto
de falha, mais um estado a reconciliar no recovery. Para o volume do MVP
(centenas a milhares de jobs), Postgres como fila é sobra de capacidade e ganha
em transacionalidade: mudar o estado do job e enfileirar o próximo passo
acontecem na mesma transação, o que elimina uma classe inteira de inconsistência.

**Consequências.** Sem fan-out nativo e sem DLQ pronta — implementamos retry e
`max_attempts` na tabela.

**Revisar se:** a fila passar de ~50 mil linhas ativas, a latência de
reivindicação passar de 100 ms, ou houver necessidade de múltiplos consumidores
distintos.

---

## ADR-0006 — `object_store` em vez de SDKs separados

**Status:** aceito

**Contexto.** O kit traz `minio = "0.10"` e `aws-sdk-s3 = "1.0"` juntos.

**Decisão.** Um trait `Storage` implementado sobre a crate `object_store`.

**Por quê.** Duas SDKs pesadas para a mesma tarefa inflam tempo de build e
superfície de dependência. `object_store` cobre local, S3, GCS e Azure com uma
API só, é mantida ativamente pelo projeto Arrow e resolve exatamente o caso de
"mesmo código no disco local e no bucket".

**Consequências.** Recursos exóticos de S3 (lifecycle, tiering) ficam fora da
abstração — configurados no bucket, não no código.

---

## ADR-0007 — `debian-slim` em vez de musl + distroless (por ora)

**Status:** aceito

**Contexto.** O desenho inicial propunha `x86_64-unknown-linux-musl` com imagem
distroless, por tamanho e superfície de ataque.

**Decisão.** `debian-bookworm-slim` com glibc no MVP.

**Por quê.** O alocador padrão do musl tem desempenho fraco em cargas
multithread com alocação intensa — que é exatamente o perfil de DSP com Rayon.
Trocar 30 MB de imagem por perda mensurável de throughput não compensa nesta
fase. A diferença de superfície de ataque importa quando houver exposição
pública real.

**Consequências.** Imagem maior. Se for para musl depois, configurar `mimalloc`
ou `jemalloc` explicitamente e medir antes/depois.

**Revisar:** ao entrar em produção com exposição pública.

---

## ADR-0008 — Sandbox por `securityContext`, não por `unshare()` no código

**Status:** aceito

**Contexto.** Foi proposto chamar `seccomp`, `chroot` e `unshare(CLONE_NEWNS)`
dentro do worker.

**Decisão.** Delegar restrição de syscalls ao runtime de container
(`securityContext.seccompProfile`, `readOnlyRootFilesystem`,
`capabilities: drop ALL`). No código, manter validação de formato, limites de
recurso e `catch_unwind`.

**Por quê.** Em Kubernetes, o runtime já aplica AppArmor/seccomp e bloqueia
escalonamento de privilégio; autoisolamento em processo costuma colidir com
isso e gerar falha por falta de permissão. Fora de container (laptop), esse tipo
de isolamento não se aplica de qualquer forma.

**Consequências.** No modo local, a defesa é validação de entrada + limites +
timeout. Aceitável: o arquivo é do próprio usuário.

---

## ADR-0009 — Provedor LLM padrão do MVP

**Status:** **proposto** — precisa de decisão antes da Sprint 2

**Contexto.** Duas opções para o padrão de fábrica:

| | Ollama local | OpenAI/Anthropic |
| --- | --- | --- |
| Custo | zero | por token |
| Privacidade | total | prompt e metadados saem |
| Instalação | usuário instala Ollama + baixa modelo (~5 GB) | só uma chave de API |
| Qualidade de tool calling | boa em modelos 8B+, inferior | melhor |
| Funciona offline | sim | não |

**Opções.**

- **A** — Ollama padrão, externo opcional. Alinha com a promessa de privacidade
  (persona P3) e custo zero, mas adiciona um passo pesado de instalação.
- **B** — Externo padrão, Ollama opcional. Onboarding mais fácil, mas exige
  cartão e envia dados.
- **C** — Detectar Ollama no boot; se existir, usar; senão, pedir a chave.

**Recomendação:** **C**. Respeita quem já tem ambiente local, não bloqueia quem
não tem, e evita fazer o usuário escolher antes de entender a diferença. Custa
uma tela a mais de onboarding.

**Decisão pendente. Dono: —**

---

## ADR-0010 — Separação de stems no MVP

**Status:** **proposto** — precisa de decisão antes da Sprint 3

**Contexto.** `stem_separation` é a ferramenta mais desejada e a mais cara. Não
existe implementação madura em Rust puro; o estado da arte (`demucs`) é PyTorch.

**Opções.**

- **A — Remover do MVP.** Simples, honesto. Perde valor real: comprimir só a
  bateria é justamente o que o prompt exemplo pede.
- **B — Binário externo opcional.** O sistema detecta `demucs` no PATH e habilita
  a ferramenta. Sem ele, a ferramenta aparece como indisponível. Custo baixo,
  valor alto para quem já tem.
- **C — ONNX Runtime embutido.** Rodar um modelo exportado via `ort`. Viável,
  mas é um projeto próprio: exportar, quantizar, validar qualidade, empacotar
  ~100 MB de pesos.

**Recomendação:** **B** no MVP, **C** depois se houver demanda. A separação
entre "ferramenta registrada" e "ferramenta disponível" já existe no contrato
(`GET /api/v1/tools` tem `available` e `unavailable_reason`), então B custa
pouco além do adapter de subprocesso.

**Decisão pendente. Dono: —**

---

## Modelo para novas ADRs

```markdown
## ADR-XXXX — Título

**Status:** proposto | aceito | substituído por ADR-YYYY
**Data:** AAAA-MM-DD
**Dono:** nome

**Contexto.** Qual problema, quais forças em jogo.

**Decisão.** O que foi decidido, em uma frase.

**Alternativas consideradas.** O que foi descartado e por quê.

**Consequências.** O que fica mais fácil, o que fica mais difícil.

**Quando revisar.** Gatilho concreto e observável.
```
