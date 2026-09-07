import { useEffect, useMemo, useState, useCallback } from 'react'
import ProposalOverlay, { type Proposal } from './components/ProposalOverlay'
import Player from './components/Player'
import { useSSE } from './hooks/useSSE'
import { useApi } from './hooks/useApi'
import type {
  ConsentInfo,
  JobMode,
  JobResponse,
  PeaksResponse,
  PipelineConfig,
  PrivacyPolicy,
  SystemInfo,
  ToolInfo,
  TrackResponse,
} from './types/api'
import { defaultPipelineConfig } from './types/api'
import { GraphValidationError, graphToPipelineConfig } from './lib/graphToPipeline'
import { ensureLocalSession } from './lib/authHeaders'
import { useGraphStore } from './store/graphStore'
import NovoRemixView from './views/NovoRemixView'
import ProjetosView from './views/ProjetosView'
import BibliotecaView from './views/BibliotecaView'
import AtividadeView from './views/AtividadeView'
import WorkspaceView from './views/WorkspaceView'

type View = 'projetos' | 'biblioteca' | 'novo-remix' | 'atividade' | 'workspace'

const NAV: Array<{ id: View; label: string }> = [
  // 4.4 do PR #59: o backend não tem domínio Project persistido — a visão
  // é um RESUMO do espaço único do tenant (projeto implícito). Chamar de
  // "Projetos" vendia gerenciamento de projetos que não existe.
  { id: 'projetos', label: 'Visão geral' },
  { id: 'biblioteca', label: 'Biblioteca' },
  { id: 'novo-remix', label: 'Novo remix' },
  { id: 'atividade', label: 'Atividade' },
  { id: 'workspace', label: 'Espaço de trabalho' },
]

/**
 * AppShell do plano de design centrado no usuário (etapa única):
 * navegação `Visão geral | Biblioteca | Novo remix | Atividade |
 * Espaço de trabalho`, com o fluxo guiado por intenção como caminho
 * principal e o canvas como modo avançado. O `PipelineConfig` continua
 * fonte de verdade (Lote 3): o grafo é a projeção editável.
 */
