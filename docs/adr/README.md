# Registro de Decisoes Arquiteturais (ADRs)

Decisoes ja tomadas, com a alternativa descartada e o motivo. Servem para nao
reabrir discussao fechada -- e para reabrir com conhecimento quando o contexto
mudar.

Formato: **Contexto · Decisao · Consequencias · Quando revisar**.

Status: `aceito` · `proposto` · `substituido`

---

## ADR-0001 -- Rust para backend e DSP

**Status:** aceito

**Contexto.** O material original esta em Python (`librosa`, `pydub`, `numpy`).
O sistema precisa processar 200+ faixas com paralelismo real, em maquina modesta.

**Decisao.** Backend e motor DSP em Rust; Python fica so em scripts auxiliares.

**Por que.** O gargalo nao e a FFT isolada -- `librosa` ja chama C por baixo. O
gargalo e orquestrar dezenas de faixas simultaneas: o GIL obriga a
`multiprocessing`, e cada processo carrega um interpretador e copias de buffers
de audio, que sao grandes. Rust da threads reais com custo de memoria baixo,
liberacao deterministica (sem picos de GC no meio de um render) e ausencia de
pausas imprevisiveis.

**Consequencias.** Ciclo de desenvolvimento mais lento no inicio; ecossistema de
IA mais pobre (mitigado pelo ADR-0004); binario unico simplifica distribuicao
para o usuario final -- o que e decisivo para a persona do laptop.

**Revisar se:** o time nao conseguir produtividade em Rust apos 3 sprints.

---

## ADR-0002 -- SSE em vez de WebSocket

**Status:** aceito

**Contexto.** A UI precisa de raciocinio do agente, progresso e propostas em
tempo real.

**Decisao.** Server-Sent Events para servidor→cliente; REST para comandos.

**Por que.** O trafego e 99% unidirecional. `EventSource` traz reconexao e
retomada por `Last-Event-ID` de graca; WebSocket exigiria heartbeat, backoff e
biblioteca de cliente. SSE e HTTP puro e atravessa proxy sem `Upgrade`. As acoes
do usuario sao pontuais e se beneficiam de serem `POST` idempotentes e
auditaveis.

**Consequencias.** Comando e telemetria em canais separados -- precisa de
`Last-Event-ID` bem implementado. Com multiplas replicas de API, o hub in-memory
exige sticky session ou backplane (ver `../11-INFRA-DEPLOY.md` §6).

**Revisar se:** houver edicao colaborativa em tempo real ou controle de
reproducao com scrub.

---

## ADR-0003 -- Persistencia dual: SQLite e PostgreSQL

**Status:** aceito

**Contexto.** O MVP roda no laptop de um musico. A mesma base precisa virar SaaS.

**Decisao.** Um trait `AudioRepo` com dois adapters, escolha em runtime por
`DATABASE_URL`.

**Por que.** Exigir Postgres no laptop mata a adocao da persona primaria. Fazer
so SQLite trava o SaaS. O custo de manter dois adapters e baixo porque o SQL e
quase identico; a divergencia real esta na fila (`SKIP LOCKED` vs
`BEGIN IMMEDIATE`) e no isolamento (RLS vs filtro no adapter).

**Consequencias.** Todo teste de integracao roda duas vezes. Migracoes em pastas
separadas. Ganho: zero friccao de instalacao.

---

## ADR-0004 -- Sem SDK de LLM; HTTP direto com abstracao propria

**Status:** aceito

**Decisao.** Trait `LlmProvider` implementada com `reqwest` + `serde`, seguindo
o formato de tool calling compativel com OpenAI.

**Por que.** Os SDKs em Rust sao nao-oficiais e ficam atras dos provedores. O
formato REST e estavel e quase universal (OpenAI, Anthropic, Ollama, vLLM). Com
a abstracao propria, trocar de provedor e mudar base URL e modelo -- nao mudar
dependencia. Prompts ficam em arquivo externo, entao ajusta-los nao recompila
nada.

**Consequencias.** Manter o cliente HTTP na mao; recursos exclusivos de um
provedor exigem trabalho extra.

