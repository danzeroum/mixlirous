import { useState, useRef, useCallback } from 'react'
import type { JobMode } from '../types/api'

interface Props {
  onUploadComplete: (trackId: string) => void
  onCreateJob: (trackId: string, prompt: string) => Promise<void>
  /** Item B3 do mapa: modo é controlado pelo usuário agora, não hardcoded. */
  mode: JobMode
  onModeChange: (mode: JobMode) => void
}

function UploadPanel({ onUploadComplete, onCreateJob, mode, onModeChange }: Props) {
  const [prompt, setPrompt] = useState('')
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
        headers: { 'Content-Type': 'application/json' },
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
        headers: { 'Content-Type': file.type || 'audio/wav' },
        body: file,
      })
      if (!uploadResp.ok) throw new Error('Failed to upload file')

      // Step 3: Register the track (once)
      const trackResp = await fetch('/api/v1/tracks', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
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
      setStatus('error')
      setMessage(e instanceof Error ? e.message : 'Upload failed')
    }
  }, [onUploadComplete])

  const handleCreateJob = useCallback(async () => {
    if (!trackId || !prompt) return
    await onCreateJob(trackId, prompt)
  }, [trackId, prompt, onCreateJob])

  return (
    <div className="bg-gray-800 p-6 rounded-lg mb-4">
      <h2 className="text-lg font-bold text-white mb-4">Upload de faixa</h2>

      <div className="mb-4">
        <label className="block text-sm text-gray-300 mb-2">Arquivo de audio</label>
        <input
          ref={fileRef}
          type="file"
          accept="audio/*,.wav,.flac,.aiff,.mp3,.m4a,.aac"
          className="w-full text-gray-300 file:mr-4 file:py-2 file:px-4 file:rounded file:bg-green-600 file:text-white file:border-0"
        />
      </div>

      <div className="mb-4">
        <label className="block text-sm text-gray-300 mb-2">Modo de processamento</label>
        <div className="flex gap-2">
          <button
            type="button"
            onClick={() => onModeChange('manual')}
            className={`flex-1 px-3 py-2 rounded text-sm ${
              mode === 'manual'
                ? 'bg-blue-600 text-white'
                : 'bg-gray-700 text-gray-300 hover:bg-gray-600'
            }`}
            title="Usa exatamente os parâmetros enviados — não chama o agente."
          >
            Manual
          </button>
          <button
            type="button"
            onClick={() => onModeChange('assisted')}
            className={`flex-1 px-3 py-2 rounded text-sm ${
              mode === 'assisted'
                ? 'bg-purple-600 text-white'
                : 'bg-gray-700 text-gray-300 hover:bg-gray-600'
            }`}
            title="O agente ReAct escolhe parâmetros a partir do prompt. Pode emitir propostas HITL."
          >
            Assistido
          </button>
        </div>
      </div>

      <div className="mb-4">
        <label className="block text-sm text-gray-300 mb-2">
          {mode === 'assisted' ? 'Descrição do remix' : 'Descrição (opcional)'}
        </label>
        <textarea
          value={prompt}
          onChange={(e) => setPrompt(e.target.value)}
          placeholder={
            mode === 'assisted'
              ? 'ex: versão de 30s para Reels, agressiva, foco na bateria'
              : 'ex: lote de 50 faixas com a mesma receita validada'
          }
          className="w-full p-2 bg-gray-700 text-white rounded border border-gray-600 resize-none h-20"
          maxLength={4096}
        />
      </div>

      <div className="flex gap-3">
        <button
          onClick={handleUpload}
          disabled={status === 'uploading'}
          className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-500 disabled:opacity-50"
        >
          {status === 'uploading' ? 'Enviando...' : 'Upload'}
        </button>
        {prompt && trackId && (
          <button
            onClick={handleCreateJob}
            disabled={status === 'uploading'}
            className={`px-4 py-2 rounded text-white disabled:opacity-50 ${
              mode === 'assisted' ? 'bg-purple-600 hover:bg-purple-500' : 'bg-green-600 hover:bg-green-500'
            }`}
          >
            {mode === 'assisted' ? 'Criar Remix (com IA)' : 'Criar Remix'}
          </button>
        )}
      </div>

      {message && (
        <p className={`mt-3 text-sm ${status === 'error' ? 'text-red-400' : 'text-green-400'}`}>
          {message}
        </p>
      )}
    </div>
  )
}

export default UploadPanel
