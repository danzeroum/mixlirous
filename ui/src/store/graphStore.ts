import { create } from 'zustand'
import {
  applyEdgeChanges,
  applyNodeChanges,
  addEdge,
  type Node,
  type Edge,
  type NodeChange,
  type EdgeChange,
  type Connection,
} from '@xyflow/react'

export interface NodeData extends Record<string, unknown> {
  label: string
  status?: string
  /** Nome da ferramenta (`GET /tools`) quando o nó veio da paleta. */
  tool?: string
}

export type RemixNode = Node<NodeData>

interface GraphState {
  nodes: RemixNode[]
  edges: Edge[]
  onNodesChange: (changes: NodeChange<RemixNode>[]) => void
  onEdgesChange: (changes: EdgeChange[]) => void
  onConnect: (connection: Connection) => void
  setGraph: (nodes: RemixNode[], edges: Edge[]) => void
  updateNodeData: (id: string, data: Partial<NodeData>) => void
  /** Item 4 do Lote 1: adiciona nó de efeito a partir da paleta de ferramentas. */
  addToolNode: (toolName: string, label: string) => void
}

// Fonte da verdade do grafo é o servidor (GET /api/v1/jobs/:id) — ver
// docs/03-CONTRATOS-API.md §3.3 e docs/05-AGENTE-IA-HITL.md §4 "Retomada
// após reload". Por isso este store não usa `zustand/persist`: persistir o
// grafo em localStorage o deixaria dessincronizado do backend após um F5.
export const useGraphStore = create<GraphState>((set, get) => ({
  nodes: [],
  edges: [],
  onNodesChange: (changes) => set({ nodes: applyNodeChanges<RemixNode>(changes, get().nodes) }),
  onEdgesChange: (changes) => set({ edges: applyEdgeChanges(changes, get().edges) }),
  onConnect: (connection) => set({ edges: addEdge(connection, get().edges) }),
  setGraph: (nodes, edges) => set({ nodes, edges }),
  updateNodeData: (id, data) =>
    set((state) => ({
      nodes: state.nodes.map((n) => (n.id === id ? { ...n, data: { ...n.data, ...data } } : n)),
    })),
  // Um nó por ferramenta — clicar de novo na mesma ferramenta não duplica.
  // A posição é distribuída em grade para os nós não nascerem empilhados.
  addToolNode: (toolName, label) =>
    set((state) => {
      const id = `tool-${toolName}`
      if (state.nodes.some((n) => n.id === id)) return state
      const position = {
        x: 60 + (state.nodes.length % 4) * 200,
        y: 60 + Math.floor(state.nodes.length / 4) * 140,
      }
      return {
        nodes: [
          ...state.nodes,
          { id, type: 'effect', position, data: { label, tool: toolName } },
        ],
      }
    }),
}))
