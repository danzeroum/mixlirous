import { useState } from 'react'
import type { PrivacyPolicy, SystemInfo } from '../types/api'

interface Props {
  info: SystemInfo | null
  /** Política de privacidade auditável (GET /system/privacy-policy). */
  politica: PrivacyPolicy | null
  /** Consentimento atual do tenant (GET /tenants/me/consent). */
  consentAceitoEm: string | null
  onAceitar: () => Promise<void>
  /** Revogação REAL (DELETE /tenants/me/consent) — remove o registro persistido. */
  onRevogar: () => Promise<void>
  /** Variante compacta para embutir no passo "Objetivo" do wizard. */
  compacto?: boolean
}

/**
 * Transparência de IA e dados (plano de design §"IA, dados e LGPD").
 *
 * Regra honesta de comunicação:
 * - Afirmações absolutas ("o áudio NÃO é enviado") SÓ aparecem quando o
 *   campo vem da política auditável do backend
 *   (`GET /system/privacy-policy` → audio_sent_to_provider === false),
 *   que é derivada da configuração real e coberta por teste de
 *   integração. Sem a política carregada, o texto é CONDICIONAL — nunca
 *   uma promessa hardcoded que o código não pode provar.
 * - O que efetivamente vai ao provedor (prompt e metadados de análise)
 *   é dito explicitamente, com base nos mesmos campos.
 * - Revogação: ação explícita (DELETE) que remove o consentimento
 *   persistido. Trocar para modo manual não revoga nada — a UI explica
 *   que a revogação vale para uso futuro e não apaga jobs/artefatos/
 *   auditoria já existentes.
 */
function PrivacyPanel({ info, politica, consentAceitoEm, onAceitar, onRevogar, compacto }: Props) {
  const [aceitando, setAceitando] = useState(false)
  const [revogando, setRevogando] = useState(false)
  const [erro, setErro] = useState<string | null>(null)
  // Consentimento aceito nesta sessão tem precedência sobre o valor vindo do
  // backend (derivação — sem setState em efeito).
  const [aceitoLocal, setAceitoLocal] = useState<string | null>(null)
  const [revogadoLocal, setRevogadoLocal] = useState(false)
  const aceitoEm = revogadoLocal ? null : (aceitoLocal ?? consentAceitoEm)

  const aceitar = async () => {
    setAceitando(true)
    setErro(null)
    try {
      await onAceitar()
      setAceitoLocal(new Date().toISOString())
      setRevogadoLocal(false)
    } catch (e) {
      setErro(e instanceof Error ? e.message : 'Não foi possível registrar o consentimento.')
    } finally {
      setAceitando(false)
    }
  }

  const revogar = async () => {
    setRevogando(true)
    setErro(null)
    try {
      await onRevogar()
      setAceitoLocal(null)
      setRevogadoLocal(true)
    } catch (e) {
      setErro(e instanceof Error ? e.message : 'Não foi possível revogar o consentimento.')
    } finally {
      setRevogando(false)
    }
  }

  // ── Linguagem de egresso de dados — condicional se não há política ──
  const audioNaoEnviado = politica?.audio_sent_to_provider === false
  const egressoTexto = audioNaoEnviado
    ? 'O áudio NÃO é enviado ao provedor — o processamento de som é executado pelo motor ' +
      'DSP do Mixlirous nesta instalação. O que sai para o provedor é o texto do seu ' +
      'objetivo e metadados técnicos da faixa (duração, sample rate, canais).'
    : 'O processamento de áudio é executado pelo motor DSP do Mixlirous. Antes de ativar ' +
      'o modo assistido, confira os dados enviados ao provedor configurado nesta ' +
      'instalação.'

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
          <dd className="text-gray-100" data-testid="privacy-egresso">
            {egressoTexto}
            {!audioNaoEnviado && (
              <span className="block text-[11px] text-gray-400 mt-1">
                (aviso detalhado indisponível agora — política de privacidade não carregada)
              </span>
            )}
          </dd>
        </div>
        <div className="flex gap-2">
          <dt className="text-gray-400 min-w-36">Onde fica o áudio:</dt>
          <dd className="text-gray-100">
            {politica?.retention_policy ??
              (info?.data_egress
                ? 'o modo atual pode exportar dados para serviços externos — confira a política desta instalação.'
                : 'neste modo local, no seu storage (disco/MinIO/S3) — sem egresso de dados.')}
          </dd>
        </div>
        <div className="flex gap-2">
          <dt className="text-gray-400 min-w-36">Ferramentas da IA:</dt>
          <dd className="text-gray-100">só usa ferramentas marcadas como disponíveis nesta instalação.</dd>
        </div>
      </dl>

      {aceitoEm ? (
        <div className="mt-3" data-testid="consent-status">
          <p className="text-xs text-green-300">
            Consentimento registrado{aceitoEm ? ` em ${new Date(aceitoEm).toLocaleString('pt-BR')}` : ''}
            {politica?.provider ? ` para o provedor ${politica.provider}` : ''}.
          </p>
          <button
            type="button"
            onClick={revogar}
            disabled={revogando}
            data-testid="consent-revoke"
            className="mt-2 px-3 py-1.5 text-xs bg-red-900/70 hover:bg-red-800 text-red-100 rounded border border-red-800 disabled:opacity-50"
            title="Remove o consentimento persistido deste tenant (DELETE /tenants/me/consent)."
          >
            {revogando ? 'Revogando…' : 'Revogar consentimento para modo assistido'}
          </button>
          <p className="text-[11px] text-gray-400 mt-1">
            A revogação vale para o uso FUTURO do modo assistido (o assistente volta a pedir
            consentimento). Ela NÃO apaga automaticamente remixes, artefatos ou registros de
            auditoria já existentes.
          </p>
        </div>
      ) : (
        <div className="mt-3">
          {revogadoLocal && (
            <p className="text-xs text-orange-300 mb-2" role="status" data-testid="consent-revoked">
              Consentimento revogado — o modo assistido fica indisponível até um novo aceite.
              Nada do que já foi criado foi apagado.
            </p>
          )}
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