function App() {
  const [view, setView] = useState<View>('novo-remix')
  const [jobId, setJobId] = useState<string | undefined>(undefined)
  const [dismissedProposalId, setDismissedProposalId] = useState<string | null>(null)
  const [trackId, setTrackId] = useState<string | null>(null)
  const [trackName, setTrackName] = useState<string | null>(null)
  // Item B3 do mapa: modo é selecionável agora (default 'manual', mas o
  // usuário pode trocar para 'assisted' para disparar o agente ReAct).
  const [mode, setMode] = useState<JobMode>('manual')
  // Item 4 do Lote 1 (plano Pareto): paleta alimentada por GET /api/v1/tools.
  const [tools, setTools] = useState<ToolInfo[] | null>(null)
  const [toolsLoading, setToolsLoading] = useState(true)
  // Plano de design: transparência de IA/dados + análise + biblioteca/atividade.
  const [systemInfo, setSystemInfo] = useState<SystemInfo | null>(null)
  const [consent, setConsent] = useState<ConsentInfo | null>(null)
  // Política de privacidade auditável (4.2): fonte autorizada para o
  // PrivacyPanel afirmar o que sai da máquina — nunca texto hardcoded.
  const [politica, setPolitica] = useState<PrivacyPolicy | null>(null)
  const [peaks, setPeaks] = useState<PeaksResponse | null>(null)
  const [tracks, setTracks] = useState<TrackResponse[] | null>(null)
  const [jobs, setJobs] = useState<JobResponse[] | null>(null)
  const { events, connected } = useSSE(jobId)
  const api = useApi()
  const { listTools } = api

  // ── Bootstrap: sessão local singleton + ferramentas + system info ──
  useEffect(() => {
    let cancelled = false
    // toolsLoading já começa true; o effect só roda uma vez (listTools é
    // estável via useCallback) — sem setState síncrono no corpo.
    //
    // PR #59: a sessão local é garantida ANTES dos GETs autenticados —
    // sem isto, systemInfo/consent/política podiam 401 por dispararem
    // antes do token existir e ficarem null para sempre (o painel de
    // privacidade travava com o botão "Concordo" desabilitado).
    const bootstrap = async () => {
      await ensureLocalSession().catch(() => {
        // Modo SaaS cuida do próprio login — os GETs vão 401 e os
        // painéis mostram estado pendente, que é o correto sem sessão.
      })
      if (cancelled) return
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
      api
        .getSystemInfo()
        .then((info) => {
          if (!cancelled) setSystemInfo(info)
        })
        .catch(() => {
          /* painel de privacidade mostra "carregando" */
        })
      api
        .getConsent()
        .then((c) => {
          if (!cancelled) setConsent(c)
        })
        .catch(() => {
          /* consentimento segue pendente — o wizard pede quando precisar */
        })
      api
        .getPrivacyPolicy()
        .then((p) => {
          if (!cancelled) setPolitica(p)
        })
        .catch(() => {
          /* sem política o painel usa linguagem condicional (não absoluta) */
        })
    }
    void bootstrap()
    return () => {
      cancelled = true
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [listTools])

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

  // ── Análise: peaks reais da faixa (Lote 2/C9) ──
  // (peaks é derivado: fora da faixa, a view recebe null — sem setState no
  // corpo do efeito, regra react-hooks/set-state-in-effect)
  useEffect(() => {
    if (!trackId) return
    let cancelled = false
    api
      .getTrackPeaks(trackId, 1024)
      .then((p) => {
        if (!cancelled) setPeaks(p)
      })
      .catch(() => {
        if (!cancelled) setPeaks(null)
      })
    return () => {
      cancelled = true
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [trackId])

  // ── Graph store (receita executável) ──
  const graphNodes = useGraphStore((s) => s.nodes)
  const graphEdges = useGraphStore((s) => s.edges)
  const [graphError, setGraphError] = useState<string | null>(null)

  const handleUploadComplete = useCallback((id: string) => {
    setTrackId(id)
  }, [])

  const handleCreateJob = useCallback(
    async (tkId: string, prompt: string) => {
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

  // ── Biblioteca / Atividade ──
  const refreshTracks = useCallback(async () => {
    try {
      const r = await api.listTracks()
      setTracks(r)
    } catch {
      setTracks([])
    }
  }, [api])

  const refreshJobs = useCallback(async () => {
    try {
      const r = await api.listJobs()
      setJobs(r.items)
    } catch {
      setJobs([])
    }
  }, [api])

  const handleUseTrack = useCallback(
    (id: string) => {
      const t = tracks?.find((x) => x.track_id === id)
      setTrackName(t?.display_name ?? null)
      setTrackId(id)
      setView('novo-remix')
    },
    [tracks]
  )

  const handleCancelJob = useCallback(
    async (targetJobId: string) => {
      try {
        await api.cancelJob(targetJobId)
        void refreshJobs()
      } catch (e) {
        console.error('cancel falhou:', e)
      }
    },
    [api, refreshJobs]
  )

  const handleRetryJob = useCallback(
    async (targetJobId: string) => {
      try {
        const r = await api.retryJob(targetJobId)
        setJobId(r.new_job_id)
        setView('novo-remix')
        void refreshJobs()
      } catch (e) {
        console.error('retry falhou:', e)
      }
    },
    [api, refreshJobs]
  )

  const handleAceitarConsent = useCallback(async () => {
    const provider = systemInfo?.llm_provider ?? 'mock'
    const c = await api.postConsent(provider)
    setConsent(c)
  }, [api, systemInfo])

  // 4.3 do PR #59: revogação REAL — DELETE remove o registro persistido;
  // trocar para modo manual não revoga nada.
  const handleRevogarConsent = useCallback(async () => {
    const c = await api.revokeConsent()
    setConsent(c)
  }, [api])

  // ── Proposta HITL (com campos explicáveis quando existirem) ──
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
    // Plano de design §HITL: confiança/risco/impacto/trecho são exibidos só
    // quando o backend os manda — nunca simulados no frontend.
    const conf = last.data.confidence
    if (typeof conf === 'number' && conf >= 0 && conf <= 1) proposal.confidence = conf
    if (last.data.risk !== undefined) proposal.risk = String(last.data.risk)
    if (last.data.impact !== undefined) proposal.impact = String(last.data.impact)
    if (last.data.at_sec !== undefined) proposal.atSec = Number(last.data.at_sec)

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

  const handleAlternative = useCallback(() => {
    if (!pendingProposal || !jobId) return
    api
      .replanProposal(jobId, pendingProposal.proposalId)
      .catch((e) => console.error('replan falhou:', e))
    // A nova proposta chega como novo `agent.proposal` — o overlay segue aberto.
  }, [pendingProposal, jobId, api])

  const handleManual = useCallback(() => {
    if (pendingProposal && jobId) {
      api.rejectProposal(jobId, pendingProposal.proposalId).catch(console.error)
    }
    setDismissedProposalId(pendingProposal?.proposalId ?? null)
    setMode('manual')
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
      {/* Sidebar com navegação global */}
      <div className="w-60 flex-shrink-0 bg-gray-850 border-r border-gray-700 p-4 overflow-y-auto flex flex-col">
        <h1 className="text-xl font-bold text-white mb-4">Mixlirous</h1>
        <nav aria-label="Navegação principal" className="space-y-1 mb-6">
          {NAV.map((n) => (
            <button
              key={n.id}
              type="button"
              onClick={() => setView(n.id)}
              aria-current={view === n.id ? 'page' : undefined}
              data-testid={`nav-${n.id}`}
              className={`w-full text-left px-3 py-2 rounded text-sm ${
                view === n.id
                  ? 'bg-purple-700 text-white font-medium'
                  : 'text-gray-300 hover:bg-gray-800'
              }`}
            >
              {n.label}
            </button>
          ))}
        </nav>

        {jobId && (
          <div className="bg-gray-800 p-3 rounded-lg mb-3">
            <p className="text-sm text-gray-200">Job: {jobId.slice(0, 8)}...</p>
            <p className="text-xs text-gray-400" aria-live="polite" data-testid="job-status">
              Status: {jobStatus || 'aguardando'}
            </p>
            <p className={`text-xs ${connected ? 'text-green-400' : 'text-gray-500'}`}>
              {connected ? 'SSE conectado' : 'SSE desconectado — reconectando'}
            </p>
          </div>
        )}

        {api.error && (
          <div className="bg-red-900/50 p-3 rounded-lg mb-3" role="alert">
            <p className="text-sm text-red-200 font-semibold">
              Erro {api.error.status || '—'}
            </p>
            <p className="text-sm text-red-200 mt-1">{api.error.message}</p>
            {/* Item C1: mostra cada campo inválido retornado pelo backend */}
            {api.error.fieldErrors().map((fe, i) => (
              <p key={i} className="text-xs text-red-300 mt-1">
                <code className="bg-red-950 px-1 rounded">{fe.field}</code>: {fe.code}
                {fe.received !== undefined && ` (recebido: ${String(fe.received)})`}
                {fe.min !== undefined && ` — mínimo: ${fe.min}`}
                {fe.max !== undefined && ` — máximo: ${fe.max}`}
              </p>
            ))}
          </div>
        )}

        {/* Canvas resumido na sidebar para contexto em qualquer visão */}
        <div className="mt-auto text-xs text-gray-500">
          {graphNodes.length > 0
            ? `${graphNodes.length} nó(s) · ${graphEdges.length} ligação(ões) na receita`
            : 'receita vazia — o default do backend será usado'}
        </div>
      </div>

      {/* Conteúdo */}
      <main className="flex-1 flex flex-col overflow-hidden" id="conteudo">
        {view === 'projetos' && (
          <div className="p-6 overflow-y-auto w-full">
            <ProjetosView
              jobs={jobs}
              tracksCount={tracks?.length ?? null}
              systemInfo={systemInfo}
              onNovoRemix={() => setView('novo-remix')}
              onVerAtividade={() => setView('atividade')}
            />
          </div>
        )}

        {view === 'biblioteca' && (
          <div className="p-6 overflow-y-auto w-full">
            <BibliotecaView
              tracks={tracks}
              carregando={false}
              onUseTrack={handleUseTrack}
              onRefresh={refreshTracks}
            />
          </div>
        )}

        {view === 'novo-remix' && (
          <NovoRemixView
            mode={mode}
            onModeChange={setMode}
            onCreateJob={handleCreateJob}
            trackId={trackId}
            trackName={trackName}
            onUploadComplete={(id) => {
              handleUploadComplete(id)
              void refreshTracks()
              // Nome vem da resposta de registro — melhor esforço via biblioteca.
              void api.listTracks().then((ts) => {
                const t = ts.find((x) => x.track_id === id)
                if (t) setTrackName(t.display_name)
              }).catch(() => {})
            }}
            tools={tools}
            toolsLoading={toolsLoading}
            graphError={graphError}
            jobId={jobId}
            events={events}
            jobCompleted={jobCompleted}
            onCancelJob={() => jobId && handleCancelJob(jobId)}
            systemInfo={systemInfo}
            politica={politica}
            consent={consent}
            onAceitarConsent={handleAceitarConsent}
            onRevogarConsent={handleRevogarConsent}
            peaks={trackId ? peaks : null}
          />
        )}

        {view === 'atividade' && (
          <div className="p-6 overflow-y-auto w-full">
            <AtividadeView
              jobs={jobs}
              carregando={false}
              jobAtivoId={jobId}
              onRefresh={refreshJobs}
              onCancel={handleCancelJob}
              onRetry={handleRetryJob}
              onAbrir={(id) => {
                setJobId(id)
                setView('novo-remix')
              }}
            />
          </div>
        )}

        {view === 'workspace' && (
          <WorkspaceView tools={tools} toolsLoading={toolsLoading} />
        )}

        {/* Player no workspace (mesmo comportamento do Lote 2/3) */}
        {view === 'workspace' && jobCompleted && jobId && (
          <div className="absolute bottom-4 left-4 right-4">
            <Player jobId={jobId} trackId={trackId} downloadUrl={jobCompleted.downloadUrl} />
          </div>
        )}
      </main>

      {/* Overlay HITL — global (aparece em qualquer visão) */}
      {pendingProposal && (
        <ProposalOverlay
          proposal={pendingProposal}
          onApprove={handleApprove}
          onReject={handleReject}
          onAlternative={handleAlternative}
          onManual={handleManual}
        />
      )}
    </div>
  )
}

export default App
