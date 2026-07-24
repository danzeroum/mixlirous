# 01 — Glossário (Linguagem Ubíqua)

Estes termos são usados **de forma idêntica** no código Rust, nos tipos
TypeScript, no Figma e nas conversas. Se um termo mudar aqui, muda em todos os
lugares no mesmo PR.

A coluna "UI (pt-BR)" é o texto que o usuário lê. A coluna "Código" é o
identificador técnico. Não misture: código em inglês, interface em português.

---

## Domínio de áudio

| Código | UI (pt-BR) | Definição |
| --- | --- | --- |
| `SourceTrack` | Faixa original | Arquivo de áudio importado, imutável. Guarda metadados derivados (BPM, grid, duração), nunca o áudio em si. |
| `BeatGrid` | Grade de batidas | Vetor de instantes (em segundos) onde caem as batidas detectadas. Base de todo corte. |
| `BeatCandidate` | Batida | Um ponto da grade, com força de transiente (`onset_strength`) associada. |
| `StrongBeat` | Batida forte | `BeatCandidate` cuja força está acima do percentil configurado (padrão P80). Único ponto onde um remix pode começar. |
| `BeatBlock` | Bloco | Unidade atômica de composição: um fragmento de áudio cortado em batidas fortes, com duração de N batidas (4, 8, 16). |
| `EnergyProfile` | Perfil de energia | Assinatura térmica de uma faixa ou bloco: RMS médio, desvio, pico, faixa dinâmica. |
| `rms_energy` | Energia | Raiz do valor quadrático médio do bloco. Proxy de "intensidade". |
| `spectral_centroid` | Brilho | Centro de massa do espectro em Hz. Proxy perceptivo de "agudo/brilhante". |
| `chroma_vector` | Perfil harmônico | 12 valores (classes de pitch). Usado para detectar repetição (refrão). |
| `ZeroCrossing` | — (invisível) | Ponto onde a onda cruza zero. Corte fora dele produz estalo. |
| `Crossfade` | Transição | Sobreposição de dois blocos com curva de ganho. Medida em ms. |
| `LUFS` | Volume percebido | Medida integrada de loudness (ITU-R BS.1770-4). Alvo padrão: −14. |
| `TruePeak` | Pico | Pico real do sinal em dBFS. Teto padrão: −1,0. |
| `Stem` | Stem | Trilha isolada por instrumento (bateria, baixo, voz, outros). |

## Domínio de composição

| Código | UI (pt-BR) | Definição |
| --- | --- | --- |
| `RemixRecipe` | Receita | Conjunto completo de parâmetros que define um remix. É o contrato que a IA preenche e o usuário edita. |
| `RemixJob` | Trabalho / Render | Uma execução da receita sobre uma faixa. Agregado raiz: tem estado, dono e histórico. |
| `PipelineConfig` | Configuração | Parte determinística da receita: duração alvo, crossfade, masterização, seleção. |
| `TargetDuration` | Duração alvo | Duração desejada + tolerância aceitável (ex.: 30 s ± 2 s). |
| `KnapsackSelection` | Seleção de blocos | Heurística que escolhe blocos maximizando energia sem estourar a duração alvo. |
| `preserve_intro_ms` / `preserve_outro_ms` | Manter início / Manter final | Trechos fixos preservados nas pontas para não perder a coerência do arranjo. |
| `Artifact` | Arquivo gerado | Resultado renderizado, com hash SHA-256 e chave de storage. |
| `AudioFingerprint` | Assinatura sonora | Vetor de features (MFCC, brilho, energia) usado para comparar renders. |

## Domínio do agente

