/**
 * Utilidades de WAV no browser — plano de design centrado no usuário (P1).
 *
 * O remix não tem `track_id` no backend (o artefato é o WAV final), então a
 * waveform do lado "remix" do preview é calculada AQUI a partir dos bytes do
 * artifact (`GET /jobs/{id}/artifact`, mesmo caminho do download). É o mesmo
 * redutor [min, max] por bucket do `tracks.rs::compute_peaks` (Lote 2, C9) —
 * as duas waveforms ficam comparáveis entre si.
 *
 * Também extrai as métricas técnicas do preview (duração, sample rate,
 * canais, pico dBFS) e o checksum SHA-256 do manifesto de exportação.
 */

/** Métricas técnicas extraídas do cabeçalho RIFF + dados. */
export interface WavMetrics {
  /** Duração em segundos (data chunk / byte rate). */
  durationSec: number
  sampleRate: number
  channels: number
  bitsPerSample: number
  /** Pico absoluto em dBFS (0 dBFS = amostra plena). */
  peakDbfs: number
  /** Pico absoluto linear (0..1+). */
  peakLinear: number
}

/** Reduz PCM já convertido para float [-1, 1] em buckets [min, max]. */
export function reduzirPeaks(
  amostras: Float32Array,
  resolution: number
): Array<[number, number]> {
  const res = Math.max(1, Math.floor(resolution))
  if (amostras.length === 0) return []
  const bucketLen = Math.ceil(amostras.length / res)
  const peaks: Array<[number, number]> = []
  for (let start = 0; start < amostras.length; start += bucketLen) {
    let min = Number.POSITIVE_INFINITY
    let max = Number.NEGATIVE_INFINITY
    const end = Math.min(start + bucketLen, amostras.length)
    for (let i = start; i < end; i++) {
      const s = amostras[i]
      if (!Number.isFinite(s)) continue
      if (s < min) min = s
      if (s > max) max = s
    }
    if (!Number.isFinite(min) || !Number.isFinite(max)) {
      peaks.push([0, 0])
    } else {
      peaks.push([min, max])
    }
  }
  return peaks
}

function toLinearInt16(v: number): number {
  return v / 32768
}

function toLinearInt24(v: number): number {
  return v / 8388608
}

/**
 * Parseia um WAV (RIFF/PCM 16/24/32-int ou 32-float). Fiel ao formato:
 * caminha pelos chunks em vez de assumir offsets fixos — arquivos com
 * chunks extra (LIST, bext) não quebram o parser. Lança `Error` com
 * mensagem amigável quando o arquivo não é um WAV suportado.
 */
export function parseWav(
  bytes: ArrayBuffer
): { metrics: WavMetrics; peaks: Array<[number, number]> } {
  const view = new DataView(bytes)
  if (bytes.byteLength < 44) throw new Error('Arquivo muito curto para ser um WAV.')
  if (view.getUint8(0) !== 0x52 || view.getUint8(1) !== 0x49) {
    // 'RI'
    throw new Error('Arquivo não começa com RIFF — não é um WAV.')
  }
  if (view.getUint8(8) !== 0x57 || view.getUint8(9) !== 0x41) {
    // 'WA'
    throw new Error('RIFF sem formato WAVE — não é um WAV suportado.')
  }

  let offset = 12 // pula RIFF....WAVE
  let fmt: { audioFormat: number; channels: number; sampleRate: number; bits: number } | null =
    null
  let dataStart = -1
  let dataLen = 0

  while (offset + 8 <= bytes.byteLength) {
    const id = String.fromCharCode(
      view.getUint8(offset),
      view.getUint8(offset + 1),
      view.getUint8(offset + 2),
      view.getUint8(offset + 3)
    )
    const size = view.getUint32(offset + 4, true)
    if (id === 'fmt ' && offset + 8 + 16 <= bytes.byteLength) {
      fmt = {
        audioFormat: view.getUint16(offset + 8, true),
        channels: view.getUint16(offset + 10, true),
        sampleRate: view.getUint32(offset + 12, true),
        bits: view.getUint16(offset + 22, true),
      }
    } else if (id === 'data') {
      dataStart = offset + 8
      dataLen = Math.min(size, bytes.byteLength - dataStart)
    }
    // Chunks são word-aligned (2 bytes).
    offset += 8 + size + (size % 2)
    if (size === 0 && id !== 'data') break // proteção contra header malformado
  }

  if (!fmt || dataStart < 0) throw new Error('WAV sem chunks fmt/data reconhecíveis.')

  const { audioFormat, channels, sampleRate, bits } = fmt
  const bytesPerSample = bits / 8
  const frames = Math.floor(dataLen / (bytesPerSample * channels))
  if (frames <= 0) throw new Error('WAV sem frames de áudio.')

  // Mistura os canais (média) para a waveform mono-comparável.
  const mono = new Float32Array(frames)
  let peak = 0
  for (let f = 0; f < frames; f++) {
    let acc = 0
    for (let c = 0; c < channels; c++) {
      const pos = dataStart + (f * channels + c) * bytesPerSample
      let lin = 0
      if (audioFormat === 3 && bits === 32) {
        lin = view.getFloat32(pos, true)
      } else if (bits === 16) {
        lin = toLinearInt16(view.getInt16(pos, true))
      } else if (bits === 32) {
        lin = toLinearInt16(view.getInt32(pos, true) / 2147483648 * 32768)
      } else if (bits === 24) {
        const b0 = view.getUint8(pos)
        const b1 = view.getUint8(pos + 1)
        const b2 = view.getUint8(pos + 2)
        let v = (b2 << 16) | (b1 << 8) | b0
        if (v & 0x800000) v -= 0x1000000 // sinal
        lin = toLinearInt24(v)
      } else if (bits === 8) {
        lin = (view.getUint8(pos) - 128) / 128
      }
      acc += lin
    }
    const s = acc / channels
    mono[f] = s
    const abs = Math.abs(s)
    if (abs > peak) peak = abs
  }

  const durationSec = frames / sampleRate
  const metrics: WavMetrics = {
    durationSec,
    sampleRate,
    channels,
    bitsPerSample: bits,
    peakLinear: peak,
    peakDbfs: peak > 0 ? 20 * Math.log10(peak) : Number.NEGATIVE_INFINITY,
  }

  return { metrics, peaks: reduzirPeaks(mono, 1024) }
}

/** SHA-256 hex (manifesto de exportação — plano de design §preview). */
export async function sha256Hex(bytes: ArrayBuffer): Promise<string> {
  const digest = await crypto.subtle.digest('SHA-256', bytes)
  return [...new Uint8Array(digest)]
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('')
}

export function formatarDuracao(seg: number): string {
  if (!Number.isFinite(seg)) return '—'
  const m = Math.floor(seg / 60)
  const s = Math.round(seg - m * 60)
  return `${m}:${String(s).padStart(2, '0')}`
}
