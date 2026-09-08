# BACKLOG — itens abertos do ciclo QA

Itens que **não** são fix neste ciclo, mas não podem se perder:

## Quarentena / flakes

- _(vazio)_

## Propostas `regua` (a abrir como issue no qa-suite)

- **R-01 (provisório — em verificação)**: `webqa-suite/requirements.txt`
  linha `httpxttp2]>=0.27` corrompida (esperado `httpx[http2]>=0.27`).
  `pip install -r requirements.txt` quebra. Contorno local: instalar
  `httpx[http2]` manualmente. **A abrir issue** com este texto.

## Pendências técnicas

- **P-01**: nginx de produção (docs/18 §5.3) só declara HSTS — faltam
  `X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY` (ou CSP
  `frame-ancestors 'none'`), `Content-Security-Policy`, e compressão
  `gzip`. Em dev o Vite cobre tudo via plugins (commit deste ciclo);
  em produção os headers precisam ser adicionados ao vhost do nginx.
  Não bloqueia o ciclo QA (suíte roda contra dev), mas é pendência
  para o deploy real.
- **P-02**: `crates/audio_api/src/routes/uploads.rs` — `upload_put`
  recebe `Bytes` (corpo em memória antes do storage). Com o teto de
  100 MB (QA-0001), o pico de RAM por upload é 100 MB. Streaming para
  disco é melhoria recomendada para ambientes com RAM limitada.
