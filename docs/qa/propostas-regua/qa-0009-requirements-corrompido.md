# Proposta de issue: QA-0009 — requirements.txt corrompido

**Alvo:** `danzeroum/qa-suite` (repo da régua)
**Status:** texto pronto, pendente de issue ser aberta (token PAT do agente
QA não tem escopo de escrita no `qa-suite`).
**Ação para o humano:** copiar o corpo abaixo para uma issue nova em
https://github.com/danzeroum/qa-suite/issues/new

---

## Title

requirements.txt com linha corrompida `httpxttp2]>=0.27` quebra `pip install`

## Body

### Contexto

Durante o ciclo WebQA contra o Mixlirous, ao instalar a suíte:

```bash
cd webqa-suite && pip install -r requirements.txt
```

O `pip` falha com erro de parsing na linha que deveria ser
`httpx[http2]>=0.27` mas está escrita como `httpxttp2]>=0.27` (faltam
os colchetes e o nome do extra `http2`).

### Sintoma

`webqa-suite/requirements.txt` (estado atual):

```
pytest>=8.0
pytest-bdd>=7.0
httpxttp2]>=0.27          ← linha corrompida
beautifulsoup4>=4.12
lxml>=5.0
PyYAML>=6.0
playwright==1.56.0
locust>=2.24
ruff>=0.4
pytest-cov>=5.0
```

Linha esperada: `httpx[http2]>=0.27`

### Workaround local

Instalar `httpx[http2]` manualmente antes de rodar o requirements:

```bash
pip install "httpx[http2]>=0.27"
pip install -r requirements.txt --no-deps  # ignora a linha corrompida
```

Ou simplesmente pular `pip install -r requirements.txt` e instalar
manualmente cada dependência.

### Impacto

- Primeira instalação da suíte sempre falha.
- Documentação/onboarding de novos QA engineers quebra.
- CI da própria suíte (se existir) quebra.

### Proposta de correção

Trocar a linha:

```diff
- httpxttp2]>=0.27
+ httpx[http2]>=0.27
```

### Reprodução

```bash
git clone https://github.com/danzeroum/qa-suite.git
cd qa-suite/webqa-suite
python -m venv .venv && source .venv/bin/activate
pip install -r requirements.txt
# → ERROR: Invalid requirement: 'httpxttp2]>=0.27'
```

Registrado em `danzeroum/mixlirous` como `QA-0009` (tipo `regua`) em
`docs/qa/ACHADOS.md`.

### Verificação

Provavelmente um typo em algum commit anterior. Sugiro também adicionar
um CI mínimo que faça `pip install -r requirements.txt` em Python limpo
para evitar regressões futuras.
