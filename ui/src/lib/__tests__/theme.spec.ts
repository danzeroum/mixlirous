/**
 * Guarda de sincronia CSS ↔ TS — docs/design/02-MAPA-MIGRACAO.md §9.
 *
 * `index.css` e `lib/theme.ts` declaram os mesmos tokens em duas linguagens.
 * Duas fontes de verdade divergem — é questão de tempo. Este teste falha se:
 *   - um token existir em um arquivo e não no outro;
 *   - um token tiver valor diferente nos dois lados.
 *
 * Também protege o bloco de acessibilidade do PR #59: apagá-lo é regressão.
 */
import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'
import { theme } from '../theme'

const here = dirname(fileURLToPath(import.meta.url))
const css = readFileSync(join(here, '..', '..', 'index.css'), 'utf-8')

const CSS_TOKEN_RE = /--color-([a-z]+)-(\d{2,3}):\s*(#[0-9a-fA-F]{6})/g

function cssTokens(): Map<string, string> {
  const map = new Map<string, string>()
  for (const m of css.matchAll(CSS_TOKEN_RE)) {
    map.set(`--color-${m[1]}-${m[2]}`, m[3].toLowerCase())
  }
  return map
}

function tsTokens(): Map<string, string> {
  const map = new Map<string, string>()
  for (const [family, steps] of Object.entries(theme)) {
    for (const [step, hex] of Object.entries(steps)) {
      map.set(`--color-${family}-${step}`, hex.toLowerCase())
    }
  }
  return map
}

describe('sincronia de tokens — index.css ↔ lib/theme.ts', () => {
  it('declara o mesmo conjunto de tokens nos dois arquivos', () => {
    const cssMap = cssTokens()
    const tsMap = tsTokens()
    expect(cssMap.size, 'index.css não declarou nenhum token @theme').toBeGreaterThan(0)
    const onlyCss = [...cssMap.keys()].filter((k) => !tsMap.has(k))
    const onlyTs = [...tsMap.keys()].filter((k) => !cssMap.has(k))
    expect(onlyCss, `tokens só no CSS (faltam no theme.ts): ${onlyCss.join(', ')}`).toEqual([])
    expect(onlyTs, `tokens só no TS (faltam no @theme): ${onlyTs.join(', ')}`).toEqual([])
  })

  it('tem o mesmo valor hex para cada token nos dois arquivos', () => {
    const cssMap = cssTokens()
    const tsMap = tsTokens()
    const divergent = [...cssMap.entries()]
      .filter(([k, v]) => tsMap.has(k) && tsMap.get(k) !== v)
      .map(([k, v]) => `${k}: css=${v} ts=${tsMap.get(k)}`)
    expect(divergent, `tokens divergentes: ${divergent.join('; ')}`).toEqual([])
  })

  it('mantém o bloco de acessibilidade do PR #59 em index.css', () => {
    expect(css, 'bloco :focus-visible do #59 sumiu — regressão').toContain(':focus-visible')
    expect(css, 'bloco prefers-reduced-motion do #59 sumiu — regressão').toContain(
      'prefers-reduced-motion',
    )
    // O anel de foco usa o token canônico (ai-400), não hex solto.
    expect(css).toMatch(/:focus-visible\s*\{[^}]*var\(--color-ai-400\)/s)
  })
})