---

## ADR-0005 -- Fila no banco, nao em broker

**Status:** aceito

**Contexto.** O desenho inicial previa RabbitMQ com DLQ e prioridades.

**Decisao.** Fila em tabela do banco. `FOR UPDATE SKIP LOCKED` no Postgres,
transacao `IMMEDIATE` no SQLite.

**Por que.** RabbitMQ e mais um servico para o usuario instalar, mais um ponto
de falha, mais um estado a reconciliar no recovery. Para o volume do MVP
(centenas a milhares de jobs), Postgres como fila e sobra de capacidade e ganha
em transacionalidade: mudar o estado do job e enfileirar o proximo passo
acontecem na mesma transacao, o que elimina uma classe inteira de inconsistencia.

**Consequencias.** Sem fan-out nativo e sem DLQ pronta -- implementamos retry e
`max_attempts` na tabela.

**Revisar se:** a fila passar de ~50 mil linhas ativas, a latencia de
reivindicacao passar de 100 ms, ou houver necessidade de multiplos consumidores
distintos.

---

## ADR-0006 -- `object_store` em vez de SDKs separados

**Status:** aceito

**Contexto.** O kit traz `minio = "0.10"` e `aws-sdk-s3 = "1.0"` juntos.

**Decisao.** Um trait `Storage` implementado sobre a crate `object_store`.

**Por que.** Duas SDKs pesadas para a mesma tarefa inflam tempo de build e
superficie de dependencia. `object_store` cobre local, S3, GCS e Azure com uma
API so, e mantida ativamente pelo projeto Arrow e resolve exatamente o caso de
"mesmo codigo no disco local e no bucket".

**Consequencias.** Recursos exoticos de S3 (lifecycle, tiering) ficam fora da
abstracao -- configurados no bucket, nao no codigo.

---

## ADR-0007 -- `debian-slim` em vez de musl + distroless (por ora)

**Status:** aceito

**Contexto.** O desenho inicial propunha `x86_64-unknown-linux-musl` com imagem
distroless, por tamanho e superficie de ataque.

**Decisao.** `debian-bookworm-slim` com glibc no MVP.

**Por que.** O alocador padrao do musl tem desempenho fraco em cargas
multithread com alocacao intensa -- que e exatamente o perfil de DSP com Rayon.
Trocar 30 MB de imagem por perda mensuravel de throughput nao compensa nesta
fase. A diferenca de superficie de ataque importa quando houver exposicao
publica real.

**Consequencias.** Imagem maior. Se for para musl depois, configurar `mimalloc`
ou `jemalloc` explicitamente e medir antes/depois.

**Revisar:** ao entrar em producao com exposicao publica.

---

## ADR-0008 -- Sandbox por `securityContext`, nao por `unshare()` no codigo

**Status:** aceito

**Contexto.** Foi proposto chamar `seccomp`, `chroot` e `unshare(CLONE_NEWNS)`
dentro do worker.

**Decisao.** Delegar restricao de syscalls ao runtime de container
(`securityContext.seccompProfile`, `readOnlyRootFilesystem`,
`capabilities: drop ALL`). No codigo, manter validacao de formato, limites de
recurso e `catch_unwind`.

**Por que.** Em Kubernetes, o runtime ja aplica AppArmor/seccomp e bloqueia
escalonamento de privilegio; autoisolamento em processo costuma colidir com
isso e gerar falha por falta de permissao. Fora de container (laptop), esse tipo
de isolamento nao se aplica de qualquer forma.

**Consequencias.** No modo local, a defesa e validacao de entrada + limites +
timeout. Aceitavel: o arquivo e do proprio usuario.

---

## ADR-0009 -- Provedor LLM: abstracao agnostica, DeepSeek como padrao inicial

**Status:** aceito
**Data:** 2026-07-24
**Dono:** `<preencher>`

**Contexto.** O sistema precisa de um LLM para o loop ReAct. As opcoes eram
rodar local (Ollama), usar servico externo, ou detectar e decidir no boot.

