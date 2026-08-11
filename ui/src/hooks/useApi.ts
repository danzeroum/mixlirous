import { useCallback, useState } from 'react'
import type {
  JobRequest,
  JobResponse,
  PresignRequest,
  PresignResponse,
  ProposalResponse,
  SystemInfo,
  TrackRequest,
  TrackResponse,
  ToolInfo,
} from '../types/api'

const BASE_URL = '/api/v1'

async function fetchJson<T>(url: string, options?: RequestInit): Promise<T> {
  const resp = await fetch(url, {
    ...options,
    headers: {
      'Content-Type': 'application/json',
      ...options?.headers,
    },
  })
  if (!resp.ok) {
    const body = await resp.text()
    throw new Error(`HTTP ${resp.status}: ${body}`)
  }
  return resp.json() as Promise<T>
}

export function useApi() {
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const withLoading = useCallback(async <T>(fn: () => Promise<T>): Promise<T> => {
    setLoading(true)
    setError(null)
    try {
      return await fn()
    } catch (e) {
      const msg = e instanceof Error ? e.message : 'Unknown error'
      setError(msg)
      throw e
    } finally {
      setLoading(false)
    }
  }, [])

  const createJob = useCallback(
    (req: JobRequest) => withLoading(() =>
      fetchJson<JobResponse>(`${BASE_URL}/jobs`, {
        method: 'POST',
        body: JSON.stringify(req),
      })
    ),
    [withLoading]
  )

  const getJob = useCallback(
    (jobId: string) => withLoading(() =>
      fetchJson<JobResponse>(`${BASE_URL}/jobs/${jobId}`)
    ),
    [withLoading]
  )

  const listJobs = useCallback(
    () => withLoading(() =>
      fetchJson<{ items: JobResponse[] }>(`${BASE_URL}/jobs`)
    ),
    [withLoading]
  )

  const createTrack = useCallback(
    (req: TrackRequest) => withLoading(() =>
      fetchJson<TrackResponse>(`${BASE_URL}/tracks`, {
        method: 'POST',
        body: JSON.stringify(req),
      })
    ),
    [withLoading]
  )

  const getPresignUrl = useCallback(
    (req: PresignRequest) => withLoading(() =>
      fetchJson<PresignResponse>(`${BASE_URL}/uploads/presign`, {
        method: 'POST',
        body: JSON.stringify(req),
      })
    ),
    [withLoading]
  )

  const listProposals = useCallback(
    (jobId: string) => withLoading(() =>
      fetchJson<ProposalResponse[]>(`${BASE_URL}/jobs/${jobId}/proposals`)
    ),
    [withLoading]
  )

  const approveProposal = useCallback(
    (jobId: string, proposalId: string) => withLoading(() =>
      fetchJson<unknown>(`${BASE_URL}/jobs/${jobId}/proposals/${proposalId}/approve`, {
        method: 'POST',
        body: JSON.stringify({}),
      })
    ),
    [withLoading]
  )

  const rejectProposal = useCallback(
    (jobId: string, proposalId: string) => withLoading(() =>
      fetchJson<unknown>(`${BASE_URL}/jobs/${jobId}/proposals/${proposalId}/reject`, {
        method: 'POST',
        body: JSON.stringify({}),
      })
    ),
    [withLoading]
  )

  const getSystemInfo = useCallback(
    () => withLoading(() => fetchJson<SystemInfo>(`${BASE_URL}/system/info`)),
    [withLoading]
  )

  const listTools = useCallback(
    () => withLoading(() => fetchJson<{ tools: ToolInfo[] }>(`${BASE_URL}/tools`)),
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