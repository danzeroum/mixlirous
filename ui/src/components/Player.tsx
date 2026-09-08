import { useState, useRef, useEffect, useCallback, useMemo } from 'react'
import Waveform from './Waveform'
import { authHeaders } from '../lib/authHeaders'
import { parseWav, sha256Hex, formatarDuracao, avisoCanais } from '../lib/wavPeaks'
import type { WavMetrics } from '../lib/wavPeaks'

interface Props {
  /** Job ID — usado para buscar o artifact remixado. */
  jobId: string
  /** Track original — alimenta a waveform do lado "original" (Lote 2). */
  trackId?: string | null
  /** URL de download publicada no evento `job.completed` (item B4). */
  downloadUrl: string
  /** Notifica o pai com as métricas do remix (plano de design §preview). */
  onMetrics?: (m: WavMetrics | null) => void
}

/** Manifesto de exportação (plano de design §preview: "manifesto/checksum"). */
interface Manifesto {
  arquivo: string
  bytes: number
  sha256: string
  job_id: string
  exportadoEm: string
  duracao: string
  sampleRate: number
  canais: number
  picoDbfs: string
}

/**
 * Player comparativo A/B (Design Brief §Tela 7 + plano de design §preview).
 *
 * - Remix: bytes do artifact (mesmo caminho do download) → waveform, métricas
 *   técnicas e SHA-256 do manifesto.
 * - Original: `GET /tracks/{id}/peaks` (Lote 2) para a waveform; o áudio em
 *   si vem de `GET /tracks/{id}/raw` (sem upload manual).
 * - A/B sincronizado: troca mantendo a posição; waveform com agulha.
 */
