/**
 * E2E do fluxo feliz completo — Lote 3, item 4 do plano Pareto
 * (adendo §3.12): upload → criação de job → aprovação de proposta HITL →
 * download do artefato.
 *
 * Roda contra a stack REAL (ver playwright.config.ts). Sem stack, a spec
 * se auto-pula — e o CI não finge que passou.
 *
 * Sessão (bootstrap de auth): a app espera `GET /auth/local-session`
 * (Lote 2, PR #56) para obter o token REST. A spec instala um fulfill
 * DESSE endpoint com um JWT assinado em-processo (HS256, segredo de
 * desenvolvimento default do `CONFIG_ENV=local` — `auth.rs`) com tenant
 * fixo: determinístico, independente de qual PR já entrou na stack, e o
 * caminho real do endpoint tem cobertura própria nos testes HTTP do
 * Lote 2. O handshake SSE (EventSource não manda header) usa o cookie
 * `mixlirous_session` emitido pelo `POST /auth/sse-session` — a spec
 * chama a rota REAL com o Bearer; em backend sem a rota (pré-Lote 2) a
 * spec se auto-pula com o motivo explícito.
 *
 * Nota sobre o passo HITL: o worker hoje decide propostas internamente
 * (item B5 do CHANGELOG — ProposalStore ainda não populado, próxima
 * sprint do plano). Quando nenhuma `agent.proposal` chega, o passo de
 * aprovação é anotado e pulado; quando chega, o overlay é aprovado de
 * verdade. O resto do fluxo (upload → job → download por cookie de
 * sessão) é exercitado integralmente.
 *
 * Nota sobre o passo da paleta: com o Lote 1 (PR #55) no build, o grafo
 * é montado pela paleta — o job parte da serialização do grafo. Sem a
 * paleta, o App envia o default explícito do grafo vazio (contrato de
 * graphToPipelineConfig) — o fluxo é exercitado nos dois estados.
 */
import { test, expect } from '@playwright/test'

/** Tenant fixo do E2E — forma v4 válida; escopa todas as linhas do run. */
const TENANT_E2E = 'e2e00000-0000-4000-8000-000000000001'

/**
 * Segredo JWT do modo local — MESMO default de `auth.rs::jwt_secret()`
 * (`CONFIG_ENV=local` sem JWT_SECRET definido). Se a stack rodar com
 * outro segredo, o E2E falha com 401 explícito — o runbook
 * (docs/17-GUIA-DE-TESTES.md) manda subir em modo local.
 */
const SEGREDO_LOCAL = 'local-dev-secret-change-me'

function b64url(bytes: Uint8Array): string {
  let s = ''
  for (const b of bytes) s += String.fromCharCode(b)
  return btoa(s).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '')
}

/** JWT HS256 assinado com o segredo de dev — claims mínimas do §1 dos contratos. */
async function mintLocalJwt(): Promise<string> {
  const enc = new TextEncoder()
  const header = b64url(enc.encode(JSON.stringify({ alg: 'HS256', typ: 'JWT' })))
  const agora = Math.floor(Date.now() / 1000)
  const payload = b64url(
    enc.encode(
      JSON.stringify({
        sub: TENANT_E2E,
        tenant_id: TENANT_E2E,
        roles: ['owner'],
        plan: 'free',
        iat: agora,
        exp: agora + 3600,
      }),
    ),
  )
  const key = await crypto.subtle.importKey(
    'raw',
    enc.encode(SEGREDO_LOCAL),
    { name: 'HMAC', hash: 'SHA-256' },
    false,
    ['sign'],
  )
  const sig = new Uint8Array(
    await crypto.subtle.sign('HMAC', key, enc.encode(`${header}.${payload}`)),
  )
  return `${header}.${payload}.${b64url(sig)}`
}

/** WAV mono PCM16 legítimo, sintetizado no próprio teste (sem fixture). */
function wavSintetico(duracaoSeg = 1, sampleRate = 8000): Buffer {
  const total = duracaoSeg * sampleRate
  const dataBytes = total * 2
  const buffer = Buffer.alloc(44 + dataBytes)

  buffer.write('RIFF', 0, 'ascii')
  buffer.writeUInt32LE(36 + dataBytes, 4)
  buffer.write('WAVE', 8, 'ascii')
  buffer.write('fmt ', 12, 'ascii')
  buffer.writeUInt32LE(16, 16) // PCM header
  buffer.writeUInt16LE(1, 20) // PCM
  buffer.writeUInt16LE(1, 22) // mono
  buffer.writeUInt32LE(sampleRate, 24)
  buffer.writeUInt32LE(sampleRate * 2, 28) // byte rate
  buffer.writeUInt16LE(2, 32) // block align
  buffer.writeUInt16LE(16, 34) // bits
  buffer.write('data', 36, 'ascii')
  buffer.writeUInt32LE(dataBytes, 40)

  for (let i = 0; i < total; i++) {
    // 440 Hz com leve batida de energia — dá trabalho real ao pipeline.
    const t = i / sampleRate
    const amp = 0.6 + 0.3 * Math.sin(2 * Math.PI * 2 * t)
    const sample = Math.round(amp * 0.6 * Math.sin(2 * Math.PI * 440 * t) * 32767)
    buffer.writeInt16LE(sample, 44 + i * 2)
  }
  return buffer
}

