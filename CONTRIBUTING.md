# Contribuindo

## Antes do primeiro commit

Leia, nesta ordem:

1. [`docs/01-GLOSSARIO.md`](docs/01-GLOSSARIO.md) — a linguagem do projeto
2. [`docs/02-ARQUITETURA.md`](docs/02-ARQUITETURA.md) — as regras de dependência
3. [`docs/03-CONTRATOS-API.md`](docs/03-CONTRATOS-API.md) — o contrato
4. [`docs/14-AUDITORIA-KIT.md`](docs/14-AUDITORIA-KIT.md) — o estado real do código

---

## Ambiente

```bash
rustup toolchain install stable
rustup component add rustfmt clippy
cargo install cargo-nextest cargo-watch

cd ui && npm ci
```

Opcional: Docker (para Postgres, MinIO e observabilidade), Ollama (para o
assistente rodando local).

### Rodando

```bash
# backend em modo local (SQLite + disco), com recarga
cargo watch -x 'run --bin audio_api'

# frontend
cd ui && npm run dev

# serviços completos
docker compose up -d
```

---

## Branches e commits

```
main       protegida. Só entra por PR com CI verde e 1 aprovação.
feat/nome-curto
fix/nome-curto
chore/nome-curto
```

Conventional Commits, escopo obrigatório:

```
feat(dsp): implementa crossfade com curva logarítmica
fix(api): corrige escopo de tenant em list_jobs
docs(contratos): adiciona evento agent.proposal
test(dsp): invariante de continuidade na emenda
chore(deps): atualiza opentelemetry para 0.27
```

Escopos: `dsp` · `domain` · `agent` · `api` · `ui` · `infra` · `docs` · `deps`

---

## Padrões de código

### Rust

- `cargo fmt` e `cargo clippy -- -D warnings` obrigatórios
- **Sem `unwrap()` / `expect()`** em código de request. Só em testes e bootstrap
- Erros com `thiserror` nas bibliotecas, `anyhow` no binário
- Toda função pública de fronteira leva `#[instrument]`
- DSP **sempre** dentro de `spawn_blocking`; nunca `.await` dentro de closure Rayon
- Valor com unidade tem sufixo no nome: `duration_ms`, `gain_db`, `freq_hz`
- Limite de parâmetro vira newtype com validação na desserialização, não `if` solto

### TypeScript

- `prettier` + `eslint`, sem warning
- Tipos da API vêm de `ui/src/types/` (gerados). **Não escrever à mão**
- Sem regra de negócio no componente: a UI desenha e envia comando
- Todo componente que busca dados trata carregando, vazio e erro

### SQL

- Toda tabela com dados de usuário tem `tenant_id NOT NULL`
- Toda query passa por `with_tenant_scope` ou `ScopedRepo`
- Migração é *forward-only*. Nunca editar uma já publicada

---

## Testes

```bash
cargo nextest run --workspace        # unitário + integração
cargo test --doc                     # exemplos da documentação
cd ui && npm test                    # UI
npx playwright test                  # E2E (precisa do backend rodando)
```

Se você tocou em DSP, adicione um invariante de propriedade. Ver
[`docs/10-TESTES-QUALIDADE.md`](docs/10-TESTES-QUALIDADE.md) §3.

---

## Mudando o contrato de API

O contrato é compartilhado. Alterar exige:

1. Atualizar `docs/03-CONTRATOS-API.md` **no mesmo PR**
2. Regenerar os tipos: `cargo test export_bindings`
3. Commitar `ui/src/types/` alterado (o CI falha se estiver dessincronizado)
4. Avisar o time — quebra o frontend se sair sem aviso

Mudar um **limite de parâmetro** exige atualizar três lugares no mesmo PR:

- o newtype em `crates/audio_core/src/domain/`
- a tabela canônica em `docs/05-AGENTE-IA-HITL.md` §3
- a resposta de `GET /api/v1/tools`

---

## Mudando prompts

1. Editar o `.prompt` em branch
2. `python scripts/prompt_linter.py prompts/catalog.json`
3. Abrir PR — o CI roda os testes de Golden Master
4. Se a distância acústica passar de 0,15, **baixe os dois WAVs do build e
   ouça** antes de aprovar
5. Nunca deletar prompt antigo — marcar como `deprecated` (projetos congelados
   dependem dele)

---

## Checklist de PR

```markdown
## O que muda
## Por quê
## Como testar

## Checklist
- [ ] clippy limpo, fmt aplicado
- [ ] Testes cobrem a lógica nova
- [ ] Se toca DSP: invariante adicionado
- [ ] Se toca contrato: docs atualizado + tipos TS regenerados
- [ ] Se toca limite: tabela canônica atualizada
- [ ] Se toca prompt: linter verde + Golden Master ouvido
- [ ] Span de tracing na fronteira nova
- [ ] Erro novo no catálogo
- [ ] Sem unwrap() fora de teste
- [ ] Checklist de segurança (docs/08 §10) quando aplicável
```

---

## Revisão de código

O revisor verifica, além do checklist:

1. As regras de dependência do `docs/02` §2 foram respeitadas?
2. Existe caminho em que um valor não validado chega ao DSP?
3. Existe caminho em que uma query roda sem escopo de tenant?
4. O erro novo é acionável pelo usuário ou é vazamento de detalhe interno?
5. Se falhar no meio, o sistema fica em estado consistente?

A pergunta 5 é a que mais pega bug neste projeto.

---

## Discussões

- **Decisão arquitetural** → ADR em `docs/adr/`, PR próprio
- **Bug** → issue com passos de reprodução e `trace_id` se houver
- **Ideia de produto** → issue com label `type/spike`, sem código antes de decidir
