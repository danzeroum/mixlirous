interface PrivacyNoticeProps {
  onAccept: () => void
  onDecline?: () => void
}

export default function PrivacyNotice({ onAccept, onDecline }: PrivacyNoticeProps) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm">
      <div className="bg-gray-800 rounded-2xl shadow-2xl max-w-md w-full mx-4 overflow-hidden">
        {/* Icon */}
        <div className="flex justify-center pt-6">
          <div className="w-16 h-16 rounded-full bg-amber-500/20 flex items-center justify-center">
            <svg className="w-8 h-8 text-amber-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126zM12 15.75h.007v.008H12v-.008z" />
            </svg>
          </div>
        </div>

        {/* Content */}
        <div className="px-6 py-5">
          <h2 className="text-lg font-bold text-white text-center mb-3">
            Privacidade e IA
          </h2>

          <div className="space-y-3 text-sm text-gray-300">
            <p>
              O Mixlirous usa um modelo de IA (LLM) para interpretar seus prompts e sugerir parametros de remix.
            </p>

            <div className="bg-gray-900/50 rounded-lg p-3 space-y-2">
              <p className="font-medium text-gray-200">O que e enviado ao provedor LLM:</p>
              <ul className="list-disc list-inside space-y-1 text-gray-400">
                <li>O prompt de texto que voce digita</li>
                <li>Metadados tecnicos da faixa (BPM, duracao, energia)</li>
                <li>O catalogo de ferramentas disponiveis</li>
              </ul>
            </div>

            <div className="bg-gray-900/50 rounded-lg p-3 space-y-2">
              <p className="font-medium text-gray-200">O que NAO e enviado:</p>
              <ul className="list-disc list-inside space-y-1 text-gray-400">
                <li>O arquivo de audio em si (nunca sai da sua maquina)</li>
                <li>Dados pessoais ou informacoes de sistema</li>
              </ul>
            </div>

            <p className="text-xs text-gray-500">
              Para usar sem nenhum envio externo, configure o Ollama como provedor LLM em config/default.yaml.
              Assim tudo roda localmente.
            </p>
          </div>
        </div>

        {/* Actions */}
        <div className="flex gap-3 px-6 py-4 border-t border-gray-700">
          <button
            onClick={onDecline}
            className="flex-1 px-4 py-2.5 text-sm text-gray-400 hover:text-white border border-gray-600 rounded-lg transition-colors"
          >
            Usar sem IA
          </button>
          <button
            onClick={onAccept}
            className="flex-1 px-4 py-2.5 text-sm font-medium bg-indigo-600 hover:bg-indigo-500 text-white rounded-lg transition-colors"
          >
            Entendi e aceito
          </button>
        </div>
      </div>
    </div>
  )
}
