/**
 * Token de sessão do modo local (docs/03 §1) — gravado no localStorage
 * por `ensureSseSession()` (useSSE). Helper único para que toda chamada
 * REST (useApi e UploadPanel) mande o mesmo Bearer, sem espalhar
 * `localStorage` pelo código.
 *
 * No modo SaaS, o login futuro grava o MESMO storage key — nenhum caller
 * muda.
 */
const TOKEN_KEY = 'mixlirous_token'

export function getStoredToken(): string | null {
  try {
    return localStorage.getItem(TOKEN_KEY)
  } catch {
    return null
  }
}

export function setStoredToken(token: string): void {
  try {
    localStorage.setItem(TOKEN_KEY, token)
  } catch {
    // privacy mode — o cookie de sessão (issue #33) segue funcionando
  }
}

/** Headers de autenticação para fetch com Bearer, quando houver token. */
export function authHeaders(): Record<string, string> {
  const token = getStoredToken()
  return token ? { Authorization: `Bearer ${token}` } : {}
}

/**
 * Sessão local como SINGLETON da página (fix de integração dos Lotes 2+3).
 *
 * `GET /auth/local-session` cria um tenant NOVO a cada chamada (single-user
 * local). Antes desta função, dois callers independentes a acionavam — o
 * bootstrap do App (Lote 3) e o `ensureSseSession` (Lote 2) — e, com o
 * StrictMode do React reexecutando efeitos duas vezes em dev, o upload e o
 * job podiam ficar no tenant de um token enquanto o cookie de SSE (usado no
 * download do artefato) ficava no tenant de outro → 404 `not_found` no
 * artifact. Se o Bearer e o cookie não falarem DO MESMO tenant, o fluxo
 * quebra em silêncio.
 *
 * Contrato desta função:
 * 1. Já existe token no localStorage → reutiliza (mesmo tenant no Bearer,
 *    no cookie de SSE e nos reloads — o cookie de local-session dura 30 dias).
 * 2. Não existe → faz o fetch UMA vez (promessa de módulo deduplica chamadas
 *    concorrentes do StrictMode/remounts), grava o token e o devolve.
 *
 * Token expirado (após 30 dias) ainda não tem recuperação automática —
 * pendência registrada no adendo Pareto §1.
 */
let sessaoLocalPromise: Promise<string | null> | null = null

export function ensureLocalSession(): Promise<string | null> {
  const existente = getStoredToken()
  if (existente) return Promise.resolve(existente)
  if (!sessaoLocalPromise) {
    sessaoLocalPromise = (async () => {
      try {
        const res = await fetch('/api/v1/auth/local-session', {
          credentials: 'same-origin',
        })
        if (res.ok) {
          const body = (await res.json()) as { token?: string }
          if (body?.token) {
            setStoredToken(body.token)
            return body.token
          }
        }
      } catch {
        // Sem backend/sem rota local (modo SaaS futuro) — segue sem token.
      }
      return null
    })()
  }
  return sessaoLocalPromise
}
