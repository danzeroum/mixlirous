import { useState, useRef } from 'react'

interface Props {
  onUploadComplete: (trackId: string) => void
  onCreateJob: (trackId: string, prompt: string) => Promise<void>
}

function UploadPanel({ onUploadComplete, onCreateJob }: Props) {
  const [prompt, setPrompt] = useState('')
  const [status, setStatus] = useState<'idle' | 'uploading' | 'registered' | 'error'>('idle')
  const [message, setMessage] = useState('')
  const [trackId, setTrackId] = useState<string | null>(null)
  const fileRef = useRef<HTMLInputElement>(null)

  const handleUpload = async () => {
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
      const { object_key, upload_url } = await presignResp.json()

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
      const track: { track_id: string } = await trackResp.json()

      setTrackId(track.track_id)
      setStatus('registered')
      setMessage('Faixa registrada!')
      onUploadComplete(track.track_id)
    } catch (e) {
      setStatus('error')
      setMessage(e instanceof Error ? e.message : 'Upload failed')
    }
  }

  const handleCreateJob = async () => {
    if (!trackId || !prompt) return
    await onCreateJob(trackId, prompt)
  }

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
        <label className="block text-sm text-gray-300 mb-2">Descricao do remix</label>
        <textarea
          value={prompt}
          onChange={(e) => setPrompt(e.target.value)}
          placeholder="ex: versao de 30s para Reels, agressiva, foco na bateria"
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
            className="px-4 py-2 bg-green-600 text-white rounded hover:bg-green-500 disabled:opacity-50"
          >
            Criar Remix
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
