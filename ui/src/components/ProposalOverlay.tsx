// Overlay de decisão humana para uma proposta do agente (HITL).
// Payload de `agent.proposal` — docs/03-CONTRATOS-API.md §5.
//
// Item C3 do mapa: permite editar parâmetros antes de aceitar.
//
// Plano de design centrado no usuário (item 4 — "HITL explicável"): a
// proposta mostra ALTERAÇÃO, RAZÃO, PARÂMETROS, CONFIANÇA, RISCO, IMPACTO e
// trecho quando o backend os envia; ações APROVAR, AJUSTAR, REJEITAR, PEDIR
// ALTERNATIVA e FAZER MANUALMENTE. Expiração usa linguagem neutra (sem
// culpar). Foco vai para o primeiro botão ao abrir; Escape recusa; o foco
// volta ao conteúdo ao fechar (tratado pelo App ao desmontar).
import { useState, useMemo, useEffect, useRef, useCallback } from 'react'

export interface Proposal {
  proposalId: string
  tool: string
  toolLabelPtbr: string
  reason: string
  parametersSuggestion: Record<string, unknown>
  expiresInSec: number
  // Campos opcionais do plano de design — exibidos SÓ se o backend mandar
  // (não simulamos valor que não existe).
  confidence?: number
  risk?: string
  impact?: string
  atSec?: number
}

interface Props {
  proposal: Proposal
  onApprove: (adjustedParameters?: Record<string, unknown>) => void
  onReject: () => void
  /** "Pedir alternativa" — POST /proposals/{id}/replan (plano §HITL). */
  onAlternative?: () => void
  /** "Fazer manualmente" — recusa a proposta e troca para o modo manual. */
  onManual?: () => void
}

