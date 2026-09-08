import { useEffect } from 'react'
import type { TrackResponse } from '../types/api'

interface Props {
  tracks: TrackResponse[] | null
  carregando: boolean
  onUseTrack: (trackId: string) => void
  onRefresh: () => void
}

/**
 * Biblioteca de faixas (plano de design §"Navegação e arquitetura").
 * Lista as faixas do tenant com a próxima ação clara ("Usar no remix").
 * Estado vazio explica o que fazer — sem culpado (§"Liderança positiva").
 */
function BibliotecaView({ tracks, carregando, onUseTrack, onRefresh }: Props) {
  useEffect(() => {
    onRefresh()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  if (carregando || tracks === null) {
    return <p className="text-ink-300 text-sm" role="status">Carregando biblioteca…</p>
  }

  if (tracks.length === 0) {
    return (
      <div className="p-6 bg-surface-800 rounded-lg border border-surface-700">
        <h2 className="text-lg font-bold text-ink-100">Sua biblioteca está vazia</h2>
        <p className="text-ink-300 text-sm mt-2">
          Envie a primeira faixa em <strong>Novo remix</strong> — WAV, FLAC ou MP3. A faixa fica
          disponível aqui para remixar quantas vezes quiser, com receitas diferentes.
        </p>
      </div>
    )
  }

  return (
    <div>
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-lg font-bold text-ink-100">
          Biblioteca <span className="text-sm font-normal text-ink-400">({tracks.length} faixa{tracks.length > 1 ? 's' : ''})</span>
        </h2>
        <button
          type="button"
          onClick={onRefresh}
          className="px-3 py-1.5 text-sm bg-surface-700 hover:bg-surface-600 text-ink-100 rounded"
        >
          Atualizar
        </button>
      </div>
      <ul className="space-y-2" aria-label="Faixas enviadas">
        {tracks.map((t) => (
          <li
            key={t.track_id}
            className="flex items-center justify-between p-3 bg-surface-800 rounded-lg border border-surface-700"
          >
            <div>
              <p className="text-ink-100 font-medium">{t.display_name || 'Faixa sem nome'}</p>
              <p className="text-xs text-ink-400">
                enviada em {new Date(t.created_at).toLocaleString('pt-BR')} · status {t.status.toLowerCase()}
              </p>
            </div>
            <button
              type="button"
              onClick={() => onUseTrack(t.track_id)}
              className="px-3 py-1.5 text-sm bg-ai-600 hover:bg-ai-500 text-white rounded"
            >
              Usar no remix
            </button>
          </li>
        ))}
      </ul>
    </div>
  )
}

export default BibliotecaView
