import { useEffect, useMemo, useState, useCallback } from 'react'
import { ReactFlowProvider } from '@xyflow/react'
import RemixCanvas from './components/RemixCanvas'
import ProposalOverlay, { type Proposal } from './components/ProposalOverlay'
import ToolPalette from './components/ToolPalette'
import UploadPanel from './components/UploadPanel'
import Player from './components/Player'
import { useSSE } from './hooks/useSSE'
import { useApi } from './hooks/useApi'
import type { JobMode, PipelineConfig, ToolInfo } from './types/api'
import { defaultPipelineConfig } from './types/api'
import { GraphValidationError, graphToPipelineConfig } from './lib/graphToPipeline'
import { ensureLocalSession } from './lib/authHeaders'
import { useGraphStore } from './store/graphStore'

function App() {
  const [jobId, setJobId] = useState<string | undefined>(undefined)
  const [dismissedProposalId, setDismissedProposalId] = useState<string | null>(null)
  const [trackId, setTrackId] = useState<string | null>(null)
  // Item B3 do mapa: modo é selecionável agora (default 'manual', mas o
  // usuário pode trocar para 'assisted' para disparar o agente ReAct).
  const [mode, setMode] = useState<JobMode>('manual')
  // Item 4 do Lote 1 (plano Pareto): paleta alimentada por GET /api/v1/tools.
  const [tools, setTools] = useState<ToolInfo[] | null>(null)
  const [toolsLoading, setToolsLoading] = useState(true)
  const { events, connected } = useSSE(jobId)
  const api = useApi()
  const { listTools } = api

  useEffect(() => {
    let cancelled = false
    // toolsLoading já começa true; o effect só roda uma vez (listTools é
    // estável via useCallback) — sem setState síncrono no corpo.
    listTools()
      .then((r) => {
        if (!cancelled) setTools(r.tools)
      })
      .catch((e) => {
        // A paleta degrada com aviso próprio; não vira erro vermelho global.
        console.error('Falha ao carregar ferramentas:', e)
        if (!cancelled) setTools(null)
      })
      .finally(() => {
        if (!cancelled) setToolsLoading(false)
      })
    return () => {
      cancelled = true
    }
  }, [listTools])

  // Lote 3 (item 1): o grafo do canvas é a fonte do pipeline_config —
  // o que o usuário montou é o que o backend executa.
  const graphNodes = useGraphStore((s) => s.nodes)
  const graphEdges = useGraphStore((s) => s.edges)
  const [graphError, setGraphError] = useState<string | null>(null)

  // Sessão local (docs/03 §1): token para os comandos REST + cookie
  // same-origin para o handshake SSE e para o download do artefato.
  // SINGLETON (ensureLocalSession): chamar `local-session` mais de uma vez
  // criaria um tenant novo a cada chamada — com o StrictMode isto desalinha
  // Bearer e cookie (fix de integração dos Lotes 2+3).
  useEffect(() => {
    void ensureLocalSession().catch(() => {
      // Modo SaaS cuida do próprio login — sem token local não há nada a
      // fazer aqui.
    })
  }, [])

  const handleUploadComplete = useCallback((id: string) => {
    setTrackId(id)
  }, [])

  const handleCreateJob = useCallback(
    async (tkId: string, prompt: string) => {
      // Item B3: agora usamos o `mode` do state em vez de hardcoded 'manual'.
      //
      // Lote 3 (item 1 do plano Pareto — canvas executável): o
      // pipeline_config deixa de ser o default fixo e passa a ser a
      // SERIALIZAÇÃO REAL do grafo montado no canvas (graphStore →
      // PipelineConfig). O que o usuário montou é o que o backend executa.
      // Grafo cíclico → invalid_graph antes de gastar um job; ferramenta
      // ghost → ghost_tool. Grafo VAZIO → default explícito (opção
      // documentada no próprio contrato de graphToPipelineConfig): enquanto
      // a paleta do Lote 1 (PR #55) não estiver no build, o canvas não tem
      // como ganhar nós pela UI, e o canvas vazio se comporta exatamente
      // como antes do Lote 3 — job criável, sem dead end.
      setGraphError(null)
      let pipelineConfig: PipelineConfig
      try {
        pipelineConfig = graphToPipelineConfig(graphNodes, graphEdges)
      } catch (e) {
        if (e instanceof GraphValidationError) {
          if (e.code !== 'empty_graph') {
            setGraphError(e.message)
            return
          }
          pipelineConfig = defaultPipelineConfig()
        } else {
          throw e
        }
      }
      try {
        const job = await api.createJob(tkId, mode, prompt, pipelineConfig)
        setJobId(job.job_id)
      } catch (e) {
        // Erro estruturado já capturado pelo useApi.error — só loga para
        // diagnóstico; a UI exibe o erro abaixo do botão Criar Remix.
        console.error('Failed to create job:', e)
      }
    },
    [api, mode, graphNodes, graphEdges]
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

        {graphError && (
          <div className="bg-orange-900/50 p-3 rounded-lg mt-4">
            <p className="text-sm text-orange-300 font-semibold">Grafo inválido</p>
            <p className="text-sm text-orange-300 mt-1">{graphError}</p>
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
      <div className="flex-1 relative">
        <ReactFlowProvider>
          <RemixCanvas />
          {/* Item 4 do Lote 1: paleta respeita `available` de GET /tools —
              ghost tools (compression, dynamic_eq) desabilitadas com motivo. */}
          <div className="absolute top-4 left-4 z-10 w-64">
            <ToolPalette tools={tools} loading={toolsLoading} />
          </div>
          {pendingProposal && (
            <ProposalOverlay
              proposal={pendingProposal}
              onApprove={handleApprove}
              onReject={handleReject}
            />
          )}
          {jobCompleted && jobId && (
            <Player
              jobId={jobId}
              trackId={trackId}
              downloadUrl={jobCompleted.downloadUrl}
            />
          )}
        </ReactFlowProvider>
      </div>
    </div>
  )
}

export default App
