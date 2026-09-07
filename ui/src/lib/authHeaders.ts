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
