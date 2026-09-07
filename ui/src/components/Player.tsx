import { useState, useRef, useEffect, useCallback } from 'react'

interface Props {
  /** Job ID — usado para buscar o artifact remixado. */
  jobId: string
  /** Track ID do job — alimenta o lado "original" do A/B (Lote 2, item 2). */
  trackId?: string | null
  /** URL de download publicada no evento `job.completed` (item B4). */
  downloadUrl: string
}

/**
 * Player comparativo A/B (Design Brief §Tela 7). Reproduz original e remix
 * lado a lado — troca instantânea mantendo a posição (é como profissionais
 * avaliam áudio).
 *
 * Item C2 do mapa: antes era um placeholder; agora conecta ao artifact real
 * via `GET /api/v1/jobs/{id}/artifact` (item B4).
 *
 * Lote 2 (item 2, adendo Pareto §3.6): o lado "original" agora liga ao
 * `track_id` do job — `GET /api/v1/tracks/{track_id}/raw` — em vez de exigir
 * upload manual do mesmo arquivo no player. O upload manual continua como
 * fallback para quando o track_id não estiver disponível (job antigo).
 */
function Player({ jobId, trackId, downloadUrl }: Props) {
  const remixAudioRef = useRef<HTMLAudioElement | null>(null)
  const originalAudioRef = useRef<HTMLAudioElement | null>(null)
  const [activeSource, setActiveSource] = useState<'remix' | 'original'>('remix')
  const [originalUrl, setOriginalUrl] = useState<string | null>(null)
  // Lote 2: com track_id no job, o original vem do backend — o upload
  // manual vira fallback (job sem track, ou usuário quer comparar com
  // outro arquivo de referência).
  const rawOriginalUrl = trackId ? `/api/v1/tracks/${trackId}/raw` : null
  const effectiveOriginalUrl = originalUrl ?? rawOriginalUrl
  // Item B4: a `downloadUrl` vem direto do evento `job.completed` do worker
  // (`/api/v1/jobs/{id}/artifact`). Usar direto em vez de ir buscar.
  const [loadError, setLoadError] = useState<string | null>(null)

  // Toggle A/B: alterna entre remix e original, mantendo a posição.
  // Se o original não estiver disponível, explica o que falta.
  const handleToggle = useCallback(() => {
    if (activeSource === 'remix' && !effectiveOriginalUrl) {
      setLoadError('Original indisponível: job sem track_id e nenhum arquivo carregado.')
      return
    }
    const newSource = activeSource === 'remix' ? 'original' : 'remix'
    const currentTime = remixAudioRef.current?.currentTime ?? 0
    // Pausa o atual, muda o ativo, posiciona o novo no mesmo instante,
    // retoma se estava tocando.
    const wasPlaying = !remixAudioRef.current?.paused
    remixAudioRef.current?.pause()
    originalAudioRef.current?.pause()
    setActiveSource(newSource)
    requestAnimationFrame(() => {
      const target = newSource === 'remix' ? remixAudioRef.current : originalAudioRef.current
      if (target) {
        target.currentTime = currentTime
        if (wasPlaying) target.play().catch(() => {})
      }
    })
  }, [activeSource, effectiveOriginalUrl])

  const handleOriginalUpload = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    if (!file) return
    if (originalUrl) URL.revokeObjectURL(originalUrl)
    setOriginalUrl(URL.createObjectURL(file))
    setLoadError(null)
  }, [originalUrl])

  // Cleanup das object URLs ao desmontar.
  useEffect(() => {
    return () => {
      if (originalUrl) URL.revokeObjectURL(originalUrl)
    }
  }, [originalUrl])

  const playRemix = useCallback(() => {
    setActiveSource('remix')
    originalAudioRef.current?.pause()
    remixAudioRef.current?.play().catch(() => {})
  }, [])

  const playOriginal = useCallback(() => {
    if (!effectiveOriginalUrl) {
      setLoadError('Original indisponível: job sem track_id e nenhum arquivo carregado.')
      return
    }
    setActiveSource('original')
    remixAudioRef.current?.pause()
    originalAudioRef.current?.play().catch(() => {})
  }, [effectiveOriginalUrl])

  return (
    <div
      data-testid="player"
      className="absolute bottom-4 left-4 right-4 bg-gray-800/95 backdrop-blur p-4 rounded-lg border border-gray-700 shadow-lg z-10"
    >
      <div className="flex items-center justify-between mb-3">
        <h3 className="text-lg font-bold text-white">
          Pronto — Job {jobId.slice(0, 8)}...
        </h3>
        {downloadUrl && (
          <a
            href={downloadUrl}
            download={`remix-${jobId}.wav`}
            data-testid="download-link"
            className="px-3 py-1.5 bg-green-600 hover:bg-green-500 text-white rounded text-sm"
          >
            ⬇ Baixar WAV
          </a>
        )}
      </div>

      <div className="grid grid-cols-2 gap-4">
        {/* REMIX */}
        <div
          className={`p-3 rounded border ${
            activeSource === 'remix'
              ? 'border-purple-500 bg-purple-950/30'
              : 'border-gray-700'
          }`}
        >
          <div className="flex items-center justify-between mb-2">
            <span className="text-sm text-gray-300">Remix</span>
            {activeSource === 'remix' && (
              <span className="text-xs text-purple-400">▶ tocando</span>
            )}
          </div>
          <audio
            ref={remixAudioRef}
            src={downloadUrl || undefined}
            controls
            className="w-full"
            onPlay={playRemix}
          />
        </div>

        {/* ORIGINAL */}
        <div
          className={`p-3 rounded border ${
            activeSource === 'original'
              ? 'border-blue-500 bg-blue-950/30'
              : 'border-gray-700'
          }`}
        >
          <div className="flex items-center justify-between mb-2">
            <span className="text-sm text-gray-300">
              Original {effectiveOriginalUrl ? (trackId && !originalUrl ? '(via track)' : '') : '(carregue abaixo)'}
            </span>
            {activeSource === 'original' && (
              <span className="text-xs text-blue-400">▶ tocando</span>
            )}
          </div>
          <audio
            ref={originalAudioRef}
            src={effectiveOriginalUrl ?? undefined}
            controls
            className="w-full"
            onPlay={playOriginal}
          />
        </div>
      </div>

      <div className="flex items-center gap-3 mt-3">
        <button
          onClick={handleToggle}
          disabled={!effectiveOriginalUrl && activeSource === 'remix'}
          className="px-3 py-1.5 bg-gray-700 hover:bg-gray-600 text-white rounded text-sm disabled:opacity-50"
          title="Alterna entre remix e original mantendo a posição (como profissionais comparam)."
        >
          ⇄ Alternar A/B
        </button>
        <label className="px-3 py-1.5 bg-gray-700 hover:bg-gray-600 text-white rounded text-sm cursor-pointer">
          ⬆ Carregar original {rawOriginalUrl ? '(substituir)' : ''}
          <input
            type="file"
            accept="audio/*,.wav,.flac,.aiff,.mp3,.m4a,.aac"
            onChange={handleOriginalUpload}
            className="hidden"
          />
        </label>
        {loadError && (
          <span className="text-xs text-red-400">{loadError}</span>
        )}
      </div>
    </div>
  )
}

export default Player
