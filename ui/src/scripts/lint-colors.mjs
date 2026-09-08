#!/usr/bin/env node
/**
 * Guarda de regressão de cor — docs/design/03-REGRAS-SEMANTICAS.md (R6).
 *
 * Falha se qualquer arquivo em `ui/src` (fora dos dois canônicos:
 * `index.css` e `lib/theme.ts`) contiver:
 *   1. classe de cor bruta de família nativa do Tailwind
 *      (gray, green, purple, blue, red, orange, yellow, ...);
 *   2. hex literal de cor.
 *
 * Não flagga `bg-transparent` nem `text-white`: não são família nativa.
 * `text-white` sobrevive APENAS dentro de botão preenchido (R5) — isso
 * é verificado em review visual, não aqui.
 */
import { readdirSync, readFileSync, statSync } from 'node:fs'
import { join, relative, sep } from 'node:path'
import { fileURLToPath } from 'node:url'

// Este arquivo vive em ui/src/scripts/ — SRC é ui/src (um nível acima).
const SRC = fileURLToPath(new URL('../', import.meta.url))

const NATIVE_FAMILIES =
  'gray|slate|zinc|neutral|stone|green|emerald|teal|cyan|sky|purple|violet|fuchsia|blue|indigo|red|rose|pink|amber|orange|yellow'

const RAW_CLASS_RE = new RegExp(
  `\\b(?:bg|text|border|ring|from|via|to|fill|stroke|divide|outline)-(${NATIVE_FAMILIES})-\\d{2,3}\\b`,
  'g',
)
const HEX_RE = /#[0-9a-fA-F]{3,8}\b/g

// R6: os únicos lugares onde um literal de cor pode existir.
const HEX_ALLOWED = new Set(['index.css', join('lib', 'theme.ts').split(sep).join('/')])

function* walk(dir) {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name)
    if (statSync(p).isDirectory()) yield* walk(p)
    else yield p
  }
}

const violations = []

for (const file of walk(SRC)) {
  if (!/\.(tsx?|css)$/.test(file)) continue
  const rel = relative(SRC, file).split(sep).join('/')
  const hexAllowed = HEX_ALLOWED.has(rel)
  const lines = readFileSync(file, 'utf-8').split('\n')

  lines.forEach((line, i) => {
    for (const raw of line.matchAll(RAW_CLASS_RE)) {
      violations.push(`${rel}:${i + 1}  classe nativa "${raw[0]}"`)
    }
    if (!hexAllowed) {
      for (const hex of line.matchAll(HEX_RE)) {
        violations.push(`${rel}:${i + 1}  hex literal "${hex[0]}"`)
      }
    }
  })
}

if (violations.length > 0) {
  console.error(
    [
      'lint:colors — violações da R6 (docs/design/03-REGRAS-SEMANTICAS.md):',
      ...violations.map((v) => `  ${v}`),
      '',
      'Famílias nativas do Tailwind e hex literais só existem em',
      'ui/src/index.css e ui/src/lib/theme.ts. Use os tokens do @theme',
      '(ex.: bg-surface-800, text-ink-300) ou importe de lib/theme.ts.',
    ].join('\n'),
  )
  process.exit(1)
}

console.log('lint:colors — ok: zero classe nativa, zero hex fora dos canônicos.')
