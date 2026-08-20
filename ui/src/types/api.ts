// API Types — Mixlirous
//
// Item B3 do mapa de ação: este arquivo foi REESCRITO para alinhar com os
// structs Rust reais em `crates/audio_core/src/domain/pipeline_config.rs`
// e `crates/audio_api/src/routes/*.rs`. A versão anterior enviava um
// `pipeline_config` que não desserializava no Rust (B1 do relatório).
//
// Long-term path (docs/03-CONTRATOS-API.md §8): usar `ts-rs` para gerar
// estes tipos a partir dos structs Rust em `cargo test export_bindings`.
// Até lá, este arquivo é a fonte da verdade no frontend; o teste de
// roundtrip em `__tests__/contract.spec.ts` valida que um JSON sample
// gera e parseia corretamente em ambos os lados.

// ─── PipelineConfig ────────────────────────────────────────────────────
// Mirror of `audio_core::domain::PipelineConfig`.
// Note: `target_duration` é um `std::time::Duration` em Rust, que serializa
// como `{secs, nanos}` (default serde). NÃO é `target_duration_sec: number`.

export interface Duration {
  secs: number
  nanos: number
}

export interface CrossfadeConfig {
  enabled: boolean
  max_duration_ms: number // 0-3000 (newtype CrossfadeMs)
  curve: CrossfadeCurve
}

export type CrossfadeCurve = 'constant_gain' | 'constant_power'

export interface MasteringConfig {
  lufs_target: number // -30 a -6 (newtype LufsTarget)
  peak_db: number
  enable_limiting: boolean
  compression_ratio: number // 1-10 (newtype CompressionRatio)
}

export interface SelectionConfig {
  min_strong_beat_percentile: number // 0-1 (newtype Percentile)
  block_size_beats: number // 1-16 (newtype BlockSizeBeats)
  preserve_intro_ms: number
  preserve_outro_ms: number
}

export type AudioCodec = 'WAV' | 'MP3' | 'AAC' | 'FLAC'

export interface AudioFormat {
  sample_rate: number
  channels: number
  bit_depth: number
  codec: AudioCodec
}

// TuningConfig mirror of `audio_core::domain::tuning_config::TuningConfig`.
// `max_global_cents` é newtype (MaxCorrectionCents, -100..=100), serializa
// transparente como f32. `min_confidence` é newtype MinConfidence (0..=1).
export type TuningMode = 'disabled' | 'analyze_only' | 'global' | 'per_stem'

export interface TuningConfig {
  enabled: boolean
  mode: TuningMode
  max_global_cents: number
  min_confidence: number
  force_tonic_hz: number | null
  force_mode: string | null
}

export interface PipelineConfig {
  target_duration: Duration
  crossfade: CrossfadeConfig
  mastering: MasteringConfig
  selection: SelectionConfig
  format: AudioFormat
  tuning: TuningConfig
}

/**
 * Helper: gera um `PipelineConfig` com defaults do Rust (`PipelineConfig::default()`).
 * Usar isto quando o caller quer apenas "deixa o backend decidir" — equivalente
 * a não enviar `pipeline_config` na request (que também é válido).
 *
 * Os valores aqui MIRROREIAM `PipelineConfig::default()` em
 * `crates/audio_core/src/domain/pipeline_config.rs` e
 * `crates/audio_core/src/domain/tuning_config.rs`. O teste
 * `crates/audio_core/tests/contract_ts_rust.rs` valida a sincronia.
 */
export function defaultPipelineConfig(): PipelineConfig {
  return {
    target_duration: { secs: 30, nanos: 0 },
    crossfade: {
      enabled: true,
      max_duration_ms: 3000,
      curve: 'constant_power',
    },
    mastering: {
      lufs_target: -14.0,
      peak_db: -1.0,
      enable_limiting: true,
      compression_ratio: 2.0,
    },
    selection: {
      min_strong_beat_percentile: 0.8,
      block_size_beats: 4,
      preserve_intro_ms: 3000,
      preserve_outro_ms: 3000,
    },
    format: {
      sample_rate: 44100,
      channels: 2,
      bit_depth: 24,
      codec: 'WAV',
    },
    tuning: {
      enabled: false,
      mode: 'disabled',
      max_global_cents: 50.0,
      min_confidence: 0.7,
      force_tonic_hz: null,
      force_mode: null,
    },
  }
}

// ─── Jobs ──────────────────────────────────────────────────────────────

export type JobMode = 'manual' | 'assisted'

export interface JobRequest {
  track_id: string
  mode: JobMode
  user_prompt?: string
  prompt_id?: string
  pipeline_config?: PipelineConfig
}

