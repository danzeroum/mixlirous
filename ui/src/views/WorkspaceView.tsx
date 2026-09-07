import { ReactFlowProvider } from '@xyflow/react'
import RemixCanvas from '../components/RemixCanvas'
import ToolPalette from '../components/ToolPalette'
import type { ToolInfo } from '../types/api'

interface Props {
  tools: ToolInfo[] | null
  toolsLoading: boolean
}

/**
 * Espaço de trabalho (plano de design §"Navegação"): o canvas como MODO
 * AVANÇADO — mesma projeção editável do `PipelineConfig`, sem o wizard.
 * Compatível com a experiência atual: paleta + grafo + overlay (que fica
 * global no App) + player quando o job conclui.
 */
function WorkspaceView({ tools, toolsLoading }: Props) {
  return (
    <div className="flex-1 relative" data-testid="workspace">
      <ReactFlowProvider>
        <RemixCanvas />
        <div className="absolute top-4 left-4 z-10 w-64">
          <ToolPalette tools={tools} loading={toolsLoading} />
        </div>
      </ReactFlowProvider>
    </div>
  )
}

export default WorkspaceView
