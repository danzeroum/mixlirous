# Handoff — Mixlirous

> **Arquivado.** Este documento descreve o handoff do kit original para este
> repositório — já aconteceu. `docs/`, `backlog/`, `README.md` e
> `CONTRIBUTING.md` citados abaixo já estão no lugar; a Sprint 0 (workspace
> Rust compilando/testando/passando clippy, `ui/` buildando) está concluída.
> Para o estado atual do projeto, ver [`README.md`](README.md). Mantido aqui
> por valor histórico (a lista de erros originais do kit, as decisões de
> ADR-0009/0010), não como instrução pendente — o restante deste arquivo
> descreve um estado passado do repositório.

> Pacote de entrega da arquitetura para **desenvolvimento** e **design**.
> Repositório destino: <https://github.com/danzeroum/mixlirous>

Este pacote transforma ~40 páginas de discussão arquitetural em documentos
acionáveis. Nada aqui é "inspiração" — cada documento tem um dono, um formato
de entrega e critérios de aceite.

---

## 1. O que é o Mixlirous

Motor de remixagem algorítmica de áudio guiado por IA. O usuário descreve a
intenção ("versão de 30s para Reels, agressiva, focada nas viradas de bateria")
e o sistema traduz isso em parâmetros determinísticos de DSP, corta a faixa em
blocos alinhados às batidas, remonta e masteriza.

**Princípio inegociável:** a IA nunca toca no buffer de áudio. Ela apenas
preenche um contrato JSON tipado, que o motor em Rust valida antes de executar.

| Nome | Uso |
| --- | --- |
| **Mixlirous** | Nome do produto e do repositório |
| **Remix AI** | Codinome interno do motor (aparece no kit e nos docs antigos) |
| `audio_core` / `audio_agent` / `audio_api` | Crates Rust do backend |

---

## 2. Como ler este pacote

### Se você é o **desenvolvedor backend**

Ordem obrigatória:

1. `docs/14-AUDITORIA-KIT.md` — **comece aqui.** O kit atual não compila. Lista
   exata dos erros e das correções da Sprint 0.
2. `docs/02-ARQUITETURA.md` — as camadas e o que pode depender de quê.
3. `docs/03-CONTRATOS-API.md` — contrato REST + catálogo de eventos SSE. Esse
   documento é a fonte da verdade compartilhada com o frontend. Alterações aqui
   exigem PR e aviso.
4. `docs/04-DOMINIO-DSP.md` — algoritmos, fórmulas, invariantes testáveis.
5. `docs/05-AGENTE-IA-HITL.md` — loop ReAct, tabela canônica de limites de
   parâmetros, ciclo de vida das propostas.
6. `docs/06-PERSISTENCIA-RESILIENCIA.md` — schema SQL, fila, recovery loop.
7. `docs/10-TESTES-QUALIDADE.md` — o que precisa de teste e com que rigor.

### Se você é o **desenvolvedor frontend**

1. `docs/03-CONTRATOS-API.md` — endpoints, envelope de parâmetros, eventos SSE.
2. `docs/12-DESIGN-BRIEF.md` — inventário de telas e matriz de estados.
3. `docs/05-AGENTE-IA-HITL.md` seção 4 — máquina de estados da proposta (é o
   fluxo mais delicado da UI).
4. `docs/14-AUDITORIA-KIT.md` seção 3 — correções do `ui/`.

### Se você é o **designer**

1. `docs/12-DESIGN-BRIEF.md` — briefing completo: personas, telas, anatomia dos
   nós, matriz de estados, tokens, acessibilidade, entregáveis esperados.
2. `docs/01-GLOSSARIO.md` — a linguagem do domínio. Usar exatamente esses termos
   na UI evita divergência entre Figma, código e conversa.
3. `docs/00-VISAO-ESCOPO.md` — para quem é e o que está fora de escopo.
4. `docs/03-CONTRATOS-API.md` seção 5 — os eventos que a UI recebe em tempo real
   definem o que pode ser animado e o que não existe.

### Se você é o **responsável pelo projeto**

1. `docs/13-ROADMAP-SPRINTS.md` — Sprint 0 + 4 sprints, com dependências,
   critérios de aceite e checklist de setup do GitHub.
2. `backlog/issues.csv` + `backlog/import-issues.sh` — importa o backlog inicial
   como issues no repositório.
