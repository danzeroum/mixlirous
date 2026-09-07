import { useState, useRef, useCallback } from 'react'
import type { JobMode } from '../types/api'
import { authHeaders } from '../lib/authHeaders'

interface Props {
  onUploadComplete: (trackId: string) => void
  onCreateJob: (trackId: string, prompt: string) => Promise<void>
  /** Item B3 do mapa: modo é controlado pelo usuário agora, não hardcoded. */
  mode: JobMode
  onModeChange: (mode: JobMode) => void
  /**
   * Plano de design (etapa única): o painel pode atuar como SÓ UPLOAD
   * (passo 1 do wizard) ou SÓ OBJETIVO (passo 3), sem duplicar os
   * `data-testid` do fluxo E2E.
   */
  soUpload?: boolean
  soObjetivo?: boolean
  /** Prompt controlado pelo pai (presets do wizard). */
  promptExterno?: [string, (v: string) => void]
  /** Track registrada fora desta instância (wizard: passo 1 faz o upload). */
  trackIdExterno?: string | null
  /** Desabilita o botão criar (ex.: consentimento pendente no assistido). */
  criarDesabilitado?: boolean
}

const UPLOAD_STATUS_ERRO = 'error'

function UploadPanel({
  onUploadComplete,
  onCreateJob,
  mode,
  onModeChange,
  soUpload = false,
  soObjetivo = false,
  promptExterno,
  trackIdExterno,
  criarDesabilitado = false,
}: Props) {
  const [promptInterno, setPromptInterno] = useState('')
  const prompt = promptExterno ? promptExterno[0] : promptInterno
  const setPrompt = promptExterno ? promptExterno[1] : setPromptInterno
  const [status, setStatus] = useState<'idle' | 'uploading' | 'registered' | 'error'>('idle')
  const [message, setMessage] = useState('')
  const [trackId, setTrackId] = useState<string | null>(null)
  const fileRef = useRef<HTMLInputElement>(null)

  const handleUpload = useCallback(async () => {
    const file = fileRef.current?.files?.[0]
    if (!file) return

    setStatus('uploading')
    setMessage(`Enviando ${file.name}...`)

    try {
      // Step 1: Get presigned URL
      const presignResp = await fetch('/api/v1/uploads/presign', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', ...authHeaders() },
        body: JSON.stringify({
          filename: file.name,
          size_bytes: file.size,
          content_type: file.type || 'audio/wav',
        }),
      })
      if (!presignResp.ok) throw new Error('Failed to get upload URL')
      const { object_key, upload_url } = await presignResp.json() as {
        object_key: string
        upload_url: string
      }

      // Step 2: PUT the file bytes to the upload URL
      const uploadResp = await fetch(upload_url, {
        method: 'PUT',
        headers: { 'Content-Type': file.type || 'audio/wav', ...authHeaders() },
        body: file,
      })
      if (!uploadResp.ok) throw new Error('Failed to upload file')

      // Step 3: Register the track (once)
      const trackResp = await fetch('/api/v1/tracks', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', ...authHeaders() },
        body: JSON.stringify({
          object_key,
          display_name: file.name.replace(/\.[^.]+$/, ''),
        }),
      })
      if (!trackResp.ok) throw new Error('Failed to register track')
      const track = await trackResp.json() as { track_id: string }

      setTrackId(track.track_id)
      setStatus('registered')
      setMessage('Faixa registrada!')
      onUploadComplete(track.track_id)
    } catch (e) {
      setStatus(UPLOAD_STATUS_ERRO)
      setMessage(e instanceof Error ? e.message : 'Upload failed')
    }
  }, [onUploadComplete])

  const handleCreateJob = useCallback(async () => {
    const id = trackIdExterno ?? trackId
    if (!id || !prompt) return
    await onCreateJob(id, prompt)
  }, [trackIdExterno, trackId, prompt, onCreateJob])

  return (
    <div className="bg-gray-800 p-6 rounded-lg mb-4">
      <h2 className="text-lg font-bold text-white mb-4">
        {soObjetivo ? 'Objetivo do remix' : 'Upload de faixa'}
      </h2>

      {!soObjetivo && (
        <div className="mb-4">
          <label className="block text-sm text-gray-300 mb-2" htmlFor="upload-arquivo">
            Arquivo de audio
          </label>
          <input
            ref={fileRef}
            id="upload-arquivo"
            type="file"
            accept="audio/*,.wav,.flac,.aiff,.mp3,.m4a,.aac"
            data-testid="upload-input"
            className="w-full text-gray-300 file:mr-4 file:py-2 file:px-4 file:rounded file:bg-green-600 file:text-white file:border-0"
          />
        </div>
      )}

      {!soUpload && (
        <div className="mb-4">
          <span className="block text-sm text-gray-300 mb-2">Como você quer remixar</span>
          <div className="flex gap-2" role="radiogroup" aria-label="Modo de processamento">
            <button
              type="button"
              role="radio"
              aria-checked={mode === 'manual'}
              onClick={() => onModeChange('manual')}
              className={`flex-1 px-3 py-2 rounded text-sm ${
                mode === 'manual'
                  ? 'bg-blue-600 text-white'
                  : 'bg-gray-700 text-gray-300 hover:bg-gray-600'
              }`}
              title="Usa exatamente a receita do canvas — não chama IA."
            >
              Manual (sem IA)
            </button>
            <button
              type="button"
              role="radio"
              aria-checked={mode === 'assisted'}
              onClick={() => onModeChange('assisted')}
              className={`flex-1 px-3 py-2 rounded text-sm ${
                mode === 'assisted'
                  ? 'bg-purple-600 text-white'
                  : 'bg-gray-700 text-gray-300 hover:bg-gray-600'
              }`}
              title="O assistente propõe a receita a partir do objetivo; você aprova antes de renderizar."
            >
              Com assistente (IA)
            </button>
          </div>
        </div>
      )}

      {!soUpload && (
        <div className="mb-4">
          <label className="block text-sm text-gray-300 mb-2" htmlFor="objetivo-remix">
            {mode === 'assisted'
              ? 'Descreva o que você quer ouvir'
              : 'Descrição (opcional — vira anotação da receita)'}
          </label>
          <textarea
            id="objetivo-remix"
            value={prompt}
            onChange={(e) => setPrompt(e.target.value)}
            data-testid="prompt-input"
            placeholder={
              mode === 'assisted'
                ? 'ex: versão de 30s para Reels, agressiva, foco na bateria'
                : 'ex: lote de 50 faixas com a mesma receita validada'
            }
            className="w-full p-2 bg-gray-700 text-white rounded border border-gray-600 resize-none h-20"
            maxLength={4096}
          />
        </div>
      )}

      {!soObjetivo && (
        <div className="flex gap-3">
          <button
            onClick={handleUpload}
            disabled={status === 'uploading'}
            data-testid="upload-button"
            className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-500 disabled:opacity-50"
          >
            {status === 'uploading' ? 'Enviando...' : 'Upload'}
          </button>
        </div>
      )}

      {!soUpload && (
        <div className="flex gap-3">
          {prompt && (trackIdExterno ?? trackId) && (
            <button
              onClick={handleCreateJob}
              disabled={status === 'uploading' || criarDesabilitado}
              data-testid="create-job"
              title={
                criarDesabilitado
                  ? 'Registre o consentimento para usar o assistente.'
                  : undefined
              }
              className={`px-4 py-2 rounded text-white disabled:opacity-50 ${
                mode === 'assisted' ? 'bg-purple-600 hover:bg-purple-500' : 'bg-green-600 hover:bg-green-500'
              }`}
            >
              {mode === 'assisted' ? 'Criar remix com assistente' : 'Criar remix com a receita do canvas'}
            </button>
          )}
        </div>
      )}

      {message && (
        <p
          data-testid="upload-status"
          aria-live="polite"
          className={`mt-3 text-sm ${status === UPLOAD_STATUS_ERRO ? 'text-red-400' : 'text-green-400'}`}
        >
          {message}
          {status === UPLOAD_STATUS_ERRO && (
            <span className="block text-xs text-gray-400 mt-1">
              Verifique a conexão e tente de novo — nada foi perdido.
            </span>
          )}
        </p>
      )}
    </div>
  )
}

export default UploadPanel
