import { defineConfig, type Plugin } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import { randomBytes } from 'node:crypto'

// QA-0005 — plugin Vite que adiciona `traceparent` (W3C Trace Context) a
// TODAS as respostas servidas pelo dev server (home, assets, etc.).
// O contrato docs/03 §1 exige: "cliente envia traceparent (W3C); servidor
// devolve no response". A API já ecoa via middleware axum; este plugin
// estende o mesmo comportamento para a UI servida pelo Vite, inclusive
// quando a request não é proxyada para /api/* (ex.: GET / que devolve o
// shell da SPA).
//
// Fluxo:
// 1. Lê `traceparent` da request.
// 2. Se presente e W3C-válido (version-trace_id-parent_id-flags), ecoa.
// 3. Senão, gera um novo via crypto.randomBytes (16+8 bytes).
// 4. Seta o header no response — não sobrescreve se já houver (raro).
//
// Em produção (nginx servindo build estático) o mesmo comportamento deve
// ser entregue por add_header no vhost (ver docs/18). Este plugin é
// específico do dev server.
function traceparentPlugin(): Plugin {
  const HEADER = 'traceparent'
  const VALID_RE = /^00-[0-9a-f]{32}-[0-9a-f]{16}-[0-9a-f]{2}$/

  function generate(): string {
    const traceId = randomBytes(16).toString('hex')
    const parentId = randomBytes(8).toString('hex')
    return `00-${traceId}-${parentId}-01`
  }

  return {
    name: 'mixlirous:traceparent-echo',
    configureServer(server) {
      server.middlewares.use((req, res, next) => {
        const incoming =
          (req.headers[HEADER] as string | undefined)?.trim() ?? ''
        const value = incoming && VALID_RE.test(incoming) ? incoming : generate()
        // Não sobrescreve se um middleware interno já setou (caso raro).
        if (!res.hasHeader(HEADER)) {
          res.setHeader(HEADER, value)
        }
        next()
      })
    },
  }
}

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss(), traceparentPlugin()],
  server: {
    port: 5173,
    open: false, // desligado para runs de QA (HEAD sem navegador)
    proxy: {
      '/api': 'http://localhost:8080',
    },
  },
})
