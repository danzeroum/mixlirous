# docs/qa/ — Registro contínuo de QA (ciclo WebQA)

Este diretório é o **registro vivo** do ciclo de qualidade executado com a
WebQA Suite (`danzeroum/qa-suite`, pasta `webqa-suite/`) contra o Mixlirous,
em ambiente **estritamente local (loopback)**.

## Contrato deste registro

- **Código é a verdade.** Documentação pode estar desatualizada; antes de
  qualquer afirmação, o código (ou a resposta HTTP real) é verificado.
- **A suíte é a régua e não se mexe na régua.** Bugs/positivos falsos da
  suíte viram achados tipo `regua` com proposta de correção **em texto**
  (issue no repo `qa-suite`), nunca commits no repo da suíte.
- **Toda descoberta entra em `ACHADOS.md`** com ID sequencial estável
  (`QA-NNNN`, nunca reutilizado). Estados possíveis:
  `aberto → em-correcao → corrigido | regredido | aceito | regua`.
  - `corrigido` exige: commit de fix, teste de regressão e laudo
    antes/depois (`laudos/`).
  - `aceito` exige justificativa técnica registrada em `DECISOES.md`.
- **Commits atômicos e rastreáveis**: `fix(qa): <resumo> [QA-NNNN]`,
  um achado por commit.
- **Segredos nunca entram aqui.** Tokens, senhas e `.env` não são
  versionados nem citados com valor.

## Estrutura

| Arquivo | Papel |
|---|---|
| `README.md` | Este contrato (você está aqui) |
| `DIARIO.md` | Log por sessão: o que rodou, o que aconteceu, incidentes |
| `ACHADOS.md` | Tabela-mestre dos achados com estado e evidências |
| `BACKLOG.md` | Itens abertos, quarentena de flakes, propostas `regua` |
| `DECISOES.md` | Decisões técnicas e itens `aguardando-humano` |
| `mapa-rotas.yaml` | Inventário de rotas da API (derivado do código) |
| `mapa-navegacao.yaml` | Mapa de navegação da UI (views + fluxos) |
| `laudos/` | Evidências: saídas da suíte antes/depois, reprodutíveis |
| `RELATORIO-FINAL.md` | Fechamento do ciclo (escrito ao final) |

## Ambientes

- **Alvo**: `http://localhost:5173` (dev server Vite da UI, com proxy
  `/api` → `http://localhost:8080`, mesma topologia do e2e do projeto —
  `ui/playwright.config.ts`). API sobe com `CONFIG_ENV=local`
  (sqlite + storage local + MockLlm, sem Postgres/MinIO/docker).
- **Carga (load)**: permanece **não autorizada** neste ciclo
  (`WEBQA_LOAD_AUTHORIZED` ausente). Alvo 100% local, mas o ciclo inicial
  optou por não autorizar rajadas além do perfis funcionais.
- `MIXLIROUS_DEV_SLICE` **não é definido** — rotas de diagnóstico
  `/api/v1/dev/slice*` permanecem desligadas (elididas do mapa ativo).
