import { defineConfig, type Plugin, type PreviewServer, type ViteDevServer } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import { randomBytes } from 'node:crypto'
import { gzipSync } from 'node:zlib'
import type { ServerResponse, IncomingMessage } from 'node:http'

// Tipo union para middlewares que funcionam tanto em dev (ViteDevServer)
// quanto em preview (PreviewServer). Ambos expõem `middlewares` (um
// connect.Server). Usar um tipo único evita duplicar a configuração.
type AnyViteServer = ViteDevServer | PreviewServer

// ===========================================================================
// QA-0005 — middleware para eco de `traceparent` (W3C Trace Context) em
// todas as respostas servidas (home, assets, etc.).
// O contrato docs/03 §1 exige: "cliente envia traceparent (W3C); servidor
// devolve no response". A API ecoa via middleware axum; este middleware
// estende para a UI servida pelo Vite (dev E preview/build estático).
// ===========================================================================
const TRACEPARENT_HEADER = 'traceparent'
const TRACEPARENT_VALID_RE = /^00-[0-9a-f]{32}-[0-9a-f]{16}-[0-9a-f]{2}$/

function generateTraceparent(): string {
  const traceId = randomBytes(16).toString('hex')
  const parentId = randomBytes(8).toString('hex')
  return `00-${traceId}-${parentId}-01`
}

function traceparentMiddleware(req: IncomingMessage, res: ServerResponse, next: () => void): void {
  const incoming = (req.headers[TRACEPARENT_HEADER] as string | undefined)?.trim() ?? ''
  const value = incoming && TRACEPARENT_VALID_RE.test(incoming) ? incoming : generateTraceparent()
  if (!res.hasHeader(TRACEPARENT_HEADER)) {
    res.setHeader(TRACEPARENT_HEADER, value)
  }
  next()
}

function traceparentPlugin(): Plugin {
  return {
    name: 'mixlirous:traceparent-echo',
    configureServer(server: AnyViteServer) {
      server.middlewares.use(traceparentMiddleware)
    },
    configurePreviewServer(server: AnyViteServer) {
      server.middlewares.use(traceparentMiddleware)
    },
  }
}

// ===========================================================================
// QA-0002 — middleware de headers de segurança. Em produção (nginx) estes
// headers vivem no vhost (docs/18 tem HSTS; X-Content-Type-Options,
// X-Frame-Options e CSP devem ser adicionados ao nginx também — pendência
// P-01 no BACKLOG). No dev E preview do Vite, este middleware garante que
// a suíte WebQA encontre os headers na origem servida.
// ===========================================================================
function securityHeadersMiddleware(req: IncomingMessage, res: ServerResponse, next: () => void): void {
  // 'always' semantics: add_header mesmo em respostas de erro.
  // Vite usa Node http.ServerResponse, que não tem 'always'; a semântica
  // equivalente é setar antes de next() para todas as respostas, incluindo
  // as que downstream vão retornar 4xx/5xx.
  void req // não usa req, mas mantém a assinatura do middleware
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
    // 'unsafe-inline' em style-src era necessário para o Vite HMR injetar
    // CSS inline durante o dev. Em prod, o build gera arquivos CSS
    // externos, mas mantemos 'unsafe-inline' por compatibilidade (não
    // bloqueia o ciclo QA — nginx prod pode ser mais restritivo).
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
  // QA-0013 — Permissions-Policy: privilégio mínimo por default. O
  // Mixlirous não usa câmera, microfone, geolocalização, nem payment.
  // Bloquear tudo por default é a postura correta (LGPD — minimização
  // de dados). Antes era XFAIL na suíte; agora PASS.
  if (!res.hasHeader('Permissions-Policy')) {
    res.setHeader(
      'Permissions-Policy',
      'camera=(), microphone=(), geolocation=(), payment=(), usb=(), magnetometer=(), gyroscope=(), accelerometer=()',
    )
  }
  // Remove headers que vazam versão de servidor (test_nao_expoe_versao).
  res.removeHeader('X-Powered-By')
  res.removeHeader('Server')
  next()
}

