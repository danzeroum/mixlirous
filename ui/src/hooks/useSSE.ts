import { useEffect, useRef, useState, useCallback } from 'react'

/**
 * Evento SSE normalizado. O campo `type` vem do campo `event:` do frame SSE
 * (e não é disparado no `onmessage` do EventSource — ver nota abaixo).
 * O campo `id` é o `Last-Event-ID` se o backend enviar; usado para replay.
 */
export interface StreamEvent {
  id?: string
  type: string
  data: Record<string, unknown>
}

/**
 * Catálogo de eventos SSE que o frontend conhece (docs/03-CONTRATOS-API.md §5).
 * Mantido aqui em vez de um enum `SSEEventType` fechado para permitir eventos
 * futuros sem quebrar consumidores — mas o `addEventListener` é registrado para
 * cada nome conhecido. Eventos desconhecidos (não neste array) ainda são
 * capturados pelo listener `message` (caso o backend os envie sem `event:`).
 */
const KNOWN_EVENT_TYPES = [
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
] as const

export interface UseSSEOptions {
  /** Callback chamado uma vez por evento recebido (em adição ao state). */
  onEvent?: (event: StreamEvent) => void
  /** Limite de eventos retidos no state (default 200 — alinha com buffer backend). */
  maxEvents?: number
}

/**
 * Consome o SSE de eventos de um job (docs/03-CONTRATOS-API.md §5).
 *
 * Item B2 do mapa de ação: a versão anterior usava apenas `source.onmessage`,
 * que só dispara para frames SSE **sem** campo `event:`. Como o backend emite
 * eventos nomeados (`agent.thought`, `job.state`, etc.), o hook recebia zero
 * frames. Aqui registramos `addEventListener('<event_name>', ...)` para cada
 * tipo conhecido — além de um fallback `onmessage` para frames sem nome.
 *
 * Outros fixes:
 * - **Reconexão**: removido o `source.close()` em `onerror` que matava o
 *   auto-reconnect nativo do EventSource. Em vez disso, marcamos `connected`
 *   como false e deixamos o browser reestabelecer a conexão sozinho.
 * - **Last-Event-ID**: capturamos o `id` de cada frame para que, na
 *   reconexão, o backend possa fazer replay (verificar docs/03 §5). O envio
 *   do header é automático pelo EventSource nativo quando ele reconecta.
 */
export function useSSE(jobId: string | undefined, options: UseSSEOptions = {}) {
  const eventSourceRef = useRef<EventSource | null>(null)
  const lastEventIdRef = useRef<string | undefined>(undefined)
  const [events, setEvents] = useState<StreamEvent[]>([])
  const [connected, setConnected] = useState(false)
  const { onEvent, maxEvents = 200 } = options

  // Manter referência estável do callback para não re-registrar listeners.
  const onEventRef = useRef(onEvent)
  useEffect(() => {
    onEventRef.current = onEvent
  }, [onEvent])

  useEffect(() => {
    if (!jobId) return

    const url = `/api/v1/jobs/${jobId}/events`
    const source = new EventSource(url)
    eventSourceRef.current = source

    const pushEvent = (type: string, raw: string) => {
      let data: Record<string, unknown>
      try {
        data = JSON.parse(raw) as Record<string, unknown>
      } catch {
        // Frame com JSON inválido: ainda criamos um evento, mas com payload
        // bruto em `data._raw` para debug — não silenciamos porque isso é
        // exatamente o tipo de coisa que precisa aparecer no DevTools.
        data = { _raw: raw }
      }
      const event: StreamEvent = { type, data }
      if (lastEventIdRef.current !== undefined) {
        event.id = lastEventIdRef.current
      }
      setEvents((prev) => {
        const next = [...prev, event]
        // Capa no máximo em `maxEvents` para não vazar memória em jobs longos.
        if (next.length > maxEvents) {
          return next.slice(next.length - maxEvents)
        }
        return next
      })
      onEventRef.current?.(event)
    }

    source.onopen = () => setConnected(true)

    // Fallback para eventos sem `event:` (default type = 'message').
    source.onmessage = (event) => {
      // `event.lastEventId` é populado quando o backend envia `id:` no frame.
      if (event.lastEventId) {
        lastEventIdRef.current = event.lastEventId
      }
      pushEvent(event.type || 'message', event.data)
    }

    // Listener explícito para cada tipo nomeado conhecido. Esta é a correção
    // do bug B4 do relatório: antes só tínhamos `onmessage`, que NÃO captura
    // eventos com `event:` definido.
    const namedHandler = (event: MessageEvent) => {
      if (event.lastEventId) {
        lastEventIdRef.current = event.lastEventId
      }
      pushEvent(event.type, event.data)
    }
    for (const type of KNOWN_EVENT_TYPES) {
      source.addEventListener(type, namedHandler)
    }

    // NÃO chamar `source.close()` em onerror — o EventSource nativo
    // reconecta sozinho (com `retry:` do backend = 3s). Fechar aqui mataria
    // essa reconexão automática. Só marcamos como desconectado.
    source.onerror = () => {
      setConnected(false)
      // O browser continua tentando reconectar em background. O
      // `lastEventIdRef` será enviado automaticamente pelo EventSource
      // quando a reconexão suceder — permitindo replay no backend (B7).
    }

    return () => {
      // Cleanup só quando o jobId muda ou o componente desmonta — aqui sim.
      source.close()
      eventSourceRef.current = null
      setConnected(false)
    }
  }, [jobId, maxEvents])

  /** Limpa o histórico de eventos (manual). */
  const clear = useCallback(() => setEvents([]), [])

  // `lastEventId` é exposto como callback em vez de ref direta — ler
  // `lastEventIdRef.current` durante o render é proibido por
  // `react-hooks/refs`. O caller que precisar pode acessar via getter.
  const getLastEventId = useCallback(() => lastEventIdRef.current, [])

  return { events, connected, clear, getLastEventId }
}
