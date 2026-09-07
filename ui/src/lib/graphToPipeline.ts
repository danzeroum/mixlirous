/**
 * Serialização do grafo do canvas → `PipelineConfig` — item 1 do Lote 3
 * (plano Pareto: "transforma o canvas de decorativo em funcional").
 *
 * Antes, `handleCreateJob` enviava `defaultPipelineConfig()` fixo: o
 * usuário montava o grafo, e o job ignorava tudo. Aqui o grafo do
 * `graphStore` vira o `PipelineConfig` que o backend executa, espelhando
 * `crates/audio_core/src/domain/pipeline_config.rs`.
 *
 * Mapeamento honesto (o que o canvas expressa hoje):
 * - **crossfade** → `crossfade.enabled` + `max_duration_ms` (se o nó
 *   carregar `max_duration_ms` no data, dentro de 0..=3000);
 * - **lufs_normalization** → `mastering.enable_limiting = true` +
 *   `lufs_target` (se o nó carregar `lufs_target` válido, −30..=−6);
 * - **time_stretch / fade_in / fade_out** → sem campo correspondente no
 *   `PipelineConfig` (time_stretch é pós-processamento; fades de borda
 *   são fixos no stitch) — registrados no summary, não silenciados;
 * - **compression / dynamic_eq / stem_separation** → ghost tools (sem DSP
 *   real): ignoradas na serialização, com erro se aparecerem (a paleta
 *   nunca as oferece — aparece só se alguém injetar grafo à mão).
 *
 * Funções puras, sem React — testáveis em Vitest e espelhadas no teste de
 * contrato Rust (`contract_ts_rust.rs::grafo_canonico_do_canvas...`).
 */
import type { Edge } from '@xyflow/react'
import type { PipelineConfig } from '../types/api'
import { defaultPipelineConfig } from '../types/api'
import type { RemixNode } from '../store/graphStore'

/** Ferramentas ghost (sem DSP real) — regra do plano: nunca serializar. */
const GHOST_TOOLS = new Set(['compression', 'dynamic_eq', 'stem_separation'])

export type GraphValidationCode = 'invalid_graph' | 'ghost_tool' | 'empty_graph'

export class GraphValidationError extends Error {
  readonly code: GraphValidationCode
  constructor(code: GraphValidationCode, message: string) {
    super(message)
    this.name = 'GraphValidationError'
    this.code = code
  }
}

export interface GraphSummary {
  /** Ferramentas de efeito em ordem topológica (ou de inserção, sem arestas). */
  toolOrder: string[]
  /** Ferramentas ignoradas por não terem campo no PipelineConfig. */
  unmappedTools: string[]
  /** Ferramentas ghost encontradas (grafo injetado à mão). */
  ghostTools: string[]
  hasCycle: boolean
}

function clampNumber(value: unknown, min: number, max: number): number | null {
  if (typeof value !== 'number' || !Number.isFinite(value)) return null
  if (value < min || value > max) return null
  return value
}

/** Ordem topológica (Kahn) das ferramentas a partir das arestas. */
function topoOrder(ids: string[], edges: Edge[]): string[] {
  const indegree = new Map<string, number>(ids.map((id) => [id, 0]))
  const adj = new Map<string, string[]>(ids.map((id) => [id, []]))
  for (const e of edges) {
    if (indegree.has(e.source) && indegree.has(e.target)) {
      adj.get(e.source)!.push(e.target)
      indegree.set(e.target, (indegree.get(e.target) ?? 0) + 1)
    }
  }
  const queue = ids.filter((id) => (indegree.get(id) ?? 0) === 0)
  const order: string[] = []
  while (queue.length > 0) {
    const id = queue.shift()!
    order.push(id)
    for (const next of adj.get(id) ?? []) {
      const left = (indegree.get(next) ?? 0) - 1
      indegree.set(next, left)
      if (left === 0) queue.push(next)
    }
  }
  return order.length === ids.length ? order : ids // ciclo → ordem de inserção
}