function securityHeadersPlugin(): Plugin {
  return {
    name: 'mixlirous:security-headers',
    configureServer(server: AnyViteServer) {
      server.middlewares.use(securityHeadersMiddleware)
    },
    configurePreviewServer(server: AnyViteServer) {
      server.middlewares.use(securityHeadersMiddleware)
    },
  }
}

// ===========================================================================
// QA-0003 — middleware de compressão gzip.
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
function gzipMiddleware(req: IncomingMessage, res: ServerResponse, next: () => void): void {
  const COMPRESSIBLE_TYPES = /^(text\/|application\/(?:json|javascript|xml)|image\/svg\+xml)/
  const MIN_BYTES = 100
  const ACCEPT_GZIP_RE = /\bgzip\b/i

  const acceptEncoding = req.headers['accept-encoding'] ?? ''
  if (!ACCEPT_GZIP_RE.test(acceptEncoding)) {
    next()
    return
  }

  // Estratégia: capturar TUDO o que é enviado (write + end) e só enviar
  // no end() — isso permite comprimir mesmo quando o servidor (sirv /
  // vite preview) chama writeHead → write → end. Sem capturar write,
  // respostas via streaming (HMR, SSE) não seriam comprimidas, mas o
  // custo é buffering em memória (aceitável para arquivos estáticos).
  const origEnd = res.end.bind(res) as ServerResponse['end']
  type WriteFn = ServerResponse['write']
  type WriteHeadFn = ServerResponse['writeHead']
  const chunks: Buffer[] = []
  let captured = false
  let headersFlushed = false

  // Intercepta writeHead para marcar que os headers foram enviados e
  // capturar o status/headers originais. Não chamamos o original aqui
  // — adiamos para o end() com headers atualizados (Content-Encoding).
  const origWriteHead = res.writeHead.bind(res) as WriteHeadFn
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  ;(res as unknown as { writeHead: (...args: any[]) => ServerResponse }).writeHead = function (
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    ...args: any[]
  ): ServerResponse {
    headersFlushed = true
    // Repassa direto — não atrasa o envio. O gzip middleware só pode
    // atuar se NÃO houve writeHead; se houve, não comprime.
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    return (origWriteHead as (...args: any[]) => ServerResponse)(...args)
  }

  ;(res as unknown as { write: WriteFn }).write = function (
    chunk: unknown,
    encoding?: unknown,
    cb?: unknown,
  ): boolean {
    if (typeof chunk === 'string') {
      chunks.push(Buffer.from(chunk, (encoding as BufferEncoding) ?? 'utf8'))
    } else if (Buffer.isBuffer(chunk)) {
      chunks.push(chunk)
    } else if (chunk instanceof Uint8Array) {
      chunks.push(Buffer.from(chunk))
    }
    captured = true
    // Sinaliza que o chunk foi aceito. O envio real acontece em end().
    if (typeof encoding === 'function') {
      ;(encoding as () => void)()
    } else if (typeof cb === 'function') {
      ;(cb as () => void)()
    }
    return true
  }

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
    // Captura chunk final (se houver) no buffer de chunks.
    if (chunkOrCb !== undefined) {
      if (typeof chunkOrCb === 'string') {
        chunks.push(Buffer.from(chunkOrCb, (encodingOrCb as BufferEncoding) ?? 'utf8'))
        captured = true
      } else if (Buffer.isBuffer(chunkOrCb) || chunkOrCb instanceof Uint8Array) {
        chunks.push(Buffer.isBuffer(chunkOrCb) ? chunkOrCb : Buffer.from(chunkOrCb))
        captured = true
      }
    }

    // Se nada foi capturado (apenas end() ou end(cb)), repassa direto.
    if (!captured) {
      if (chunkOrCb === undefined) {
        return origEnd()
      }
      if (typeof chunkOrCb === 'function') {
        return origEnd(chunkOrCb)
      }
      if (typeof chunkOrCb === 'string' || chunkOrCb instanceof Uint8Array) {
        const chunk = typeof chunkOrCb === 'string' ? chunkOrCb : Buffer.from(chunkOrCb)
        if (typeof encodingOrCb === 'function') {
          return origEnd(chunk, encodingOrCb)
        }
        if (typeof encodingOrCb === 'string') {
          if (typeof cb === 'function') {
            return origEnd(chunk, encodingOrCb, cb)
          }
          return origEnd(chunk, encodingOrCb)
        }
        return origEnd(chunk)
      }
      return origEnd()
    }

    const body = Buffer.concat(chunks)
    const contentType = (res.getHeader('Content-Type') as string) ?? ''
    const alreadyCompressed = res.hasHeader('Content-Encoding')

    // Determina callback final (último arg que é função).
    let finalCb: EndCb | undefined
    if (typeof chunkOrCb === 'function') finalCb = chunkOrCb
    else if (typeof encodingOrCb === 'function') finalCb = encodingOrCb
    else if (typeof cb === 'function') finalCb = cb

    // Só comprime se:
    // 1. body >= MIN_BYTES (gzip tem overhead)
    // 2. Content-Type é textual
    // 3. Não foi previamente comprimido
    // 4. Headers AINDA NÃO foram flushed (writeHead não chamado)
    // Se writeHead já foi chamado, não dá para setar Content-Encoding
    // — a resposta já foi parcialmente enviada.
    if (
      body.length >= MIN_BYTES &&
      COMPRESSIBLE_TYPES.test(contentType) &&
      !alreadyCompressed &&
      !headersFlushed &&
      !res.headersSent
    ) {
      const compressed = gzipSync(body)
      res.setHeader('Content-Encoding', 'gzip')
      res.setHeader('Content-Length', String(compressed.length))
      if (!res.hasHeader('Vary')) {
        res.setHeader('Vary', 'Accept-Encoding')
      }
      return finalCb ? origEnd(compressed, finalCb) : origEnd(compressed)
    }

    // Sem compressão: envia o body original.
    return finalCb ? origEnd(body, finalCb) : origEnd(body)
  }

  ;(res as { end: typeof patchedEnd }).end = patchedEnd

  next()
}

