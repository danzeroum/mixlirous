# Proposta de issue: QA-0008 — HTTPS em loopback

**Alvo:** `danzeroum/qa-suite` (repo da régua)
**Status:** texto pronto, pendente de issue ser aberta (token PAT do agente
QA não tem escopo de escrita no `qa-suite` — apenas no `mixlirous`).
**Ação para o humano:** copiar o corpo abaixo para uma issue nova em
https://github.com/danzeroum/qa-suite/issues/new

---

## Title

test_https_e_usado reprova loopback autorizado (alvo local http://localhost)

## Body

### Contexto

Durante o ciclo WebQA contra o Mixlirous (branch `qa/ciclo-inicial`,
ciclo 1+2), o check `checks/backend/test_http_basics.py::test_https_e_usado`
falhou consistentemente com a mensagem:

```
AssertionError: Tráfego final não está sobre HTTPS — risco de segurança e LGPD (dados pessoais em trânsito sem criptografia).
assert 'http' == 'https'
```

### Sintoma

```python
def test_https_e_usado(home_response):
    assert home_response.url.scheme == "https", (
        "Tráfego final não está sobre HTTPS — risco de segurança e LGPD "
        "(dados pessoais em trânsito sem criptografia)."
    )
```

O check faz `assert home_response.url.scheme == "https"` — sem discriminar
entre ambiente de produção (que **deve** ter HTTPS) e ambiente de
desenvolvimento local autorizado (loopback, onde HTTPS é desnecessário e
tecnicamente complicado por causa de certificados auto-assinados).

### Por que é régua, não defeito do alvo

O playbook de QA do Mixlirous (e da maioria dos alvos web) executa contra
`http://localhost:PORTA` por design:

- Dev server (Vite preview, axum) roda em HTTP loopback.
- HTTPS em loopback exigiria certificado auto-assinado (HSTS não enforced),
  aumentando fricção sem ganho real de segurança — `localhost` não é
  exposto externamente.
- Em produção, HTTPS é garantido pelo nginx/LB na borda, não pela aplicação.

O check `test_http_redireciona_para_https` (logo abaixo) já tem a cláusula
correta:

```python
if not settings.target_url.startswith("https://"):
    pytest.skip("Alvo já configurado sem HTTPS.")
```

Mas `test_https_e_usado` não tem essa discriminação — sempre reprova, mesmo
em loopback.

### Proposta de correção

```python
def test_https_e_usado(home_response, settings):
    # Em alvo explicitamente http:// (loopback/dev autorizado), skip em vez de reprovar.
    # Em alvo https://, o check é enforcing — reprovar se não estiver sobre HTTPS.
    if settings.target_url.startswith("http://") and (
        "localhost" in settings.target_url or "127.0.0.1" in settings.target_url
    ):
        pytest.skip(
            "Alvo local (loopback) — HTTPS é garantido pelo LB/nginx em produção, "
            "não pela aplicação. Para validar HTTPS, rode a suíte contra a URL "
            "de produção."
        )
    assert home_response.url.scheme == "https", (
        "Tráfego final não está sobre HTTPS — risco de segurança e LGPD "
        "(dados pessoais em trânsito sem criptografia)."
    )
```

### Impacto

Sem essa correção, qualquer alvo local marcado para `http://localhost:N`
sempre terá 1 failure em `test_http_basics.py`, mascarando regressões reais
que poderiam aparecer nesse mesmo arquivo.

### Reprodução

```bash
WEBQA_TARGET_URL=http://localhost:5174 pytest checks/backend/test_http_basics.py::test_https_e_usado
```

Registrado em `danzeroum/mixlirous` como `QA-0008` (tipo `regua`) em
`docs/qa/ACHADOS.md`.