function detectCycle(ids: string[], edges: Edge[]): boolean {
  const WHITE = 0, GRAY = 1, BLACK = 2
  const color = new Map<string, number>(ids.map((id) => [id, WHITE]))
  const adj = new Map<string, string[]>(ids.map((id) => [id, []]))
  for (const e of edges) {
    if (color.has(e.source) && color.has(e.target)) adj.get(e.source)!.push(e.target)
  }
  const visit = (id: string): boolean => {
    color.set(id, GRAY)
    for (const next of adj.get(id) ?? []) {
      const c = color.get(next) ?? WHITE
      if (c === GRAY) return true
      if (c === WHITE && visit(next)) return true
    }
    color.set(id, BLACK)
    return false
  }
  for (const id of ids) {
    if ((color.get(id) ?? WHITE) === WHITE && visit(id)) return true
  }
  return false
}

/** Resume o grafo: ferramentas, ordem, ciclo, ignoradas e ghost. */
export function summarizeGraph(nodes: RemixNode[], edges: Edge[]): GraphSummary {
  const effectNodes = nodes.filter((n) => typeof n.data?.tool === 'string')
  const ids = effectNodes.map((n) => n.id)
  const orderedIds = topoOrder(ids, edges)
  const byId = new Map(effectNodes.map((n) => [n.id, n]))

  const toolOrder: string[] = []
  const unmappedTools: string[] = []
  const ghostTools: string[] = []
  const UNMAPPED = new Set(['time_stretch', 'fade_in', 'fade_out'])

  for (const id of orderedIds) {
    const tool = byId.get(id)!.data.tool as string
    if (GHOST_TOOLS.has(tool)) {
      ghostTools.push(tool)
      continue
    }
    toolOrder.push(tool)
    if (UNMAPPED.has(tool)) unmappedTools.push(tool)
  }

  return {
    toolOrder,
    unmappedTools,
    ghostTools,
    hasCycle: detectCycle(ids, edges),
  }
}

/**
 * Converte o grafo em `PipelineConfig`. O canvas é a fonte da verdade:
 * sem nó de crossfade, o crossfade desabilita (o default do backend só
 * vale quando NENHUM grafo é montado).
 *
 * @throws GraphValidationError em grafo cíclico (`invalid_graph`), com
 * ferramenta ghost presente (`ghost_tool`) ou sem nenhum nó de efeito
 * (`empty_graph`) — nesse caso o caller deve optar por enviar o config
 * default explícito (mesma struct, crossfade default).
 */
export function graphToPipelineConfig(
  nodes: RemixNode[],
  edges: Edge[],
  base: PipelineConfig = defaultPipelineConfig(),
): PipelineConfig {
  const summary = summarizeGraph(nodes, edges)

  if (summary.hasCycle) {
    throw new GraphValidationError(
      'invalid_graph',
      'O grafo tem um ciclo — o backend rejeitaria com 422 invalid_graph (docs/03 §4).',
    )
  }
  if (summary.ghostTools.length > 0) {
    throw new GraphValidationError(
      'ghost_tool',
      `Ferramentas sem DSP real no grafo: ${summary.ghostTools.join(', ')}. ` +
        'Elas nunca podem ser serializadas para o pipeline (regra do plano).',
    )
  }
  if (summary.toolOrder.length === 0) {
    throw new GraphValidationError(
      'empty_graph',
      'Nenhuma ferramenta no canvas — adicione uma pela paleta antes de criar o remix.',
    )
  }

  const config: PipelineConfig = structuredClone(base)
  const nodeByTool = new Map<string, RemixNode>()
  for (const n of nodes) {
    if (typeof n.data?.tool === 'string' && !nodeByTool.has(n.data.tool)) {
      nodeByTool.set(n.data.tool, n)
    }
  }

  // crossfade: presente → habilitado (+ duração do nó se válida)
  if (summary.toolOrder.includes('crossfade')) {
    config.crossfade.enabled = true
    const node = nodeByTool.get('crossfade')!
    const ms = clampNumber(node.data?.max_duration_ms, 0, 3000)
    if (ms !== null) config.crossfade.max_duration_ms = ms
  } else {
    config.crossfade.enabled = false
  }

  // lufs_normalization: presente → limiting on (+ alvo do nó se válido)
  if (summary.toolOrder.includes('lufs_normalization')) {
    config.mastering.enable_limiting = true
    const node = nodeByTool.get('lufs_normalization')!
    const target = clampNumber(node.data?.lufs_target, -30, -6)
    if (target !== null) config.mastering.lufs_target = target
  }

  return config
}
