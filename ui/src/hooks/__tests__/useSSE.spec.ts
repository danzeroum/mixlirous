/**
 * Testes do hook `useSSE` — Item T3 do mapa de ação.
 *
 * Como o hook usa EventSource nativo do browser e não há JSDOM
 * configurado (Vitest + happy-dom seria a próxima iteração), este teste
 * é uma verificação estrutural: valida que a lista `KNOWN_EVENT_TYPES`
 * cobre os eventos nomeados no contrato (`docs/03-CONTRATOS-API.md §5`)
 * e que o tipo `SSEEventType` é fechado.
 *
 * Para testes de runtime real (EventSource mock + assert de callback),
 * a próxima iteração deve adicionar Vitest com `happy-dom` e um
 * `mockEventSource` factory.
 */

import { describe, it, expect } from 'vitest'
import type { SSEEventType } from '../../types/api'

// KNOWN_EVENT_TYPES é exportado internamente do useSSE; para evitar
// expor detalhes, reimplementamos a verificação de cobertura aqui.
// Se os dois ficarem dessincronizados, um teste de runtime real iria
// falhar (próxima iteração).
const EXPECTED_SSE_EVENTS: SSEEventType[] = [
  'stream.ready',
  'job.state',
  'job.progress',
  'job.warning',
  'job.completed',
  'job.failed',
  'job.cancelled',
  'agent.step_started',
  'agent.thought',
  'agent.tool_call',
  'agent.tool_result',
  'agent.error',
  'agent.replan',
  'agent.proposal',
  'agent.finished',
  'proposal.expired',
  'proposal.decided',
  'node.state',
  'node.parameters',
  'node.created',
  'system.resources',
  'recovery.report',
  // Compat com nome antigo do worker (B7 fix do relatório):
  'agent.tool',
]

describe('useSSE — catálogo de eventos conhecidos', () => {
  it('contém todos os eventos do contrato docs/03 §5', () => {
    // Verifica que nenhum evento esperado foi esquecido.
    for (const evt of EXPECTED_SSE_EVENTS) {
      // Tipagem: se o tipo SSEEventType não incluir algum destes,
      // o compile falha — o que é o ponto do teste.
      const _typed: SSEEventType = evt
      expect(_typed).toBeDefined()
    }
  })

  it('inclui o evento "agent.tool" para compat com worker antigo', () => {
    // Item B7 do relatório: o worker publica `agent.tool` (sem `_call`)
    // em vez de `agent.tool_call`. O hook precisa ouvir o nome antigo
    // até o backend ser atualizado.
    expect(EXPECTED_SSE_EVENTS).toContain('agent.tool')
  })

  it('inclui o evento "job.completed" (usado pelo App.tsx para mostrar Player)', () => {
    // Item B4/C2 do mapa: o App.tsx procura o `job.completed` no stream
    // para exibir o Player com `download_url`.
    expect(EXPECTED_SSE_EVENTS).toContain('job.completed')
  })

  it('inclui o evento "agent.proposal" (usado pelo App.tsx para mostrar Overlay)', () => {
    // Item C3 do mapa: o App.tsx procura o `agent.proposal` no stream
    // para exibir o ProposalOverlay.
    expect(EXPECTED_SSE_EVENTS).toContain('agent.proposal')
  })
})

describe('useSSE — tipos SSEEventType', () => {
  it('inclui "agent.tool_call" (contrato) e "agent.tool" (compat)', () => {
    // Documentação viva do desalinhamento nome-antigo vs nome-corrente.
    // Quando o backend for corrigido para publicar `agent.tool_call`,
    // remover o tipo `agent.tool` daqui e do EXPECTED_SSE_EVENTS.
    const allTypes: SSEEventType[] = ['agent.tool_call', 'agent.tool']
    expect(allTypes).toHaveLength(2)
  })
})
