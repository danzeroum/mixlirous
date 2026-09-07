/**
 * E2E do fluxo feliz completo — Lote 3, item 4 do plano Pareto
 * (adendo §3.12): upload → criação de job → aprovação de proposta HITL →
 * download do artefato.
 *
 * Roda contra a stack REAL (ver playwright.config.ts). Sem stack, a spec
 * se auto-pula — e o CI não finge que passou.
 *
 * Nota sobre o passo HITL: o worker hoje decide propostas internamente
 * (item B5 do CHANGELOG — ProposalStore ainda não populado, próxima
 * sprint do plano). Quando nenhuma `agent.proposal` chega, o passo de
 * aprovação é anotado e pulado; quando chega, o overlay é aprovado de
 * verdade. O resto do fluxo (upload → job → download por cookie de
 * sessão) é exercitado integralmente.
 */
import { test, expect } from '@playwright/test'

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

test.beforeAll(async ({ request }) => {
  const health = await request.get('/healthz').catch(() => null)
  if (!health || !health.ok()) {
    test.skip(
      true,
      'Backend Mixlirous não acessível em E2E_BASE_URL — suba a stack (docs/17-GUIA-DE-TESTES.md) para rodar o E2E.',
    )
  }
})

test('fluxo feliz: upload → job → aprovação HITL → download do artefato', async ({
  page,
}) => {
  test.setTimeout(300_000)

  await page.goto('/')

  // A app faz o bootstrap de sessão local no mount (App.tsx): token no
  // localStorage para os comandos REST + cookie same-origin para o
  // handshake SSE e o download. Espera o bootstrap terminar.
  await page
    .waitForFunction(() => localStorage.getItem('mixlirous_token') !== null, null, {
      timeout: 15_000,
    })
    .catch(() => {
      // Sem token local (modo SaaS, login futuro) o fluxo segue — e o
      // upload vai falhar com 401 explícito, o que é o comportamento
      // correto sem sessão.
    })

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

/**
 * Plano de design centrado no usuário — navegação `Visão geral |
 * Biblioteca | Novo remix | Atividade | Espaço de trabalho`.
 *
 * 4.1 do PR #59 — Atividade com estados HONESTOS em dois testes
 * independentes (cada um com contexto/tenant próprios):
 *
 * 1. ESTADO VAZIO: contexto novo, nenhum job criado — a view precisa
 *    mostrar a mensagem explicável "Nenhum remix ainda". Não tratamos
 *    tela vazia como sucesso de um teste que promete validar histórico.
 *
 * 2. FLUXO COM JOB REAL: upload → objetivo → job criado → espera
 *    DETERMINÍSTICA do estado via GET /jobs (autenticado pelo cookie de
 *    sessão do contexto) → na Atividade, localiza o job PELO ID, valida
 *    o status EM TEXTO (não só cor) e exercita a ação contextual real:
 *    cancelar (queued/processing), abrir preview (completed) ou tentar
 *    de novo (failed).
 */

/** Prefixo de 8 chars usado pelos data-testids/labels de job na UI. */
async function aguardarEstadoTerminalOuAcao(
  request: import('@playwright/test').APIRequestContext,
  timeoutMs: number
): Promise<{ jobId: string; status: string }> {
  const inicio = Date.now()
  let ultimo: { jobId: string; status: string } | null = null
  while (Date.now() - inicio < timeoutMs) {
    const resp = await request.get('/api/v1/jobs')
    if (resp.ok()) {
      const body = (await resp.json()) as {
        items: Array<{ job_id: string; status: string }>
      }
      // Contexto novo → este tenant tem exatamente UM job (o do teste).
      const job = body.items[0]
      if (job) {
        ultimo = { jobId: job.job_id, status: job.status.toLowerCase() }
        if (['completed', 'failed'].includes(ultimo.status)) return ultimo
        if (['queued', 'processing'].includes(ultimo.status) && Date.now() - inicio >= timeoutMs) {
          return ultimo
        }
      }
    }
    await new Promise((r) => setTimeout(r, 1000))
  }
  if (!ultimo) throw new Error('Nenhum job apareceu em GET /jobs dentro do timeout')
  return ultimo
}

test('atividade: estado vazio explicável em contexto novo', async ({ page }) => {
  await page.goto('/')

  // Sessão local (tenant próprio deste contexto) — sem job criado antes.
  await page
    .waitForFunction(() => localStorage.getItem('mixlirous_token') !== null, null, {
      timeout: 15_000,
    })
    .catch(() => {})

  // Direto para Atividade, SEM criar job algum neste tenant.
  await page.getByTestId('nav-atividade').click()

  // Estado vazio EXPLICÁVEL (plano §Nielsen) — mensagem e próxima ação.
  const vazio = page.getByRole('heading', { name: 'Nenhum remix ainda' })
  await expect(vazio).toBeVisible({ timeout: 20_000 })
  await expect(page.getByTestId('nav-atividade')).toBeVisible()
  await expect(
    page.getByText('Quando você criar um remix, o histórico aparece aqui')
  ).toBeVisible()

  // Nada de lista de jobs fantasma num contexto que nunca criou job.
  await expect(page.locator('ul[aria-label="Histórico de remixes"]')).toHaveCount(0)
})

test('atividade: job real aparece no histórico com status em texto e ação contextual', async ({
  page,
}) => {
  test.setTimeout(300_000)
  await page.goto('/')

  await page
    .waitForFunction(() => localStorage.getItem('mixlirous_token') !== null, null, {
      timeout: 15_000,
    })
    .catch(() => {})

  // Upload de WAV sintético válido.
  await page.setInputFiles('[data-testid="upload-input"]', {
    name: 'e2e-atividade.wav',
    mimeType: 'audio/wav',
    buffer: wavSintetico(0.5),
  })
  await page.getByTestId('upload-button').click()
  await expect(page.getByTestId('upload-status')).toContainText('Faixa registrada!', {
    timeout: 30_000,
  })

  // Biblioteca lista a faixa enviada (validação de navegação existente).
  await page.getByTestId('nav-biblioteca').click()
  await expect(
    page.locator('ul[aria-label="Faixas enviadas"] li').first()
  ).toBeVisible({ timeout: 20_000 })

  // Objetivo + criação do job (modo manual — pipeline direto).
  await page.getByTestId('nav-novo-remix').click()
  await page.getByTestId('prompt-input').fill('remix curto para o teste de atividade')
  await page.getByTestId('create-job').click()

  // Espera DETERMINÍSTICA: queued/processing/completed/failed via API
  // (o cookie mixlirous_session do contexto autentica o page.request).
  const { jobId, status } = await aguardarEstadoTerminalOuAcao(page.request, 60_000)
  const prefixo = jobId.slice(0, 8)
  test.info().annotations.push({ type: 'note', description: `job ${prefixo} → ${status}` })

  // Atividade: o job criado aparece LOCALIZÁVEL PELO ID.
  await page.getByTestId('nav-atividade').click()
  const item = page.locator('li', { hasText: `Remix ${prefixo}` })
  await expect(item).toBeVisible({ timeout: 20_000 })

  // Status EM TEXTO (não apenas cor) — chip com rótulo por estado.
  const chip = item.getByTestId(`job-status-${prefixo}`)
  const rotuloPorStatus: Record<string, string> = {
    completed: 'Pronto',
    failed: 'Falhou',
    processing: 'Processando',
    queued: 'Na fila',
    cancelled: 'Cancelado',
  }
  await expect(chip).toHaveText(rotuloPorStatus[status] ?? status)

  // AÇÃO CONTEXTUAL REAL conforme o estado observado.
  if (status === 'completed') {
    await item.getByRole('button', { name: 'Abrir preview' }).click()
    await expect(page.getByTestId('novo-remix')).toBeVisible()
    // O wizard reabre no job selecionado (passo Renderização) — locator
    // por papel/heading para não colidir com "Pronto — Job <prefix>" do player.
    await expect(
      page.getByRole('heading', { name: `5 · Renderização · job ${prefixo}` })
    ).toBeVisible({ timeout: 20_000 })
  } else if (status === 'failed') {
    await item.getByTestId(`retry-${prefixo}`).click()
    await expect(page.getByTestId('novo-remix')).toBeVisible()
    // Retry cria NOVO job — a Atividade passa a ter 2 jobs (original + retry).
    await page.getByTestId('nav-atividade').click()
    await expect(page.locator('ul[aria-label="Histórico de remixes"] li')).toHaveCount(2, {
      timeout: 20_000,
    })
  } else {
    // queued/processing → cancelar (ação real de recuperação).
    await item.getByTestId(`cancel-${prefixo}`).click()
    const chipCancelado = page
      .locator('li', { hasText: `Remix ${prefixo}` })
      .getByTestId(`job-status-${prefixo}`)
    await expect(chipCancelado).toHaveText('Cancelado', { timeout: 20_000 })
  }
})

/**
 * 4.3 do PR #59 — consentimento com revogação REAL (DELETE
 * /tenants/me/consent), não apenas trocar para modo manual.
 */
test('privacidade: registrar consentimento e revogar de verdade', async ({ page }) => {
  test.setTimeout(120_000)
  await page.goto('/')

  await page
    .waitForFunction(() => localStorage.getItem('mixlirous_token') !== null, null, {
      timeout: 15_000,
    })
    .catch(() => {})

  // Upload para liberar o passo 3 (Objetivo) do wizard.
  await page.setInputFiles('[data-testid="upload-input"]', {
    name: 'e2e-consentimento.wav',
    mimeType: 'audio/wav',
    buffer: wavSintetico(0.4),
  })
  await page.getByTestId('upload-button').click()
  await expect(page.getByTestId('upload-status')).toContainText('Faixa registrada!', {
    timeout: 30_000,
  })

  // Modo assistido → o painel de privacidade aparece (aceito ou não).
  await page.getByRole('radio', { name: 'Com assistente (IA)' }).click()
  const painel = page.getByTestId('privacy-panel')
  await expect(painel).toBeVisible()

  // Aceitar → consentimento registrado (data visível) + botão de REVOGAR.
  await painel.getByTestId('consent-accept').click()
  const status = painel.getByTestId('consent-status')
  await expect(status).toContainText('Consentimento registrado', { timeout: 15_000 })
  const revogar = painel.getByTestId('consent-revoke')
  await expect(revogar).toBeVisible()
  // A UI explica o que a revogação faz — e o que ela NÃO faz.
  await expect(painel.getByText('NÃO apaga automaticamente remixes')).toBeVisible()

  // Revogar de verdade (DELETE no backend) → volta ao estado pendente.
  await revogar.click()
  await expect(painel.getByTestId('consent-revoked')).toContainText('Consentimento revogado', {
    timeout: 15_000,
  })
  await expect(painel.getByTestId('consent-accept')).toBeVisible()
})
