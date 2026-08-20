import { useMemo, useState, useCallback } from 'react'
import { ReactFlowProvider } from '@xyflow/react'
import RemixCanvas from './components/RemixCanvas'
import ProposalOverlay, { type Proposal } from './components/ProposalOverlay'
import UploadPanel from './components/UploadPanel'
import Player from './components/Player'
import { useSSE } from './hooks/useSSE'
import { useApi } from './hooks/useApi'
import type { JobMode, PipelineConfig } from './types/api'
import { defaultPipelineConfig } from './types/api'

function App() {
  const [jobId, setJobId] = useState<string | undefined>(undefined)
  const [dismissedProposalId, setDismissedProposalId] = useState<string | null>(null)
  const [trackId, setTrackId] = useState<string | null>(null)
  // Item B3 do mapa: modo é selecionável agora (default 'manual', mas o
  // usuário pode trocar para 'assisted' para disparar o agente ReAct).
  const [mode, setMode] = useState<JobMode>('manual')
  const { events, connected } = useSSE(jobId)
  const api = useApi()

  const handleUploadComplete = useCallback((id: string) => {
    setTrackId(id)
  }, [])

  const handleCreateJob = useCallback(
    async (tkId: string, prompt: string) => {
      // Item B3: agora usamos o `mode` do state em vez de hardcoded 'manual'.
      // O `pipeline_config` enviado agora é o default válido do Rust
      // (defaultPipelineConfig gera a struct que desserializa corretamente).
      // Antes, o shape era incompatível e a request falhava com 422.
      const pipelineConfig: PipelineConfig = defaultPipelineConfig()
      try {
        const job = await api.createJob(tkId, mode, prompt, pipelineConfig)
        setJobId(job.job_id)
      } catch (e) {
        // Erro estruturado já capturado pelo useApi.error — só loga para
        // diagnóstico; a UI exibe o erro abaixo do botão Criar Remix.
        console.error('Failed to create job:', e)
      }
    },
    [api, mode]
  )

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

  const handleApprove = useCallback(
    (adjustedParameters?: Record<string, unknown>) => {
      if (!pendingProposal || !jobId) return
      // Item C3: passamos os parâmetros ajustados (se houver) via body.
      api
        .approveProposal(jobId, pendingProposal.proposalId, {
          parameters: adjustedParameters,
        })
        .catch(console.error)
      setDismissedProposalId(pendingProposal.proposalId)
    },
    [pendingProposal, jobId, api]
  )

  const handleReject = useCallback(() => {
    if (!pendingProposal || !jobId) return
    api.rejectProposal(jobId, pendingProposal.proposalId).catch(console.error)
    setDismissedProposalId(pendingProposal.proposalId)
  }, [pendingProposal, jobId, api])

  const jobStatus = useMemo(() => {
    const lastState = [...events].reverse().find((e) => e.type === 'job.state')
    return lastState ? String(lastState.data.status) : null
  }, [events])

  const jobCompleted = useMemo(() => {
    const completed = [...events]
      .reverse()
      .find((e) => e.type === 'job.completed')
    return completed
      ? {
          downloadUrl: String(completed.data.download_url ?? ''),
          artifactKey: String(completed.data.artifact_object_key ?? ''),
        }
      : null
  }, [events])

  return (
    <div className="flex h-screen bg-gray-900">
      {/* Sidebar */}
      <div className="w-80 flex-shrink-0 bg-gray-850 border-r border-gray-700 p-4 overflow-y-auto">
        <h1 className="text-xl font-bold text-white mb-4">Mixlirous</h1>
        <UploadPanel
          onUploadComplete={handleUploadComplete}
          onCreateJob={handleCreateJob}
          mode={mode}
          onModeChange={setMode}
        />

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
            <p className="text-sm text-red-300 font-semibold">
              Erro {api.error.status || '—'}
            </p>
            <p className="text-sm text-red-300 mt-1">{api.error.message}</p>
            {/* Item C1: mostra cada campo inválido retornado pelo backend */}
            {api.error.fieldErrors().map((fe, i) => (
              <p key={i} className="text-xs text-red-400 mt-1">
                <code className="bg-red-950 px-1 rounded">{fe.field}</code>: {fe.code}
                {fe.received !== undefined && ` (recebido: ${String(fe.received)})`}
                {fe.min !== undefined && ` — mínimo: ${fe.min}`}
                {fe.max !== undefined && ` — máximo: ${fe.max}`}
              </p>
            ))}
          </div>
        )}
      </div>

      {/* Canvas */}
      <div className="flex-1">
        <ReactFlowProvider>
          <RemixCanvas />
          {pendingProposal && (
            <ProposalOverlay
              proposal={pendingProposal}
              onApprove={handleApprove}
              onReject={handleReject}
            />
          )}
          {jobCompleted && jobId && (
            <Player jobId={jobId} downloadUrl={jobCompleted.downloadUrl} />
          )}
        </ReactFlowProvider>
      </div>
    </div>
  )
}

export default App
