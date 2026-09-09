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

- **P-01 (RESOLVIDO em docs/18 §5.4)**: nginx de produção agora tem
  bloco com todos os headers de segurança (X-Content-Type-Options,
  X-Frame-Options, CSP sem hash axe, Permissions-Policy, Referrer-Policy),
  compressão gzip, e `server_tokens off`. A CSP de produção **não**
  inclui o hash do axe-core que existe em dev (vite.config.ts) apenas
  para desbloquear o teste de a11y — em produção a suíte não roda contra
  a URL pública, e a exceção de ferramenta de teste não deve virar
  regra de produção.
- **P-02**: `crates/audio_api/src/routes/uploads.rs` — `upload_put`
  recebe `Bytes` (corpo em memória antes do storage). Com o teto de
  100 MB (QA-0001), o pico de RAM por upload é 100 MB. Streaming para
  disco é melhoria recomendada para ambientes com RAM limitada.

## Issues `regua` pendentes de abertura no `qa-suite`

- **R-01**: propostas de issue salvas em `docs/qa/propostas-regua/`:
  - `qa-0008-https-loopback.md` — `test_https_e_usado` reprova
    loopback autorizado (alvo local `http://localhost`). Texto pronto
    para colar em https://github.com/danzeroum/qa-suite/issues/new.
  - `qa-0009-requirements-corrompido.md` — `requirements.txt` da
    suíte com linha `httpxttp2]>=0.27` (faltam `[` e `http2`).
    Texto pronto para colar.
  - **Motivo de não ter sido aberto**: o token PAT do agente QA tem
    escopo de escrita apenas no repo `mixlirous` — não consegue criar
    issues no `qa-suite`. Ação manual do humano necessária.