| Código | UI (pt-BR) | Definição |
| --- | --- | --- |
| `Agent` | Assistente | O orquestrador LLM. Nunca chamado de "IA" isolada na UI — é "o assistente". |
| `Thought` | Raciocínio | Texto que o agente emite explicando a decisão. Exibido em streaming. |
| `Tool` / `AudioToolDef` | Ferramenta | Ação disponível ao agente (compressão, crossfade, EQ...). Sempre tipada. |
| `ToolCall` | Chamada de ferramenta | Invocação concreta: nome + parâmetros. Validada antes de executar. |
| `ToolBudget` | Passos restantes | Número máximo de chamadas por execução (padrão 5). Impede loop infinito. |
| `Proposal` | Sugestão | Pedido do agente para adicionar um nó que o usuário não desenhou. Exige aprovação. |
| `ParameterSource` | Origem do valor | `LLM_INFERRED` (sugerido) ou `USER_DEFINED` (travado). Governa precedência. |
| `ValidationLayer` | — (invisível) | Camada em Rust que rejeita parâmetros fora dos limites. |
| `Replan` | Nova tentativa | Após rejeição, o agente refaz a estratégia com as ferramentas permitidas. |

## Domínio do grafo (UI)

| Código | UI (pt-BR) | Definição |
| --- | --- | --- |
| `Graph` / DAG | Fluxo | O desenho completo do pipeline no canvas. Acíclico e direcionado. |
| `Node` | Nó | Uma etapa do fluxo. Tipos: `source`, `analysis`, `agent`, `processor`, `mastering`, `output`. |
| `Edge` | Conexão | Ligação entre dois nós. Carrega o tipo de dado (áudio ou parâmetros). |
| `NodeStatus` | Estado do nó | `idle` · `proposed` · `queued` · `running` · `completed` · `failed` · `rejected` |
| `Canvas` | Área de trabalho | A superfície de edição do fluxo. |
| `PropertiesPanel` | Painel de propriedades | Painel lateral com os controles do nó selecionado. |

## Domínio de plataforma

| Código | UI (pt-BR) | Definição |
| --- | --- | --- |
| `Tenant` | Espaço / Conta | Unidade de isolamento: um músico ou uma banda. Presente desde o dia 1. |
| `Project` | Projeto | Agrupamento de faixas e fluxos. |
| `Worker` | Processador | Instância que consome a fila e executa DSP. Contável e escalável. |
| `Autopilot` | Piloto automático | Modo em que o sistema decide o número de workers sozinho. |
| `RecoveryLoop` | Recuperação | Rotina de boot que reconcilia jobs interrompidos com os arquivos em disco. |
| `VersionFreeze` | Versões congeladas | Trava do modelo LLM + versão do prompt para um projeto. |
| `GoldenMaster` | Referência sonora | WAV de referência usado para detectar regressão acústica no CI. |

---

## Termos proibidos

Usar estes termos gera confusão. As alternativas estão à direita.

| Não use | Use |
| --- | --- |
| "corte", "segmento", "pedaço" | **bloco** (`BeatBlock`) |
| "música", "áudio" (para o input) | **faixa** (`SourceTrack`) |
| "sugestão da IA" para valor de slider | **valor inferido** (`LLM_INFERRED`) |
| "sugestão" para pedido de nó novo | **proposta** (`Proposal`) |
| "task", "tarefa", "processo" | **trabalho** (`RemixJob`) |
| "IA", "bot", "robô" na interface | **assistente** |
| "fade" genérico | **transição** (crossfade) ou **fade de entrada/saída** |
| "volume" para LUFS | **volume percebido** (LUFS) vs **pico** (dBFS) |

## Convenções de nomenclatura

| Contexto | Convenção | Exemplo |
| --- | --- | --- |
| Campos JSON da API | `snake_case` | `block_size_beats` |
| Tipos Rust | `PascalCase` | `BeatBlock` |
| Tipos TypeScript | `PascalCase` | `BeatBlock` |
| Variáveis TS/JS | `camelCase` | `blockSizeBeats` (converter na borda) |
| Eventos SSE | `dominio.acao` | `agent.thought`, `job.completed` |
| Nomes de arquivo (Rust) | `snake_case.rs` | `zero_cross.rs` |
| Componentes React | `PascalCase.tsx` | `ProposalOverlay.tsx` |
| Unidades em nomes | sufixo explícito | `_ms`, `_db`, `_hz`, `_sec`, `_lufs` |

> A regra de sufixo de unidade não é opcional. `duration` é ambíguo;
> `duration_ms` não é. Já custou bug em produção em todo projeto de áudio.
