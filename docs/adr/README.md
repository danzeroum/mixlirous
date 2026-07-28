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
exige sticky session ou backplane (ver `../11-INFRA-DEPLOY.md` §6).

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

## ADR-0009 — Provedor LLM: abstração agnóstica, DeepSeek como padrão inicial

**Status:** aceito
**Data:** 2026-07-24
**Dono:** `<preencher>`

**Contexto.** O sistema precisa de um LLM para o loop ReAct. As opções eram
rodar local (Ollama), usar serviço externo, ou detectar e decidir no boot.

**Decisão.** Opção C — **detectar Ollama no boot; se não houver, usar o provedor
configurado; se não houver configuração, pedir a chave.** A camada de provedor é
agnóstica por contrato, e o padrão inicial do projeto é **DeepSeek**.

**Por quê.** Respeita quem já tem ambiente local, não bloqueia quem não tem, e
não obriga o usuário a escolher antes de entender a diferença. O agnosticismo já
estava decidido na ADR-0004 — esta ADR só nomeia o padrão de fábrica.

**Consequências.**

Nenhuma mudança arquitetural. O DeepSeek é compatível com o formato OpenAI por
configuração, então o adapter existente atende: muda `base_url`, `model` e a
chave. Nada é compilado dentro do binário.

Três armadilhas específicas do DeepSeek que precisam virar tarefa:

1. **Não fixe o nome do modelo no código.** Os nomes `deepseek-chat` e
   `deepseek-reasoner` estão sendo descontinuados — a data anunciada é
   **2026-07-24**, hoje. Eles correspondiam aos modos sem raciocínio e com
   raciocínio do `deepseek-v4-flash`. Os nomes atuais são `deepseek-v4-flash` e
   `deepseek-v4-pro`. O modelo é campo de configuração, sempre.

2. **Modo de raciocínio quebra o loop ReAct se mal implementado.** Com o modo
   de raciocínio ativo, turnos com chamada de ferramenta exigem que o
   `reasoning_content` seja preservado nas requisições seguintes, senão a API
   devolve 400. Nosso loop é multi-turno por definição (budget 5), então isso
   aparece já no segundo passo. Teste com um caso de duas ferramentas
   encadeadas, não com um só.

3. **Avaliar o modo estrito.** Existe um modo beta que força os argumentos da
   chamada de ferramenta a aderirem ao JSON Schema, ativado por uma base URL
   diferente e `strict: true` na definição da função. Vale medir: se reduzir a
   taxa de rejeição do nosso validador, compensa. O validador continua sendo a
   rede de segurança de qualquer forma — modo estrito não substitui a
   ADR-0004/validação.

Detalhe operacional: alguns clientes esperam `/v1` na base URL e outros não.
Não duplique `/v1/v1`.

**O custo do agnosticismo — pague explicitamente.**

Trocar de provedor muda o áudio de saída. Isso colide com a doutrina de version
freeze de `09-MLOPS-GOLDEN-MASTER.md`:

- O registro de version freeze passa a gravar **provedor + modelo + hash do
  prompt** como uma tripla, não só o hash do prompt.
- Os Golden Masters passam a ser **por provedor e modelo**. Trocar de provedor
  exige regerar e reescutar.
- Os 10 casos de teste A1–A10 de `05-AGENTE-IA-HITL.md` viram **suíte de
  conformidade de provedor**. Um provedor que não passa é marcado como não
  suportado — nunca aceito com degradação silenciosa.

Esse último ponto é a regra que importa. Um provedor que erra tool calling não
falha ruidosamente: ele propõe parâmetros ruins, o validador rejeita, e o usuário
vê um agente que "não entende" em vez de um erro de configuração.

**Privacidade.** `08-SEGURANCA-MULTITENANCY.md` já estabelece que no modo
assistido o prompt e os metadados vão ao LLM, e **o áudio nunca vai**. Com
provedor externo configurável, o aviso ao usuário precisa **nomear o provedor
ativo** e indicar que os dados saem da máquina — antes da primeira execução em
modo assistido, não escondido nos termos. Para usuários no Brasil, transferência
internacional de dados é item de LGPD e precisa estar declarada.

