# ACHADOS — tabela-mestre do ciclo WebQA

Regras (ver `README.md`): ID estável `QA-NNNN` (nunca reutilizado);
`corrigido` exige commit + teste de regressão + laudo antes/depois;
`aceito` exige justificativa em `DECISOES.md`; `regua` = defeito da
própria suíte (proposta de correção em texto, issue no qa-suite —
**nunca** commit na suíte).

| ID | Severidade | Perfil/Origem | Alvo | Sintoma | Estado | Fix (commit) | Teste de regressão | Laudo a/d |
|---|---|---|---|---|---|---|---|---|
| QA-0001 | alta | backend/e2e + curl | `crates/audio_api/src/routes/mod.rs` (api_router) | `PUT /api/v1/uploads/{key}` com >2 MB retorna **413** (default do axum); nginx prod aceita 100 MB (`client_max_body_size`) — **upload real quebrado em produção**; via proxy vite vira 502 | em-correcao | — | — | laudos/backend-antes.txt + curl |
| QA-0002 | alta | backend (test_security_headers) | origem servida (vite dev) + docs/18 (nginx prod) | Sem `X-Content-Type-Options: nosniff`, sem `X-Frame-Options`/`frame-ancestors`, sem CSP. HSTS existe só no nginx prod (docs/18), ausente na origem dev | aberto | — | — | laudos/backend-antes.txt |
| QA-0003 | média | backend (test_resposta_comprimida) | origem servida (vite dev) + docs/18 | HTML servido sem compressão gzip (dev); vhost nginx de produção também não declara gzip | aberto | — | — | laudos/backend-antes.txt |
| QA-0004 | média | backend (test_404_tratado) | `ui/vite.config.ts` (appType spa fallback) | Rota inexistente devolve **200** com o shell da SPA (fallback) — 404 não é tratado; **e pior**: `/healthz` na origem dev devolve 200 fake (HTML) em vez do health real | aberto | — | — | laudos/backend-antes.txt |
| QA-0005 | média | backend (test_correlacao xfail) + docs/03 §1 | `crates/audio_api` (middleware) | Contrato docs/03: "cliente envia `traceparent` (W3C); servidor devolve no response" — **não implementado** (extractor existe, eco não). Sem header de correlação em ambas as origens | aberto | — | — | curl + laudo |
| QA-0006 | média | backend (test_erro_estruturado, skip) + docs/03 §1 | `crates/audio_api` (fallback de erro) | Contrato docs/03: erros `application/problem+json` (RFC 7807) — 404 de /api/* devolve corpo **vazio**; handlers devolvem texto puro | aberto | — | — | curl + laudo |
| QA-0007 | baixa | backend (test_endpoint_de_saude) | `ui/vite.config.ts` (proxy) | `/healthz` não é proxyado na origem dev — check de saúde passa por engano via fallback SPA (QA-0004); com 404 corrigido, health real precisa ser exposto (nginx prod já expõe) | aberto | — | — | curl + laudo |
| QA-0008 | régua | backend (test_https_e_usado) | suíte: checks/backend/test_http_basics.py:17 | Check exige HTTPS incondicional; alvo local autorizado é `http://localhost` (loopback). Falha sem ser defeito do alvo | regua | — (não se corrige no mixlirous) | — | laudos/backend-antes.txt |
| QA-0009 | régua | instalação da suíte | suíte: webqa-suite/requirements.txt | Linha corrompida `httpxttp2]>=0.27` (esperado `httpx[http2]>=0.27`) quebra `pip install -r requirements.txt` | regua | — (contorno local documentado) | — | logs de instalação |
