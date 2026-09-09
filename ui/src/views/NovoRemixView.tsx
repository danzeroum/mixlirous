import { useEffect, useMemo, useRef, useState } from 'react'
import { ReactFlowProvider } from '@xyflow/react'
import RemixCanvas from '../components/RemixCanvas'
import ToolPalette from '../components/ToolPalette'
import UploadPanel from '../components/UploadPanel'
import Player from '../components/Player'
import Waveform from '../components/Waveform'
import JobTimeline from '../components/JobTimeline'
import PrivacyPanel from '../components/PrivacyPanel'
import type {
  ConsentInfo,
  PeaksResponse,
  PrivacyPolicy,
  SystemInfo,
  ToolInfo,
  JobMode,
} from '../types/api'
import type { StreamEvent } from '../hooks/useSSE'
import { avisoCanais, formatarDuracao } from '../lib/wavPeaks'
import type { WavMetrics } from '../lib/wavPeaks'

interface Props {
  // upload / objetivo
  mode: JobMode
  onModeChange: (m: JobMode) => void
  onCreateJob: (trackId: string, prompt: string) => Promise<void>
  trackId: string | null
  trackName: string | null
  onUploadComplete: (trackId: string) => void
  // receita
  tools: ToolInfo[] | null
  toolsLoading: boolean
  graphError: string | null
  // render / preview
  jobId: string | undefined
  events: StreamEvent[]
  jobCompleted: { downloadUrl: string; artifactKey: string } | null
  onCancelJob: () => void
  // privacidade / análise
  systemInfo: SystemInfo | null
  /** Política de privacidade auditável (GET /system/privacy-policy). */
  politica: PrivacyPolicy | null
  consent: ConsentInfo | null
  onAceitarConsent: () => Promise<void>
  /** Revogação REAL (DELETE /tenants/me/consent). */
  onRevogarConsent: () => Promise<void>
  peaks: PeaksResponse | null
}

/** Presets em linguagem musical (plano de design §"Nielsen": modo simples). */
const PRESETS: Array<{ rotulo: string; texto: string }> = [
  { rotulo: 'Versão curta p/ Reels', texto: 'versão de 30s para Reels, agressiva, focada nas viradas de bateria' },
  { rotulo: 'Transição suave', texto: 'transição suave entre músicas, crossfade longo, sem mudanças bruscas' },
  { rotulo: 'Energia p/ pista', texto: 'versão para pista, energia alta do início ao fim, cortes alinhados ao beat' },
  { rotulo: 'Intro estendida', texto: 'preserve a intro e estenda a entrada antes do primeiro drop' },
]

/**
 * Fluxo guiado por intenção (plano de design — item 1): upload → análise →
 * objetivo → receita → render → preview/exportação. Cada passo mostra o
 * estado e a próxima ação; os passos já disponíveis ficam montados (sem
 * esconder contexto) e numerados, com a seção ativa destacada.
 *
 * O canvas é a PROJEÇÃO editável da receita: mudou o grafo, mudou o
 * `PipelineConfig` que o backend executa (item 3 do plano).
 */
