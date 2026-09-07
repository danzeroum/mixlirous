# Plano: transformar o Mixlirous em uma ferramenta profissional

O Mixlirous deve ser tratado como uma plataforma de remixagem assistida e controlável, não apenas como uma UI para demonstrar um pipeline. O objetivo de produção deve ser permitir que um usuário envie um áudio, defina uma intenção musical, construa ou aceite um plano de processamento, compare o resultado de forma confiável e exporte um arquivo com qualidade técnica verificável.

A base atual é promissora — UI React/Vite, upload presigned, job, SSE, HITL, player A/B e download —, mas a integração declarada da UI ainda é 80%, o canvas não alimenta o `pipeline_config` efetivo, e há déficits relevantes no motor DSP e na segurança operacional.

> Ver `docs/ADENDO-PARETO-PRODUCAO.md` para correções validadas contra o código atual e a sequência priorizada 80/20 de execução.

## Meta de produto

### Fluxo profissional mínimo

O primeiro release utilizável por músicos, produtores e sound designers deve entregar esta jornada sem intervenções técnicas:

1. Criar conta, entrar no workspace e acessar projetos anteriores.
2. Criar projeto e enviar um ou mais arquivos de áudio.
3. Validar formato, duração, sample rate, canais, pico, LUFS e possíveis problemas de clipping.
4. Escolher um objetivo: preparar para DJ set, transição entre faixas, ajuste de duração, normalização, criação de intro/outro, remix assistido ou edição manual.
5. Escolher entre:
   - **Assistido por IA:** prompt em linguagem natural, por exemplo: “crie uma transição de 16 compássos, reduza a energia no fim e preserve o vocal”.
   - **Manual:** construir e parametrizar o pipeline explicitamente.
   - **Híbrido:** IA propõe; usuário revisa, bloqueia parâmetros, edita e aprova.
6. Executar preview renderizado e acompanhar progresso em tempo real.
7. Comparar original, preview e render final em A/B sincronizado.
8. Validar métricas de qualidade e aprovar ou rejeitar o resultado.
9. Exportar WAV/FLAC de produção e, quando aplicável, MP3/AAC de referência.
10. Reproduzir ou versionar uma receita de processamento em outro projeto.

### Critério de “pronto para produção”

O produto só deve ser promovido de beta para produção quando cumprir simultaneamente:

- Todo controle visual da UI altera de fato o pipeline executado.
- Nenhuma ferramenta é exibida como disponível se não produzir alteração mensurável no áudio.
- Um job concluído possui artefato, metadados, logs de processamento, versão de receita e métricas de qualidade.
- O usuário pode cancelar, repetir, recuperar, comparar e baixar o resultado sem depender de terminal.
- A plataforma é segura para múltiplos usuários/tenants e não expõe banco, MinIO ou segredos.
- O comportamento é testado com áudios reais, não somente fixtures sintéticas e testes de contrato.

## Fase 0 — Fundar a verdade operacional

Antes de criar funcionalidades, congele e corrija a divergência entre código, documentação e status interno.

O README ainda afirma que a UI está “sem fluxo de upload/job”, porém `UploadPanel.tsx` já chama `POST /api/v1/uploads/presign`, há criação de job e a UI tem componentes de player e HITL. Essa inconsistência precisa ser eliminada para impedir decisões erradas de produto e desenvolvimento.

### Entregáveis

- Criar um documento único: `docs/PRODUCTION_READINESS.md`.
- Definir para cada capacidade os estados:
  - `implemented`
  - `integrated`
  - `tested-with-real-audio`
  - `production-ready`
  - `blocked`
- Atualizar README, changelog, `.dev/module-status.yaml`, roadmap e contratos com a mesma fonte de verdade.
- Instituir uma matriz “controle da UI → endpoint → struct Rust → etapa DSP → métrica de saída → teste”.
- Transformar todas as pendências abertas em épicos de release, com dono, prioridade, critério de aceite e bloqueios explícitos.

### Gate de saída

Nenhuma tela deve anunciar uma capacidade que não possua implementação, integração e teste correspondente.

## Fase 1 — Tornar o canvas executável