// `JobResponse` documentado em docs/03-CONTRATOS-API.md §3.3. O backend atual
// retorna `JobSummary` (subset); quando o backend expandir para o contrato
// completo, este tipo já está pronto para receber os campos adicionais.
export interface JobResponse {
  job_id: string
  status: string
  stream_url: string
  created_at: string
  trace_id: string
  // Campos do JobResponse completo (quando backend implementar — item C4 do relatório)
  pipeline_config?: PipelineConfig
  graph?: unknown
  agent_run?: {
    tool_budget: number
    tools_used: number
    steps: Array<{
      step: number
      thought: string
      tool: string
      parameters: Record<string, unknown>
      result: string
      duration_ms: number
    }>
  }
  artifact?: {
    object_key: string
    sha256: string
    size_bytes: number
    duration_sec: number
    lufs: number
    true_peak_db: number
  }
  warnings?: Array<{
    code: string
    severity: 'info' | 'warning'
    at_sec: number | null
    message_ptbr: string
    hint_ptbr: string | null
    measured: Record<string, unknown> | null
  }>
}

// ─── Tracks ────────────────────────────────────────────────────────────

export interface TrackRequest {
  object_key: string
  display_name: string
  project_id?: string
}

export interface TrackResponse {
  track_id: string
  status: string
  // Aviso: o backend atualmente retorna `stream_url` apontando para
  // `/api/v1/tracks/{id}/events`, rota que NÃO existe no router (C8 do
  // relatório). Não usar até o backend implementar a rota SSE de track.
  stream_url: string
  display_name: string
  created_at: string
}

// ─── Uploads ───────────────────────────────────────────────────────────

export interface PresignRequest {
  filename: string
  size_bytes: number
  content_type: string
}

export interface PresignResponse {
  object_key: string
  upload_url: string
  method: string
  headers: Record<string, string>
  expires_at: string
}

// ─── Proposals ─────────────────────────────────────────────────────────

export type ProposalStatus =
  | 'pending'
  | 'approved'
  | 'rejected'
  | 'replanned'
  | 'expired'

export interface ProposalResponse {
  proposal_id: string
  status: ProposalStatus
  job_id: string
  tool: string
  tool_label_ptbr: string
  reason: string
  confidence: number
  expires_at: string
  // `parameters_suggestion` é `serde_json::Value` no Rust — aqui genérico.
  parameters_suggestion?: Record<string, unknown>
}

/**
 * Body do POST /approve — opcionalmente ajusta parâmetros (item C3 do mapa
 * de ação: o overlay de proposta é editável, e o ajuste vai via approve).
 */
export interface ApproveRequestBody {
  parameters?: Record<string, unknown>
}

// ─── System / Tools ────────────────────────────────────────────────────

export interface SystemInfo {
  version: string
  database_backend: string
  llm_provider: string
  llm_model: string
  data_egress: boolean
  cpu_cores: number
}

export interface ToolInfo {
  name: string
  label_ptbr: string
  category: string
  available: boolean
  parameters: ToolParam[]
  unavailable_reason?: string
}

export interface ToolParam {
  name: string
  type: string
  min?: number
  max?: number
  default?: unknown
  enum?: string[]
  unit?: string
}

// ─── SSE Events ─────────────────────────────────────────────────────────

export type SSEEventType =
  | 'stream.ready'
  | 'job.state'
  | 'job.progress'
  | 'job.warning'
  | 'job.completed'
  | 'job.failed'
  | 'job.cancelled'
  | 'agent.step_started'
  | 'agent.thought'
  | 'agent.tool_call'
  | 'agent.tool_result'
  | 'agent.error'
  | 'agent.replan'
  | 'agent.proposal'
  | 'agent.finished'
  | 'proposal.expired'
  | 'proposal.decided'
  | 'node.state'
  | 'node.parameters'
  | 'node.created'
  | 'system.resources'
  | 'recovery.report'
  // Compat com nome antigo (worker ainda usa em produção)
  | 'agent.tool'

export interface SSEEvent {
  type: SSEEventType
  data: Record<string, unknown>
}

// ─── Errors ────────────────────────────────────────────────────────────

/**
 * Erro estruturado RFC 7807 (docs/03-CONTRATOS-API.md §4). Quando o backend
 * devolve 422 (parameter_out_of_bounds), o frontend usa `errors[]` para
 * mapear cada campo inválido ao input correspondente (item C1 do mapa).
 */
export interface ApiError {
  type: string
  title: string
  status: number
  code: string
  detail: string
  instance?: string
  trace_id?: string
  errors?: Array<{
    field: string
    code: string
    min?: number
    max?: number
    received?: unknown
    enum?: string[]
  }>
}

/**
 * Tipa um erro de rede/HTTP para a UI. Se for um RFC 7807 estruturado, expõe
 * `apiError` para o caller extrair os campos invalidados; senão, expõe só
 * `message`.
 */
export class ApiRequestError extends Error {
  status: number
  apiError?: ApiError

  constructor(status: number, message: string, apiError?: ApiError) {
    super(message)
    this.name = 'ApiRequestError'
    this.status = status
    this.apiError = apiError
  }

  /**
   * Retorna `errors[]` do RFC 7807 se houver, ou array vazio. Helper para
   * a UI mapear campo → mensagem (item C1).
   */
  fieldErrors(): Array<NonNullable<NonNullable<ApiError['errors']>[number]>> {
    return this.apiError?.errors ?? []
  }
}
