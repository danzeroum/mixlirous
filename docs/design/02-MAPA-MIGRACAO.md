# Mapa de migração — classe antiga → token

Este é o mapa **normativo**. A substituição não é 1-para-1 mecânica: várias
classes antigas mudam de degrau para atingir contraste. Onde a coluna
"Observação" existe, ela prevalece sobre a substituição literal.

## 1. Neutros

| Classe antiga | Token novo | Observação |
|---|---|---|
| `bg-gray-900` | `bg-surface-900` | |
| `bg-gray-850` | `bg-surface-850` | **corrige bug**: `gray-850` não existe no Tailwind v4 |
| `bg-gray-800` | `bg-surface-800` | |
| `bg-gray-800/95` | `bg-surface-800/95` | preservar opacidade |
| `bg-gray-800/60` | `bg-surface-800/60` | |
| `bg-gray-900/60` `bg-gray-900/70` | `bg-surface-900/60` `/70` | |
| `bg-gray-700` | `bg-surface-700` | |
| `bg-gray-600` | `bg-surface-600` | |
| `bg-gray-500` | `bg-surface-500` | |
| `bg-black/60` | `bg-surface-950/70` | backdrop de modal; preto puro some contra a base quente |
| `bg-transparent` | `bg-transparent` | bolinha vazia de etapa não atingida (JobTimeline): ausência de preenchimento não é família de cor — a borda vira `border-surface-500` |
| `border-gray-700` | `border-surface-700` | |
| `border-gray-600` | `border-surface-600` | |
| `border-gray-500` | `border-surface-500` | |
| `to-gray-800` `from-purple-900/60` | `to-surface-800` `from-ai-800/60` | gradiente do hero |

## 2. Texto

| Classe antiga | Token novo | Observação |
|---|---|---|
| `text-white` | `text-ink-100` | 47 ocorrências. **Exceção:** manter `text-white` apenas dentro de botão preenchido, onde o par foi calculado com branco puro |
| `text-gray-100` | `text-ink-100` | |
| `text-gray-200` | `text-ink-200` | |
| `text-gray-300` | `text-ink-300` | |
| `text-gray-400` | `text-ink-400` | 42 ocorrências |
| `text-gray-500` | `text-ink-500` | **não descer abaixo disso** |

## 3. Ação primária (verde → teal)

| Classe antiga | Token novo | Observação |
|---|---|---|
| `bg-green-600` | `bg-action-700` | **muda de degrau**: `action-600` + branco = 4.49, falha AA |
| `bg-green-500` (hover) | `bg-action-600` | manter a relação hover mais claro que o repouso |
| `hover:bg-green-500` | `hover:bg-action-600` | |
| `text-green-400` `text-green-300` `text-green-200` | `text-action-400` `-300` `-200` | |
| `border-green-500` `border-green-700` | `border-action-600` `border-action-800` | |
| `bg-green-900/60` | `bg-action-950/70` | |

## 4. IA / modo assistido (purple/violet → ai)

| Classe antiga | Token novo | Observação |
|---|---|---|
| `bg-purple-600` | `bg-ai-600` | |
| `bg-purple-700` | `bg-ai-700` | |
| `bg-purple-500` | `bg-ai-500` | hover |
| `bg-purple-950/30` | `bg-ai-950/40` | opacidade sobe: a base quente absorve mais |
| `bg-purple-900/60` | `bg-ai-800/60` | |
| `text-purple-300` `text-purple-200` | `text-ai-300` `text-ai-200` | 9 + 1 ocorrências |
| `border-purple-400` | `border-ai-400` | |
| `border-purple-500` | `border-ai-400` | borda do item selecionado (par de `bg-ai-950/40`): 5.76:1 ≥ 3 (R4); `ai-500` daria 3.51:1, menos proeminente que o par original (~4.6:1) |
| `border-purple-700` | `border-ai-600` | borda do chip "Processando" (AtividadeView): texto `ai-200` sobre `bg-ai-800/60` dá 10.51:1; borda sutil coerente com os chips irmãos (action-800, manual-700, danger-700), significado carregado pelo texto (R1) |
| `border-purple-800` | `border-ai-700` | borda decorativa do card de cabeçalho com gradiente (ProjetosView); cabeçalho editorial pode usar marca (R2) |
| `#a78bfa` (focus em `index.css`) | `var(--color-ai-400)` | |

## 5. Modo manual / faixa original (blue → manual)

