import { useEffect } from 'react'
import type { JobResponse } from '../types/api'

interface Props {
  jobs: JobResponse[] | null
  carregando: boolean
  jobAtivoId?: string
  onRefresh: () => void
  onCancel: (jobId: string) => void
  onRetry: (jobId: string) => void
  onAbrir: (jobId: string) => void
}

/** Rótulo e cor por estado — estado SEMPRE em texto, cor é só reforço (Nielsen). */
function chip(status: string): { label: string; classe: string; acao: 'cancel' | 'retry' | 'abrir' | 'nada' } {
  switch (status.toLowerCase()) {
    case 'completed':
      return { label: 'Pronto', classe: 'bg-action-950/70 text-action-200 border-action-800', acao: 'abrir' }
    case 'failed':
      return { label: 'Falhou', classe: 'bg-danger-800/60 text-danger-200 border-danger-700', acao: 'retry' }
    case 'cancelled':
      return { label: 'Cancelado', classe: 'bg-surface-800 text-ink-300 border-surface-600', acao: 'nada' }
    case 'processing':
      return { label: 'Processando', classe: 'bg-ai-800/60 text-ai-200 border-ai-600', acao: 'cancel' }
    case 'queued':
      return { label: 'Na fila', classe: 'bg-manual-800/60 text-manual-200 border-manual-700', acao: 'cancel' }
    default:
      return { label: status, classe: 'bg-surface-800 text-ink-300 border-surface-600', acao: 'nada' }
  }
}

/**
 * Atividade (plano de design §"Navegação"): todos os jobs com estado, hora e
 * a próxima ação possível — cancelar em andamento, tentar de novo em falha
 * (C12: cria NOVO job, o original fica no histórico), abrir o preview.
 */
function AtividadeView({ jobs, carregando, jobAtivoId, onRefresh, onCancel, onRetry, onAbrir }: Props) {
  useEffect(() => {
    onRefresh()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  if (carregando || jobs === null) {
    return <p className="text-ink-300 text-sm" role="status">Carregando atividade…</p>
  }

  if (jobs.length === 0) {
    return (
      <div className="p-6 bg-surface-800 rounded-lg border border-surface-700">
        <h2 className="text-lg font-bold text-ink-100">Nenhum remix ainda</h2>
        <p className="text-ink-300 text-sm mt-2">
          Quando você criar um remix, o histórico aparece aqui com estado, avisos e ações de
          recuperação (cancelar, tentar de novo).
        </p>
      </div>
    )
  }

  return (
    <div>
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-lg font-bold text-ink-100">
          Atividade <span className="text-sm font-normal text-ink-400">({jobs.length} job{jobs.length > 1 ? 's' : ''})</span>
        </h2>
        <button
          type="button"
          onClick={onRefresh}
          className="px-3 py-1.5 text-sm bg-surface-700 hover:bg-surface-600 text-ink-100 rounded"
        >
          Atualizar
        </button>
      </div>
      <ul className="space-y-2" aria-label="Histórico de remixes">
        {jobs.map((j) => {
          const c = chip(j.status)
          return (
            <li
              key={j.job_id}
              className={`p-3 rounded-lg border ${
                j.job_id === jobAtivoId ? 'border-ai-400 bg-ai-950/40' : 'border-surface-700 bg-surface-800'
              }`}
            >
              <div className="flex items-center justify-between gap-3">
                <div className="min-w-0">
                  <p className="text-ink-100 font-medium truncate">
                    Remix {j.job_id.slice(0, 8)}
                    {j.job_id === jobAtivoId && (
                      <span className="ml-2 text-xs text-ai-300">este espaço</span>
                    )}
                  </p>
                  <p className="text-xs text-ink-400">
                    criado em {new Date(j.created_at).toLocaleString('pt-BR')}
                  </p>
                </div>
                <div className="flex items-center gap-2 flex-shrink-0">
                  <span
                    data-testid={`job-status-${j.job_id.slice(0, 8)}`}
                    className={`px-2 py-0.5 text-xs rounded border ${c.classe}`}
                  >
                    {c.label}
                  </span>
                  {c.acao === 'cancel' && (
                    <button
                      type="button"
                      onClick={() => onCancel(j.job_id)}
                      data-testid={`cancel-${j.job_id.slice(0, 8)}`}
                      className="px-3 py-1.5 text-xs bg-surface-700 hover:bg-surface-600 text-ink-100 rounded"
                      title="Interrompe o job. Nada é cobrado nem perdido — o original continua na biblioteca."
                    >
                      Cancelar
                    </button>
                  )}
                  {c.acao === 'retry' && (
                    <button
                      type="button"
                      onClick={() => onRetry(j.job_id)}
                      data-testid={`retry-${j.job_id.slice(0, 8)}`}
                      className="px-3 py-1.5 text-xs bg-manual-600 hover:bg-manual-500 text-white rounded"
                      title="Cria uma nova tentativa com a mesma receita; o job original fica guardado."
                    >
                      Tentar de novo
                    </button>
                  )}
                  {c.acao === 'abrir' && (
                    <button
                      type="button"
                      onClick={() => onAbrir(j.job_id)}
                      className="px-3 py-1.5 text-xs bg-action-700 hover:bg-action-600 text-white rounded"
                    >
                      Abrir preview
                    </button>
                  )}
                </div>
              </div>
            </li>
          )
        })}
      </ul>
    </div>
  )
}

export default AtividadeView
