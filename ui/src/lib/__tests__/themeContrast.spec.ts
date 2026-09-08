/**
 * Teste de contraste calculado — WCAG 2.1 (docs/design/01-TOKENS.md).
 *
 * Não confia na tabela: implementa a fórmula de luminância relativa,
 * recalcula cada par declarado como aprovado e compara com o mínimo.
 * Se alguém trocar um hex, o teste diz qual par quebrou e por quanto —
 * a tabela deixa de ser promessa e vira garantia.
 *
 * Fonte dos valores: `lib/theme.ts` (sincronizado com index.css pelo
 * teste irmão `theme.spec.ts`). Nenhum hex literal aqui: até o branco
 * de referência da WCAG é computado, porque hex fora dos arquivos
 * canônicos é proibido por R6.
 */
import { describe, expect, it } from 'vitest'
import { theme } from '../theme'

const MIN_TEXT = 4.5
const MIN_GRAPHIC = 3.0

type Scale = Record<string, string>
const scales = theme as unknown as Record<string, Scale>

/** 'ai-400' → hex, direto da fonte canônica. Falha se o token não existir. */
function tok(ref: string): string {
  const idx = ref.lastIndexOf('-')
  const scale = scales[ref.slice(0, idx)]
  const step = ref.slice(idx + 1)
  if (!scale || scale[step] === undefined) throw new Error(`token desconhecido: ${ref}`)
  return scale[step]
}

function channel255(hex: string, i: number): number {
  return parseInt(hex.replace('#', '').slice(i * 2, i * 2 + 2), 16)
}

/** Luminância relativa — WCAG 2.1, sem atalhos. */
function luminance(hex: string): number {
  const f = (c: number): number =>
    c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4
  return (
    0.2126 * f(channel255(hex, 0) / 255) +
    0.7152 * f(channel255(hex, 1) / 255) +
    0.0722 * f(channel255(hex, 2) / 255)
  )
}

function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x)
  return (hi + 0.05) / (lo + 0.05)
}

/** Mistura alfa de `fg` sobre `bg` — como o browser compõe `bg-x/NN` sobre um fundo. */
function blend(fg: string, alpha: number, bg: string): string {
  const out = ['#']
  for (let i = 0; i < 3; i++) {
    const v = Math.round(channel255(fg, i) * alpha + channel255(bg, i) * (1 - alpha))
    out.push(v.toString(16).padStart(2, '0'))
  }
  return out.join('')
}

/** Branco puro computado — hex literal de cor fora dos canônicos é proibido (R6). */
const WHITE = '#' + [255, 255, 255].map((n) => n.toString(16).padStart(2, '0')).join('')

/** Texto ≥ 4.5:1 sobre o fundo da aplicação (surface-900) — 01-TOKENS.md. */
const TEXT_ON_900 = [
  'ink-100', 'ink-200', 'ink-300', 'ink-400', 'ink-500',
  'action-400', 'ai-400', 'manual-400', 'warn-400', 'danger-400',
]

/** Texto ≥ 4.5:1 sobre painéis (surface-800). */
const TEXT_ON_800 = ['ink-100', 'ink-200', 'ink-300', 'ink-400', 'ink-500']

/** Botão preenchido + branco (R5) — o degrau documentado de cada família. */
const WHITE_ON_BUTTON = ['action-700', 'ai-600', 'manual-600', 'warn-600', 'danger-600']

/** Branco sobre badge (X-800) e fundo tênue (X-950). */
const WHITE_ON_TENUOUS = [
  'action-800', 'action-950', 'ai-800', 'ai-950',
  'manual-800', 'manual-950', 'warn-800', 'warn-950',
  'danger-800', 'danger-950',
]

/** Texto X-200 sobre fundo X-800/950 da própria família. */
const FAMILY_ON_TENUOUS: Array<[string, string]> = [
  ['action-200', 'action-800'], ['action-200', 'action-950'],
  ['ai-200', 'ai-800'], ['ai-200', 'ai-950'],
  ['manual-200', 'manual-800'], ['manual-200', 'manual-950'],
  ['warn-200', 'warn-800'], ['warn-200', 'warn-950'],
  ['danger-200', 'danger-800'], ['danger-200', 'danger-950'],
]

/** Borda/ícone que carrega significado ≥ 3:1 contra o fundo (R4). */
const GRAPHIC_ON_900 = [
  'action-400', 'ai-400', 'manual-400', 'warn-400', 'danger-400',
  'action-500', 'ai-500', 'manual-500', 'warn-500', 'danger-500',
]

