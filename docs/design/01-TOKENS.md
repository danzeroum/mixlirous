# Tokens de tema — referência de contraste

Fonte canônica: [`tokens/theme.css`](./tokens/theme.css) (destino `ui/src/index.css`)
e [`tokens/theme.ts`](./tokens/theme.ts) (destino `ui/src/lib/theme.ts`).

Todos os valores abaixo foram calculados com a fórmula de luminância relativa da
WCAG 2.1. `bg` = `surface-900` (`#141010`); `painel` = `surface-800` (`#241d1c`).

## surface — base neutra-quente

| Token | Hex | Uso |
|---|---|---|
| `surface-950` | `#0d0a0a` | poço, backdrop de modal |
| `surface-900` | `#141010` | fundo da aplicação |
| `surface-850` | `#1b1615` | sidebar (corrige o `bg-gray-850` inexistente) |
| `surface-800` | `#241d1c` | painéis, cards, overlays |
| `surface-700` | `#392f2d` | controles, tracks, chips |
| `surface-600` | `#4a3d3b` | bordas e divisores |
| `surface-500` | `#6a5b58` | bordas de ênfase |

## ink — texto

| Token | Hex | vs bg | vs painel | Uso |
|---|---|---|---|---|
| `ink-100` | `#f4f1f0` | 16.81 | 14.74 | texto principal |
| `ink-200` | `#e5dedc` | 14.23 | 12.48 | títulos secundários |
| `ink-300` | `#c1b6b3` | 9.55 | 8.37 | texto de apoio |
| `ink-400` | `#a69996` | 6.86 | 6.01 | labels, metadados |
| `ink-500` | `#928481` | 5.25 | 4.61 | **mínimo permitido** |

Não existe token de texto abaixo de `ink-500`. Se um mockup pedir cinza mais
apagado, a resposta correta é reduzir o peso da fonte, não o contraste.

## action — ação primária (teal)

| Token | Hex | vs bg | branco sobre | Uso |
|---|---|---|---|---|
| `action-200` | `#b5e3e1` | 13.53 | — | texto sobre `action-800/950` |
| `action-300` | `#8bd0cd` | 10.80 | — | ícone de ênfase |
| `action-400` | `#3eb1ae` | 7.29 | — | **texto/ícone de ação sobre escuro** |
| `action-500` | `#2b9c98` | 5.68 | 3.33 | hover de botão preenchido |
| `action-600` | `#1f8481` | 4.21 | 4.49 | cor de marca da ação (CTA original) |
| `action-700` | `#1a6b68` | 3.01 | **6.27** | **fundo de botão + texto branco** |
| `action-800` | `#144d4b` | 1.97 | 9.58 | borda de badge |
| `action-950` | `#0d2b2a` | 1.26 | 15.04 | fundo tênue de sucesso |

> Atenção: `action-600` com texto branco dá 4.49 — falha por 0.01. Botão
> preenchido usa **`action-700`**. `action-600` é reservado a bordas, focus ring
> e superfícies decorativas de marca.

## ai — modo assistido / proposta do agente (violeta)

| Token | Hex | vs bg | branco sobre | Uso |
|---|---|---|---|---|
| `ai-200` | `#e3c9e8` | 12.40 | — | texto sobre `ai-800/950` |
| `ai-300` | `#cda5d5` | 8.94 | — | ícone de ênfase |
| `ai-400` | `#b57dbf` | 5.98 | — | **texto/ícone de IA + anel de foco** |
| `ai-500` | `#9752a3` | 3.65 | 5.18 | hover, seleção no canvas |
| `ai-600` | `#773b81` | 2.45 | **7.70** | **fundo de botão + texto branco** |
| `ai-700` | `#572b5e` | 1.72 | 10.98 | superfície de marca (amostra do poster) |
| `ai-800` | `#412046` | 1.37 | 13.82 | borda de painel de IA |
| `ai-950` | `#2a152e` | 1.12 | 16.87 | fundo tênue de IA |

## manual — modo manual / faixa original (azul-ciano)