**Decisao.** Opcao C -- **detectar Ollama no boot; se nao houver, usar o provedor
configurado; se nao houver configuracao, pedir a chave.** A camada de provedor e
agnostica por contrato, e o padrao inicial do projeto e **DeepSeek**.

**Por que.** Respeita quem ja tem ambiente local, nao bloqueia quem nao tem, e
nao obriga o usuario a escolher antes de entender a diferenca. O agnosticismo ja
estava decidido na ADR-0004 -- esta ADR so nomeia o padrao de fabrica.

**Consequencias.**

Nenhuma mudanca arquitetural. O DeepSeek e compativel com o formato OpenAI por
configuracao, entao o adapter existente atende: muda `base_url`, `model` e a
chave. Nada e compilado dentro do binario.

Tres armadilhas especificas do DeepSeek que precisam virar tarefa:

1. **Nao fixe o nome do modelo no codigo.** Os nomes `deepseek-chat` e
   `deepseek-reasoner` estao sendo descontinuados -- a data anunciada e
   **2026-07-24**, hoje. Eles correspondiam aos modos sem raciocinio e com
   raciocinio do `deepseek-v4-flash`. Os nomes atuais sao `deepseek-v4-flash` e
   `deepseek-v4-pro`. O modelo e campo de configuracao, sempre.

2. **Modo de raciocinio quebra o loop ReAct se mal implementado.** Com o modo
   de raciocinio ativo, turnos com chamada de ferramenta exigem que o
   `reasoning_content` seja preservado nas requisicoes seguintes, senao a API
   devolve 400. Nosso loop e multi-turno por definicao (budget 5), entao isso
   aparece ja no segundo passo. Teste com um caso de duas ferramentas
   encadeadas, nao com um so.

3. **Avaliar o modo estrito.** Existe um modo beta que forca os argumentos da
   chamada de ferramenta a aderirem ao JSON Schema, ativado por uma base URL
   diferente e `strict: true` na definicao da funcao. Vale medir: se reduzir a
   taxa de rejeicao do nosso validador, compensa. O validador continua sendo a
   rede de seguranca de qualquer forma -- modo estrito nao substitui a
   ADR-0004/validacao.

Detalhe operacional: alguns clientes esperam `/v1` na base URL e outros nao.
Nao duplique `/v1/v1`.

**O custo do agnosticismo -- pague explicitamente.**

Trocar de provedor muda o audio de saida. Isso colide com a doutrina de version
freeze de `09-MLOPS-GOLDEN-MASTER.md`:

- O registro de version freeze passa a gravar **provedor + modelo + hash do
  prompt** como uma tripla, nao so o hash do prompt.
- Os Golden Masters passam a ser **por provedor e modelo**. Trocar de provedor
  exige regerar e reescutar.
- Os 10 casos de teste A1–A10 de `05-AGENTE-IA-HITL.md` viram **suite de
  conformidade de provedor**. Um provedor que nao passa e marcado como nao
  suportado -- nunca aceito com degradacao silenciosa.

Esse ultimo ponto e a regra que importa. Um provedor que erra tool calling nao
falha ruidosamente: ele propoe parametros ruins, o validador rejeita, e o usuario
ve um agente que "nao entende" em vez de um erro de configuracao.

**Privacidade.** `08-SEGURANCA-MULTITENANCY.md` ja estabelece que no modo
assistido o prompt e os metadados vao ao LLM, e **o audio nunca vai**. Com
provedor externo configuravel, o aviso ao usuario precisa **nomear o provedor
ativo** e indicar que os dados saem da maquina -- antes da primeira execucao em
modo assistido, nao escondido nos termos. Para usuarios no Brasil, transferencia
internacional de dados e item de LGPD e precisa estar declarada.

**Quando revisar.** Se a suite A1–A10 reprovar o provedor padrao, ou se o custo
por render inviabilizar o modelo de negocio.

---

## ADR-0010 -- Separacao de stems: binario externo opcional

**Status:** aceito
**Data:** 2026-07-24
**Dono:** `<preencher>`