Este é o principal gap funcional. Hoje, o canvas React Flow existe, mas `App.tsx` cria o job a partir de `defaultPipelineConfig()`. Portanto, o grafo manipulado pelo usuário não é a fonte do processamento que será renderizado.

### Arquitetura recomendada

Adotar `PipelineConfig` como **modelo de domínio canônico** e o React Flow como sua projeção visual editável:

```text
PipelineConfig (fonte de verdade no backend)
        ↕ serialização versionada
Graph DTO (nodes, edges, positions, UI metadata)
        ↕
React Flow / graphStore
```

O layout visual pode continuar client-side, mas os nós, conexões, parâmetros, locks, estado e ordem de execução precisam ser persistidos pelo backend.

### Backend

Implementar:

- `GET /api/v1/tools`
  - Lista dinâmica de ferramentas efetivamente instaladas e habilitadas.
  - Metadados: categoria, rótulo PT-BR, parâmetros, unidade, mínimo, máximo, default, enum, dependências, disponibilidade e motivo de indisponibilidade.
- `GET /api/v1/jobs/:job_id`
  - Retornar o grafo canônico, configuração efetiva, status de cada nó e resultado.
- `PATCH /api/v1/jobs/:job_id/graph`
  - Atualização transacional de nós, arestas e ordenação.
- `PATCH /api/v1/jobs/:job_id/nodes/:node_id/parameters`
  - Definir parâmetros manualmente e marcar `source = USER_DEFINED`.
- `DELETE /api/v1/jobs/:job_id/nodes/:node_id/parameters/:key`
  - Desbloquear campo e devolver o controle para default ou IA.
- `POST /api/v1/jobs/:job_id/validate`
  - Validar topologia, limites de parâmetros, pré-requisitos e custos estimados sem renderizar.
- `POST /api/v1/jobs/:job_id/preview`
  - Gerar render reduzido para a faixa/trecho selecionado.
- `POST /api/v1/jobs/:job_id/render`
  - Submeter render final versionado e reprodutível.

O próprio contrato já prevê o PATCH para parâmetros por nó e a origem `USER_DEFINED`, mas esse endpoint aparece entre os REST endpoints documentados e ausentes do router.

### Front-end

Substituir o canvas genérico por um editor de pipeline profissional:

- Paleta de ferramentas alimentada por `GET /tools`, nunca hardcoded.
- Nós com estado: pendente, válido, inválido, renderizando, concluído, warning e erro.
- Painel de propriedades com controles tipados:
  - slider e entrada numérica para valores contínuos;
  - enum/select;
  - toggle;
  - duração em ms/segundos/compássos;
  - parâmetros em dB, LUFS, BPM e porcentagem;
  - botão de cadeado para parâmetros definidos pelo usuário.
- Validação visual de arestas:
  - impedir grafo cíclico;
  - impedir nós incompatíveis;
  - indicar ferramentas que exigem análise prévia ou múltiplas faixas.
- Histórico local com undo/redo.
- Autosave com estado “salvo”, “salvando” e “falhou”.
- Grafo carregado a partir do job existente, não apenas criado no browser.
- Importação/exportação de receitas versionadas em JSON.

### Critério de aceite

Mover um nó, adicionar um fade, trocar sua duração e renderizar deve produzir metadados de job que comprovem a mesma operação e um áudio audivelmente alterado de forma correspondente.

## Fase 2 — Completar o motor sonoro

Atualmente, as ferramentas com DSP real e testado são `crossfade`, `fade_in`, `fade_out`, `time_stretch` e `lufs_normalization`; `compression` e `dynamic_eq` não têm módulo DSP correspondente e precisam permanecer indisponíveis até sua implementação real.

Além disso, a documentação ainda descreve `DefaultMixer::render_stitched` como placeholder para concatenação de blocos, sem crossfade, fades ou masterização plenamente orquestrados. Esse ponto deve ser resolvido ou comprovadamente invalidado por teste de render ponta a ponta antes de qualquer alegação de uso profissional.

### Sequência de implementação DSP

