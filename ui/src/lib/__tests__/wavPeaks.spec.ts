import { describe, it, expect } from 'vitest'
import { parseWav, reduzirPeaks, formatarDuracao, avisoCanais } from '../wavPeaks'

/** Monta um WAV PCM16 mono válido (mesmo formato que o backend produz). */
function wavPcm16(
  amostras: number[],
  sampleRate = 8000,
  canais = 1
): ArrayBuffer {
  // `amostras` já vem interleaved (L,R,L,R para estéreo).
  const dataBytes = amostras.length * 2
  const buffer = new ArrayBuffer(44 + dataBytes)
  const view = new DataView(buffer)
  const escrever = (off: number, s: string) => {
    for (let i = 0; i < s.length; i++) view.setUint8(off + i, s.charCodeAt(i))
  }
  escrever(0, 'RIFF')
  view.setUint32(4, 36 + dataBytes, true)
  escrever(8, 'WAVE')
  escrever(12, 'fmt ')
  view.setUint32(16, 16, true)
  view.setUint16(20, 1, true) // PCM
  view.setUint16(22, canais, true)
  view.setUint32(24, sampleRate, true)
  view.setUint32(28, sampleRate * 2 * canais, true)
  view.setUint16(32, 2 * canais, true)
  view.setUint16(34, 16, true)
  escrever(36, 'data')
  view.setUint32(40, dataBytes, true)
  let off = 44
  for (const s of amostras) {
    view.setInt16(off, s, true)
    off += 2
  }
  return buffer
}

describe('reduzirPeaks', () => {
  it('retorna [min, max] por bucket (mesma semântica do compute_peaks em Rust)', () => {
    const amostras = new Float32Array([0, 0.5, -0.5, 0, -1, 1])
    const peaks = reduzirPeaks(amostras, 3)
    expect(peaks).toHaveLength(3)
    // 6 amostras / 3 buckets = 2 amostras por bucket
    expect(peaks[0]).toEqual([0, 0.5])
    expect(peaks[1]).toEqual([-0.5, 0])
    expect(peaks[2]).toEqual([-1, 1])
  })

  it('entrada vazia → sem buckets', () => {
    expect(reduzirPeaks(new Float32Array([]), 8)).toEqual([])
  })

  it('NaN não vira pico (bucket vira plano zero)', () => {
    const amostras = new Float32Array([NaN, NaN, NaN, NaN])
    const peaks = reduzirPeaks(amostras, 2)
    expect(peaks).toEqual([
      [0, 0],
      [0, 0],
    ])
  })
})

describe('parseWav', () => {
  it('extrai métricas corretas de um PCM16 mono 8 kHz', () => {
    // 1 s de senoide a 440 Hz, amplitude ~0.6
    const sr = 8000
    const amostras: number[] = []
    for (let i = 0; i < sr; i++) {
      amostras.push(Math.round(0.6 * Math.sin((2 * Math.PI * 440 * i) / sr) * 32767))
    }
    const { metrics, peaks } = parseWav(wavPcm16(amostras, sr))
    expect(metrics.sampleRate).toBe(8000)
    expect(metrics.channels).toBe(1)
    expect(metrics.bitsPerSample).toBe(16)
    expect(metrics.durationSec).toBeCloseTo(1.0, 2)
    expect(metrics.peakLinear).toBeGreaterThan(0.55)
    expect(metrics.peakLinear).toBeLessThan(0.65)
    expect(metrics.peakDbfs).toBeGreaterThan(-6)
    expect(metrics.peakDbfs).toBeLessThan(-3)
    expect(peaks.length).toBeGreaterThan(0)
  })

  it('estéreo: média dos canais na waveform, métricas com 2 canais', () => {
    const sr = 8000
    // 2 frames estéreo interleaved (L,R,L,R) — a média anula o sinal.
    const amostras = [16384, -16384, 16384, -16384]
    const { metrics } = parseWav(wavPcm16(amostras, sr, 2))
    expect(metrics.channels).toBe(2)
    expect(metrics.durationSec).toBeCloseTo(2 / sr, 5)
    // média (16384 + -16384)/2 → amostra 0 em ambos os frames
    expect(metrics.peakLinear).toBeCloseTo(0, 5)
  })

  it('arquivo não-WAV lança erro amigável', () => {
    const naoWav = new ArrayBuffer(64)
    expect(() => parseWav(naoWav)).toThrow()
  })
})

describe('formatarDuracao', () => {
  it('formata m:ss', () => {
    expect(formatarDuracao(65)).toBe('1:05')
    expect(formatarDuracao(9)).toBe('0:09')
    expect(formatarDuracao(Number.NaN)).toBe('—')
  })
})

describe('avisoCanais — Fase A do épico estéreo (transparência de downmix)', () => {
  it('não avisa para mono e para undefined/null (respostas antigas)', () => {
    expect(avisoCanais(1)).toBeNull()
    expect(avisoCanais(undefined)).toBeNull()
    expect(avisoCanais(null)).toBeNull()
  })

  it('avisa com texto honesto para estéreo', () => {
    const aviso = avisoCanais(2)
    expect(aviso).toBeTruthy()
    expect(aviso).toContain('estéreo')
    expect(aviso).toContain('mono')
    expect(aviso).toContain('esquerda e direita')
  })

  it('avisa para multicanal sem nomear errado', () => {
    expect(avisoCanais(6)).toContain('6 canais')
  })
})
