import { useState } from 'react'

interface WelcomeOnboardingProps {
  onDismiss: () => void
}

const STEPS = [
  {
    title: 'Bem-vindo ao Mixlirous',
    content: `O Mixlirous e uma ferramenta de remix inteligente que usa IA para analisar, selecionar e recombinar trechos de audio. Faca upload de uma faixa, descreva o remix desejado e ouca o resultado.

Este guia rapido vai te preparar para o primeiro uso.`,
  },
  {
    title: '1. Envie uma faixa',
    content: `Use o painel lateral esquerdo para fazer upload de um arquivo de audio (WAV, MP3, FLAC, OGG).

A faixa sera analisada automaticamente: batidas, secoes, energia e caracteristicas espectrais sao extraidas pelo motor DSP.`,
  },
  {
    title: '2. Descreva o remix',
    content: `Digite um prompt em linguagem natural descrevendo o que deseja:

- "Crie um remix de 30 segundos com as partes mais energeticas"
- "Faca um medley com os melhores trechos, crossfade suave"
- "Selecione 20 segundos do verso principal"

O agente de IA vai propor parametros otimos para o seu pedido.`,
  },
  {
    title: '3. Aprove ou ajuste',
    content: `O agente envia propostas com parametros sugeridos (crossfade, LUFS, compressor).

Voce pode:
- Aprovar a proposta e deixar o DSP renderizar
- Rejeitar e pedir um replanejamento
- Ajustar manualmente os parametros antes de aprovar`,
  },
  {
    title: '4. Ouca e exporte',
    content: `Quando o render terminar, ouca o resultado diretamente no navegador.

Compare com o original, faca ajustes se necessario e exporte o arquivo final.

Dica: use fones de ouvido para avaliar melhor a qualidade do crossfade e da masterizacao.`,
  },
]

export default function WelcomeOnboarding({ onDismiss }: WelcomeOnboardingProps) {
  const [step, setStep] = useState(0)
  const total = STEPS.length
  const isLast = step === total - 1

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm">
      <div className="bg-gray-800 rounded-2xl shadow-2xl max-w-lg w-full mx-4 overflow-hidden">
        <div className="bg-gradient-to-r from-indigo-600 to-purple-600 px-6 py-4">
          <h2 className="text-xl font-bold text-white">{STEPS[step].title}</h2>
          <div className="mt-3 h-1 bg-white/20 rounded-full">
            <div
              className="h-full bg-white rounded-full transition-all duration-300"
              style={{ width: `${((step + 1) / total) * 100}%` }}
            />
          </div>
        </div>

        <div className="px-6 py-6">
          <p className="text-gray-300 whitespace-pre-line leading-relaxed text-sm">
            {STEPS[step].content}
          </p>
        </div>

        <div className="flex items-center justify-between px-6 py-4 border-t border-gray-700">
          <span className="text-xs text-gray-500">
            {step + 1} / {total}
          </span>
          <div className="flex gap-3">
            <button
              onClick={() => setStep((s) => Math.max(0, s - 1))}
              disabled={step === 0}
              className="px-4 py-2 text-sm text-gray-300 hover:text-white disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
            >
              Anterior
            </button>
            {isLast ? (
              <button
                onClick={onDismiss}
                className="px-6 py-2 text-sm font-medium bg-indigo-600 hover:bg-indigo-500 text-white rounded-lg transition-colors"
              >
                Comecar a usar
              </button>
            ) : (
              <button
                onClick={() => setStep((s) => s + 1)}
                className="px-6 py-2 text-sm font-medium bg-indigo-600 hover:bg-indigo-500 text-white rounded-lg transition-colors"
              >
                Proximo
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  )
}