| Prioridade | Capacidade | Motivo |
|---|---|---|
| P0 | Pipeline de render canônico | Garantir que toda receita aprovada execute exatamente o que foi configurado |
| P0 | Crossfade com potência constante | Eliminar perda de energia e emendas audíveis |
| P0 | Correção de LUFS + limiter | Resolver a issue #37: limiter desfaz ganho de LUFS em material percussivo |
| P0 | Beat/onset robusto | Resolver #27: limiar absoluto que falha em áudio real |
| P1 | Time stretch sem alterar pitch | Resolver #36: varispeed não atende edição musical profissional |
| P1 | EQ paramétrico/dynamic EQ | Controle tonal indispensável |
| P1 | Compressão | Controle dinâmico indispensável |
| P1 | Análise harmônica/key detection | Necessária para transições e sugestões musicais melhores |
| P2 | Separação de stems | Feature premium, condicionada a binário/modelos detectáveis |
| P2 | Reverb, delay, filtro criativo, pitch shift | Ampliação criativa depois do core confiável |

### Princípios de engenharia sonora

- Processamento interno em PCM float32 ou float64, com prevenção explícita de NaN/Inf.
- Preservar sample rate de origem até etapa de exportação, salvo transformação intencional.
- Suportar estéreo de verdade; evitar downmix silencioso.
- Parâmetros temporais referenciáveis por segundos e por grade musical.
- Todas as ações devem registrar:
  - entrada;
  - parâmetros efetivos;
  - versão do algoritmo;
  - seed, quando houver aleatoriedade;
  - checksum de artefatos;
  - métricas antes/depois.
- Usar render não destrutivo: a origem nunca é sobrescrita.
- Medir peak, true peak, LUFS integrado, LRA, clipping, duração, BPM estimado e confiança.

### Testes acústicos obrigatórios

- Golden masters com tolerância definida por algoritmo.
- Teste de diferença RMS/energia em junções de crossfade.
- Teste de clipping e true peak após gain/limiting.
- Teste de preservação de duração e pitch no time stretch.
- Fixtures reais: fala, vocal, bateria, música eletrônica, mix denso, material de baixo volume e áudio com transientes.
- Escuta cega por pelo menos dois avaliadores para mudanças algorítmicas relevantes.
- Teste de não-operação: cada ferramenta habilitada precisa mudar o buffer esperado; ferramenta sem efeito deve falhar no CI.

A issue #37 é P0, há ambiguidade de BPM em fixtures reais (#18), e a detecção de beats tem histórico de falha fora de testes sintéticos (#27); esses riscos tornam inviável declarar confiabilidade sonora antes de uma suíte acústica robusta.

## Fase 3 — UX de áudio profissional

A UI existente deve evoluir de painel de demonstração para estação de trabalho orientada a decisão auditiva.

### Tela de projeto

- Navegação por projetos, faixas, versões e renders.
- Estados claros: rascunho, analisando, aguardando aprovação, renderizando, concluído, falhou, cancelado e expirado.
- Autosave e aviso contra perda de edição.
- Histórico de receita e possibilidade de duplicar versão.
- Permitir renomear, etiquetar, arquivar e excluir projetos/faixas.

### Upload e preparação

- Drag-and-drop, seletor de arquivo e fila de uploads.
- Progresso real: pré-assinatura, transferência, verificação, análise e pronto para processar.
- Validação antes de iniciar:
  - extensões e MIME;
  - tamanho;
  - duração máxima;
  - sample rate;
  - canais;
  - codec;
  - arquivo corrompido;
  - duplicidade por checksum.
- Mostrar análise inicial antes de render:
  - waveform;
  - duração;
  - BPM/key quando disponível;
  - LUFS;
  - peak/true peak;
  - clipping;
  - formato e tamanho.

`get_track_peaks` hoje é citado como retornando array sempre vazio; waveform e navegação temporal não devem entrar em beta público antes de isso ser corrigido, coberto por testes e exercitado com áudio real.

### Edição e preview

- Waveform multicanal com zoom e seleção de intervalo.
- Marcação de cue points, regiões e loops.
- Grid de BPM configurável.
- Preview de trecho, por exemplo, 8/16/32 compássos ou seleção manual.
- Player A/B:
  - sincronização por relógio comum;
  - troca sem reset de playhead;
  - normalização de volume para comparação justa;
  - indicação da cadeia/versão comparada;
  - seleção original vs. preview vs. render final.
