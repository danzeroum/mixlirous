import { defineConfig } from '@playwright/test'

/**
 * Playwright — Lote 3, item 4 do plano Pareto (1 spec do fluxo feliz).
 *
 * A spec sobe contra a stack REAL (backend + UI), não contra mocks:
 *   - E2E_BASE_URL (default http://localhost:5173) — dev server do Vite
 *     com o proxy /api → localhost:8080 (vite.config.ts), ou o binário
 *     com o frontend embutido.
 *   - Backend em CONFIG_ENV=local (single-user, GET /auth/local-session).
 *   Sem a stack de pé, a spec se auto-pula (beforeAll checa /system/info).
 * Runbook completo em docs/17-GUIA-DE-TESTES.md.
 */
const BASE_URL = process.env.E2E_BASE_URL ?? 'http://localhost:5173'

export default defineConfig({
  testDir: './e2e',
  timeout: 240_000,
  expect: { timeout: 20_000 },
  fullyParallel: false,
  workers: 1,
  reporter: process.env.CI ? 'github' : 'list',
  use: {
    baseURL: BASE_URL,
    trace: 'retain-on-failure',
    actionTimeout: 15_000,
  },
  projects: [{ name: 'chromium', use: { browserName: 'chromium' } }],
})
