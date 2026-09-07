import { useMemo } from 'react'
import type { StreamEvent } from '../hooks/useSSE'

interface Props {
  events: StreamEvent[]
  /** Estado terminal derivado (para a frase final da timeline). */
  terminal?: 'completed' | 'failed' | 'cancelled' | null
}

/** Ordem de exibição das fases de um job (Nielsen: visibilidade do estado). */
const FASES: Array<{ status: string; titulo: string; descricao: string }> = [
  { status: 'queued', titulo: 'Na fila', descricao: 'O remix aguarda um worker livre.' },
  { status: 'processing', titulo: 'Processando', descricao: 'Corte em blocos, crossfade e masterização.' },
  { status: 'completed', titulo: 'Pronto', descricao: 'O WAV remixado está disponível para ouvir e exportar.' },
]

/**
 * Timeline do job (plano de design §Nielsen: "timeline de job"). Mostra as
 * fases com estado atual marcado e `aria-live="polite"` para leitores de
 * tela. Falhas e cancelamento são exibidos com a próxima ação possível
 * (recuperação sem culpa — plano §"Liderança positiva").
 */
function JobTimeline({ events, terminal }: Props) {
  const fases = useMemo(() => {
    const estados = events.filter((e) => e.type === 'job.state').map((e) => String(e.data.status))
    const atual = terminal ?? (estados.length > 0 ? estados[estados.length - 1] : 'queued')
    return FASES.map((f) => ({
      ...f,
      feita: atual === 'completed' || (f.status === 'queued' && atual !== 'queued') || (f.status === 'processing' && atual === 'completed'),
      atual: atual === f.status,
    }))
  }, [events, terminal])

  const falha = useMemo(() => {
    const last = [...events].reverse().find((e) => e.type === 'job.failed')
    if (!last) return null
    return {
      codigo: String(last.data.code ?? 'job_failed'),
      detalhe: String(last.data.detail ?? 'Não foi possível concluir o remix.'),
    }
  }, [events])

  const cancelado = useMemo(
    () => events.some((e) => e.type === 'job.cancelled' || (e.type === 'job.state' && String(e.data.status) === 'cancelled')),
    [events]
  )

  /**
   * Avisos do pipeline (job.warning) — plano de design: "todo aviso é
   * explicável". O aviso `mono_downmix` (Fase A do épico estéreo) carrega
   * nos `measured` os canais source/analysis/processing/output e a política
   * de downmix; exibimos a mensagem do backend SEMPRE em texto (nunca só
   * cor) e os metadados quando vierem.
   */
  const avisos = useMemo(
    () =>
      events
        .filter((e) => e.type === 'job.warning')
        .map((e) => ({
          code: String(e.data.code ?? 'pipeline_warning'),
          message: String(e.data.message_ptbr ?? ''),
          hint: e.data.hint_ptbr != null ? String(e.data.hint_ptbr) : null,
          measured: (e.data.measured ?? null) as Record<string, unknown> | null,
        })),
    [events]
  )

  return (
    <div aria-live="polite" data-testid="job-timeline">
      <ol className="space-y-2 mt-2">
        {fases.map((f) => (
          <li key={f.status} className="flex items-start gap-2">
            <span
              aria-hidden
              className={`mt-1 inline-block w-2.5 h-2.5 rounded-full border ${
                f.feita
                  ? 'bg-green-500 border-green-500'
                  : f.atual
                    ? 'bg-purple-500 border-purple-400 animate-pulse'
                    : 'bg-transparent border-gray-500'
              }`}
            />
            <div>
              <p className={`text-sm font-medium ${f.atual ? 'text-white' : f.feita ? 'text-gray-200' : 'text-gray-400'}`}>
                {f.titulo}
                {f.atual && <span className="ml-2 text-xs text-purple-300">agora</span>}
              </p>
              <p className="text-xs text-gray-400">{f.descricao}</p>
            </div>
          </li>
        ))}
      </ol>

      {falha && (
        <div role="status" className="mt-3 p-3 rounded bg-red-950/60 border border-red-800">
          <p className="text-sm text-red-200 font-semibold">O remix não concluiu — e não foi culpa sua.</p>
          <p className="text-xs text-red-300 mt-1">
            <code className="bg-red-900/70 px-1 rounded">{falha.codigo}</code> {falha.detalhe}
          </p>
          <p className="text-xs text-red-300 mt-1">
            Use “Tentar de novo” para criar uma nova tentativa com a mesma receita; o job original
            fica guardado no histórico.
          </p>
        </div>
      )}

      {avisos.length > 0 && (
        <ul className="mt-3 space-y-2" aria-label="Avisos do processamento">
          {avisos.map((a, i) => (
            <li
              key={`${a.code}-${i}`}
              role="status"
              data-testid={`job-warning-${a.code}`}
              className="p-3 rounded bg-orange-950/60 border border-orange-800"
            >
              <p className="text-sm text-orange-200">{a.message}</p>
              {a.hint && <p className="text-xs text-orange-300/90 mt-1">{a.hint}</p>}
              {a.measured && (
                <p className="text-[11px] text-orange-300/80 mt-1 font-mono">
                  {Object.entries(a.measured)
                    .map(([k, v]) => `${k}: ${String(v)}`)
                    .join(' · ')}
                </p>
              )}
            </li>
          ))}
        </ul>
      )}

      {cancelado && (
        <div role="status" className="mt-3 p-3 rounded bg-gray-800 border border-gray-600">
          <p className="text-sm text-gray-200 font-semibold">Remix cancelado.</p>
          <p className="text-xs text-gray-400 mt-1">
            Nenhum resultado foi guardado. Você pode criar um novo remix quando quiser.
          </p>
        </div>
      )}
    </div>
  )
}

export default JobTimeline