- Medidores em tempo real de peak, LUFS short-term e ganho aplicado.
- Warnings compreensíveis: clipping, redução extrema, render com possível artefato, ferramenta indisponível, job reprocessado.

### HITL e IA

O fluxo de propostas já é baseado em eventos `agent.proposal`, e o overlay permite “Aprovar com ajuste”. A próxima etapa é elevar isso a um sistema auditável de revisão, não a um popup isolado.

Cada proposta deve apresentar:

- O que a IA quer fazer.
- Por que quer fazer, com evidência da análise.
- Impacto previsto: duração, LUFS, BPM, tonalidade, energia e risco.
- Parâmetros propostos e limites.
- Preview antes/depois do trecho afetado.
- Ações: aprovar, aprovar editando, rejeitar com motivo, pedir alternativa e bloquear parâmetros.
- Registro de decisão, usuário, data e versão do modelo/prompt.

## Fase 4 — Agente de IA seguro e controlável

A IA precisa ser uma camada de **orquestração explicável**, não uma caixa-preta que altera áudio.

### Regras de operação

- O agente só chama ferramentas retornadas como `available: true`.
- Toda chamada passa por validação determinística de schema, limites, pré-condições e custo.
- O agente nunca envia parâmetros fora do envelope permitido.
- Alterações irreversíveis exigem confirmação humana.
- Toda proposta contém receita estruturada, não apenas linguagem natural.
- O modelo deve ser configurável por ambiente, com fallback controlado e sem provider mock em produção.
- Prompts devem ser versionados, carregados do diretório `prompts/` e incluídos na trilha de auditoria.

O changelog registra que `MockLlm` constava como provider ativo em produção e que o prompt de sistema estava hardcoded em `react_kernel.rs`; ambos devem ser tratados como bloqueadores de release.

### Reprodutibilidade

Um render deve carregar um `render_manifest.json` contendo:

```json
{
  "project_id": "…",
  "job_id": "…",
  "source_artifact_sha256": "…",
  "pipeline_config_version": "…",
  "tool_sequence": ["fade_in", "time_stretch", "lufs_normalization"],
  "effective_parameters": {},
  "agent_model": "…",
  "prompt_id": "…",
  "prompt_version": "…",
  "algorithm_versions": {},
  "created_at": "…",
  "output_sha256": "…"
}
```

Isso permite reproduzir, auditar e depurar qualquer render posteriormente.

## Fase 5 — Backend, jobs e resiliência

### Modelo de execução

Separar API, worker e processamento pesado:

```text
Browser
  → API pública
  → Postgres: jobs, versões, auditoria, tenants
  → fila durável
  → workers DSP/IA escaláveis
  → MinIO/S3: originais, previews, artefatos e manifests
  → SSE/WebSocket gateway
```

### Jobs profissionais

Implementar máquina de estados explícita:

```text
draft
→ uploading
→ uploaded
→ analyzing
→ awaiting_approval
→ queued
→ processing
→ preview_ready
→ rendering_final
→ completed
```

Estados alternativos:

```text
failed | cancelled | expired | recovery_pending
```

Para cada transição:

- idempotency key;
- evento auditável;
- timestamp;
- actor;
- motivo/erro estruturado;
- política de retry;
- cleanup de arquivos temporários;
- recuperação após reinicio de worker.

### Funcionalidades obrigatórias

- Cancelamento cooperativo de job.
- Retry manual e automático com limite.
- Detecção de worker perdido via heartbeat.
- Retomada ou requeue após crash.
- Dead-letter queue para falhas persistentes.
- Controle de concorrência por tenant.
- Limites de duração, tamanho, custo e render simultâneo.
- Previews cacheáveis por hash de input + receita + intervalo.
- URLs de download temporárias e revogáveis.
- Exclusão lógica e física sujeita a política de retenção.

## Fase 6 — Segurança, privacidade e multitenancy

