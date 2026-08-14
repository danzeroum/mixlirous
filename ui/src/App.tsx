import { useMemo, useState, useCallback, useEffect } from 'react'
import { ReactFlowProvider } from '@xyflow/react'
import RemixCanvas from './components/RemixCanvas'
import ProposalOverlay, { type Proposal } from './components/ProposalOverlay'
import UploadPanel from './components/UploadPanel'
import WelcomeOnboarding from './components/WelcomeOnboarding'
import PrivacyNotice from './components/PrivacyNotice'
import { useParamStream } from './hooks/useParamStream'
import { useApi } from './hooks/useApi'

const ONBOARDING_KEY = 'mixlirous_onboarding_done'
const PRIVACY_KEY = 'mixlirous_privacy_accepted'

function App() {
  const [jobId, setJobId] = useState<string | undefined>(undefined)
  const [dismissedProposalId, setDismissedProposalId] = useState<string | null>(null)
  const [trackId, setTrackId] = useState<string | null>(null)
  const [showOnboarding, setShowOnboarding] = useState(false)
  const [showPrivacy, setShowPrivacy] = useState(false)
  const { events, connected } = useParamStream(jobId)
  const api = useApi()

  useEffect(() => {
    const onboardingDone = localStorage.getItem(ONBOARDING_KEY)
    const privacyAccepted = localStorage.getItem(PRIVACY_KEY)
    if (!onboardingDone) setShowOnboarding(true)
    if (!privacyAccepted) setShowPrivacy(true)
  }, [])

  const handleDismissOnboarding = useCallback(() => {
    localStorage.setItem(ONBOARDING_KEY, '1')
    setShowOnboarding(false)
  }, [])

  const handleAcceptPrivacy = useCallback(() => {
    localStorage.setItem(PRIVACY_KEY, '1')
    setShowPrivacy(false)
  }, [])

  const handleDeclinePrivacy = useCallback(() => {
    localStorage.setItem(PRIVACY_KEY, 'manual_only')
    setShowPrivacy(false)
  }, [])

  const handleUploadComplete = useCallback((id: string) => {
    setTrackId(id)
  }, [])

  const handleCreateJob = useCallback(async (tkId: string, prompt: string) => {
    try {
      const job = await api.createJob({
        track_id: tkId,
        mode: 'manual',
        user_prompt: prompt,
        pipeline_config: {
          crossfade: { duration_ms: 1000, curve: 'constant_power' },
          mastering: { lufs_target: -14.0, peak_db: -1.0, compression_ratio: 4.0 },
        },
      })
      setJobId(job.job_id)
    } catch (e) {
      console.error('Failed to create job:', e)
    }
  }, [api])

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

  const handleApprove = useCallback(() => {
    if (!pendingProposal || !jobId) return
    api.approveProposal(jobId, pendingProposal.proposalId).catch(console.error)
    setDismissedProposalId(pendingProposal.proposalId)
  }, [pendingProposal, jobId, api])

  const handleReject = useCallback(() => {
    if (!pendingProposal || !jobId) return
    api.rejectProposal(jobId, pendingProposal.proposalId).catch(console.error)
    setDismissedProposalId(pendingProposal.proposalId)
  }, [pendingProposal, jobId, api])

  const jobStatus = useMemo(() => {
    const lastState = [...events].reverse().find((e) => e.type === 'job.state')
    return lastState ? String(lastState.data.status) : null
  }, [events])

  return (
    <div className="flex h-screen bg-gray-900">
      <div className="w-80 flex-shrink-0 bg-gray-850 border-r border-gray-700 p-4 overflow-y-auto">
        <h1 className="text-xl font-bold text-white mb-4">Mixlirous</h1>
        <UploadPanel onUploadComplete={handleUploadComplete} onCreateJob={handleCreateJob} />
        {trackId && (
          <div className="bg-gray-800 p-3 rounded-lg mt-4">
            <p className="text-sm text-gray-300">Faixa: {trackId.slice(0, 8)}...</p>
          </div>
        )}
        {jobId && (
          <div className="bg-gray-800 p-3 rounded-lg mt-4">
            <p className="text-sm text-gray-300">Job: {jobId.slice(0, 8)}...</p>
            <p className="text-xs text-gray-400">Status: {jobStatus || 'aguardando'}</p>
            {connected && <p className="text-xs text-green-400">SSE conectado</p>}
          </div>
        )}
        {api.error && (
          <div className="bg-red-900/50 p-3 rounded-lg mt-4">
            <p className="text-sm text-red-300">{api.error}</p>
          </div>
        )}
      </div>
      <div className="flex-1">
        <ReactFlowProvider>
          <RemixCanvas />
          {pendingProposal && (
            <ProposalOverlay proposal={pendingProposal} onApprove={handleApprove} onReject={handleReject} />
          )}
        </ReactFlowProvider>
      </div>
      {showOnboarding && <WelcomeOnboarding onDismiss={handleDismissOnboarding} />}
      {showPrivacy && <PrivacyNotice onAccept={handleAcceptPrivacy} onDecline={handleDeclinePrivacy} />}
    </div>
  )
}

export default App