function Player({ jobId, trackId, downloadUrl, onMetrics }: Props) {
  const remixAudioRef = useRef<HTMLAudioElement | null>(null)
  const originalAudioRef = useRef<HTMLAudioElement | null>(null)
  const [activeSource, setActiveSource] = useState<'remix' | 'original'>('remix')
  const [loadError, setLoadError] = useState<string | null>(null)
  const [progresso, setProgresso] = useState(0)
  const [metricsRemix, setMetricsRemix] = useState<WavMetrics | null>(null)
  const [peaksRemix, setPeaksRemix] = useState<Array<[number, number]> | null>(null)
  const [peaksOriginal, setPeaksOriginal] = useState<Array<[number, number]> | null>(null)
  // Fase A do épico estéreo: canais do ORIGINAL (decode real no backend) —
  // alimenta o aviso "arquivo estéreo → render mono" do preview.
  const [canaisOriginal, setCanaisOriginal] = useState<number | null>(null)
  const [manifesto, setManifesto] = useState<Manifesto | null>(null)
  const [erroRemix, setErroRemix] = useState<string | null>(null)

  // 1. Baixa o remix uma vez: waveform + métricas + checksum do manifesto.
  useEffect(() => {
    let cancelado = false
    ;(async () => {
      try {
        const resp = await fetch(downloadUrl, { headers: authHeaders() })
        if (!resp.ok) throw new Error(`artifact HTTP ${resp.status}`)
        const bytes = await resp.arrayBuffer()
        const { metrics, peaks } = parseWav(bytes)
        if (cancelado) return
        setMetricsRemix(metrics)
        setPeaksRemix(peaks)
        onMetrics?.(metrics)
        const sha = await sha256Hex(bytes)
        if (cancelado) return
        setManifesto({
          arquivo: `remix-${jobId}.wav`,
          bytes: bytes.byteLength,
          sha256: sha,
          job_id: jobId,
          exportadoEm: new Date().toISOString(),
          duracao: formatarDuracao(metrics.durationSec),
          sampleRate: metrics.sampleRate,
          canais: metrics.channels,
          picoDbfs: metrics.peakDbfs.toFixed(1),
        })
      } catch (e) {
        if (!cancelado) {
          // Sem waveform/checksum o player continua funcional — degrada com aviso.
          setErroRemix(e instanceof Error ? e.message : 'Falha ao analisar o remix.')
          onMetrics?.(null)
        }
      }
    })()
    return () => {
      cancelado = true
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [downloadUrl, jobId])

  // 2. Waveform do original (peaks do backend — Lote 2/C9).
  useEffect(() => {
    if (!trackId) return
    let cancelado = false
    ;(async () => {
      try {
        const resp = await fetch(`/api/v1/tracks/${trackId}/peaks?resolution=512`, {
          headers: authHeaders(),
        })
        if (!resp.ok) return
        const body = (await resp.json()) as { peaks: Array<[number, number]>; channels?: number }
        if (!cancelado) {
          setPeaksOriginal(body.peaks)
          if (typeof body.channels === 'number') setCanaisOriginal(body.channels)
        }
      } catch {
        // waveform original é opcional — não bloqueia o preview
      }
    })()
    return () => {
      cancelado = true
    }
  }, [trackId])

  // 3. Áudio do original por track_id (Lote 2) — derivado, sem effect:
  //    rota raw quando existe track; upload manual vira fallback (e tem
  //    precedência). URLs blob são revogadas no efeito de cleanup abaixo.
  const rawUrl = trackId ? `/api/v1/tracks/${trackId}/raw` : null
  const [arquivoManual, setArquivoManual] = useState<string | null>(null)
  const originalUrl = arquivoManual ?? rawUrl

  // Progresso para a agulha da waveform ativa.
  const handleTimeUpdate = useCallback(() => {
    const el = activeSource === 'remix' ? remixAudioRef.current : originalAudioRef.current
    if (!el || !Number.isFinite(el.duration) || el.duration <= 0) return
    setProgresso(el.currentTime / el.duration)
  }, [activeSource])

  // Toggle A/B: alterna entre remix e original, mantendo a posição.
  const handleToggle = useCallback(() => {
    if (activeSource === 'remix' && !originalUrl) {
      setLoadError('Carregue o arquivo original para comparar A/B.')
      return
    }
    const newSource = activeSource === 'remix' ? 'original' : 'remix'
    const currentTime = remixAudioRef.current?.currentTime ?? 0
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
  }, [activeSource, originalUrl])

  const handleOriginalUpload = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    if (!file) return
    if (arquivoManual) URL.revokeObjectURL(arquivoManual)
    setArquivoManual(URL.createObjectURL(file))
    setPeaksOriginal(null)
    setLoadError(null)
  }, [arquivoManual])

  // Cleanup das object URLs ao desmontar.
  useEffect(() => {
    return () => {
      if (arquivoManual) URL.revokeObjectURL(arquivoManual)
    }
  }, [arquivoManual])

  const playRemix = useCallback(() => {
    setActiveSource('remix')
    originalAudioRef.current?.pause()
    remixAudioRef.current?.play().catch(() => {})
  }, [])

  const playOriginal = useCallback(() => {
    if (!originalUrl) {
      setLoadError('Carregue o arquivo original para comparar A/B.')
      return
    }
    setActiveSource('original')
    remixAudioRef.current?.pause()
    originalAudioRef.current?.play().catch(() => {})
  }, [originalUrl])

  const tamanhoMb = useMemo(
    () => (manifesto ? (manifesto.bytes / (1024 * 1024)).toFixed(2) : null),
    [manifesto]
  )

  return (
    <div
      data-testid="player"
      className="bg-surface-800/95 backdrop-blur p-4 rounded-lg border border-surface-700 shadow-lg"
    >
      <div className="flex items-center justify-between mb-3">
        <h3 className="text-lg font-bold text-ink-100">
          Pronto — Job {jobId.slice(0, 8)}...
        </h3>
        {downloadUrl && (
          <a
            href={downloadUrl}
            download={`remix-${jobId}.wav`}
            data-testid="download-link"
            onClick={() => setManifesto((m) => (m ? { ...m, exportadoEm: new Date().toISOString() } : m))}
            className="px-3 py-1.5 bg-action-700 hover:bg-action-600 text-white rounded text-sm"
          >
            ⬇ Exportar WAV
          </a>
        )}
      </div>

      {/* Waveforms comparadas — mesma escala, agulha no lado ativo */}
      <div className="grid grid-cols-2 gap-4 mb-3">
        <div>
          <p className="text-xs text-ink-400 mb-1">
            Remix {metricsRemix && `· ${formatarDuracao(metricsRemix.durationSec)}`}
          </p>
          {peaksRemix ? (
            <Waveform
              peaks={peaksRemix}
              height={56}
              progress={activeSource === 'remix' ? progresso : undefined}
              ariaLabel="Forma de onda do remix"
              className="w-full bg-surface-900/60 rounded"
            />
          ) : (
            <div className="h-14 rounded bg-surface-900/60" aria-hidden />
          )}
        </div>
        <div>
          <p className="text-xs text-ink-400 mb-1">Original</p>
          {peaksOriginal ? (
            <Waveform
              peaks={peaksOriginal}
              height={56}
              progress={activeSource === 'original' ? progresso : undefined}
              ariaLabel="Forma de onda do original"
              className="w-full bg-surface-900/60 rounded"
            />
          ) : (
            <div className="h-14 rounded bg-surface-900/60" aria-hidden />
          )}
          {avisoCanais(canaisOriginal) && (
            <p role="status" data-testid="player-mono-notice" className="mt-1 text-[11px] text-warn-300">
              {avisoCanais(canaisOriginal)}
            </p>
          )}
        </div>
      </div>

      <div className="grid grid-cols-2 gap-4">
        {/* REMIX */}
        <div
          className={`p-3 rounded border ${
            activeSource === 'remix'
              ? 'border-ai-400 bg-ai-950/40'
              : 'border-surface-700'
          }`}
        >
          <div className="flex items-center justify-between mb-2">
            <span className="text-sm text-ink-300">Remix</span>
            {activeSource === 'remix' && (
              <span className="text-xs text-ai-300">▶ tocando</span>
            )}
          </div>
          <audio
            ref={remixAudioRef}
            src={downloadUrl || undefined}
            controls
            className="w-full"
            onPlay={playRemix}
            onTimeUpdate={handleTimeUpdate}
          />
        </div>

        {/* ORIGINAL */}
        <div
          className={`p-3 rounded border ${
            activeSource === 'original'
              ? 'border-manual-400 bg-manual-950/40'
              : 'border-surface-700'
          }`}
        >
          <div className="flex items-center justify-between mb-2">
            <span className="text-sm text-ink-300">
              Original {originalUrl ? '' : '(carregue abaixo)'}
            </span>
            {activeSource === 'original' && (
              <span className="text-xs text-manual-300">▶ tocando</span>
            )}
          </div>
          <audio
            ref={originalAudioRef}
            src={originalUrl ?? undefined}
            controls
            className="w-full"
            onPlay={playOriginal}
            onTimeUpdate={handleTimeUpdate}
          />
        </div>
      </div>

      <div className="flex items-center gap-3 mt-3">
        <button
          onClick={handleToggle}
          disabled={!originalUrl && activeSource === 'remix'}
          className="px-3 py-1.5 bg-surface-700 hover:bg-surface-600 text-ink-100 rounded text-sm disabled:opacity-50"
          title="Alterna entre remix e original mantendo a posição (como profissionais comparam)."
        >
          ⇄ Alternar A/B
        </button>
        {!trackId && (
          <label className="px-3 py-1.5 bg-surface-700 hover:bg-surface-600 text-ink-100 rounded text-sm cursor-pointer">
            ⬆ Carregar original
            <input
              type="file"
              accept="audio/*,.wav,.flac,.aiff,.mp3,.m4a,.aac"
              onChange={handleOriginalUpload}
              className="hidden"
            />
          </label>
        )}
        {loadError && (
          <span className="text-xs text-danger-400" role="alert">{loadError}</span>
        )}
      </div>

      {/* Manifesto de exportação — integridade verificável */}
      {manifesto && (
        <details className="mt-3 text-xs" data-testid="export-manifest">
          <summary className="cursor-pointer text-ink-300 select-none">
            Manifesto do arquivo ({tamanhoMb} MB)
          </summary>
          <dl className="mt-2 grid grid-cols-2 gap-x-4 gap-y-1 bg-surface-900/70 p-3 rounded font-mono">
            <dt className="text-ink-400">arquivo</dt>
            <dd className="text-ink-100 break-all">{manifesto.arquivo}</dd>
            <dt className="text-ink-400">bytes</dt>
            <dd className="text-ink-100">{manifesto.bytes}</dd>
            <dt className="text-ink-400">duração</dt>
            <dd className="text-ink-100">{manifesto.duracao}</dd>
            <dt className="text-ink-400">sample rate</dt>
            <dd className="text-ink-100">{manifesto.sampleRate} Hz</dd>
            <dt className="text-ink-400">canais</dt>
            <dd className="text-ink-100">{manifesto.canais}</dd>
            <dt className="text-ink-400">pico</dt>
            <dd className="text-ink-100">{manifesto.picoDbfs} dBFS</dd>
            <dt className="text-ink-400">job_id</dt>
            <dd className="text-ink-100 break-all">{manifesto.job_id}</dd>
            <dt className="text-ink-400">sha256</dt>
            <dd className="text-ink-100 break-all" data-testid="manifest-sha256">{manifesto.sha256}</dd>
            <dt className="text-ink-400">exportado em</dt>
            <dd className="text-ink-100">{new Date(manifesto.exportadoEm).toLocaleString('pt-BR')}</dd>
          </dl>
        </details>
      )}

      {erroRemix && (
        <p className="mt-2 text-xs text-warn-300" role="status">
          Não consegui analisar o WAV para waveform/checksum ({erroRemix}) — o download continua
          funcionando normalmente.
        </p>
      )}
    </div>
  )
}

export default Player
