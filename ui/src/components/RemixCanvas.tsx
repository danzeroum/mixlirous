import { useCallback } from 'react'
import { ReactFlow, Controls, MiniMap, Background, BackgroundVariant, type NodeProps, type Connection } from '@xyflow/react'
import '@xyflow/react/dist/style.css'
import { useGraphStore, type RemixNode } from '../store/graphStore'

function AudioNode({ data }: NodeProps<RemixNode>) {
  return (
    <div className="p-2 bg-blue-600 rounded shadow">
      <span className="text-white">{data.label}</span>
      {data.status && <div className="text-xs text-blue-200">{data.status}</div>}
    </div>
  )
}

function EffectNode({ data }: NodeProps<RemixNode>) {
  return (
    <div className="p-2 bg-purple-600 rounded shadow">
      <span className="text-white">{data.label}</span>
    </div>
  )
}

const nodeTypes = { audio: AudioNode, effect: EffectNode }

function RemixCanvas() {
  const { nodes, edges, onNodesChange, onEdgesChange, onConnect } = useGraphStore()

  const handleConnect = useCallback((connection: Connection) => onConnect(connection), [onConnect])

  if (nodes.length === 0) {
    return (
      <div className="w-full h-full flex items-center justify-center text-gray-400">
        <p>Envie uma faixa para começar a montar o remix.</p>
      </div>
    )
  }

  return (
    <ReactFlow
      nodes={nodes}
      edges={edges}
      nodeTypes={nodeTypes}
      onNodesChange={onNodesChange}
      onEdgesChange={onEdgesChange}
      onConnect={handleConnect}
      fitView
      className="w-full h-full"
    >
      <Controls />
      <MiniMap nodeColor="#2563eb" />
      <Background variant={BackgroundVariant.Dots} />
    </ReactFlow>
  )
}

export default RemixCanvas
