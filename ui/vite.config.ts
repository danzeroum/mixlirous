import { defineConfig, type Plugin } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import { randomBytes } from 'node:crypto'
import { gzipSync } from 'node:zlib'
import type { ServerResponse, IncomingMessage } from 'node:http'

// ===========================================================================
// QA-0005 — plugin Vite para eco de `traceparent` (W3C Trace Context) em
// todas as respostas servidas pelo dev server (home, assets, etc.).
// O contrato docs/03 §1 exige: "cliente envia traceparent (W3C); servidor
// devolve no response". A API ecoa via middleware axum; este plugin
// estende para a UI servida pelo Vite.
// ===========================================================================
const TRACEPARENT_HEADER = 'traceparent'
const TRACEPARENT_VALID_RE = /^00-[0-9a-f]{32}-[0-9a-f]{16}-[0-9a-f]{2}$/

function generateTraceparent(): string {
  const traceId = randomBytes(16).toString('hex')
  const parentId = randomBytes(8).toString('hex')
  return `00-${traceId}-${parentId}-01`
}

function traceparentPlugin(): Plugin {
  return {
    name: 'mixlirous:traceparent-echo',
    configureServer(server) {
      server.middlewares.use((req, res, next) => {
        const incoming =
          (req.headers[TRACEPARENT_HEADER] as string | undefined)?.trim() ?? ''
        const value =
          incoming && TRACEPARENT_VALID_RE.test(incoming)
            ? incoming
            : generateTraceparent()
        if (!res.hasHeader(TRACEPARENT_HEADER)) {
          res.setHeader(TRACEPARENT_HEADER, value)
        }
        next()
      })
    },
  }
}

// ===========================================================================
// QA-0002 — plugin Vite para headers de segurança. Em produção (nginx)
// estes headers vivem no vhost (docs/18 tem HSTS; X-Content-Type-Options,
// X-Frame-Options e CSP devem ser adicionados ao nginx também — pendência
// registrada no DIARIO). No dev server, este plugin garante que a suíte
// WebQA encontre os headers na origem servida.
//
// Headers aplicados:
// - Strict-Transport-Security: max-age=63072000 (igual docs/18 §5.3).
//   Em HTTP (dev) browsers ignoram, mas o header está presente para a
//   suíte verificar; em HTTPS (prod) ele é enforcing.
// - X-Content-Type-Options: nosniff (OWASP — previne MIME sniffing).
// - X-Frame-Options: DENY (mitigação de clickjacking; mais simples que
//   CSP frame-ancestors para o dev server).
// - Content-Security-Policy: default-src 'self' (mitigação de XSS;
//   permite inline styles do Vite HMR via 'unsafe-inline' em style-src,
//   e connect-src 'self' para /api/*).
// - Referrer-Policy: strict-origin-when-cross-origin (default moderno).
// - X-Powered-By: removido (não expor versão — test_nao_expoe_versao).
// ===========================================================================
function securityHeadersPlugin(): Plugin {
  return {
    name: 'mixlirous:security-headers',
    configureServer(server) {
      server.middlewares.use((req, res, next) => {
        // 'always' semantics: add_header mesmo em respostas de erro.
        // Vite usa Node http.ServerResponse, que não tem 'always'; a
        // semântica equivalente é setar antes de next() para todas as
        // respostas, incluindo as que downstream vão retornar 4xx/5xx.
        if (!res.hasHeader('Strict-Transport-Security')) {
          res.setHeader('Strict-Transport-Security', 'max-age=63072000')
        }
        if (!res.hasHeader('X-Content-Type-Options')) {
          res.setHeader('X-Content-Type-Options', 'nosniff')
        }
        if (!res.hasHeader('X-Frame-Options')) {
          res.setHeader('X-Frame-Options', 'DENY')
        }
        if (!res.hasHeader('Content-Security-Policy')) {
          // 'unsafe-inline' em style-src é necessário para o Vite HMR
          // injetar CSS inline durante o dev. Em prod, o build gera
          // arquivos CSS externos e a regra pode ser mais restritiva.
          res.setHeader(
            'Content-Security-Policy',
            "default-src 'self'; " +
              "style-src 'self' 'unsafe-inline'; " +
              "script-src 'self'; " +
              "img-src 'self' data: blob:; " +
              "font-src 'self' data:; " +
              "connect-src 'self' ws: wss:; " +
              "media-src 'self' blob:; " +
              "frame-ancestors 'none'",
          )
        }
        if (!res.hasHeader('Referrer-Policy')) {
          res.setHeader('Referrer-Policy', 'strict-origin-when-cross-origin')
        }
        // Remove headers que vazam versão de servidor (test_nao_expoe_versao).
        res.removeHeader('X-Powered-By')
        res.removeHeader('Server')
        next()
      })
    },
  }
}

