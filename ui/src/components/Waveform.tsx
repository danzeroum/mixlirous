import { useMemo } from 'react'
import { canvasColors } from '../lib/theme'

interface Props {
  /** Pares [min, max] por bucket — backend (`compute_peaks`) ou `wavPeaks.ts`. */
  peaks: Array<[number, number]>
  /** Altura em px do SVG. */
  height?: number
  /** 0..1 — posição de reprodução (opcional; desenha a agulha). */
  progress?: number
  /** Rótulo acessível (role="img"). */
  ariaLabel: string
  className?: string
}

/**
 * Waveform SVG a partir dos pares [min, max] por bucket.
 *
 * Plano de design §preview: "waveform, regiões, cue points" — esta versão
 * cobre a waveform + agulha de progresso; regiões entram quando o job
 * publicar os blocos escolhidos. Não depende de cor isolada para
 * comunicar estado (Nielsen) — a agulha é forma + posição.
 */
function Waveform({ peaks, height = 64, progress, ariaLabel, className }: Props) {
  const w = peaks.length
  const paths = useMemo(() => {
    if (w === 0) return null
    const mid = height / 2
    const scale = (height / 2 - 2) * 0.92
    // Um único path de barras finas — sem estado de cor, sem animação.
    let d = ''
    for (let i = 0; i < w; i++) {
      const [min, max] = peaks[i]
      const y1 = mid - Math.min(max, 1) * scale
      const y2 = mid - Math.max(min, -1) * scale
      const x = i + 0.5
      d += `M${x.toFixed(1)},${y1.toFixed(1)}L${x.toFixed(1)},${y2.toFixed(1)}`
    }
    return d
  }, [peaks, w, height])

  const needle =
    progress !== undefined && w > 0 ? Math.max(0, Math.min(1, progress)) * w : null

  return (
    <svg
      role="img"
      aria-label={ariaLabel}
      viewBox={`0 0 ${Math.max(w, 1)} ${height}`}
      preserveAspectRatio="none"
      className={className ?? 'w-full'}
      style={{ height }}
      data-testid="waveform"
    >
      {/* Linha central (referência de silêncio) */}
      <line
        x1="0"
        y1={height / 2}
        x2={Math.max(w, 1)}
        y2={height / 2}
        stroke={canvasColors.waveformTrack}
        strokeWidth="1"
        vectorEffect="non-scaling-stroke"
      />
      {paths && (
        <path d={paths} stroke={canvasColors.waveformSource} strokeWidth="1" vectorEffect="non-scaling-stroke" />
      )}
      {needle !== null && (
        <line
          x1={needle}
          y1="0"
          x2={needle}
          y2={height}
          stroke={canvasColors.waveformPlayhead}
          strokeWidth="2"
          vectorEffect="non-scaling-stroke"
        />
      )}
    </svg>
  )
}

export default Waveform