function NovoRemixView(props: Props) {
  const {
    mode, onModeChange, onCreateJob, trackId, trackName, onUploadComplete,
    tools, toolsLoading, graphError, jobId, events, jobCompleted, onCancelJob,
    systemInfo, politica, consent, onAceitarConsent, onRevogarConsent, peaks,
  } = props

  const [prompt, setPrompt] = useState('')
  const [metricsRemix, setMetricsRemix] = useState<WavMetrics | null>(null)
  const renderRef = useRef<HTMLDivElement>(null)

  // Em job criado → leva o usuário ao passo de render (Nielsen: próxima ação).
  useEffect(() => {
    if (jobId) renderRef.current?.scrollIntoView({ behavior: 'smooth', block: 'start' })
  }, [jobId])

  const jobStatus = useMemo(() => {
    const last = [...events].reverse().find((e) => e.type === 'job.state')
    return last ? String(last.data.status) : null
  }, [events])

  const podeCriar = Boolean(trackId)
  const consentOk = mode === 'manual' || consent?.assisted_mode_accepted_at != null
  void podeCriar

  return (
    <div className="w-full max-w-3xl mx-auto px-4 py-6 space-y-6 overflow-y-auto" data-testid="novo-remix">
      {/* ── Passo 1 · Upload ── */}
      <section aria-labelledby="passo-upload" role="region">
        <h2 id="passo-upload" className="text-sm font-bold text-ai-300 uppercase tracking-wide mb-2">
          1 · Envie a faixa
        </h2>
        <UploadPanel
          onUploadComplete={onUploadComplete}
          onCreateJob={async () => { /* a criação fica no passo 3 (Objetivo) */ }}
          mode={mode}
          onModeChange={onModeChange}
          soUpload
        />
      </section>

      {/* ── Passo 2 · Análise ── */}
      {trackId && (
        <section aria-labelledby="passo-analise" role="region" className="bg-surface-800/60 rounded-lg border border-surface-700 p-4">
          <h2 id="passo-analise" className="text-sm font-bold text-ai-300 uppercase tracking-wide mb-2">
            2 · Análise
          </h2>
          <p className="text-sm text-ink-200 mb-3">
            Faixa <strong>{trackName ?? trackId.slice(0, 8)}</strong> pronta. A forma da onda abaixo
            vem dos picos reais calculados pelo backend (mesmos dados que guiam o corte em blocos).
          </p>
          {peaks ? (
            <>
              <Waveform peaks={peaks.peaks} height={72} ariaLabel={`Forma de onda da faixa ${trackName ?? ''}`} />
              {avisoCanais(peaks.channels) && (
                <p
                  role="status"
                  data-testid="mono-notice"
                  className="mt-2 p-2 rounded bg-warn-950/60 border border-warn-800 text-xs text-warn-200"
                >
                  {avisoCanais(peaks.channels)}
                  {peaks.channels !== undefined && (
                    <span className="block mt-1 text-warn-300/80 font-mono">
                      source_channels: {peaks.channels} · processing_channels: 1 ·
                      output_channels: 1 · channel_policy: downmix_arithmetic_mean
                    </span>
                  )}
                </p>
              )}
              <p className="text-xs text-ink-400 mt-2">
                {peaks.resolution} buckets
                {peaks.sample_rate ? ` · ${peaks.sample_rate} Hz` : ''} · BPM, tom e seções aparecem
                aqui quando a análise musical for exposta pelo backend (roadmap Beta — não
                simulamos valores).
              </p>
            </>
          ) : (
            <p className="text-xs text-ink-400" role="status">Calculando picos da faixa…</p>
          )}
        </section>
      )}

      {/* ── Passo 3 · Objetivo ── */}
      {trackId && (
        <section aria-labelledby="passo-objetivo" role="region" className="bg-surface-800/60 rounded-lg border border-surface-700 p-4">
          <h2 id="passo-objetivo" className="text-sm font-bold text-ai-300 uppercase tracking-wide mb-2">
            3 · Objetivo
          </h2>
          <div className="flex flex-wrap gap-2 mb-3" role="group" aria-label="Pontos de partida">
            {PRESETS.map((p) => (
              <button
                key={p.rotulo}
                type="button"
                onClick={() => setPrompt(p.texto)}
                className="px-2.5 py-1 text-xs bg-surface-700 hover:bg-surface-600 text-ink-100 rounded-full border border-surface-600"
                title="Preenche o objetivo — edite à vontade"
              >
                {p.rotulo}
              </button>
            ))}
          </div>
          <UploadPanel
            onUploadComplete={onUploadComplete}
            onCreateJob={onCreateJob}
            mode={mode}
            onModeChange={onModeChange}
            promptExterno={[prompt, setPrompt]}
            trackIdExterno={trackId}
            soObjetivo
            criarDesabilitado={!consentOk}
          />
          {mode === 'assisted' && (
            <div className="mt-3">
              {/* Plano de design §LGPD: o painel fica VISÍVEL no modo assistido
                  mesmo com consentimento aceito — é onde o usuário vê a
                  política vigente e REVOGA (4.3). Escondê-lo após o aceite
                  deixaria a revogação inacessível no fluxo principal. */}
              <PrivacyPanel
                info={systemInfo}
                politica={politica}
                consentAceitoEm={consent?.assisted_mode_accepted_at ?? null}
                onAceitar={onAceitarConsent}
                onRevogar={onRevogarConsent}
                compacto
              />
            </div>
          )}
        </section>
      )}

      {/* ── Passo 4 · Receita (canvas) ── */}
      <section aria-labelledby="passo-receita" role="region" className="bg-surface-800/60 rounded-lg border border-surface-700 p-4">
        <h2 id="passo-receita" className="text-sm font-bold text-ai-300 uppercase tracking-wide mb-2">
          4 · Receita (canvas)
        </h2>
        <p className="text-xs text-ink-400 mb-3">
          O que está montado aqui é o que o backend executa. Ferramentas indisponíveis nesta
          instalação aparecem desabilitadas com o motivo.
        </p>
        {graphError && (
          <p role="alert" className="mb-3 p-2 rounded bg-warn-950/60 border border-warn-800 text-sm text-warn-200">
            {graphError}
          </p>
        )}
        <div className="h-72 rounded border border-surface-700 relative overflow-hidden">
          <ReactFlowProvider>
            <RemixCanvas />
            <div className="absolute top-2 left-2 z-10 w-56">
              <ToolPalette tools={tools} loading={toolsLoading} />
            </div>
          </ReactFlowProvider>
        </div>
      </section>

      {/* ── Passo 5 · Render ── */}
      <section ref={renderRef} aria-labelledby="passo-render" role="region">
        {jobId && (
          <div className="bg-surface-800/60 rounded-lg border border-surface-700 p-4">
            <div className="flex items-center justify-between">
              <h2 id="passo-render" className="text-sm font-bold text-ai-300 uppercase tracking-wide">
                5 · Renderização · job {jobId.slice(0, 8)}
              </h2>
              {(jobStatus === 'queued' || jobStatus === 'processing') && (
                <button
                  type="button"
                  onClick={onCancelJob}
                  data-testid="cancel-job"
                  className="px-3 py-1.5 text-xs bg-surface-700 hover:bg-surface-600 text-ink-100 rounded"
                  title="Interrompe o remix. Você não perde a faixa nem a receita."
                >
                  Cancelar remix
                </button>
              )}
            </div>
            <JobTimeline events={events} />
          </div>
        )}
      </section>

      {/* ── Passo 6 · Preview & exportação ── */}
      {jobCompleted && jobId && (
        <section aria-labelledby="passo-preview" role="region">
          <h2 id="passo-preview" className="sr-only">Preview e exportação</h2>
          <Player
            jobId={jobId}
            trackId={trackId}
            downloadUrl={jobCompleted.downloadUrl}
            onMetrics={setMetricsRemix}
          />
          {metricsRemix && (
            <p className="text-xs text-ink-400 mt-2">
              Remix: {formatarDuracao(metricsRemix.durationSec)} · {metricsRemix.sampleRate} Hz ·{' '}
              {metricsRemix.channels === 1 ? 'mono' : `${metricsRemix.channels} canais`} · pico{' '}
              {metricsRemix.peakDbfs.toFixed(1)} dBFS
            </p>
          )}
        </section>
      )}
    </div>
  )
}

export default NovoRemixView