/**
 * Renderiza um campo editável para cada parâmetro da sugestão.
 * - number  → input number
 * - string  → input text
 * - boolean → checkbox
 * - array/string-enum → textarea JSON
 * - outros  → textarea JSON (power user)
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

function ProposalOverlay({ proposal, onApprove, onReject, onAlternative, onManual }: Props) {
  // Inicializa os parâmetros editáveis com a sugestão do agente.
  const [editedParams, setEditedParams] = useState<Record<string, unknown>>(
    () => ({ ...proposal.parametersSuggestion })
  )
  const approveRef = useRef<HTMLButtonElement>(null)

  // Foco no primeiro botão de ação ao abrir (plano §acessibilidade).
  useEffect(() => {
    approveRef.current?.focus()
  }, [])

  // Escape = recusar (ação reversível; nada se perde).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onReject()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [onReject])

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

  // Countdown de TTL — neutro: informa o tempo, não pressiona (§HITL).
  const [remainingSec, setRemainingSec] = useState(proposal.expiresInSec)
  useEffect(() => {
    const interval = setInterval(() => {
      setRemainingSec((s) => (s > 0 ? s - 1 : 0))
    }, 1000)
    return () => clearInterval(interval)
  }, [])

  const resumo = useMemo(
    () =>
      Object.entries(proposal.parametersSuggestion)
        .map(([k, v]) => `${k}: ${typeof v === 'number' ? Number(v.toFixed(3)) : JSON.stringify(v)}`)
        .join(', '),
    [proposal.parametersSuggestion]
  )

  const handleAlternative = useCallback(() => {
    onAlternative?.()
  }, [onAlternative])

  return (
    <div
      data-testid="proposal-overlay"
      className="fixed inset-0 bg-black/60 flex items-center justify-center z-50"
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="proposal-title"
        className="bg-gray-800 rounded-lg p-6 w-[520px] max-w-full max-h-[90vh] overflow-y-auto border border-gray-600"
      >
        <div className="flex items-center justify-between mb-4">
          <h3 id="proposal-title" className="text-xl font-bold text-white">
            Proposta do assistente
          </h3>
          <span className="text-xs text-gray-400" aria-live="off">
            {remainingSec > 0 ? `a proposta fica disponível por ${remainingSec}s` : 'proposta expirada — pode continuar no modo manual'}
          </span>
        </div>

        {/* O QUE muda */}
        <div className="mb-3">
          <p className="text-gray-300 mb-1 text-sm font-semibold">Alteração sugerida</p>
          <div className="bg-gray-700 p-3 rounded text-sm text-gray-100">
            <span className="text-purple-300 font-medium">{proposal.toolLabelPtbr}</span>
            {resumo && <span className="text-gray-300"> — {resumo}</span>}
          </div>
          {proposal.atSec !== undefined && (
            <p className="text-xs text-gray-400 mt-1">Trecho: a partir de {proposal.atSec}s</p>
          )}
        </div>

        {/* POR QUÊ */}
        <div className="mb-3">
          <p className="text-gray-300 mb-1 text-sm font-semibold">Por quê</p>
          <div className="bg-gray-700 p-3 rounded text-sm text-gray-100">{proposal.reason}</div>
        </div>

        {/* CONFIANÇA / RISCO / IMPACTO — só quando o backend envia */}
        {(proposal.confidence !== undefined || proposal.risk || proposal.impact) && (
          <div className="mb-3 grid grid-cols-3 gap-2 text-xs">
            {proposal.confidence !== undefined && (
              <div className="bg-gray-700 p-2 rounded">
                <p className="text-gray-400">Confiança</p>
                <p className="text-gray-100 font-medium">{Math.round(proposal.confidence * 100)}%</p>
              </div>
            )}
            {proposal.risk && (
              <div className="bg-gray-700 p-2 rounded">
                <p className="text-gray-400">Risco</p>
                <p className="text-gray-100 font-medium">{proposal.risk}</p>
              </div>
            )}
            {proposal.impact && (
              <div className="bg-gray-700 p-2 rounded">
                <p className="text-gray-400">Impacto</p>
                <p className="text-gray-100 font-medium">{proposal.impact}</p>
              </div>
            )}
          </div>
        )}

        {/* PARÂMETROS editáveis */}
        {Object.keys(proposal.parametersSuggestion).length > 0 && (
          <div className="mb-4">
            <p className="text-gray-300 mb-2 text-sm font-semibold">
              Parâmetros {hasEdits && '(editados por você)'}:
            </p>
            <div className="bg-gray-700 p-3 rounded space-y-2">
              {Object.entries(proposal.parametersSuggestion).map(([paramName]) => (
                <div key={paramName}>
                  <label className="block text-xs text-gray-400 mb-1">{paramName}</label>
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
              <p className="text-xs text-yellow-300 mt-2">
                Os valores editados sobrescrevem a sugestão da IA — a versão aprovada vai para a
                receita executada.
              </p>
            )}
          </div>
        )}

        <div className="flex flex-wrap gap-2 justify-end">
          {onManual && (
            <button
              onClick={onManual}
              data-testid="proposal-manual"
              className="px-3 py-2 bg-gray-700 hover:bg-gray-600 rounded text-white text-sm"
              title="Recusa a proposta e troca para o modo manual — você controla tudo."
            >
              Fazer manualmente
            </button>
          )}
          {onAlternative && (
            <button
              onClick={handleAlternative}
              data-testid="proposal-alternative"
              className="px-3 py-2 bg-blue-600 hover:bg-blue-500 rounded text-white text-sm"
              title="Pede uma nova sugestão ao assistente a partir do mesmo objetivo."
            >
              Pedir alternativa
            </button>
          )}
          <button
            onClick={onReject}
            data-testid="proposal-reject"
            className="px-3 py-2 bg-gray-600 hover:bg-gray-500 rounded text-white text-sm"
            title="Recusa a proposta. O remix continua com a receita atual."
          >
            Recusar
          </button>
          <button
            ref={approveRef}
            onClick={() => onApprove(hasEdits ? editedParams : undefined)}
            data-testid="proposal-approve"
            className="px-4 py-2 bg-green-600 hover:bg-green-500 rounded text-white font-medium text-sm"
          >
            {hasEdits ? 'Aprovar com ajuste' : 'Aprovar'}
          </button>
        </div>
        <p className="text-[11px] text-gray-500 mt-3">
          Esc recusa a proposta. Aprovar aplica os parâmetros na receita antes da renderização.
        </p>
      </div>
    </div>
  )
}

export default ProposalOverlay
