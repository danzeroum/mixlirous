import { defineConfig } from 'vitest/config'

/**
 * Vitest — testes UNITÁRIOS de `src/` apenas.
 *
 * Os specs de `ui/e2e/` são Playwright (browser, contra a stack real) e
 * NÃO podem ser coletados aqui — o glob default do Vitest (qualquer
 * arquivo ponto spec ponto ts) os pegava
 * e o Vitest quebrava ao carregar `test.beforeAll` do Playwright.
 * O runner dos E2E é `npm run test:e2e` (playwright.config.ts).
 */
export default defineConfig({
  test: {
    environment: 'node',
    include: ['src/**/*.{test,spec}.?(c|m)[jt]s?(x)'],
  },
})
