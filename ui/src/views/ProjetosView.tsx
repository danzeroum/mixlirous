import type { JobResponse } from '../types/api'

interface Props {
  jobs: JobResponse[] | null
  tracksCount: number | null
  systemInfo: { version: string; database_backend: string; llm_provider: string } | null
  onNovoRemix: () => void
  onVerAtividade: () => void
}

/**
 * Projetos — a home (plano de design §"Navegação"). Hoje o Mixlirous opera
 * com um projeto implícito (o tenant local); a visão resume o que existe e
 * oferece a próxima ação: um novo remix. Estado honesto — nada de contadores
 * fake.
 */
function ProjetosView({ jobs, tracksCount, systemInfo, onNovoRemix, onVerAtividade }: Props) {
  const prontos = jobs?.filter((j) => j.status.toLowerCase() === 'completed').length ?? 0
  const emAndamento = jobs?.filter((j) => ['queued', 'processing'].includes(j.status.toLowerCase())).length ?? 0

  return (
    <div className="space-y-4">
      <div className="p-6 bg-gradient-to-br from-purple-900/60 to-gray-800 rounded-lg border border-purple-800">
        <h2 className="text-xl font-bold text-white">Bem-vindo ao Mixlirous</h2>
        <p className="text-gray-200 mt-2 max-w-2xl text-sm">
          Descreva a intenção musical, ouça a proposta do assistente, compare com o original e
          exporte o WAV — com a receita guardada para repetir. O canvas continua disponível para
          quem quer mexer nos parâmetros diretamente.
        </p>
        <div className="flex gap-3 mt-4">
          <button
            type="button"
            onClick={onNovoRemix}
            data-testid="cta-novo-remix"
            className="px-4 py-2 bg-purple-600 hover:bg-purple-500 text-white rounded font-medium"
          >
            + Novo remix
          </button>
          <button
            type="button"
            onClick={onVerAtividade}
            className="px-4 py-2 bg-gray-700 hover:bg-gray-600 text-white rounded"
          >
            Ver atividade
          </button>
        </div>
      </div>

      <div className="grid grid-cols-3 gap-3">
        <div className="p-4 bg-gray-800 rounded-lg border border-gray-700">
          <p className="text-2xl font-bold text-white">{tracksCount ?? '—'}</p>
          <p className="text-xs text-gray-400">faixas na biblioteca</p>
        </div>
        <div className="p-4 bg-gray-800 rounded-lg border border-gray-700">
          <p className="text-2xl font-bold text-white">{prontos}</p>
          <p className="text-xs text-gray-400">remixes prontos</p>
        </div>
        <div className="p-4 bg-gray-800 rounded-lg border border-gray-700">
          <p className="text-2xl font-bold text-white">{emAndamento}</p>
          <p className="text-xs text-gray-400">em andamento agora</p>
        </div>
      </div>

      {systemInfo && (
        <p className="text-xs text-gray-400">
          Instalação: v{systemInfo.version} · banco {systemInfo.database_backend} · assistente{' '}
          {systemInfo.llm_provider}
        </p>
      )}
    </div>
  )
}

export default ProjetosView
