/**
 * Testes do filtro de ferramentas — Item 4 do Lote 1 (plano Pareto).
 * Garante que ghost tools (compression, dynamic_eq) ficam na lista
 * indisponível mesmo se o backend mudar a ordem do registry.
 */
import { describe, it, expect } from 'vitest'
import {
  describeUnavailableReason,
  filterAvailableTools,
  partitionToolsByAvailability,
} from '../toolFilter'
import type { ToolInfo } from '../../types/api'

function tool(overrides: Partial<ToolInfo>): ToolInfo {
  return {
    name: 'x',
    label_ptbr: 'X',
    category: 'stitching',
    available: true,
    parameters: [],
    ...overrides,
  }
}

const REGISTRY_MIRROR: ToolInfo[] = [
  tool({ name: 'crossfade', label_ptbr: 'Transição', category: 'stitching' }),
  tool({ name: 'fade_in', label_ptbr: 'Fade de entrada', category: 'stitching' }),
  tool({ name: 'fade_out', label_ptbr: 'Fade de saída', category: 'stitching' }),
  tool({ name: 'time_stretch', label_ptbr: 'Esticamento', category: 'mastering' }),
  tool({ name: 'lufs_normalization', label_ptbr: 'Normalização LUFS', category: 'mastering' }),
  tool({
    name: 'compression',
    label_ptbr: 'Compressão',
    category: 'mastering',
    available: false,
    unavailable_reason: 'not_implemented',
  }),
  tool({
    name: 'dynamic_eq',
    label_ptbr: 'EQ dinâmico',
    category: 'mastering',
    available: false,
    unavailable_reason: 'not_implemented',
  }),
  tool({
    name: 'stem_separation',
    label_ptbr: 'Separação de stems',
    category: 'analysis',
    available: false,
    unavailable_reason: 'not_implemented',
  }),
]

describe('partitionToolsByAvailability', () => {
  it('espelha o estado real do registry: 5 disponíveis, 3 ghost', () => {
    const { available, unavailable } = partitionToolsByAvailability(REGISTRY_MIRROR)
    expect(available.map((t) => t.name)).toEqual([
      'crossfade',
      'fade_in',
      'fade_out',
      'time_stretch',
      'lufs_normalization',
    ])
    expect(unavailable.map((t) => t.name)).toEqual([
      'compression',
      'dynamic_eq',
      'stem_separation',
    ])
  })

  it('não perde nem duplica ferramenta na partição', () => {
    const { available, unavailable } = partitionToolsByAvailability(REGISTRY_MIRROR)
    expect(available.length + unavailable.length).toBe(REGISTRY_MIRROR.length)
  })

  it('registry vazio produz duas listas vazias (sem erro)', () => {
    const { available, unavailable } = partitionToolsByAvailability([])
    expect(available).toEqual([])
    expect(unavailable).toEqual([])
  })
})

describe('filterAvailableTools', () => {
  it('retorna só as com available: true', () => {
    expect(filterAvailableTools(REGISTRY_MIRROR)).toHaveLength(5)
  })
})

describe('describeUnavailableReason', () => {
  it('traduz not_implemented mencionando a regra de DSP', () => {
    const msg = describeUnavailableReason('not_implemented')
    expect(msg).toContain('DSP')
  })

  it('traduz requires_plan_pro', () => {
    expect(describeUnavailableReason('requires_plan_pro')).toContain('Pro')
  })

  it('código desconhecido preserva o código na mensagem', () => {
    expect(describeUnavailableReason('motivo_novo_do_backend')).toContain(
      'motivo_novo_do_backend',
    )
  })

  it('reason ausente devolve mensagem genérica truthy', () => {
    expect(describeUnavailableReason(undefined)).toBeTruthy()
    expect(describeUnavailableReason('')).toBeTruthy()
  })
})