**Contexto.** `stem_separation` e a ferramenta mais desejada -- o prompt de
exemplo pede comprimir so a bateria -- e a mais cara. Nao existe implementacao
madura em Rust puro; o estado da arte (`demucs`) e PyTorch.

**Decisao.** Opcao B -- **detectar o binario externo no PATH e habilitar a
ferramenta quando presente.** Sem ele, a ferramenta aparece como indisponivel
com motivo explicito.

**Por que.** O contrato ja tem o mecanismo: `GET /api/v1/tools` expoe
`available` e `unavailable_reason`. O desenvolvedor ja usou exatamente esse
campo na Sprint 0, com `available: false` para `stem_separation`. O custo
restante e um adapter de subprocesso, nao superficie de contrato nova.

**Alternativas descartadas.** Remover do MVP perde valor real. Embutir um modelo
ONNX e um projeto proprio -- exportar, quantizar, validar qualidade, empacotar
~100 MB de pesos.

**Consequencias.**

- O agente precisa saber que a ferramenta esta indisponivel **antes** de propor
  usa-la, senao propoe algo impossivel e o usuario ve uma falha em vez de uma
  ausencia. A lista de ferramentas disponiveis entra no contexto do prompt.
- A UI precisa de um estado visual para ferramenta indisponivel, com o motivo
  legivel e instrucao de instalacao. Isso e item para o designer.
- Subprocesso significa tempo limite, limite de memoria e tratamento de saida
  corrompida. Um binario externo pode travar, e o job nao pode travar junto.
- O orcamento de performance de `04-DOMINIO-DSP.md` (< 20 s sem LLM) **nao vale**
  quando a separacao esta ativa. Precisa de orcamento proprio e de aviso na UI.
- Fica fora do binario unico que entregamos. A promessa de instalacao sem
  friccao continua valendo para tudo, menos esta ferramenta.

**Quando revisar.** Se surgir implementacao em Rust com qualidade comparavel, ou
se a demanda justificar o projeto de embutir ONNX.

---

## ADR-0011 -- Propriedade intelectual: implementar de literatura, nunca traduzir codigo copyleft

**Status:** proposto -- **nao vale enquanto nao tiver dono**
**Data:** 2026-07-27
**Dono:** `<preencher>`

> Esta ADR foi redigida a partir do texto ja especificado em
> [`../16-CORRECOES-DSP.md`](../16-CORRECOES-DSP.md) §T0.1, que a exige com dono
> nomeado. **Enquanto o campo acima estiver em branco, ela nao esta aceita** -- o
> ato que falta e uma pessoa assumir, nao escrever. Ao preencher, trocar o status
> para `aceito`.

**Contexto.** O Mixlirous implementa DSP de audio -- limiters, crossfade,
medicao de loudness, deteccao de onset -- para o qual existe implementacao de
referencia madura e legivel no Audacity. A tentacao de consulta-la e grande e o
custo de ceder e o projeto inteiro.

O Audacity e **GPL**: GPLv3 no projeto, GPLv2-or-later na maioria dos arquivos.
Copiar **ou traduzir** qualquer arquivo de `au3/` para Rust cria obra derivada e
propaga o copyleft para o Mixlirous inteiro, **incluindo o binario distribuido**.
Nao e risco de processo distante: e a licenca do produto mudando por causa de um
arquivo.

**Decisao.** Implementar DSP a partir de literatura publicada e de crates com
licenca permissiva. Nunca a partir de codigo copyleft, nem por copia nem por
traducao.

**Regras.**

- **Proibido:** abrir o repositorio do Audacity -- ou qualquer base copyleft --
  durante a implementacao de um modulo de DSP equivalente. Ler e reescrever "com
  as proprias palavras" logo em seguida **conta como reproducao**. A separacao
  precisa ser de fonte, nao de redacao.
- **Permitido:** implementar a partir de literatura publicada (DAFX/Zolzer, EBU
  TECH 3341/3342, ITU-R BS.1770, papers revisados) e usar crates com licenca
  permissiva -- MIT, BSD, Apache-2.0.
- **Obrigatorio:** todo modulo de DSP novo cita a fonte no cabecalho.

