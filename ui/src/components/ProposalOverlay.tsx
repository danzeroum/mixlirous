// Overlay de decisão humana para uma proposta do agente (HITL).
// Payload de `agent.proposal` — docs/03-CONTRATOS-API.md §5.
//
// Item C3 do mapa: agora permite editar parâmetros antes de aceitar.
// O usuário pode ajustar (ex.: trocar ratio 4.0 → 3.0) e os parâmetros
// ajustados vão no body do POST /approve. Antes, o overlay só tinha
// Aprovar/Recusar; agora tem "Aprovar com ajuste".
import { useState, useMemo, useEffect } from 'react'

export interface Proposal {
  proposalId: string
  tool: string
  toolLabelPtbr: string
  reason: string
  parametersSuggestion: Record<string, unknown>
  expiresInSec: number
}

interface Props {
  proposal: Proposal
  /**
   * Item C3: agora recebe os parâmetros ajustados como argumento.
   * Se o usuário não mexeu em nada, é `undefined` (approve "seco").
   */
  onApprove: (adjustedParameters?: Record<string, unknown>) => void
  onReject: () => void
}

/**
 * Renderiza um campo editável para cada parâmetro da sugestão.
 * - number  → input number
 * - string  → input text
 * - boolean → checkbox
 * - array/string-enum → select
 * - outros  → textarea JSON (power user)
 *
 * Não é uma UI final — é funcional. O design final virá do Design Brief
 * §Tela 6. Aqui priorizamos "pode editar" em vez de "bonito".
 */
function ParameterField({
  value,
  onChange,
}: {
  name: string
  value: unknown
  onChange: (v: unknown) => void
}) {
  const [jsonText, setJsonText] = useState(() => {
    try {
      return JSON.stringify(value, null, 2)
    } catch {
      return String(value)
    }
  })

  if (typeof value === 'number') {
    return (
      <input
        type="number"
        value={typeof value === 'number' ? value : 0}
        step="any"
        onChange={(e) => onChange(parseFloat(e.target.value))}
        className="w-full p-1.5 bg-gray-700 text-white rounded border border-gray-600 text-sm"
      />
    )
  }

  if (typeof value === 'boolean') {
    return (
      <input
        type="checkbox"
        checked={value}
        onChange={(e) => onChange(e.target.checked)}
        className="w-4 h-4"
      />
    )
  }

  if (typeof value === 'string') {
    return (
      <input
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="w-full p-1.5 bg-gray-700 text-white rounded border border-gray-600 text-sm"
      />
    )
  }

  if (Array.isArray(value)) {
    // Renderiza como textarea JSON — arrays podem ser de enum, objetos, etc.
    // O power user edita o JSON; a UI "bonita" fica para a próxima iteração.
    return (
      <textarea
        value={jsonText}
        onChange={(e) => setJsonText(e.target.value)}
        onBlur={() => {
          try {
            onChange(JSON.parse(jsonText))
          } catch {
            // JSON inválido — mantém o texto; o usuário corrigirá.
          }
        }}
        className="w-full p-1.5 bg-gray-700 text-white rounded border border-gray-600 text-sm font-mono"
        rows={3}
      />
    )
  }

  // Default: textarea JSON.
  return (
    <textarea
      value={jsonText}
      onChange={(e) => setJsonText(e.target.value)}
      onBlur={() => {
        try {
          onChange(JSON.parse(jsonText))
        } catch {
          // ignora
        }
      }}
      className="w-full p-1.5 bg-gray-700 text-white rounded border border-gray-600 text-sm font-mono"
      rows={3}
    />
  )
}

function ProposalOverlay({ proposal, onApprove, onReject }: Props) {
  // Inicializa os parâmetros editáveis com a sugestão do agente.
  const [editedParams, setEditedParams] = useState<Record<string, unknown>>(
    () => ({ ...proposal.parametersSuggestion })
  )

  // Detecta se o usuário mexeu em algo — usado para label do botão.
  const hasEdits = useMemo(() => {
    const keys = new Set([
      ...Object.keys(editedParams),
      ...Object.keys(proposal.parametersSuggestion),
    ])
    for (const k of keys) {
      if (JSON.stringify(editedParams[k]) !== JSON.stringify(proposal.parametersSuggestion[k])) {
        return true
      }
    }
    return false
  }, [editedParams, proposal.parametersSuggestion])

  // Countdown de TTL (design brief §Tela 6: contador discreto, não agressivo).
  const [remainingSec, setRemainingSec] = useState(proposal.expiresInSec)
  useEffect(() => {
    const interval = setInterval(() => {
      setRemainingSec((s) => (s > 0 ? s - 1 : 0))
    }, 1000)
    return () => clearInterval(interval)
  }, [])

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="bg-gray-800 rounded-lg p-6 w-[480px] max-w-full max-h-[90vh] overflow-y-auto">
        <div className="flex items-center justify-between mb-4">
          <h3 className="text-xl font-bold text-white">Proposta do assistente</h3>
          <span className="text-xs text-gray-400">
            expira em {remainingSec}s
          </span>
        </div>

        <div className="mb-4">
          <p className="text-gray-300 mb-2 text-sm">Raciocínio:</p>
          <div className="bg-gray-700 p-3 rounded text-sm text-gray-100">
            {proposal.reason}
          </div>
        </div>

        <div className="mb-4">
          <p className="text-gray-300 mb-2 text-sm">Ferramenta sugerida:</p>
          <div className="bg-gray-700 p-3 rounded text-sm text-gray-100">
            {proposal.toolLabelPtbr}
          </div>
        </div>

        {Object.keys(proposal.parametersSuggestion).length > 0 && (
          <div className="mb-4">
            <p className="text-gray-300 mb-2 text-sm">
              Parâmetros {hasEdits && '(editados)'}:
            </p>
            <div className="bg-gray-700 p-3 rounded space-y-2">
              {Object.entries(proposal.parametersSuggestion).map(([paramName]) => (
                <div key={paramName}>
                  <label className="block text-xs text-gray-400 mb-1">
                    {paramName}
                  </label>
                  <ParameterField
                    name={paramName}
                    value={editedParams[paramName]}
                    onChange={(v) =>
                      setEditedParams((prev) => ({ ...prev, [paramName]: v }))
                    }
                  />
                </div>
              ))}
            </div>
            {hasEdits && (
              <p className="text-xs text-yellow-400 mt-2">
                ⚠ Os valores editados sobrescrevem a sugestão da IA.
              </p>
            )}
          </div>
        )}

        <div className="flex gap-3 justify-end">
          <button
            onClick={onReject}
            className="px-4 py-2 bg-gray-600 hover:bg-gray-500 rounded text-white"
          >
            Recusar
          </button>
          <button
            onClick={() => onApprove(hasEdits ? editedParams : undefined)}
            className="px-4 py-2 bg-green-600 hover:bg-green-500 rounded text-white"
          >
            {hasEdits ? 'Aprovar com ajuste' : 'Aprovar'}
          </button>
        </div>
      </div>
    </div>
  )
}

export default ProposalOverlay
