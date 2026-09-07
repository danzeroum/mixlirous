/**
 * Filtro de ferramentas da paleta — Item 4 do Lote 1 do plano Pareto.
 *
 * O backend (`GET /api/v1/tools`, espelho de `limits::tool_registry`) já
 * marca `available: false` para ferramentas sem DSP real (compression,
 * dynamic_eq — as "ghost tools" de `docs/03-ADENDO-R2-CONTRATOS.md` §4.1).
 * A UI ignorava esse campo; a paleta podia oferecer uma ferramenta que não
 * produz efeito nenhum no áudio — exatamente o padrão que a regra
 * "never mark available without real DSP" existe para impedir.
 *
 * Funções puras para facilitar o teste unitário (sem React).
 */
import type { ToolInfo } from '../types/api'

/** Traduções PT-BR dos códigos de `unavailable_reason` (docs/03 §3.7). */
const REASON_PTBR: Record<string, string> = {
  not_implemented: 'Sem implementação de DSP — adicionar ao canvas não mudaria o áudio.',
  requires_plan_pro: 'Disponível apenas no plano Pro.',
}

/**
 * Texto para o tooltip da ferramenta indisponível. Códigos desconhecidos
 * caem numa mensagem genérica que PRESERVA o código — o usuário pode
 * reportar, e não viramos um buraco negro de erro silencioso.
 */
export function describeUnavailableReason(reason?: string): string {
  if (!reason) return 'Ferramenta indisponível.'
  return REASON_PTBR[reason] ?? `Indisponível (${reason}).`
}

/** Só as ferramentas que o usuário pode de fato usar. */
export function filterAvailableTools(tools: ToolInfo[]): ToolInfo[] {
  return tools.filter((t) => t.available)
}

/**
 * Partição disponível/indisponível. Mantida separada de `filterAvailable`
 * porque a paleta precisa mostrar as duas listas (a indisponível com o
 * motivo) sem perder nenhuma ferramenta no caminho.
 */
export function partitionToolsByAvailability(tools: ToolInfo[]): {
  available: ToolInfo[]
  unavailable: ToolInfo[]
} {
  const available: ToolInfo[] = []
  const unavailable: ToolInfo[] = []
  for (const t of tools) {
    if (t.available) {
      available.push(t)
    } else {
      unavailable.push(t)
    }
  }
  return { available, unavailable }
}