let tokenE2E: string

test.beforeAll(async ({ request }) => {
  const health = await request.get('/healthz').catch(() => null)
  if (!health || !health.ok()) {
    test.skip(
      true,
      'Backend Mixlirous não acessível em E2E_BASE_URL — suba a stack (docs/17-GUIA-DE-TESTES.md) para rodar o E2E.',
    )
  }
  tokenE2E = await mintLocalJwt()
})

test('fluxo feliz: upload → job → aprovação HITL → download do artefato', async ({
  page,
}) => {
  test.setTimeout(300_000)

  // Bootstrap de sessão: fulfill determinístico do endpoint que a app
  // consome no mount (ver nota de header). Instalado ANTES do goto.
  await page.route('**/api/v1/auth/local-session', (route) =>
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ token: tokenE2E }),
    }),
  )

  await page.goto('/')

  // A app faz o bootstrap de sessão local no mount (App.tsx): token no
  // localStorage para os comandos REST. Espera o bootstrap terminar.
  await page
    .waitForFunction(() => localStorage.getItem('mixlirous_token') !== null, null, {
      timeout: 15_000,
    })
    .catch(() => {
      // Sem token local (modo SaaS, login futuro) o fluxo segue — e o
      // upload vai falhar com 401 explícito, o que é o comportamento
      // correto sem sessão.
    })

  // Cookie de SSE: o EventSource não manda header, então o handshake
  // valida o cookie `mixlirous_session` emitido AQUI (rota real do Lote 2,
  // só para Bearer válido). Sem a rota (backend pré-Lote 2) a spec se
  // auto-pula — SSE sem auth não completa o fluxo feliz, e o CI não finge.
  const sseSession = await page.request
    .post('/api/v1/auth/sse-session', {
      headers: { Authorization: `Bearer ${tokenE2E}` },
    })
    .catch(() => null)
  if (!sseSession || !sseSession.ok()) {
    test.skip(
      true,
      'Backend sem POST /auth/sse-session (Lote 2, PR #56) — EventSource não autentica e o fluxo feliz completo exige SSE. Suba a stack pós-Lote 2.',
    )
  }

  // 1. Upload: presign → PUT → POST /tracks.
  await page.setInputFiles('[data-testid="upload-input"]', {
    name: 'e2e-fluxo-feliz.wav',
    mimeType: 'audio/wav',
    buffer: wavSintetico(),
  })
  await page.getByTestId('upload-button').click()
  await expect(page.getByTestId('upload-status')).toContainText('Faixa registrada!', {
    timeout: 30_000,
  })

  // 2b. Canvas executável (Lote 3): se a paleta do Lote 1 (PR #55) já
  // estiver no build, monta o grafo POR ELA — o job parte da serialização
  // do grafo (crossfade + normalização LUFS; compression/EQ dinâmico
  // aparecem desabilitados na paleta — ghost tools). Sem a paleta, o App
  // cai no default explícito do grafo vazio e o resto do fluxo é
  // equivalente — a spec fica verde nos dois estados do produto.
  const temPaleta = await page
    .getByRole('button', { name: '+ Transição' })
    .waitFor({ state: 'visible', timeout: 5_000 })
    .then(() => true)
    .catch(() => false)
  if (temPaleta) {
    await page.getByRole('button', { name: '+ Transição' }).click()
    await page.getByRole('button', { name: '+ Normalização LUFS' }).click()
  }

  // 2. Criação do job (modo manual — pipeline direto, sem LLM).
  await page.getByTestId('prompt-input').fill('versão de 30s para o e2e do fluxo feliz')
  await page.getByTestId('create-job').click()

  // 3. HITL: se o agente emitir `agent.proposal`, o overlay abre e é
  // aprovado de verdade. Sem proposta (B5 pendente), anota e segue.
  const overlay = page.getByTestId('proposal-overlay')
  try {
    await overlay.waitFor({ state: 'visible', timeout: 20_000 })
    await page.getByTestId('proposal-approve').click()
    test.info().annotations.push({
      type: 'note',
      description: 'Proposta HITL recebida e aprovada no overlay.',
    })
  } catch {
    test.info().annotations.push({
      type: 'note',
      description:
        'Nenhuma agent.proposal em 20s — worker ainda não popula o ProposalStore ' +
        '(item B5, próxima sprint do plano). O passo de aprovação é exercitado ' +
        'assim que a proposta existir.',
    })
  }

  // 4. Conclusão: o worker processa e o evento job.completed abre o
  // player com o link de download (via SSE autenticado por cookie).
  await expect(page.getByTestId('player')).toBeVisible({ timeout: 180_000 })

  // 5. Download do artefato: o page.request compartilha os cookies do
  // browser — prova o caminho completo (cookie de sessão → rota artifact
  // → WAV legítimo).
  const href = await page.getByTestId('download-link').getAttribute('href')
  expect(href).toContain('/artifact')
  const download = await page.request.get(href!)
  expect(download.ok()).toBeTruthy()
  expect(download.headers()['content-type'] ?? '').toContain('audio/wav')
  const body = await download.body()
  expect(body.length).toBeGreaterThan(44)
  expect(body.subarray(0, 4).toString('ascii')).toBe('RIFF')
})
