repo: danzeroum/mixlirous
branch: main

## Last sync

date: 2026-07-24T19:26:00Z

### Updated in this project

- Protótipo completo do Mixlirous em `Mixlirous.dc.html` (13 telas navegáveis, tema escuro).
- Auditorias de arquitetura incorporadas: barra de espaço (tenant + papel + cota), selo de conexão do stream, registro de atividade, diff acústico do golden master, aviso de canário, proposta com parâmetros/confiança/replanejar, tela de acesso e sessão.
- Contraste dos textos terciários ajustado para AA (4,74:1).
- `Handoff Frontend.dc.html`: documento imprimível com inventário de telas, 61 ligações UI→API, catálogo SSE, contratos de props, tokens, acessibilidade medida, breakpoints e 11 lacunas de backend.
- Direção visual e tokens derivados de `12-DESIGN-BRIEF.md`; vocabulário de UI de `01-GLOSSARIO.md`.
- Estados de nó, ciclo de vida da proposta (HITL) e limites de parâmetro seguem `05-AGENTE-IA-HITL.md`.
- Estados de trabalho, eventos SSE e catálogo de erros seguem `03-CONTRATOS-API.md`.

## Screen map

| Tela (no protótipo) | Origem no repositório |
| --- | --- |
| 01 Primeiro uso | 00-VISAO-ESCOPO.md §4, README ADR-0009 |
| 02 Biblioteca | 12-DESIGN-BRIEF.md §4, 03-CONTRATOS-API.md §3.2 |
| 03 Editor de fluxo (canvas + paleta) | 12-DESIGN-BRIEF.md Tela 3, 01-GLOSSARIO.md (grafo) |
| 04 Painel de propriedades | 05-AGENTE-IA-HITL.md §3 (tabela de limites), 03-CONTRATOS-API.md §3.4 |
| 05 Raciocínio do assistente | 05-AGENTE-IA-HITL.md §2, 03-CONTRATOS-API.md §5 (agent.thought) |
| 06 Proposta / variações | 05-AGENTE-IA-HITL.md §4, 03-CONTRATOS-API.md §3.5 |
| 07 Resultado e comparação A/B | 12-DESIGN-BRIEF.md Tela 7, 03-CONTRATOS-API.md (artifact) |
| 08 Trabalhos e lote | 03-CONTRATOS-API.md §3.3, §6 (máquina de estados) |
| 09 Recursos e escala | 03-CONTRATOS-API.md §3.1, README ADR-0007 |
| 10 Configurações / version freeze | 09-MLOPS-GOLDEN-MASTER.md, 03-CONTRATOS-API.md §3.8 |
| 11 Erros e recuperação | 03-CONTRATOS-API.md §4 (catálogo), 06-PERSISTENCIA-RESILIENCIA.md |
| DS Fundamentos | 12-DESIGN-BRIEF.md §5 (tokens), §4 (matriz de estados) |
| 12 Registro de atividade | 08-SEGURANCA-MULTITENANCY.md (audit_event), 05-AGENTE-IA-HITL.md P6 |
| 13 Acesso e sessão | 03-CONTRATOS-API.md §1 (JWT, sessão local), 08-SEGURANCA-MULTITENANCY.md |
| Handoff para dev | 03-CONTRATOS-API.md (todo), 05-AGENTE-IA-HITL.md §3, 12-DESIGN-BRIEF.md §5–§9 |
