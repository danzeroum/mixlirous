import type { JobResponse } from '../types/api'

interface Props {
  jobs: JobResponse[] | null
  tracksCount: number | null
  systemInfo: { version: string; database_backend: string; llm_provider: string } | null
  onNovoRemix: () => void
  onVerAtividade: () => void
}

/**
 * Visão geral (plano de design §"Navegação").
 *
 * 4.4 do PR #59: o backend NÃO tem domínio `Project` persistido — o
 * sistema opera com um projeto implícito por tenant. Esta visão é o
 * resumo do SEU ESPAÇO ("projeto atual do seu espaço"), não um
 * gerenciador de projetos; não sugerimos criação/lista/versionamento
 * de projetos que não existem. Estado honesto — nada de contadores
 * fake. Evolução para domínio Project real: backlog no adendo.
 */
function ProjetosView({ jobs, tracksCount, systemInfo, onNovoRemix, onVerAtividade }: Props) {
  const prontos = jobs?.filter((j) => j.status.toLowerCase() === 'completed').length ?? 0
  const emAndamento = jobs?.filter((j) => ['queued', 'processing'].includes(j.status.toLowerCase())).length ?? 0

  return (
    <div className="space-y-4">
      <div className="p-6 bg-gradient-to-br from-ai-800/60 to-surface-800 rounded-lg border border-ai-700">
        <p className="text-xs uppercase tracking-wide text-ai-300 mb-1" data-testid="espaco-rotulo">
          Projeto atual do seu espaço
        </p>
        <h2 className="text-xl font-bold text-ink-100">Bem-vindo ao Mixlirous</h2>
        <p className="text-ink-200 mt-2 max-w-2xl text-sm">
          Descreva a intenção musical, ouça a proposta do assistente, compare com o original e
          exporte o WAV — com a receita guardada para repetir. O canvas continua disponível para
          quem quer mexer nos parâmetros diretamente.
        </p>
        <div className="flex gap-3 mt-4">
          <button
            type="button"
            onClick={onNovoRemix}
            data-testid="cta-novo-remix"
            className="px-4 py-2 bg-ai-600 hover:bg-ai-500 text-white rounded font-medium"
          >
            + Novo remix
          </button>
          <button
            type="button"
            onClick={onVerAtividade}
            className="px-4 py-2 bg-surface-700 hover:bg-surface-600 text-ink-100 rounded"
          >
            Ver atividade
          </button>
        </div>
      </div>

      <div className="grid grid-cols-3 gap-3">
        <div className="p-4 bg-surface-800 rounded-lg border border-surface-700">
          <p className="text-2xl font-bold text-ink-100">{tracksCount ?? '—'}</p>
          <p className="text-xs text-ink-400">faixas na biblioteca</p>
        </div>
        <div className="p-4 bg-surface-800 rounded-lg border border-surface-700">
          <p className="text-2xl font-bold text-ink-100">{prontos}</p>
          <p className="text-xs text-ink-400">remixes prontos</p>
        </div>
        <div className="p-4 bg-surface-800 rounded-lg border border-surface-700">
          <p className="text-2xl font-bold text-ink-100">{emAndamento}</p>
          <p className="text-xs text-ink-400">em andamento agora</p>
        </div>
      </div>

      {systemInfo && (
        <p className="text-xs text-ink-400">
          Instalação: v{systemInfo.version} · banco {systemInfo.database_backend} · assistente{' '}
          {systemInfo.llm_provider}
        </p>
      )}

      <p className="text-xs text-ink-500">
        O Mixlirous opera hoje com um projeto único e implícito (o seu espaço). Gestão completa
        de projetos (criar, listar, versionar) não existe ainda — está registrada como evolução
        posterior no backlog (docs/ADENDO-PARETO-PRODUCAO.md).
      </p>
    </div>
  )
}

export default ProjetosView
