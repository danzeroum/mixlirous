/**
 * Mixlirous — tokens de tema para contextos que NÃO aceitam classes Tailwind.
 * Destino: `ui/src/lib/theme.ts`.
 *
 * Necessário porque três superfícies desenham com valor de cor literal e não
 * com className:
 *   - `components/Waveform.tsx`  → Canvas 2D (fillStyle / strokeStyle)
 *   - `components/RemixCanvas.tsx` → React Flow (edge style, node style)
 *   - qualquer SVG gerado dinamicamente
 *
 * REGRA: este arquivo deve espelhar exatamente `ui/src/index.css`.
 * O teste `ui/src/lib/__tests__/theme.spec.ts` falha se divergirem.
 */

export const theme = {
  surface: {
    950: '#0d0a0a',
    900: '#141010',
    850: '#1b1615',
    800: '#241d1c',
    700: '#392f2d',
    600: '#4a3d3b',
    500: '#6a5b58',
  },
  ink: {
    100: '#f4f1f0',
    200: '#e5dedc',
    300: '#c1b6b3',
    400: '#a69996',
    500: '#928481',
  },
  action: {
    200: '#b5e3e1',
    300: '#8bd0cd',
    400: '#3eb1ae',
    500: '#2b9c98',
    600: '#1f8481',
    700: '#1a6b68',
    800: '#144d4b',
    950: '#0d2b2a',
  },
  ai: {
    200: '#e3c9e8',
    300: '#cda5d5',
    400: '#b57dbf',
    500: '#9752a3',
    600: '#773b81',
    700: '#572b5e',
    800: '#412046',
    950: '#2a152e',
  },
  manual: {
    200: '#bed8e4',
    300: '#94bed1',
    400: '#5c9ebc',
    500: '#3380a3',
    600: '#256583',
    700: '#1d4d63',
    800: '#153747',
    950: '#0f242f',
  },
  brand: {
    300: '#e6b0a8',
    400: '#d5877b',
    500: '#c06b5d',
    600: '#a85548',
    700: '#824035',
    950: '#331915',
  },
  warn: {
    200: '#f1d1b1',
    300: '#e0ad7b',
    400: '#d69451',
    500: '#c8823c',
    600: '#a1662b',
    800: '#613d1a',
    950: '#33210f',
  },
  danger: {
    200: '#f4bdc0',
    300: '#eb9498',
    400: '#e25057',
    500: '#cf3038',
    600: '#ae292f',
    700: '#872227',
    800: '#63171b',
    950: '#340e10',
  },
} as const;

/**
 * Papéis semânticos usados no desenho de waveform e grafo.
 * Preferir estas constantes a acessar `theme.*` diretamente no componente:
 * assim o significado fica explícito e a troca de token é feita em um lugar só.
 */
export const canvasColors = {
  /** Trilho/fundo da forma de onda (antes: #374151 = gray-700). */
  waveformTrack: theme.surface[700],
  /** Forma de onda da faixa original / modo manual (antes: #a78bfa = violet-400). */
  waveformSource: theme.manual[400],
  /** Forma de onda do render processado (antes: #34d399 = emerald-400). */
  waveformRendered: theme.action[400],
  /** Cursor de reprodução. */
  waveformPlayhead: theme.ink[100],
  /** Região selecionada para preview. */
  waveformSelection: theme.ai[500],
  /** Aresta padrão do grafo do canvas (antes: #2563eb = blue-600). */
  graphEdge: theme.manual[500],
  /** Aresta de um ramo proposto pela IA. */
  graphEdgeAi: theme.ai[500],
  /** Aresta inválida (ciclo, ferramenta indisponível). */
  graphEdgeInvalid: theme.danger[500],
} as const;

export type ThemeScale = keyof typeof theme;