| Classe antiga | Token novo |
|---|---|
| `bg-blue-600` | `bg-manual-600` |
| `bg-blue-500` | `bg-manual-500` |
| `bg-blue-950/30` | `bg-manual-950/40` |
| `bg-blue-900/60` | `bg-manual-800/60` |
| `text-blue-300` `text-blue-200` | `text-manual-300` `text-manual-200` |
| `border-blue-500` `border-blue-700` | `border-manual-500` `border-manual-700` |

## 6. Atenção (orange/yellow → warn)

| Classe antiga | Token novo | Observação |
|---|---|---|
| `text-orange-400` `text-orange-300` `text-orange-200` | `text-warn-400` `-300` `-200` | |
| `text-yellow-300` | `text-warn-300` | unifica dois usos hoje inconsistentes |
| `bg-orange-950/60` | `bg-warn-950/60` | |
| `border-orange-800` | `border-warn-800` | |

## 7. Erro (red → danger)

| Classe antiga | Token novo | Observação |
|---|---|---|
| `bg-red-950` `bg-red-950/60` | `bg-danger-950` `bg-danger-950/60` | |
| `bg-red-900` `bg-red-900/50` `/60` `/70` | `bg-danger-800` + mesma opacidade | |
| `bg-red-800` (hover) | `bg-danger-700` | hover do botão de revogar consentimento (PrivacyPanel): `danger-700` é o "hover destrutivo" documentado em 01-TOKENS; `danger-200` sobre ele dá 5.63:1 |
| `text-red-100` | `text-danger-200` | texto do mesmo botão: 9.08:1 sobre `bg-danger-800/70` (repouso) |
| `text-red-400` `text-red-300` `text-red-200` | `text-danger-400` `-300` `-200` | |
| `border-red-700` `border-red-800` | `border-danger-700` `border-danger-800` | |

## 8. Hex literais fora do CSS

Estes **não** viram classe: os componentes desenham em Canvas 2D / React Flow.
Devem importar de `ui/src/lib/theme.ts`.

| Arquivo | Linha aprox. | Hex antigo | Substituir por | Observação |
|---|---|---|---|---|
| `components/Waveform.tsx` | 60 | `#374151` | `canvasColors.waveformTrack` | stroke da linha de base do SVG |
| `components/Waveform.tsx` | 65 | `#a78bfa` | `canvasColors.waveformSource` | stroke da forma de onda |
| `components/Waveform.tsx` | 73 | `#34d399` | `canvasColors.waveformPlayhead` | **divergência registrada**: a linha atual é o cursor de reprodução (agulha), não a forma de onda renderizada; `waveformPlayhead` (ink-100) é a constante do spec para esse papel — `waveformRendered` fica para quando o render for desenhado |
| `components/RemixCanvas.tsx` | 50 | `#2563eb` | `canvasColors.graphEdge` | usado como `nodeColor` do `<MiniMap>` do React Flow; destino conforme spec (azul manual) |
| `src/index.css` | 10 | `#a78bfa` | `var(--color-ai-400)` | anel de foco; comentário atualizado para citar token e contraste reais (6.0:1 sobre surface-900) |

## 9. Arquivos a alterar

Inventário completo do escopo (branch do PR #59, 2.401 linhas):

```
ui/src/index.css                       ← recebe o bloco @theme
ui/src/lib/theme.ts                    ← NOVO
ui/src/lib/__tests__/theme.spec.ts     ← NOVO (guarda de sincronia)
ui/src/App.tsx                         (483 linhas)
ui/src/components/JobTimeline.tsx      (99)
ui/src/components/Player.tsx           (357)
ui/src/components/PrivacyPanel.tsx     (109)
ui/src/components/ProposalOverlay.tsx  (326)
ui/src/components/RemixCanvas.tsx      (56)
ui/src/components/ToolPalette.tsx      (88)
ui/src/components/UploadPanel.tsx      (241)
ui/src/components/Waveform.tsx         (82)
ui/src/views/AtividadeView.tsx         (143)
ui/src/views/BibliotecaView.tsx        (78)
ui/src/views/NovoRemixView.tsx         (235)
ui/src/views/ProjetosView.tsx          (74)
ui/src/views/WorkspaceView.tsx         (30)
```

Nenhum arquivo de teste existente deve mudar de expectativa: a migração é
puramente visual. Se um teste quebrar, é sinal de que um `data-testid`, um texto
ou uma estrutura foi alterado por engano — reverta a alteração estrutural, não
o teste.