**Sobre as citacoes -- a parte que mais falha.** A politica inteira depende de as
referencias serem verdadeiras. Um cabecalho apontando para uma fonte que nao diz
aquilo e **pior que cabecalho nenhum**: e documentacao falsa de procedencia, e
ela sobrevive anos porque parece rigor. **Confira cada citacao contra um exemplar
antes do commit.**

Estado das citacoes que ja circularam neste projeto:

| Referencia | Estado | Nota |
|---|---|---|
| DAFX 2a ed. §4.2 "Dynamic Range Control" | **conferida** | Cobre limiters e compressores com a arquitetura envelope follower → curva estatica → filtro de suavizacao → multiplicador. E a referencia correta para o T3.2. |
| "DAFX 2a ed., Equal Power Crossfade, p. 46" | **nao confirmada** | Nao parece existir -- a p. 46 cai no capitulo de filtros. Crossfade de potencia constante e resultado elementar e pode ser citado de qualquer texto de processamento de sinais. **Nao copie essa citacao sem verificar.** |

A segunda linha e o exemplo de por que esta secao existe: a referencia circulou
em documentos externos com aparencia de verificada.

**Adocao de biblioteca externa.** Antes de avaliar qualquer biblioteca por
qualidade tecnica, verifique a licenca -- a ordem inversa leva a escolher e
descobrir depois, quando ja ha codigo em cima. Isto vale imediatamente para a
[#36](https://github.com/danzeroum/mixlirous/issues/36) (estiramento temporal com
preservacao de tom), onde as implementacoes mais maduras sao copyleft ou de
licenca dupla. **Nenhuma licenca de biblioteca especifica foi conferida contra a
fonte oficial nesta redacao** -- conferir e parte do trabalho da #36, sob a mesma
regra do quadro acima.

**Alternativas consideradas.**

- *Consultar o Audacity so para entender, sem copiar.* Descartada: e exatamente
  o caso que a regra do "com as proprias palavras" cobre, e a linha entre
  entender e reproduzir nao e defensavel depois do fato.
- *Licenciar o Mixlirous como GPL e resolver o problema por adesao.* Descartada
  aqui por ser decisao de produto, nao de arquitetura -- e ainda nao tomada
  (`README.md`: licenca a definir antes do primeiro release publico). Se um dia
  for tomada, esta ADR e substituida, nao contornada.

**Consequencias.**

- Implementar de literatura e mais lento que traduzir codigo pronto. E custo
  aceito, nao descuido de planejamento.
- Todo modulo novo carrega uma citacao que alguem precisa ter conferido. O
  checklist de PR precisa cobrir isso.
- Ferramentas cujo estado da arte e copyleft -- separacao de stems, estiramento
  com preservacao de tom -- ou entram por binario externo invocado como
  subprocesso (ver ADR-0010), ou saem do escopo. Vincular como biblioteca e a
  opcao que a licenca fecha.

**Sobre a lacuna do `docs/15`.** A numeracao dos documentos salta de 14 para 16.
Um `docs/15-AUDACITY-TRIAGEM.md` foi referido em conversa mas **nunca foi
commitado** -- `git log --all --diff-filter=A` nao retorna nada, em ref nenhuma.
Vale registrar que isso pode ser deliberado: um documento que **triage o
codigo-fonte do Audacity** e precisamente o tipo de artefato que esta politica
probe produzir durante a implementacao de modulo equivalente. A lacuna fica
explicada aqui em vez de ficar como vao sem motivo.

**Quando revisar.** Se a licenca do Mixlirous for definida como copyleft, ou se
surgir implementacao permissiva para alguma capacidade hoje bloqueada por
licenca.

---

## ADR-0012 -- Pipeline DSP estruturado em `audio_core`

**Status:** aceito
**Data:** 2026-08-12
**Dono:** `<preencher>`

**Contexto.** O pipeline de remix e definido em `docs/04-DOMINIO-DSP.md` §2 como
uma cadeia fixa de sete etapas:

