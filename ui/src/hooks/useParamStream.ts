// Re-export para compat com imports antigos. O hook real agora é `useSSE`
// (ver `useSSE.ts`) — o nome `useParamStream` era confuso: não é sobre
// "param stream", é sobre Server-Sent Events. Mantido aqui para não quebrar
// imports durante a transição. Em iteração futura, mover todos os imports
// para `useSSE` e remover este arquivo.
export { useSSE as useParamStream } from './useSSE'
export type { StreamEvent, UseSSEOptions } from './useSSE'
