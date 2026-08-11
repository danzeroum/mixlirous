// API Types — Mixlirous
// Gerado a partir dos structs Rust (ts-rs marker)
// Sprint 3: definições manuais até ts-rs ser integrado no build

export interface JobRequest {
  track_id: string
  mode: 'manual' | 'assisted'
  user_prompt?: string
  prompt_id?: string
  pipeline_config?: PipelineConfig
}

export interface JobResponse {
  job_id: string
  status: string
  stream_url: string
  created_at: string
  trace_id: string
}

export interface PipelineConfig {
  target_duration_sec?: number
  block_size_beats?: number
  crossfade: { duration_ms: number; curve: string }
  mastering: {
    lufs_target: number
    peak_db: number
    compression_ratio: number
  }
}

export interface TrackRequest {
  object_key: string
  display_name: string
  project_id?: string
}

export interface TrackResponse {
  track_id: string
  status: string
  stream_url: string
  display_name: string
  created_at: string
}

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

export interface ProposalResponse {
  proposal_id: string
  status: 'pending' | 'approved' | 'rejected' | 'replanned' | 'expired'
  job_id: string
  tool: string
  tool_label_ptbr: string
  reason: string
  confidence: number
  expires_at: string
}

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

export type SSEEventType =
  | 'stream.ready'
  | 'job.state'
  | 'job.progress'
  | 'job.completed'
  | 'job.failed'
  | 'agent.thought'
  | 'agent.tool_call'
  | 'agent.proposal'
  | 'agent.error'
  | 'proposal.decided'

export interface SSEEvent {
  type: SSEEventType
  data: Record<string, unknown>
}