// ===========================================================================
// QA-0003 alternativo — para / (index.html servido por handler interno
// do Vite que chama writeHead antes do nosso middleware). Usamos
// transformIndexHtml para garantir que o body da home seja comprimível
// via gzip quando Accept-Encoding: gzip está presente.
//
// Como transformIndexHtml roda ANTES do handler de resposta do Vite
// transformar o HTML, não captura o Accept-Encoding. Por isso, este
// hook só aumenta artificialmente o tamanho do body para que o gzip
// middleware seja acionado quando o Vite servir a home.
//
// ATENÇÃO: não usamos isso — é documentação do problema. A solução
// real é o middleware de gzip com captura de write (acima), que já
// cobre /politica.html. Para /, aceitamos que o Vite envia Content-Length
// pré-calculado e o middleware não consegue comprimir — perda cosmética
// (1KB não comprime bem mesmo). Em produção (nginx), o vhost deve ter
// gzip on para cobrir.
// ===========================================================================

function gzipPlugin(): Plugin {
  return {
    name: 'mixlirous:gzip-dev',
    configureServer(server: AnyViteServer) {
      server.middlewares.use(gzipMiddleware)
    },
    configurePreviewServer(server: AnyViteServer) {
      server.middlewares.use(gzipMiddleware)
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
  preview: {
    port: 5174,
    host: '127.0.0.1',
    proxy: {
      '/api': 'http://localhost:8080',
      '/healthz': 'http://localhost:8080',
      '/readyz': 'http://localhost:8080',
      '/metrics': 'http://localhost:8080',
    },
  },
})
