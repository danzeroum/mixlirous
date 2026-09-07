# Paleta Mixlirous — origem e justificativa

> Este documento explica **de onde cada cor veio**. Ele não é decorativo: qualquer
> alteração de token deve atualizar a tabela de amostragem abaixo, senão a marca
> deixa de ser rastreável ao material de origem.

## 1. Estado anterior (o que estamos substituindo)

Levantamento feito no código real, não na documentação:

- `ui/src/index.css` continha apenas `@import 'tailwindcss';` mais o bloco de
  acessibilidade adicionado no PR #59. **Não existia nenhum `@theme`**, portanto
  a paleta do produto era 100% o default do Tailwind, aplicado via classes
  utilitárias avulsas.
- Tailwind v4 confirmado: `tailwindcss ^4.3.3` + `@tailwindcss/vite ^4.3.3`.
- **Bug ativo:** `ui/src/App.tsx` usa `bg-gray-850` na sidebar. A escala padrão do
  Tailwind v4 vai de `50` a `950` em passos de `100`; `850` não existe, a classe
  não gera CSS e a sidebar herda silenciosamente o fundo do container. O bug
  existe na `main` e **também** na branch do PR #59.
- Existiam 4 valores hex literais fora do CSS: `#2563eb` (RemixCanvas),
  `#374151`, `#a78bfa`, `#34d399` (Waveform) e `#a78bfa` (index.css).

Inventário de classes de cor em `ui/src` na branch do PR #59 — 2.401 linhas,
14 componentes/views, **66 combinações distintas** de classe de cor:

| Família | Ocorrências | Papel semântico observado |
|---|---|---|
| `gray-*` + `white` | ~200 | superfície e texto |
| `purple-*` / `violet-*` | ~25 | modo assistido, nó de efeito, foco |
| `green-*` | ~15 | ação primária (aprovar, baixar, enviar) |
| `blue-*` | ~13 | modo manual, faixa original |
| `red-*` | ~15 | erro |
| `orange-*` / `yellow-*` | ~5 | atenção (uso inconsistente) |

O mapeamento semântico já era coerente. **O problema não é a semântica, é a
ausência de tokens e o desalinhamento com a marca.**

## 2. Materiais de marca e amostragem

Duas referências, com papéis diferentes. **Ambas importam** — usar só uma leva à
conclusão errada.

### 2.1 Mockup de landing page — referência de *interface*

É o material que mostra UI real. Amostragem por região:

| Elemento | Hex amostrado | Leitura |
|---|---|---|
| Barra de navegação | `#293038` | cinza-azulado frio, próximo do `gray-800` atual |
| CTA "Join the Mixlirous Beta" | `#1F8481` | **teal**, H≈178° |
| Botão secundário "Get Started" | outline sobre `#293038` | chrome frio |

**Consequência:** a ação primária da marca é **teal**, não verde-esmeralda. O
`green-600` atual está na família certa mas com matiz deslocado (~145°) e
saturação alta demais.

### 2.2 Poster de estúdio — referência de *identidade e atmosfera*

Amostragem por região (mediana de cada recorte):

| Região | Hex | HSV |
|---|---|---|
| Banner lateral direito | `#572C5F` | H=291° S=0.54 V=0.37 |
| LED / parede esquerda | `#5C2229` | H=353° S=0.63 V=0.36 |
| Backdrop central | `#C68278` | H=8° S=0.39 V=0.78 |
| Case tweed (guitarra) | `#9B6530` | H=30° S=0.69 V=0.61 |
| Escuros de fundo | `#180E11` | H=342° S=0.42 V=0.09 |
| Holograma "Agent AI Mix Assist" | violeta + ciano | H≈290° / H≈201° |

**Consequência:** existe vínculo semântico real e não acidental — no poster, o
agente de IA é representado em **violeta com ciano**, exatamente o par que o
frontend já usa para *assistido* e *manual*.

## 3. Decisões de composição

Onde as duas referências divergem, valem estas decisões — e o motivo:

| Decisão | Motivo |
|---|---|
| **Base neutra-quente** (`#141010`, H=8°, S≈0.13), não marrom-avermelhado puro | O `#190F13` do poster é iluminação prática de fotografia. Como fundo de aplicação de sessão longa ele fica sujo e reduz a legibilidade da forma de onda. A base quente com saturação baixa puxa a temperatura da marca sem virar sépia, e continua compatível com o chrome frio do mockup. |
| **Teal como ação primária**, substituindo `green-*` | Amostrado do CTA real da landing (`#1F8481`). É a única cor de ação que existe em material de marca. |
| **Violeta como acento de IA**, ancorado em `#572C5F` | Confirma o papel já usado no código e o holograma do poster. |
| **Coral restrito à marca** | O coral é dominante no poster, mas colide semanticamente com erro. Fica reservado a logo, hero e realces editoriais — nunca a estado funcional. |
| **Âmbar para atenção**, substituindo `orange-*`/`yellow-*` | Amostrado do case tweed. Unifica dois usos hoje inconsistentes. |
| **Erro migra para `danger` em H=357°** | Afasta o vermelho de erro do coral da marca (H=8°), preservando a distinção sob daltonismo protan/deutan. |
| **Azul-ciano do holograma para modo manual** | Mantém o par violeta/ciano do material de marca. |

## 4. Critério de aceitação da paleta

A implementação só está correta se, ao final:

1. Nenhum valor hex de cor existe em `ui/src` fora de `index.css` e `lib/theme.ts`.
2. Nenhuma classe de família nativa do Tailwind (`gray`, `green`, `purple`,
   `blue`, `red`, `orange`, `yellow`, `violet`, `emerald`, `slate`) aparece em
   `ui/src`.
3. Todo par texto/fundo efetivamente renderizado atinge no mínimo **4.5:1**
   (texto normal) ou **3:1** (texto ≥ 18.66px bold / ≥ 24px, e bordas de
   componente que carregam significado).
4. O bug do `bg-gray-850` deixou de existir.