**Quando revisar.** Se a suíte A1–A10 reprovar o provedor padrão, ou se o custo
por render inviabilizar o modelo de negócio.

---

## ADR-0010 — Separação de stems: binário externo opcional

**Status:** aceito
**Data:** 2026-07-24
**Dono:** `<preencher>`

**Contexto.** `stem_separation` é a ferramenta mais desejada — o prompt de
exemplo pede comprimir só a bateria — e a mais cara. Não existe implementação
madura em Rust puro; o estado da arte (`demucs`) é PyTorch.

**Decisão.** Opção B — **detectar o binário externo no PATH e habilitar a
ferramenta quando presente.** Sem ele, a ferramenta aparece como indisponível
com motivo explícito.

**Por quê.** O contrato já tem o mecanismo: `GET /api/v1/tools` expõe
`available` e `unavailable_reason`. O desenvolvedor já usou exatamente esse
campo na Sprint 0, com `available: false` para `stem_separation`. O custo
restante é um adapter de subprocesso, não superfície de contrato nova.

**Alternativas descartadas.** Remover do MVP perde valor real. Embutir um modelo
ONNX é um projeto próprio — exportar, quantizar, validar qualidade, empacotar
~100 MB de pesos.

**Consequências.**

- O agente precisa saber que a ferramenta está indisponível **antes** de propor
  usá-la, senão propõe algo impossível e o usuário vê uma falha em vez de uma
  ausência. A lista de ferramentas disponíveis entra no contexto do prompt.
- A UI precisa de um estado visual para ferramenta indisponível, com o motivo
  legível e instrução de instalação. Isso é item para o designer.
- Subprocesso significa tempo limite, limite de memória e tratamento de saída
  corrompida. Um binário externo pode travar, e o job não pode travar junto.
- O orçamento de performance de `04-DOMINIO-DSP.md` (< 20 s sem LLM) **não vale**
  quando a separação está ativa. Precisa de orçamento próprio e de aviso na UI.
- Fica fora do binário único que entregamos. A promessa de instalação sem
  fricção continua valendo para tudo, menos esta ferramenta.

**Quando revisar.** Se surgir implementação em Rust com qualidade comparável, ou
se a demanda justificar o projeto de embutir ONNX.

---

## ADR-0011 — Propriedade intelectual: implementar de literatura, nunca traduzir código copyleft

**Status:** proposto — **não vale enquanto não tiver dono**
**Data:** 2026-07-27
**Dono:** `<preencher>`

> Esta ADR foi redigida a partir do texto já especificado em
> [`../16-CORRECOES-DSP.md`](../16-CORRECOES-DSP.md) §T0.1, que a exige com dono
> nomeado. **Enquanto o campo acima estiver em branco, ela não está aceita** — o
> ato que falta é uma pessoa assumir, não escrever. Ao preencher, trocar o status
> para `aceito`.

**Contexto.** O Mixlirous implementa DSP de áudio — limiters, crossfade,
medição de loudness, detecção de onset — para o qual existe implementação de
referência madura e legível no Audacity. A tentação de consultá-la é grande e o
custo de ceder é o projeto inteiro.

O Audacity é **GPL**: GPLv3 no projeto, GPLv2-or-later na maioria dos arquivos.
Copiar **ou traduzir** qualquer arquivo de `au3/` para Rust cria obra derivada e
propaga o copyleft para o Mixlirous inteiro, **incluindo o binário distribuído**.
Não é risco de processo distante: é a licença do produto mudando por causa de um
arquivo.

**Decisão.** Implementar DSP a partir de literatura publicada e de crates com
licença permissiva. Nunca a partir de código copyleft, nem por cópia nem por
tradução.

**Regras.**

