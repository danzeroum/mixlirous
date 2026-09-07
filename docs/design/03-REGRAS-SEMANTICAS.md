# Regras semânticas de cor

Regras invioláveis. Uma PR que viole qualquer item abaixo deve ser rejeitada em
review, mesmo que os gates estejam verdes.

## R1 — Cor nunca é o único portador de significado

Todo estado (sucesso, erro, aviso, processando, indisponível) precisa de **cor +
um segundo canal**: ícone, rótulo textual ou forma. Isso vale especialmente para
a paleta atual, onde `brand` (coral, H=8°) e `danger` (H=357°) ficam próximos sob
daltonismo protan.

Errado: um ponto colorido indicando status do job.
Certo: ponto colorido + texto "Processando" + `aria-label`.

## R2 — `brand-*` é exclusivamente de marca

Permitido: logo, hero, cabeçalho editorial, ilustração, estado vazio decorativo.
**Proibido:** botão de estado, badge de status, mensagem, borda de validação,
qualquer elemento cujo significado o usuário precise decodificar.

Motivo: o coral colide com o vermelho de erro. Se o coral também significar algo
funcional, o usuário perde a única pista confiável de que algo deu errado.

## R3 — Um papel, uma família

| Família | Significa | Nunca significa |
|---|---|---|
| `action` | ação primária que o usuário inicia (enviar, aprovar, renderizar, baixar) | sucesso passivo, marca |
| `ai` | conteúdo gerado ou proposto pelo agente, modo assistido | ação neutra, seleção genérica |
| `manual` | controle direto do usuário, faixa original, modo manual | link, informação genérica |
| `warn` | degradação, limitação real, baixa confiança | erro |
| `danger` | erro e ação destrutiva | aviso |
| `surface`/`ink` | estrutura e texto | estado |

Se um elemento não se encaixa em nenhuma família, ele é neutro: `surface` +
`ink`. Não invente um sexto significado.

## R4 — Contraste mínimo verificável

- Texto normal: **≥ 4.5:1**.
- Texto grande (≥ 24px, ou ≥ 18.66px bold): **≥ 3:1**.
- Borda ou ícone que carrega significado: **≥ 3:1** contra o fundo adjacente.
- Anel de foco: **≥ 3:1** contra o fundo **e** contra o elemento focado.

Nunca ajuste um par reduzindo o texto para "texto grande" só para passar. Suba o
degrau do token.

## R5 — Botão preenchido usa o degrau documentado

| Papel | Fundo | Texto |
|---|---|---|
| Primário | `action-700` | `text-white` |
| IA | `ai-600` | `text-white` |
| Manual | `manual-600` | `text-white` |
| Atenção | `warn-600` | `text-white` |
| Destrutivo | `danger-600` | `text-white` |
| Secundário | `surface-700` | `text-ink-100` |

Não usar `action-600` como fundo de botão: com branco dá 4.49:1 e reprova AA.

## R6 — Nenhum hex fora dos dois arquivos canônicos

Os únicos lugares onde um literal de cor pode existir são `ui/src/index.css` e
`ui/src/lib/theme.ts`. Canvas 2D, React Flow e SVG dinâmico importam de
`canvasColors`.

Guarda automatizada obrigatória — deve falhar o lint:

```bash
# nenhum hex de cor em componentes
! grep -rnE "#[0-9a-fA-F]{3,8}\b" ui/src --include=*.tsx

# nenhuma família nativa do Tailwind
! grep -rnE "\b(bg|text|border|ring|from|via|to|fill|stroke|divide|outline)-(gray|slate|zinc|neutral|stone|green|emerald|teal|cyan|sky|purple|violet|fuchsia|blue|indigo|red|rose|pink|amber|orange|yellow)-[0-9]{2,3}" ui/src
```

## R7 — Estado degradado é `warn`, não silêncio

Casos que **obrigatoriamente** usam `warn-*` com texto explícito:

- Ferramenta indisponível na paleta (sem processamento DSP real).
- Faixa estéreo cujo render atual sairá em mono — ver
  `docs/ADENDO-ESTEREO-E-AUDIO-SUITE.md` quando existir.
- Análise com baixa confiança (BPM, onset).
- Proposta da IA expirada.

Ocultar a limitação é pior que exibi-la em âmbar.

## R8 — Modo alto contraste e movimento reduzido

`prefers-reduced-motion: reduce` já está tratado e deve ser preservado.
`prefers-contrast: more` deve elevar `ink-400`→`ink-200`, `ink-500`→`ink-300` e
engrossar bordas para `surface-500`. Não introduzir uma segunda paleta.

## R9 — A paleta não muda por gosto

Alterar um token exige: atualizar `theme.css`, `theme.ts`, a tabela de contraste
em `01-TOKENS.md` e a amostragem em `00-PALETA-E-ORIGEM.md`, com o motivo. Um
token sem rastreabilidade ao material de marca não entra.