3. `docs/adr/README.md` — decisões técnicas registradas, com alternativas
   descartadas. Serve para não reabrir discussões já fechadas.

---

## 3. Estrutura do pacote

```
mixlirous-handoff/
├── HANDOFF.md                      ← este arquivo
├── README.md                       ← README para a raiz do repositório
├── CONTRIBUTING.md                 ← fluxo de branch, commits, PR
├── docs/
│   ├── 00-VISAO-ESCOPO.md          Produto, personas, escopo do MVP, não-metas
│   ├── 01-GLOSSARIO.md             Linguagem ubíqua (dev + design + produto)
│   ├── 02-ARQUITETURA.md           Camadas, dependências, fluxo de execução
│   ├── 03-CONTRATOS-API.md         ★ REST + SSE + erros + máquina de estados
│   ├── 04-DOMINIO-DSP.md           Entidades, algoritmos, invariantes
│   ├── 05-AGENTE-IA-HITL.md        ★ ReAct, limites de parâmetros, propostas
│   ├── 06-PERSISTENCIA-RESILIENCIA.md  Schema, fila, recovery
│   ├── 07-OBSERVABILIDADE.md       OTel, métricas, alertas
│   ├── 08-SEGURANCA-MULTITENANCY.md   JWT, RLS, sandbox, prompt injection
│   ├── 09-MLOPS-GOLDEN-MASTER.md   Prompts como código, regressão acústica
│   ├── 10-TESTES-QUALIDADE.md      Pirâmide de testes, DoD
│   ├── 11-INFRA-DEPLOY.md          Docker, botão de escala, caminho SaaS
│   ├── 12-DESIGN-BRIEF.md          ★ Briefing de design
│   ├── 13-ROADMAP-SPRINTS.md       ★ Plano de execução
│   ├── 14-AUDITORIA-KIT.md         ★ Estado real do kit e correções
│   └── adr/README.md               10 decisões arquiteturais registradas
└── backlog/
    ├── issues.csv                  Backlog inicial importável
    └── import-issues.sh            Script de importação via gh CLI
```

★ = leitura obrigatória antes da primeira linha de código.

---

## 4. Como levar isso para o repositório

```bash
git clone https://github.com/danzeroum/mixlirous.git
cd mixlirous

# 1. Documentação
cp -r /caminho/mixlirous-handoff/docs .
cp /caminho/mixlirous-handoff/README.md .
cp /caminho/mixlirous-handoff/CONTRIBUTING.md .
mkdir -p .github && cp -r /caminho/mixlirous-handoff/backlog .

# 2. Esqueleto de código (kit já existente)
unzip remix-ai-kit.zip -d .

git checkout -b chore/handoff-inicial
git add -A
git commit -m "docs: pacote de arquitetura e handoff inicial"
git push -u origin chore/handoff-inicial
```

Depois, no repositório:

```bash
bash backlog/import-issues.sh   # cria labels, milestones e issues
```

---

## 5. Três avisos honestos

**1. O kit não compila.** São ~15 erros reais (imports faltando, variante de
enum inexistente, macro `s!` não importada, string não fechada, versões de
crates incompatíveis). Isso é normal para um scaffold gerado, mas precisa estar
claro: a Sprint 0 existe só para isso. Detalhes em `docs/14-AUDITORIA-KIT.md`.

**2. O escopo é grande para 4 semanas.** O plano original de 4 sprints entrega a
V1 completa (DAG visual + ReAct + SSE + persistência dual + MLOps +
observabilidade). Isso é realista para 2 devs experientes em tempo integral, ou
~8 semanas para 1 dev. `docs/13-ROADMAP-SPRINTS.md` marca o que é
**núcleo** (não corta) e o que é **adiável** se o prazo apertar.

**3. Duas decisões continuam abertas** e precisam de dono antes da Sprint 2:

| Decisão | Impacto se não resolver |
| --- | --- |
| Provedor LLM padrão do MVP (OpenAI vs Ollama local) | Define se o produto funciona offline no laptop do músico |
| Separação de stems (modelo/binário externo vs remover do MVP) | É a ferramenta mais pesada do registry e não tem implementação em Rust puro |

Estão registradas como `ADR-0009` e `ADR-0010`, com status *proposto*.
