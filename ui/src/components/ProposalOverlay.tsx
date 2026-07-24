// Overlay de decisão humana para uma proposta do agente (HITL).
// Payload de `agent.proposal` — docs/03-CONTRATOS-API.md §5.
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
  onApprove: () => void
  onReject: () => void
}

function ProposalOverlay({ proposal, onApprove, onReject }: Props) {
  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="bg-gray-800 rounded-lg p-6 w-96">
        <h3 className="text-xl font-bold text-white mb-4">Proposta de IA</h3>
        <div className="mb-4">
          <p className="text-gray-300 mb-2">Raciocínio:</p>
          <div className="bg-gray-700 p-3 rounded text-sm text-gray-100">{proposal.reason}</div>
        </div>
        <div className="mb-4">
          <p className="text-gray-300 mb-2">Ferramenta sugerida:</p>
          <div className="bg-gray-700 p-3 rounded text-sm text-gray-100">{proposal.toolLabelPtbr}</div>
        </div>
        <p className="text-xs text-gray-400 mb-4">Expira em {proposal.expiresInSec}s</p>
        <div className="flex gap-3 justify-end">
          <button onClick={onReject} className="px-4 py-2 bg-gray-600 rounded text-white">
            Rejeitar
          </button>
          <button onClick={onApprove} className="px-4 py-2 bg-green-600 rounded text-white">
            Aprovar
          </button>
        </div>
      </div>
    </div>
  )
}

export default ProposalOverlay
