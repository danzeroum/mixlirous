import { useMemo } from 'react'
import type { ToolInfo } from '../types/api'
import { describeUnavailableReason, partitionToolsByAvailability } from '../lib/toolFilter'
import { useGraphStore } from '../store/graphStore'

interface Props {
  /** Lista de `GET /api/v1/tools` (App.tsx busca e repassa). `null` = ainda carregando. */
  tools: ToolInfo[] | null
  loading?: boolean
}

/**
 * Paleta de ferramentas do canvas — alimentada por `GET /api/v1/tools`
 * (docs/03-CONTRATOS-API.md §3.7), nunca hardcoded.
 *
 * Item 4 do Lote 1 do plano Pareto: ferramentas com `available: false`
 * (compression, dynamic_eq — as "ghost tools" do adendo R2 §4.1) ficam
 * desabilitadas, com o motivo no tooltip (`title`) e um selo visível.
 * Clicar numa disponível adiciona um nó de efeito ao canvas.
 */
function ToolPalette({ tools, loading }: Props) {
  const addToolNode = useGraphStore((s) => s.addToolNode)

  const groups = useMemo(() => {
    if (!tools) return []
    const byCategory = new Map<string, ToolInfo[]>()
    for (const t of tools) {
      const list = byCategory.get(t.category) ?? []
      list.push(t)
      byCategory.set(t.category, list)
    }
    return [...byCategory.entries()].sort(([a], [b]) => a.localeCompare(b))
  }, [tools])

  if (loading || tools === null) {
    return (
      <div className="p-3 bg-gray-800/95 rounded-lg border border-gray-700 text-xs text-gray-400">
        Carregando ferramentas…
      </div>
    )
  }

  return (
    <div className="p-3 bg-gray-800/95 rounded-lg border border-gray-700 max-h-72 overflow-y-auto">
      <h3 className="text-xs font-bold text-gray-300 uppercase tracking-wide mb-2">
        Ferramentas
      </h3>
      {groups.map(([category, list]) => {
        const { available, unavailable } = partitionToolsByAvailability(list)
        return (
          <div key={category} className="mb-3 last:mb-0">
            <div className="text-[10px] text-gray-500 uppercase mb-1">{category}</div>
            <div className="flex flex-wrap gap-1.5">
              {available.map((t) => (
                <button
                  key={t.name}
                  type="button"
                  onClick={() => addToolNode(t.name, t.label_ptbr)}
                  title={t.label_ptbr}
                  className="px-2 py-1 text-xs bg-purple-700 hover:bg-purple-600 text-white rounded disabled:opacity-40"
                >
                  + {t.label_ptbr}
                </button>
              ))}
              {unavailable.map((t) => (
                <button
                  key={t.name}
                  type="button"
                  disabled
                  aria-disabled
                  title={describeUnavailableReason(t.unavailable_reason)}
                  className="px-2 py-1 text-xs bg-gray-700 text-gray-400 rounded cursor-not-allowed line-through"
                >
                  {t.label_ptbr}
                  <span className="ml-1 text-[10px] no-underline text-orange-400">
                    (indisponível)
                  </span>
                </button>
              ))}
            </div>
          </div>
        )
      })}
    </div>
  )
}

export default ToolPalette
