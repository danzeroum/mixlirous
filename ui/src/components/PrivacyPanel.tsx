import { useState } from 'react'
import type { SystemInfo } from '../types/api'

interface Props {
  info: SystemInfo | null
  /** Consentimento atual do tenant (GET /tenants/me/consent). */
  consentAceitoEm: string | null
  onAceitar: () => Promise<void>
  /** Variante compacta para embutir no passo "Objetivo" do wizard. */
  compacto?: boolean
}

/**
 * Transparência de IA e dados (plano de design §"IA, dados e LGPD"):
 * - mostra provedor/modelo do LLM e o que é enviado a ele;
 * - consentimento separado para o modo assistido (IA), com registro;
 * - deixa claro o que fica local (áudio, DSP) e o que sai (prompt).
 */
function PrivacyPanel({ info, consentAceitoEm, onAceitar, compacto }: Props) {
  const [aceitando, setAceitando] = useState(false)
  const [erro, setErro] = useState<string | null>(null)
  // Consentimento aceito nesta sessão tem precedência sobre o valor vindo do
  // backend (derivação — sem setState em efeito).
  const [aceitoLocal, setAceitoLocal] = useState<string | null>(null)
  const aceitoEm = aceitoLocal ?? consentAceitoEm

  const aceitar = async () => {
    setAceitando(true)
    setErro(null)
    try {
      await onAceitar()
      setAceitoLocal(new Date().toISOString())
    } catch (e) {
      setErro(e instanceof Error ? e.message : 'Não foi possível registrar o consentimento.')
    } finally {
      setAceitando(false)
    }
  }

  return (
    <div
      data-testid="privacy-panel"
      className={`rounded-lg border border-gray-600 bg-gray-800 ${compacto ? 'p-3' : 'p-4'}`}
    >
      <h3 className={`font-bold text-white ${compacto ? 'text-sm' : 'text-base'}`}>
        IA, dados e privacidade
      </h3>

      <dl className={`mt-2 space-y-1 text-xs ${compacto ? '' : 'text-sm'}`}>
        <div className="flex gap-2">
          <dt className="text-gray-400 min-w-36">Provedor do assistente:</dt>
          <dd className="text-gray-100 font-medium">
            {info ? `${info.llm_provider} (${info.llm_model})` : 'carregando…'}
          </dd>
        </div>
        <div className="flex gap-2">
          <dt className="text-gray-400 min-w-36">O que vai para a IA:</dt>
          <dd className="text-gray-100">
            apenas o texto do seu objetivo (prompt). O áudio NÃO é enviado ao provedor — o
            processamento de som é local, determinístico (DSP em Rust).
          </dd>
        </div>
        <div className="flex gap-2">
          <dt className="text-gray-400 min-w-36">Onde fica o áudio:</dt>
          <dd className="text-gray-100">
            {info?.data_egress
              ? 'o modo atual pode exportar dados para serviços externos.'
              : 'neste modo local, no seu storage (disco/MinIO/S3) — sem egresso de dados.'}
          </dd>
        </div>
        <div className="flex gap-2">
          <dt className="text-gray-400 min-w-36">Ferramentas da IA:</dt>
          <dd className="text-gray-100">só usa ferramentas marcadas como disponíveis nesta instalação.</dd>
        </div>
      </dl>

      {aceitoEm ? (
        <p className="mt-3 text-xs text-green-300" data-testid="consent-status">
          Consentimento registrado{aceitoEm ? ` em ${new Date(aceitoEm).toLocaleString('pt-BR')}` : ''}.
          Você pode revogar trocando para o modo manual a qualquer momento.
        </p>
      ) : (
        <div className="mt-3">
          <button
            type="button"
            onClick={aceitar}
            disabled={aceitando || !info}
            data-testid="consent-accept"
            className="px-3 py-1.5 text-sm bg-purple-600 hover:bg-purple-500 text-white rounded disabled:opacity-50"
          >
            {aceitando ? 'Registrando…' : 'Concordo — usar modo assistido'}
          </button>
          <p className="text-[11px] text-gray-400 mt-1">
            Necessário uma vez por tenant para o agente de IA montar a receita. O modo manual não
            usa IA e não precisa de consentimento.
          </p>
        </div>
      )}

      {erro && (
        <p role="alert" className="mt-2 text-xs text-red-300">
          {erro} — tente novamente; nada foi perdido.
        </p>
      )}
    </div>
  )
}

export default PrivacyPanel