// ===========================================================================
// QA-0003 — plugin Vite para compressão gzip em dev server.
// Vite usa `sirv` que pula compressão para localhost/127.0.0.1 (raciocínio:
// localhost não tem gargalo de banda). A suíte WebQA testa de fora do
// processo (httpx), então a compressão é mensurável — e o teste falha
// porque o header Content-Encoding nunca é setado.
//
// Abordagem: monkey-patch `res.end` para capturar o body quando enviado
// como string/Buffer único (caso do index.html), comprimir se for textual
// e maior que o limiar, e setar Content-Encoding antes de delegar ao
// `end` original. Não intercepta `res.write` (streaming) para não quebrar
// HMR/WebSocket — apenas o caso comum de resposta completa.
// ===========================================================================
function gzipPlugin(): Plugin {
  const COMPRESSIBLE_TYPES = /^(text\/|application\/(?:json|javascript|xml)|image\/svg\+xml)/
  const MIN_BYTES = 100
  const ACCEPT_GZIP_RE = /\bgzip\b/i

  return {
    name: 'mixlirous:gzip-dev',
    configureServer(server) {
      server.middlewares.use((req: IncomingMessage, res: ServerResponse, next) => {
        const acceptEncoding = req.headers['accept-encoding'] ?? ''
        if (!ACCEPT_GZIP_RE.test(acceptEncoding)) {
          next()
          return
        }

        const origEnd = res.end.bind(res) as ServerResponse['end']
        let patched = false

        // Tipos da sobrecarga de ServerResponse.end:
        //   end(cb?: (err?: Error) => void): this
        //   end(chunk: Uint8Array | string, cb?: (err?: Error) => void): this
        //   end(chunk: Uint8Array | string, encoding: BufferEncoding, cb?: (err?: Error) => void): this
        // Como TypeScript não consegue desambiguar via union em runtime, fazemos
        // a discriminação manual com `typeof` antes de chamar o original.
        type EndChunk = Uint8Array | string
        type EndCb = (err?: Error) => void

        const patchedEnd = function (
          this: ServerResponse,
          chunkOrCb?: EndChunk | EndCb,
          encodingOrCb?: BufferEncoding | EndCb,
          cb?: EndCb,
        ): ServerResponse {
          if (patched) {
            return origEnd(chunkOrCb as EndChunk | undefined, encodingOrCb as BufferEncoding | undefined, cb as EndCb | undefined)
          }
          patched = true

          // Normalize args: end(), end(cb), end(chunk), end(chunk, cb),
          // end(chunk, encoding), end(chunk, encoding, cb).
          let body: Buffer | null = null
          let enc: BufferEncoding | undefined
          let callback: EndCb | undefined

          if (chunkOrCb === undefined) {
            // end() — sem body, repassa direto.
            return origEnd()
          }
          if (typeof chunkOrCb === 'string') {
            body = Buffer.from(chunkOrCb, (encodingOrCb as BufferEncoding) ?? 'utf8')
            enc = typeof encodingOrCb === 'string' ? encodingOrCb : undefined
            callback = typeof encodingOrCb === 'function' ? encodingOrCb : (typeof cb === 'function' ? cb : undefined)
          } else if (Buffer.isBuffer(chunkOrCb) || chunkOrCb instanceof Uint8Array) {
            body = Buffer.isBuffer(chunkOrCb) ? chunkOrCb : Buffer.from(chunkOrCb)
            enc = typeof encodingOrCb === 'string' ? (encodingOrCb as BufferEncoding) : undefined
            callback = typeof encodingOrCb === 'function' ? encodingOrCb : (typeof cb === 'function' ? cb : undefined)
          } else if (typeof chunkOrCb === 'function') {
            // end(cb) — sem body, callback é o primeiro arg.
            callback = chunkOrCb
          }

          const contentType = (res.getHeader('Content-Type') as string) ?? ''
          const alreadyCompressed = res.hasHeader('Content-Encoding')

          if (
            body &&
            body.length >= MIN_BYTES &&
            COMPRESSIBLE_TYPES.test(contentType) &&
            !alreadyCompressed
          ) {
            const compressed = gzipSync(body)
            res.setHeader('Content-Encoding', 'gzip')
            res.setHeader('Content-Length', String(compressed.length))
            if (!res.hasHeader('Vary')) {
              res.setHeader('Vary', 'Accept-Encoding')
            }
            return origEnd(compressed, undefined, callback)
          }

          // Sem compressão: repassa argumentos originais.
          if (body) {
            return origEnd(body, enc, callback)
          }
          return origEnd(callback)
        }

        ;(res as { end: typeof patchedEnd }).end = patchedEnd

        next()
      })
    },
  }
}

// ===========================================================================
// QA-0004 + QA-0007 — Vite por padrão usa appType: 'spa' que serve
// index.html para qualquer rota não mapeada (200, fallback). Isso:
// 1. Faz /webqa-rota-inexistente-9f3a devolver 200 (QA-0004 quebra).
// 2. Faz /healthz na origem dev devolver 200 com HTML em vez do health
//    real da API (QA-0007 — check de saúde "passa" pelo motivo errado).
//
// Mitigação: proxyar /healthz, /readyz, /metrics para a API + mudar para
// appType 'mpa' (sem fallback SPA). Como a UI NÃO usa react-router (ver
// DIARIO.md S1: "5 views por estado, não por URL"), só a raiz / precisa
// servir index.html — o que o Vite faz naturalmente em modo MPA.
// ===========================================================================

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [
    react(),
    tailwindcss(),
    traceparentPlugin(),
    securityHeadersPlugin(),
    gzipPlugin(),
  ],
  // QA-0004: 'mpa' desliga o fallback SPA — rotas não mapeadas devolvem
  // 404 em vez de 200 com index.html. A UI não usa react-router (DIARIO
  // S1), então só a raiz / precisa servir index.html, e o Vite faz isso
  // naturalmente em modo MPA. Modo SPA só faria sentido se houvesse
  // rotas client-side como /projects, /library, etc.
  appType: 'mpa',
  server: {
    port: 5173,
    open: false, // desligado para runs de QA (HEAD sem navegador)
    proxy: {
      '/api': 'http://localhost:8080',
      // QA-0007: /healthz, /readyz, /metrics não estavam sendo proxyados
      // — o check de saúde "passava" via fallback SPA (200 HTML). Agora
      // proxyados para a API, devolvendo o health real (JSON).
      '/healthz': 'http://localhost:8080',
      '/readyz': 'http://localhost:8080',
      '/metrics': 'http://localhost:8080',
    },
  },
})
