# Plano de Design Centrado no Usuário — Mixlirous

## Objetivo

Transformar o Mixlirous em uma ferramenta profissional de decisão musical: intenção → proposta compreensível → preview A/B → decisão humana → render reproduzível/exportável. O canvas é modo avançado, não a entrada obrigatória.

## Princípios

- Intenção antes de configuração; audição antes de confiança.
- Toda proposta IA é explicável, editável, rejeitável e reversível.
- Todo controle visual altera o áudio ou fica indisponível com motivo.
- Upload, análise, proposta, preview, render e exportação exibem estado e próxima ação claros.
- Erros, expiração e rejeição são recuperáveis e não culpam o usuário.
- Privacidade por padrão: finalidade, provedor, retenção, exportação e exclusão visíveis.

## Personas e jornada

| Persona | Objetivo | Experiência-alvo |
|---|---|---|
| DJ/produtor | Transição, intro/outro e preparação rápida | Intenção guiada, preview e exportação sem conhecer DSP |
| Sound designer | Controle e repetição | Receita versionada, modo avançado e locks |
| Usuário assistido | Delegar sem perder autoria | Evidência, impacto e decisão humana |
| Operador | Segurança e previsibilidade | Atividade, quota, falhas, recuperação e auditoria |

`Entrar → projeto → upload → análise → intenção → receita → preview A/B → render → exportar → reutilizar receita`.

## Informação e fluxos

Navegação: `Projetos | Biblioteca | Novo remix | Atividade | Espaço de trabalho`. Dentro de projeto: `Resumo → Faixas → Receita → Preview → Renders → Histórico`.

Novo remix: upload validado; análise de formato, duração, sample rate, canais, BPM/key com confiança, LUFS, peak/true peak e clipping; objetivo musical; prompt com transparência sobre dados/provedor.

Receita: `PipelineConfig` é fonte de verdade; React Flow é projeção editável. Persistir grafo, ordem, parâmetros, locks e posição. Paleta via `GET /api/v1/tools`; item indisponível fica desabilitado com motivo. Implementar controles tipados, `USER_DEFINED`, undo/redo, autosave e validação.

HITL: mostrar mudança, justificativa, trecho, parâmetros, confiança, risco, impacto e preview antes/depois. Permitir aprovar, ajustar, rejeitar, pedir alternativa ou fazer manualmente. Expiração usa linguagem neutra.

Preview: waveform, regiões, cue points, seleção temporal, A/B sincronizado com compensação de volume, métricas e exportação WAV/FLAC com manifesto/checksum.

## Nielsen e acessibilidade

Exigir timeline de job, linguagem musical no modo simples, cancelar/retry/desfazer, schema como fonte da UI, validação pré-render e mensagens de recuperação. Garantir contraste AA, foco visível, teclado, `aria-live`, rótulos de leitor de tela, nenhuma dependência exclusiva de cor e `prefers-reduced-motion`.

## IA, dados e LGPD

IA só chama ferramentas `available: true`; propostas têm receita estruturada, limites e auditoria. Mostrar modelo/provedor e oferecer manual quando IA falhar. Validar integridade do áudio e exibir confiança/origem/correção manual de BPM, key e seções.

Avaliar com conjunto separado de áudio real: voz, vocal, bateria, eletrônica, mix denso, baixo volume, áudio masterizado, mono/estéreo e diferentes BPM/sample rates. Aprovação combina integridade técnica, métricas acústicas, escuta humana e reprodutibilidade.

Expor finalidade no upload, consentimento separado para retenção/melhoria, retenção configurável, exportação/exclusão e isolamento por tenant. Não usar áudio/prompt para treino/benchmark sem base legal e consentimento aplicável.

## Liderança positiva

Incerteza é informação; erro inclui recuperação; rejeitar IA melhora a receita; falha de provedor abre modo manual; expiração não culpa usuário. Medir sucesso de tarefa e confiança, não apenas autoaprovação.

## Backlog Pareto

| P | Entrega | Arquivos |
|---|---|---|
| P0 | Fluxo intenção → preview → decisão → exportação | `ui/src/components/UploadPanel.tsx`, `ui/src/components/Player.tsx`, `ui/src/components/ProposalOverlay.tsx` |
| P0 | Canvas → `PipelineConfig` real | `ui/src/App.tsx`, `ui/src/store/graphStore.ts`, `ui/src/types/api.ts`, `crates/audio_core/src/domain/pipeline_config.rs` |
| P0 | HITL explicável/auditável | `ui/src/components/ProposalOverlay.tsx`, `ui/src/hooks/useSSE.ts`, `crates/audio_api/src/routes/proposals.rs` |
| P1 | Waveform/peaks/regiões | `crates/audio_api/src/routes/tracks.rs`, `ui/src/components/Player.tsx` |
| P1 | Projetos/biblioteca/atividade | `ui/src/`, `crates/audio_api/src/routes/` |
| P1 | Usabilidade, Playwright e áudio real | `ui/e2e/`, `crates/audio_api/tests/`, `crates/audio_core/tests/` |
| P1 | Consentimento e transparência | `ui/src/`, `crates/audio_api/src/middleware/`, `docs/08-SEGURANCA-MULTITENANCY.md` |

## Gates

**Alpha:** upload, objetivo, HITL explicável, preview A/B, exportação, canvas correspondente ao render, cancel/retry e E2E.

**Beta:** waveform, regiões, receitas versionadas, dados corrigíveis e testes em áudio real.

**Produção:** acessibilidade, LGPD, auditoria, backup/restore, monitoramento, runbooks, SLOs medidos e nenhum P0/P1 de som, integridade ou segurança.

## Métricas

Medir tempo até preview/render aceito, sucesso e abandono por tarefa, ajustes/rejeições de IA, retry/cancel/falha, exportações, clipping/true peak/delta LUFS/duração e confiança: “entendi”, “consigo repetir”, “usaria em set/publicação”.