function expectMin(ratio: number, min: number, label: string): void {
  expect(
    ratio,
    `${label}: medido ${ratio.toFixed(2)}:1, mínimo ${min}:1`,
  ).toBeGreaterThanOrEqual(min)
}

describe('contraste WCAG 2.1 — pares declarados aprovados em 01-TOKENS.md', () => {
  it('texto atinge 4.5:1 sobre o fundo da aplicação (surface-900)', () => {
    for (const fg of TEXT_ON_900) expectMin(contrast(tok(fg), tok('surface-900')), MIN_TEXT, `${fg} sobre surface-900`)
  })

  it('texto atinge 4.5:1 sobre painéis (surface-800)', () => {
    for (const fg of TEXT_ON_800) expectMin(contrast(tok(fg), tok('surface-800')), MIN_TEXT, `${fg} sobre surface-800`)
  })

  it('botão secundário (surface-700 + ink-100, R5) atinge 4.5:1', () => {
    expectMin(contrast(tok('ink-100'), tok('surface-700')), MIN_TEXT, 'ink-100 sobre surface-700')
  })

  it('branco sobre botão preenchido atinge 4.5:1 nos degraus documentados (R5)', () => {
    for (const bg of WHITE_ON_BUTTON) expectMin(contrast(WHITE, tok(bg)), MIN_TEXT, `branco sobre ${bg}`)
  })

  it('branco sobre badge e fundo tênue atinge 4.5:1', () => {
    for (const bg of WHITE_ON_TENUOUS) expectMin(contrast(WHITE, tok(bg)), MIN_TEXT, `branco sobre ${bg}`)
  })

  it('texto X-200 sobre fundo tênue da própria família atinge 4.5:1', () => {
    for (const [fg, bg] of FAMILY_ON_TENUOUS) expectMin(contrast(tok(fg), tok(bg)), MIN_TEXT, `${fg} sobre ${bg}`)
  })

  it('borda/ícone significativo atinge 3:1 sobre o fundo (R4)', () => {
    for (const fg of GRAPHIC_ON_900) expectMin(contrast(tok(fg), tok('surface-900')), MIN_GRAPHIC, `${fg} sobre surface-900`)
  })

  it('anel de foco (ai-400) atinge 3:1 contra o fundo e 4.5:1 como texto (R4)', () => {
    expectMin(contrast(tok('ai-400'), tok('surface-900')), MIN_GRAPHIC, 'anel de foco ai-400 sobre surface-900')
    expectMin(contrast(tok('ai-400'), tok('surface-800')), MIN_GRAPHIC, 'anel de foco ai-400 sobre surface-800')
  })

  // Pares com opacidade das classes resolvidas no mapa (§4/§7) — o browser
  // compõe /NN sobre o fundo real, então o par efetivo é o blend.
  it('borda do item selecionado (ai-400 sobre ai-950/40) atinge 3:1', () => {
    const bg = blend(tok('ai-950'), 0.4, tok('surface-900'))
    expectMin(contrast(tok('ai-400'), bg), MIN_GRAPHIC, `ai-400 sobre blend(ai-950/40)=${bg}`)
  })

  it('botão de revogar consentimento (danger-200 sobre danger-800/70 e hover danger-700) atinge 4.5:1', () => {
    const rest = blend(tok('danger-800'), 0.7, tok('surface-900'))
    expectMin(contrast(tok('danger-200'), rest), MIN_TEXT, `danger-200 sobre blend(danger-800/70)=${rest}`)
    expectMin(contrast(tok('danger-200'), tok('danger-700')), MIN_TEXT, 'danger-200 sobre danger-700 (hover)')
  })

  it('chip Processando (ai-200 sobre ai-800/60) atinge 4.5:1', () => {
    const bg = blend(tok('ai-800'), 0.6, tok('surface-900'))
    expectMin(contrast(tok('ai-200'), bg), MIN_TEXT, `ai-200 sobre blend(ai-800/60)=${bg}`)
  })

  // Guarda do motivo pelo qual o botão preenchido usa action-700, não action-600.
  // Se alguém reamostrar o teal (R9), este teste quebra e obriga a atualizar
  // 01-TOKENS.md, o mapa e este arquivo juntos.
  it('action-600 com branco NÃO atinge AA (4.49) — por isso R5 manda usar action-700', () => {
    const ratio = contrast(WHITE, tok('action-600'))
    expect(ratio, 'action-600 + branco deveria continuar no 4.49 documentado').toBeGreaterThan(4.4)
    expect(ratio, 'action-600 + branco passou a atingir AA? atualize 01-TOKENS.md e R5').toBeLessThan(MIN_TEXT)
  })
})
