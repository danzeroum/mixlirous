/**
 * Testes da serialização grafo → PipelineConfig (Lote 3, item 1).
 * O golden do grafo canônico é espelhado no teste de contrato Rust
 * `contract_ts_rust.rs::grafo_canonico_do_canvas_desserializa_no_rust`.
 */
import { describe, it, expect } from 'vitest'
import type { Edge } from '@xyflow/react'
import {
  GraphValidationError,
  graphToPipelineConfig,
  summarizeGraph,
} from '../graphToPipeline'
import type { RemixNode } from '../../store/graphStore'

function toolNode(id: string, tool: string, data: Record<string, unknown> = {}): RemixNode {
  return {
    id,
    type: 'effect',
    position: { x: 0, y: 0 },
    data: { label: tool, tool, ...data },
  }
}

function edge(source: string, target: string): Edge {
  return { id: `e-${source}-${target}`, source, target }
}

const GOLDEN_CANONICO = {
  target_duration: { secs: 30, nanos: 0 },
  crossfade: { enabled: true, max_duration_ms: 1200, curve: 'constant_power' },
  mastering: {
    lufs_target: -14.0,
    peak_db: -1.0,
    enable_limiting: true,
    compression_ratio: 2.0,
  },
  selection: {
    min_strong_beat_percentile: 0.8,
    block_size_beats: 4,
    preserve_intro_ms: 3000,
    preserve_outro_ms: 3000,
  },
  format: { sample_rate: 44100, channels: 2, bit_depth: 24, codec: 'WAV' },
  tuning: {
    enabled: false,
    mode: 'disabled',
    max_global_cents: 50.0,
    min_confidence: 0.7,
    force_tonic_hz: null,
    force_mode: null,
  },
}

describe('graphToPipelineConfig — grafo canônico', () => {
  it('crossfade + lufs produzem o golden desserializável no Rust', () => {
    const nodes = [
      toolNode('n1', 'crossfade', { max_duration_ms: 1200 }),
      toolNode('n2', 'lufs_normalization'),
    ]
    const edges = [edge('n1', 'n2')]
    const config = graphToPipelineConfig(nodes, edges)
    expect(config).toEqual(GOLDEN_CANONICO)
  })

  it('grafo vazio lança empty_graph (canvas não pode fingir um default)', () => {
    expect(() => graphToPipelineConfig([], [])).toThrowError(GraphValidationError)
    try {
      graphToPipelineConfig([], [])
    } catch (e) {
      expect((e as GraphValidationError).code).toBe('empty_graph')
    }
  })

  it('sem nó de crossfade, o crossfade desabilita (canvas manda)', () => {
    const nodes = [toolNode('n1', 'lufs_normalization')]
    const config = graphToPipelineConfig(nodes, [])
    expect(config.crossfade.enabled).toBe(false)
    expect(config.mastering.lufs_target).toBe(-14.0)
  })

  it('ciclo lança invalid_graph (o backend rejeitaria com 422)', () => {
    const nodes = [toolNode('a', 'crossfade'), toolNode('b', 'lufs_normalization')]
    const edges = [edge('a', 'b'), edge('b', 'a')]
    expect(() => graphToPipelineConfig(nodes, edges)).toThrowError(/ciclo/)
  })

  it('ghost tool injetada à mão lança ghost_tool (nunca chega ao backend)', () => {
    const nodes = [toolNode('a', 'compression')]
    expect(() => graphToPipelineConfig(nodes, [])).toThrowError(/compression/)
  })

  it('valores fora dos newtypes caem no default (idem TryFrom do Rust)', () => {
    const nodes = [
      toolNode('a', 'crossfade', { max_duration_ms: 50000 }),
      toolNode('b', 'lufs_normalization', { lufs_target: 0 }),
    ]
    const config = graphToPipelineConfig(nodes, [edge('a', 'b')])
    expect(config.crossfade.max_duration_ms).toBe(3000)
    expect(config.mastering.lufs_target).toBe(-14.0)
  })

  it('lufs_target válido do nó é respeitado', () => {
    const nodes = [toolNode('a', 'lufs_normalization', { lufs_target: -16.0 })]
    const config = graphToPipelineConfig(nodes, [])
    expect(config.mastering.lufs_target).toBe(-16.0)
  })

  it('time_stretch/fades são aceitos e reportados como unmapped', () => {
    const nodes = [
      toolNode('a', 'fade_in'),
      toolNode('b', 'time_stretch'),
      toolNode('c', 'lufs_normalization'),
    ]
    const edges = [edge('a', 'b'), edge('b', 'c')]
    const summary = summarizeGraph(nodes, edges)
    expect(summary.toolOrder).toEqual(['fade_in', 'time_stretch', 'lufs_normalization'])
    expect(summary.unmappedTools.sort()).toEqual(['fade_in', 'time_stretch'])
    const config = graphToPipelineConfig(nodes, edges)
    expect(config.mastering.enable_limiting).toBe(true)
  })

  it('ordem topológica segue as arestas, não a inserção', () => {
    const nodes = [
      toolNode('n2', 'lufs_normalization'),
      toolNode('n1', 'crossfade', { max_duration_ms: 1200 }),
    ]
    const edges = [edge('n1', 'n2')]
    const summary = summarizeGraph(nodes, edges)
    expect(summary.toolOrder[0]).toBe('crossfade')
  })

  it('arestas ligando nós desconhecidos são ignoradas com segurança', () => {
    const nodes = [toolNode('n1', 'crossfade', { max_duration_ms: 800 })]
    const edges = [edge('fantasma', 'n1')]
    const config = graphToPipelineConfig(nodes, edges)
    expect(config.crossfade.max_duration_ms).toBe(800)
  })

  it('o golden é estável contra defaultPipelineConfig (contrato duplo com o Rust)', () => {
    const nodes = [
      toolNode('n1', 'crossfade', { max_duration_ms: 1200 }),
      toolNode('n2', 'lufs_normalization'),
    ]
    const config = graphToPipelineConfig(nodes, [])
    // Gera exatamente o JSON embutido no teste Rust — se um campo mudar
    // de um lado, um dos dois testes quebra primeiro.
    expect(JSON.parse(JSON.stringify(config))).toEqual(GOLDEN_CANONICO)
  })
})