| Token | Hex | vs bg | branco sobre | Uso |
|---|---|---|---|---|
| `manual-200` | `#bed8e4` | 12.72 | — | texto sobre `manual-800/950` |
| `manual-300` | `#94bed1` | 9.49 | — | ícone de ênfase |
| `manual-400` | `#5c9ebc` | 6.36 | — | **texto/ícone + waveform da faixa original** |
| `manual-500` | `#3380a3` | 4.28 | 4.41 | aresta padrão do grafo |
| `manual-600` | `#256583` | 2.94 | **6.43** | **fundo de botão + texto branco** |
| `manual-700` | `#1d4d63` | 2.06 | 9.15 | borda |
| `manual-800` | `#153747` | 1.50 | 12.58 | borda de painel |
| `manual-950` | `#0f242f` | 1.18 | 15.99 | fundo tênue |

## brand — coral (uso exclusivo de marca)

| Token | Hex | vs bg | branco sobre |
|---|---|---|---|
| `brand-300` | `#e6b0a8` | 10.02 | — |
| `brand-400` | `#d5877b` | 6.82 | — |
| `brand-500` | `#c06b5d` | 4.95 | 3.82 |
| `brand-600` | `#a85548` | 3.65 | 5.18 |
| `brand-700` | `#824035` | 2.46 | 7.68 |
| `brand-950` | `#331915` | 1.16 | 16.27 |

**Proibido** em botão de estado, badge de status, mensagem de erro ou qualquer
elemento cujo significado o usuário precise decodificar. Ver
[`03-REGRAS-SEMANTICAS.md`](./03-REGRAS-SEMANTICAS.md).

## warn — atenção / degradação (âmbar)

| Token | Hex | vs bg | branco sobre | Uso |
|---|---|---|---|---|
| `warn-200` | `#f1d1b1` | 13.04 | — | texto sobre `warn-800/950` |
| `warn-300` | `#e0ad7b` | 9.39 | — | ícone |
| `warn-400` | `#d69451` | 7.39 | — | **texto de aviso sobre escuro** |
| `warn-500` | `#c8823c` | 6.04 | 3.13 | borda de aviso |
| `warn-600` | `#a1662b` | 4.00 | 4.72 | fundo de botão + branco |
| `warn-800` | `#613d1a` | 1.97 | 9.59 | borda de painel |
| `warn-950` | `#33210f` | 1.23 | 15.38 | fundo tênue de aviso |

Casos de uso obrigatórios: ferramenta indisponível, faixa estéreo que será
renderizada em mono, baixa confiança de análise, proposta de IA expirada.

## danger — erro e destruição

| Token | Hex | vs bg | branco sobre | Uso |
|---|---|---|---|---|
| `danger-200` | `#f4bdc0` | 11.60 | — | texto sobre `danger-800/950` |
| `danger-300` | `#eb9498` | 8.30 | — | texto de erro secundário |
| `danger-400` | `#e25057` | 4.97 | 3.81 | **texto/ícone de erro sobre escuro** |
| `danger-500` | `#cf3038` | 3.72 | 5.08 | borda de erro |
| `danger-600` | `#ae292f` | 2.84 | **6.65** | **fundo de botão destrutivo + branco** |
| `danger-700` | `#872227` | 2.06 | 9.18 | hover destrutivo |
| `danger-800` | `#63171b` | 1.50 | 12.58 | borda de painel de erro |
| `danger-950` | `#340e10` | 1.09 | 17.27 | fundo tênue de erro |

## Resumo operacional

| Papel | Texto sobre escuro | Fundo de botão preenchido | Fundo tênue |
|---|---|---|---|
| Ação primária | `action-400` | `action-700` | `action-950` |
| IA / assistido | `ai-400` | `ai-600` | `ai-950` |
| Manual / original | `manual-400` | `manual-600` | `manual-950` |
| Atenção | `warn-400` | `warn-600` | `warn-950` |
| Erro / destrutivo | `danger-400` | `danger-600` | `danger-950` |
| Neutro | `ink-100`/`ink-300` | `surface-700` | `surface-800` |
