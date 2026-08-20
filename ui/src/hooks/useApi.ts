import { useCallback, useState } from 'react'
import type {
  ApiError,
  ApiRequestError,
  ApproveRequestBody,
  JobMode,
  JobRequest,
  JobResponse,
  PipelineConfig,
  PresignRequest,
  PresignResponse,
  ProposalResponse,
  SystemInfo,
  TrackRequest,
  TrackResponse,
  ToolInfo,
} from '../types/api'

const BASE_URL = '/api/v1'

/**
 * Tenta parsear o body como RFC 7807 (ApiError). Se não for JSON, ou se
 * faltar campos obrigatórios, devolve um ApiError mínimo com o status code.
 * Item C1 do mapa: o `fetchJson` é o ponto único onde o erro estruturado
 * é materializado — chamadas específicas só precisam passar o campo esperado.
 */
async function fetchJson<T>(url: string, options?: RequestInit): Promise<T> {
  const resp = await fetch(url, {
    ...options,
    headers: {
      'Content-Type': 'application/json',
      ...options?.headers,
    },
  })

  if (!resp.ok) {
    const body = await resp.text().catch(() => '')
    let apiError: ApiError | undefined
    try {
      const parsed = JSON.parse(body) as ApiError
      if (parsed && typeof parsed.status === 'number') {
        apiError = parsed
      }
    } catch {
      // Body não é JSON — provavelmente é uma string de erro do axum
      // (quando o handler retorna `(StatusCode, String)` em vez de
      // `application/problem+json`). Construímos um ApiError sintético.
    }
    const message = apiError?.detail ?? apiError?.title ?? body ?? `HTTP ${resp.status}`
    throw new ApiRequestError(resp.status, message, apiError)
  }

  return resp.json() as Promise<T>
}

export function useApi() {
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<ApiRequestError | null>(null)

  const withLoading = useCallback(async <T>(fn: () => Promise<T>): Promise<T> => {
    setLoading(true)
    setError(null)
    try {
      return await fn()
    } catch (e) {
      const err =
        e instanceof ApiRequestError
          ? e
          : new ApiRequestError(0, e instanceof Error ? e.message : 'Unknown error')
      setError(err)
      throw err
    } finally {
      setLoading(false)
    }
  }, [])

  const createJob = useCallback(
    (
      trackId: string,
      mode: JobMode,
      prompt: string,
      pipelineConfig?: PipelineConfig
    ) =>
      withLoading(() => {
        const req: JobRequest = {
          track_id: trackId,
          mode,
          // Só enviamos `user_prompt` quando o usuário digitou algo. Para
          // modo manual, é opcional; para assisted, o backend ignora se
          // não houver (com warning — ver worker.rs:400).
          user_prompt: prompt || undefined,
          pipeline_config: pipelineConfig,
        }
        return fetchJson<JobResponse>(`${BASE_URL}/jobs`, {
          method: 'POST',
          body: JSON.stringify(req),
        })
      }),
    [withLoading]
  )

  const getJob = useCallback(
    (jobId: string) =>
      withLoading(() => fetchJson<JobResponse>(`${BASE_URL}/jobs/${jobId}`)),
    [withLoading]
  )

  const listJobs = useCallback(
    () =>
      withLoading(() =>
        fetchJson<{ items: JobResponse[] }>(`${BASE_URL}/jobs`)
      ),
    [withLoading]
  )

  const createTrack = useCallback(
    (req: TrackRequest) =>
      withLoading(() =>
        fetchJson<TrackResponse>(`${BASE_URL}/tracks`, {
          method: 'POST',
          body: JSON.stringify(req),
        })
      ),
    [withLoading]
  )

  const getPresignUrl = useCallback(
    (req: PresignRequest) =>
      withLoading(() =>
        fetchJson<PresignResponse>(`${BASE_URL}/uploads/presign`, {
          method: 'POST',
          body: JSON.stringify(req),
        })
      ),
    [withLoading]
  )

  const listProposals = useCallback(
    (jobId: string) =>
      withLoading(() =>
        fetchJson<ProposalResponse[]>(`${BASE_URL}/jobs/${jobId}/proposals`)
      ),
    [withLoading]
  )

  /**
   * Item C3: `approve` agora aceita `parameters` (para o overlay editável).
   * Antes mandava `{}` — agora manda os ajustes do usuário.
   */
  const approveProposal = useCallback(
    (jobId: string, proposalId: string, body?: ApproveRequestBody) =>
      withLoading(() =>
        fetchJson<unknown>(
          `${BASE_URL}/jobs/${jobId}/proposals/${proposalId}/approve`,
          { method: 'POST', body: JSON.stringify(body ?? {}) }
        )
      ),
    [withLoading]
  )

  const rejectProposal = useCallback(
    (jobId: string, proposalId: string, reason?: string) =>
      withLoading(() =>
        fetchJson<unknown>(
          `${BASE_URL}/jobs/${jobId}/proposals/${proposalId}/reject`,
          { method: 'POST', body: JSON.stringify({ reason: reason ?? undefined }) }
        )
      ),
    [withLoading]
  )

  const getSystemInfo = useCallback(
    () => withLoading(() => fetchJson<SystemInfo>(`${BASE_URL}/system/info`)),
    [withLoading]
  )

  const listTools = useCallback(
    () =>
      withLoading(() => fetchJson<{ tools: ToolInfo[] }>(`${BASE_URL}/tools`)),
    [withLoading]
  )

  return {
    loading,
    error,
    createJob,
    getJob,
    listJobs,
    createTrack,
    getPresignUrl,
    listProposals,
    approveProposal,
    rejectProposal,
    getSystemInfo,
    listTools,
  }
}