```
[decode] → [analyze] → [segment] → [select] → [stitch] → [master] → [encode]
   A          B            C          D          E          F          G
```

Cada etapa B–F tem implementacao individual em `dsp/` (crossfade, fades, LUFS,
limiter, compressor, beat tracking, knapsack, chroma), mas **nao existe um tipo
ou trait que as conecte**. A cadeia e montada ad-hoc em tres lugares:

1. `examples/fatia_vertical.rs` -- script de integracao que chama cada funcao na
   mao; funciona, mas nao e reutilizavel.
2. `DefaultMixer::render_stitched` -- placeholder que so concatena blocos sem
   crossfade, fades nem masterizacao.
3. `worker.rs` (audio_api) -- placeholder que pula o DSP e codifica o PCM bruto.

O problema: qualquer mudanca na ordem da cadeia (ex.: `docs/04` §8 diz que
compressor → limiter → LUFS e ordem fixa nao negociavel) precisa ser replicada
em tres lugares, e nenhum deles e testavel como unidade.

**Decisao.** Criar um trait `RemixPipeline` em `audio_core::dsp::pipeline` com
uma implementacao padrao `DefaultRemixPipeline` que orquestra as etapas B–F
dentro do `audio_core`, recebendo PCM decodificado + `PipelineConfig` e
devolvendo PCM processado. O worker e o exemplo passam a chamar esse pipeline
em vez de remontar a cadeia.

**Por que.** A cadeia e fixa por especificacao (§8: compressor → limiter → LUFS,
nao negociavel). Codificar essa ordem em um so lugar elimina uma classe inteira
de bug de ordenacao. O trait permite trocar a implementacao em teste (pipeline
mock que devolve PCM constante) sem tocar no worker. O `PipelineConfig` ja
existe como tipo de dominio -- o pipeline so precisa consumi-lo.

**Alternativas consideradas.**

- *Funcao solta `run_pipeline(pcm, config) -> Result<Vec<f32>>`.* Descartada:
  nao e testavel com mock e nao permite que o agente (Sprint 3) injete etapas
  customizadas (ex.: stem separation via subprocesso, ADR-0010).
- *Pipeline no `audio_api`.* Descartada: `audio_core` e a biblioteca sem I/O de
  rede (ADR-0001). Se o pipeline ficasse na API, nao poderia ser chamado do
  exemplo nem de testes unitarios do proprio `audio_core`.
- *Extender `AudioMixer::render_stitched`.* Descartada: o trait `AudioMixer` nao
  recebe `PipelineConfig` (so blocos + PCM), e a emenda e uma etapa de cinco
  -- forcar tudo num metodo quebra a object safety e nao separa selecao de
  masterizacao.

**Consequencias.**

- O pipeline inteiro vira testavel como unidade (PCM de entrada → PCM de
  saida), sem levantar servidor HTTP.
- `fatia_vertical.rs` e `worker.rs` reduzem a chamada ao pipeline; a logica de
  orquestracao vive em um lugar so.
- `DefaultMixer::render_stitched` continua existindo como metodo individual
  (usado em testes de crossfade isolado), mas nao e mais o ponto de entrada do
  DSP.
- O pipeline emite `PipelineResult` com `warnings[]`, que o worker pode
  publicar via SSE sem decidir o que e aviso.
- Etapas opcionais (compressor, time-stretch) sao controladas por flags em
  `PipelineConfig`, nao por branches no codigo.

**Quando revisar.** Se surgir necessidade de pipeline ramificado (ex.: two-pass
onde o segundo depende do resultado do fingerprint do primeiro).

---

## Modelo para novas ADRs

```markdown
## ADR-XXXX -- Titulo

**Status:** proposto | aceito | substituido por ADR-YYYY
**Data:** AAAA-MM-DD
**Dono:** nome

**Contexto.** Qual problema, quais forcas em jogo.

**Decisao.** O que foi decidido, em uma frase.

**Alternativas consideradas.** O que foi descartado e por que.

**Consequencias.** O que fica mais facil, o que fica mais dificil.

**Quando revisar.** Gatilho concreto e observavel.
```
