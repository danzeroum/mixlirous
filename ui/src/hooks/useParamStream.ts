import { useEffect, useRef, useState } from 'react'

export interface StreamEvent {
  type: string
  data: Record<string, unknown>
}

/** Consome o SSE de eventos de um job (docs/03-CONTRATOS-API.md §5). */
export function useParamStream(jobId: string | undefined) {
  const eventSourceRef = useRef<EventSource | null>(null)
  const [events, setEvents] = useState<StreamEvent[]>([])
  const [connected, setConnected] = useState(false)

  useEffect(() => {
    if (!jobId) return

    const source = new EventSource(`/api/v1/jobs/${jobId}/events`)
    eventSourceRef.current = source

    source.onopen = () => setConnected(true)

    source.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data) as Record<string, unknown>
        setEvents((prev) => [...prev, { type: event.type || 'message', data }])
      } catch {
        // ignora frames com JSON inválido
      }
    }

    source.onerror = () => {
      setConnected(false)
      source.close()
    }

    return () => {
      source.close()
      eventSourceRef.current = null
    }
  }, [jobId])

  return { events, connected }
}
