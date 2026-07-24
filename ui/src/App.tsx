import { useMemo, useState } from 'react'
import { ReactFlowProvider } from '@xyflow/react'
import RemixCanvas from './components/RemixCanvas'
import ProposalOverlay, { type Proposal } from './components/ProposalOverlay'
import { useParamStream } from './hooks/useParamStream'

function App() {
  // Sprint 0 ainda não tem fluxo de upload/criação de job — sem jobId, o
  // hook simplesmente não abre conexão SSE. Isso passa a existir quando o
  // fluxo de tracks/jobs for ligado (Sprint 1+).
  const [jobId] = useState<string | undefined>(undefined)
  const [dismissedProposalId, setDismissedProposalId] = useState<string | null>(null)
  const { events } = useParamStream(jobId)

  const pendingProposal = useMemo<Proposal | null>(() => {
    const last = [...events].reverse().find((e) => e.type === 'agent.proposal')
    if (!last) return null

    const proposal: Proposal = {
      proposalId: String(last.data.proposal_id ?? ''),
      tool: String(last.data.tool ?? ''),
      toolLabelPtbr: String(last.data.tool_label_ptbr ?? last.data.tool ?? ''),
      reason: String(last.data.reason ?? ''),
      parametersSuggestion: (last.data.parameters_suggestion as Record<string, unknown>) ?? {},
      expiresInSec: Number(last.data.expires_in_sec ?? 0),
    }

    return proposal.proposalId === dismissedProposalId ? null : proposal
  }, [events, dismissedProposalId])

  return (
    <div className="flex h-screen bg-gray-900">
      <ReactFlowProvider>
        <RemixCanvas />
        {pendingProposal && (
          <ProposalOverlay
            proposal={pendingProposal}
            onApprove={() => setDismissedProposalId(pendingProposal.proposalId)}
            onReject={() => setDismissedProposalId(pendingProposal.proposalId)}
          />
        )}
      </ReactFlowProvider>
    </div>
  )
}

export default App