- **Proibido:** abrir o repositório do Audacity — ou qualquer base copyleft —
  durante a implementação de um módulo de DSP equivalente. Ler e reescrever "com
  as próprias palavras" logo em seguida **conta como reprodução**. A separação
  precisa ser de fonte, não de redação.
- **Permitido:** implementar a partir de literatura publicada (DAFX/Zölzer, EBU
  TECH 3341/3342, ITU-R BS.1770, papers revisados) e usar crates com licença
  permissiva — MIT, BSD, Apache-2.0.
- **Obrigatório:** todo módulo de DSP novo cita a fonte no cabeçalho.

**Sobre as citações — a parte que mais falha.** A política inteira depende de as
referências serem verdadeiras. Um cabeçalho apontando para uma fonte que não diz
aquilo é **pior que cabeçalho nenhum**: é documentação falsa de proveniência, e
ela sobrevive anos porque parece rigor. **Confira cada citação contra um exemplar
antes do commit.**

Estado das citações que já circularam neste projeto:

| Referência | Estado | Nota |
|---|---|---|
| DAFX 2ª ed. §4.2 "Dynamic Range Control" | **conferida** | Cobre limiters e compressores com a arquitetura envelope follower → curva estática → filtro de suavização → multiplicador. É a referência correta para o T3.2. |
| "DAFX 2ª ed., Equal Power Crossfade, p. 46" | **não confirmada** | Não parece existir — a p. 46 cai no capítulo de filtros. Crossfade de potência constante é resultado elementar e pode ser citado de qualquer texto de processamento de sinais. **Não copie essa citação sem verificar.** |

A segunda linha é o exemplo de por que esta seção existe: a referência circulou
em documentos externos com aparência de verificada.

**Adoção de biblioteca externa.** Antes de avaliar qualquer biblioteca por
qualidade técnica, verifique a licença — a ordem inversa leva a escolher e
descobrir depois, quando já há código em cima. Isto vale imediatamente para a
[#36](https://github.com/danzeroum/mixlirous/issues/36) (estiramento temporal com
preservação de tom), onde as implementações mais maduras são copyleft ou de
licença dupla. **Nenhuma licença de biblioteca específica foi conferida contra a
fonte oficial nesta redação** — conferir é parte do trabalho da #36, sob a mesma
regra do quadro acima.

**Alternativas consideradas.**

- *Consultar o Audacity só para entender, sem copiar.* Descartada: é exatamente
  o caso que a regra do "com as próprias palavras" cobre, e a linha entre
  entender e reproduzir não é defensável depois do fato.
- *Licenciar o Mixlirous como GPL e resolver o problema por adesão.* Descartada
  aqui por ser decisão de produto, não de arquitetura — e ainda não tomada
  (`README.md`: licença a definir antes do primeiro release público). Se um dia
  for tomada, esta ADR é substituída, não contornada.

**Consequências.**

- Implementar de literatura é mais lento que traduzir código pronto. É custo
  aceito, não descuido de planejamento.
- Todo módulo novo carrega uma citação que alguém precisa ter conferido. O
  checklist de PR precisa cobrir isso.
- Ferramentas cujo estado da arte é copyleft — separação de stems, estiramento
  com preservação de tom — ou entram por binário externo invocado como
  subprocesso (ver ADR-0010), ou saem do escopo. Vincular como biblioteca é a
  opção que a licença fecha.

**Sobre a lacuna do `docs/15`.** A numeração dos documentos salta de 14 para 16.
Um `docs/15-AUDACITY-TRIAGEM.md` foi referido em conversa mas **nunca foi
commitado** — `git log --all --diff-filter=A` não retorna nada, em ref nenhuma.
Vale registrar que isso pode ser deliberado: um documento que **triage o
código-fonte do Audacity** é precisamente o tipo de artefato que esta política
proíbe produzir durante a implementação de módulo equivalente. A lacuna fica
explicada aqui em vez de ficar como vão sem motivo.

**Quando revisar.** Se a licença do Mixlirous for definida como copyleft, ou se
surgir implementação permissiva para alguma capacidade hoje bloqueada por
licença.

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