A exposição de Postgres/MinIO em `0.0.0.0` com credenciais hardcoded (#28) é bloqueador absoluto de produção. A documentação também registra URI de banco hardcoded em `production.yaml`; segredos devem migrar para variáveis de ambiente ou secret manager.

### Requisitos mínimos

- Postgres e MinIO em rede Docker interna, sem portas públicas.
- Somente Nginx/Caddy/API gateway exposto à internet.
- TLS obrigatório, HSTS, headers de segurança e rate limiting.
- `JWT_SECRET` obrigatório e fail-closed fora de `local`; o código atual registra risco de fallback hardcoded se o segredo não estiver definido.
- Rotação de segredos e credenciais.
- JWT de curta duração, refresh token seguro ou sessão baseada em cookie `HttpOnly`, `Secure`, `SameSite`.
- Escopo por tenant em toda query, storage key e evento.
- Autorização por papel: owner, editor, reviewer, viewer, operador.
- Isolamento de objetos por prefixo/tenant e validação contra path traversal.
- Varredura de arquivos enviados e quotas por tenant.
- Auditoria imutável de upload, render, download, mudança de receita e decisão HITL.
- Política de retenção e exclusão.
- Termos de uso e declaração clara sobre direitos autorais do áudio enviado.
- LGPD: finalidade, base legal, retenção, exportação e exclusão de dados pessoais.

### SSE autenticado

A issue #33 registra a limitação de que `EventSource` não manda header `Authorization`. A solução recomendada é evitar token em query string e adotar uma destas estratégias: cookie de sessão seguro no mesmo domínio, endpoint que emite token SSE de curta duração e uso único, ou migração para WebSocket autenticado no handshake.

## Fase 7 — Observabilidade e operação

O repositório já possui métricas Prometheus, auditoria, recovery loop, cleanup e dashboards Grafana, o que é uma boa fundação. Esses recursos precisam ser conectados a SLOs operacionais e runbooks de incidente.

### Métricas de produto

- Uploads concluídos/falhos.
- Tempo até primeiro preview.
- Tempo total de render.
- Taxa de aprovação/rejeição de propostas.
- Taxa de retry e falha por ferramenta.
- Comparações A/B efetuadas.
- Exportações concluídas.
- Uso por tenant e custo por render.

### Métricas técnicas

- Latência p50/p95/p99 de API.
- Profundidade e idade da fila.
- CPU, memória, disco e I/O por worker.
- Erros de decode, DSP, storage, LLM e SSE.
- Tempo de geração de waveform/peaks.
- Falhas de autenticação e rate limiting.
- Taxa de reconexão/replay de eventos.
- Jobs stuck por estado.

### SLO inicial

| Indicador | Meta inicial |
|---|---:|
| Disponibilidade da API | 99,5% mensal |
| Upload pequeno concluído | 99% |
| Job com estado terminal | 99% |
| Recuperação após queda de worker | 95% sem intervenção |
| Erro de exportação após job concluído | < 0,5% |
| Reconexão de streaming | < 10 segundos |
| Incidentes P0 sem alerta | 0 |

## Fase 8 — Qualidade e entrega contínua

Há Playwright configurado no projeto, mas a busca atual só identifica teste de UI estrutural para SSE; não há evidência de specs browser E2E versionados. O teste E2E existente é no backend Rust.

### Pirâmide de testes

- **Rust unitários:** algoritmos DSP, serialização, validação, estados de job e autorização.
- **Rust integração:** Postgres, MinIO/S3, fila, upload presigned, worker, SSE e recovery.
- **Contrato:** OpenAPI/JSON schema compartilhado entre Rust e TypeScript.
- **Acústicos:** golden masters e propriedades de sinal.
- **Front-end unitário:** componentes, hooks, serialização do canvas e tratamento de erro.
- **Browser E2E com Playwright:** upload real, criação de job, SSE, HITL, preview, render, download, cancelamento, retry e reconexão.
- **Carga:** uploads paralelos, jobs longos, desconexão de clientes e reinicio de workers.
- **Segurança:** dependências, SAST, secret scanning, container scanning e DAST no staging.

### Gates obrigatórios no CI

- `cargo fmt --check`
- Clippy sem warnings permitidos arbitrariamente.
- Testes Rust e TypeScript.
- Build do front-end.
- Testes Playwright contra stack efêmera.
- Testes de contrato TS↔Rust.
- Testes acústicos/golden master.
- `cargo audit`, npm audit com política explícita e scanner de imagens.
- Secret scanning.
- Build de imagem reprodutível com SBOM.
- Merge bloqueado se faltar aprovação, check ou migração revisada.

A issue #5 alerta que nenhum contexto de CI roda incondicionalmente em PR, o que pode travar ou enfraquecer os required checks; essa configuração deve ser corrigida logo na fundação do pipeline.

## Roadmap de releases

### Release 0.1 — Closed alpha

**Objetivo:** validar um fluxo real com usuários internos e poucos áudios.

- Corrigir segredos, redes Docker e auth SSE.
- Tornar pipeline/grafo canônico e executável.
- Fechar PATCH de parâmetros, cancelamento e retry.
- Completar preview e A/B com waveform.
- Limitar escopo sonoro a fades, crossfade, normalização e render seguro.
- Corrigir LUFS/limiter, onset e testes acústicos básicos.
- Fazer E2E browser do fluxo integral.
- Rodar em ambiente staging separado.

**Gate:** 20–50 renders reais sem inconsistência entre canvas, receita, metadados e artefato.

### Release 0.2 — Private beta

**Objetivo:** uso recorrente por produtores convidados.

- Time stretch com pitch preservation.
- EQ paramétrico e compressão reais.
- Receitas versionadas, histórico, duplicação de projeto e rollback.
- Projetos multi-faixa e fluxo de transição.
- Quotas, billing interno/cost accounting e relatórios de uso.
- Dashboards, alertas, backups e runbooks.
- Avaliação humana estruturada de qualidade dos renders.

**Gate:** 100+ renders reais, com taxa de falha, tempos e feedback de áudio dentro dos SLOs.

### Release 1.0 — Produção

**Objetivo:** ferramenta profissional pública, segura e suportável.

- Multitenancy auditado.
- Controle de acesso e privacidade completos.
- Política de retenção/exclusão efetiva.
- Disaster recovery testado.
- Processo de suporte e incidente.
- Runbook operacional.
- SLA publicado.
- Documentação de usuário, administrador e API.
- Monitoramento 24/7 proporcional ao público atendido.

**Gate:** auditoria de segurança aprovada, restore de backup testado, caos/recovery exercitado, E2E verde, avaliação acústica aprovada e nenhum P0/P1 aberto que comprometa integridade, segurança ou resultado sonoro.

## Ordem recomendada de execução

A sequência abaixo evita construir uma interface sofisticada sobre um motor ainda inconcluso:

1. Segurança de produção: #28, `JWT_SECRET` fail-closed, secret management, isolamento de rede e estratégia de autenticação SSE (#33).
2. Verdade de integração: corrigir docs/status e montar matriz UI → API → DSP → teste.
3. Canvas executável: transformar `graphStore` em `PipelineConfig` validado e persistido.
4. Controle manual completo: PATCH/DELETE de parâmetros, locks, validation endpoint e preview.
5. Estabilidade DSP: corrigir #37, #27, #18, crossfade e garantir que o pipeline realmente aplique os blocos.
6. Qualidade sonora profissional: time stretch sem varispeed, EQ, compressão e stems somente quando houver implementação verificável.
7. UX de DAW leve: waveform, regiões, grid, player A/B confiável, versões e receitas.
8. Agente auditável: prompts versionados, provider real, explicabilidade, aprovação e fallback.
9. E2E browser, carga, segurança, staging, observabilidade e runbooks.
10. Alpha fechado, beta privada e só então produção pública.

A ideia central é simples: **cada controle precisa ter consequência sonora comprovável, e cada consequência sonora precisa ser reprodutível, auditável e segura**. Hoje o Mixlirous já possui componentes importantes do fluxo; o plano acima transforma essa base em uma ferramenta em que um profissional possa confiar para trabalhar — e não apenas uma interface capaz de demonstrar o conceito.
