# 12 — Design Brief

> Documento para o designer. Não exige leitura do código. Onde há restrição
> técnica, ela está marcada com **[técnico]** e explicada.

---

## 1. O produto em uma frase

Uma ferramenta que transforma horas de gravação de ensaio em recortes curtos e
publicáveis, guiada por uma conversa em linguagem natural — com todos os
controles manuais disponíveis para quem quiser assumir o comando.

## 2. Para quem

| Persona | Contexto | O que teme |
| --- | --- | --- |
| **Músico solo** (primária) | Laptop, 200 arquivos de jam, publica em Reels | Perder tempo, não entender o que aconteceu |
| **Banda** | VPS compartilhada, 4 pessoas, lotes grandes | Sobrescrever o trabalho de outro, perder um lote |
| **Produtor de estúdio** | Máquina isolada, material de cliente | Que o áudio vaze; que o som mude sem aviso |

Nenhum deles é designer de produto. Nenhum deles quer aprender uma DAW nova.
Todos eles reconhecem imediatamente quando um áudio soa mal.

## 3. Princípios de design deste produto

1. **A máquina propõe, a pessoa decide.** Nenhuma ação da IA acontece sem que o
   usuário veja o que vai acontecer e possa recusar. Isso não é um recurso —
   é a personalidade do produto.
2. **Mostrar o raciocínio.** O tempo de espera é inevitável (LLM + DSP). Em vez
   de um spinner, mostramos o assistente pensando. Espera narrada é espera
   tolerável, e ensina o usuário sobre mixagem.
3. **A origem de cada valor é visível.** O usuário sempre sabe se aquele número
   veio dele, da IA ou do padrão. Sem isso, ele não confia em nada na tela.
4. **Falhar em voz alta.** Se um render se perdeu, dizemos. Se o assistente
   está fora do ar, dizemos. Silêncio em ferramenta criativa gera desconfiança
   permanente.
5. **Áudio antes de gráfico.** A prova do produto é ouvir. O botão de tocar
   precisa estar sempre a um clique, em qualquer estado, sem carregar página.

---

## 4. Inventário de telas

| # | Tela | Prioridade |
| --- | --- | --- |
| 1 | Primeiro uso / onboarding | Alta |
| 2 | Biblioteca de faixas | Alta |
| 3 | **Editor de fluxo (canvas)** — tela principal | Crítica |
| 4 | Painel de propriedades do nó | Crítica |
| 5 | Painel de raciocínio do assistente | Crítica |
| 6 | Overlay de proposta (HITL) | Crítica |
| 7 | Resultado e comparação | Alta |
| 8 | Lista de trabalhos / lote | Média |
| 9 | Recursos e escala | Média |
| 10 | Configurações do projeto (version freeze) | Média |
| 11 | Estados de erro e recuperação | Alta |

---

### Tela 3 — Editor de fluxo (a tela principal)

Layout de três regiões:

```
┌─────────────────────────────────────────────────────────────────────┐
│  cabeçalho: faixa · duração · BPM · [▶ tocar]   [Renderizar]        │
├──────────┬────────────────────────────────────────┬─────────────────┤
│          │                                        │                 │
│ paleta   │            CANVAS (DAG)                │  propriedades   │
│ de nós   │                                        │  do nó          │
│          │   ┌────┐   ┌────┐   ┌────┐   ┌────┐    │                 │
│ ○ fonte  │   │ 🎵 │──▶│ 📊 │──▶│ 🤖 │──▶│ 🎚 │    │  [sliders]      │
│ ○ análise│   └────┘   └────┘   └────┘   └────┘    │                 │
│ ○ assist.│                                        │                 │
│ ○ efeito │                                        │                 │
│ ○ master │                                        │                 │
│          ├────────────────────────────────────────┤                 │
│          │  ▼ Raciocínio do assistente   [passo 2/5]                │
│          │  "A faixa está em 128 BPM. Como o pedido…"               │
└──────────┴────────────────────────────────────────┴─────────────────┘
```

**[técnico]** O canvas usa React Flow. Isso define comportamentos que já vêm
prontos e não devem ser redesenhados: zoom com scroll, pan com arrastar, seleção
por área, minimapa, controles de zoom. O designer customiza a **aparência** dos
nós e arestas, não a mecânica.

#### Anatomia do nó

```
┌──────────────────────────────────┐
│ ◉  Compressão            [•••]   │  ← ícone · título · menu
├──────────────────────────────────┤
│ ratio        4.0 :1        🤖    │  ← valor + origem
│ threshold  −14.5 dB        🔒    │
├──────────────────────────────────┤
│ ▓▓▓▓▓▓▓▓░░░░░░  processando…    │  ← barra só quando ativo
└──────────────────────────────────┘
   ◀ entrada                saída ▶
```

Elementos obrigatórios:

| Elemento | Regra |
| --- | --- |
| Ícone de tipo | Um por categoria (fonte, análise, assistente, efeito, master, saída) |
| Título | Nome em português do glossário |
| 2–3 parâmetros | Os mais relevantes; o resto no painel lateral |
| Indicador de origem | 🤖 inferido pela IA · 🔒 travado pelo usuário · (nada) = padrão |
| Estado visual | Ver matriz abaixo |
| Conectores | Entrada à esquerda, saída à direita |

#### Matriz de estados do nó — **entregável obrigatório**

Cada estado precisa de tratamento visual distinto **sem depender só de cor**
(daltonismo, e o canvas costuma ser escuro):

| Estado | Quando | Tratamento sugerido |
| --- | --- | --- |
| `idle` | Configurado, aguardando | Neutro, borda sólida fina |
| `proposed` | Sugerido pela IA, não aprovado | **Tracejado + pulsação suave + ícone 🤖 no canto**; visualmente "não é real ainda" |
| `queued` | Na fila | Neutro com ícone de relógio |
| `running` | Processando | Borda animada + barra de progresso |
| `completed` | Concluído | Marca de conferido; borda de acento |
| `failed` | Erro | Ícone de alerta + borda de erro + link "ver detalhes" |
| `rejected` | Proposta recusada | Fantasma, opacidade baixa, some após 3 s |
| `locked` | Job finalizado | Cadeado; controles desabilitados |

O estado `proposed` é o mais importante do sistema inteiro. O usuário precisa
entender, sem ler, que **aquilo ainda não existe** e depende dele.

---

### Tela 5 — Painel de raciocínio

Onde o assistente "pensa em voz alta". Recebe texto em streaming, palavra a
palavra.

```
┌────────────────────────────────────────────────────────┐
│ 🤖 Assistente                        passo 2 de 5      │
├────────────────────────────────────────────────────────┤
│ A faixa está em 128 BPM com energia concentrada         │
│ entre 1:30 e 2:10. Como o pedido enfatiza as viradas    │
│ de bateria, vou priorizar blocos com transiente alto▊   │
│                                                        │
│ ✓ Detectar BPM               128,4 BPM         1,2 s   │
│ ✓ Selecionar blocos          8 blocos          0,4 s   │
│ ⟳ Aplicar compressão                                   │
└────────────────────────────────────────────────────────┘
```

**[técnico]** O texto chega como fragmentos (`delta`) via SSE. Precisa acumular
suavemente, com cursor piscando — **sem** relayout a cada fragmento. Um painel
que "pula" a cada palavra é desconfortável de ler.

Precisa de: estado colapsado (só o passo atual), estado expandido (histórico
completo), e estado de erro ("assistente indisponível — usando configuração
manual").

---

### Tela 6 — Overlay de proposta (o momento decisivo)

Aparece quando o assistente quer adicionar algo que o usuário não pediu.

```
┌──────────────────────────────────────────────────────┐
│  🤖 O assistente sugere                       1:47   │
│                                                      │
│  Adicionar  ┃ Separação de stems ┃                   │
│                                                      │
│  "O pedido enfatiza as viradas de bateria.           │
│   Separar os stems permite comprimir só a            │
│   percussão, sem afetar o resto da mixagem."         │
│                                                      │
│  Onde: antes de Compressão                           │
│                                                      │
│  [ Aprovar ]   [ Recusar ]   [ Por quê? ]            │
└──────────────────────────────────────────────────────┘
```

Requisitos:

1. **Não bloqueia a tela inteira.** Painel lateral ou card flutuante — o usuário
   precisa continuar vendo o canvas para entender onde o nó entraria.
2. **Destaca a posição.** O nó de referência ("antes de Compressão") ganha um
   halo enquanto a proposta está aberta.
3. **Mostra o tempo restante** (120 s). Sem contagem regressiva agressiva; um
   contador discreto basta. Ao expirar, fecha com um toast neutro.
4. **"Recusar" não é destrutivo.** Não usar vermelho de erro. Recusar é uma
   escolha legítima; o assistente vai tentar outro caminho. O copy do toast
   depois: "Ok — vou tentar de outro jeito."
5. **Estado de espera após decidir.** Botões desabilitam imediatamente (evita
   duplo clique) e mostram progresso.

Variações a desenhar: proposta única · duas propostas seguidas (fila, não
empilhamento) · proposta expirada · proposta decidida em outra aba.

---

### Tela 7 — Resultado e comparação

```
┌──────────────────────────────────────────────────────────┐
│  Pronto — 30,4 s                       [⬇ Baixar WAV]    │
│                                                          │
│  ▁▃▅█▇▅▃▁▂▄▆█▇▅▃▂▁▃▅▇█▆▄▂▁▃▅▇█▆▄▂▁      ← waveform      │
│  ▶ ━━━━━━●──────────────────────  0:12 / 0:30            │
│                                                          │
│  Volume percebido  −14,1 LUFS      Pico  −1,0 dB         │
│  Blocos usados     8 de 47                               │
│                                                          │
│  ⚠ Uma emenda ficou brusca (aos 0:18)      [ver]         │
│                                                          │
│  [ Comparar com o original ]  [ Gerar outra versão ]      │
└──────────────────────────────────────────────────────────┘
```

Detalhes que importam:

- **Marcadores de emenda na waveform.** O usuário quer saber onde estão os
  cortes para avaliar se ficaram bons.
- **Comparação A/B** com troca instantânea entre original e resultado, mantendo
  a posição de reprodução. É como profissionais avaliam áudio.
- **Warnings são informativos, não erros.** "Emenda brusca" é um aviso do motor,
  não uma falha. Tratamento visual de atenção, não de erro.

---

### Tela 9 — Recursos e escala

```
┌──────────────────────────────────────────────┐
│  Processadores                               │
│  ●━━━━━━━━━━○──────────────  4 de 7          │
│                                              │
│  Sua máquina tem 8 núcleos.                  │
│  Cada processador usa cerca de 1 núcleo.     │
│                                              │
│  ○ Piloto automático                         │
│                                              │
│  Na fila: 3    Processando: 4                │
│  CPU: ▓▓▓▓▓▓░░░░  41%                        │
└──────────────────────────────────────────────┘
```

Com piloto automático ativo, o slider vira **somente leitura** mas continua
visível, mostrando o alvo calculado. Mais um histórico curto: "Aumentou de 2
para 4 há 3 min". Ver a automação agir é o que constrói confiança nela.

---

### Tela 11 — Recuperação e erros

**Banner de recuperação** (aparece uma vez, após queda):

```
┌────────────────────────────────────────────────────────┐
│  🔄 Recuperamos seu trabalho                           │
│  2 renders concluídos · 1 reenfileirado · 0 perdidos   │
│                                              [Fechar]  │
└────────────────────────────────────────────────────────┘
```

Se algo se perdeu, o tom muda e a informação fica mais visível — mas nunca
acusatório nem alarmista:

```
┌────────────────────────────────────────────────────────┐
│  ⚠ 1 render se perdeu no desligamento                  │
│  "Jam 04 — versão TikTok" precisa ser refeito.         │
│                             [Refazer]  [Ver detalhes]  │
└────────────────────────────────────────────────────────┘
```

**Erros com `trace_id`:** todo erro técnico mostra um identificador copiável em
um clique, com o texto "Se precisar de ajuda, envie este código".

---

## 5. Direção visual

### Contexto de uso

O usuário está em ambiente de estúdio ou quarto, muitas vezes à noite, com
monitor calibrado. Tema **escuro por padrão** não é moda — é conforto real nesse
contexto e é o que toda ferramenta de áudio faz. Tema claro fica como opção.

### O que evitar

- Estética "dashboard de SaaS" (cards brancos, azul corporativo, gráficos
  genéricos). Isso é uma ferramenta criativa, não um painel de métricas.
- Skeuomorfismo de estúdio (knobs de metal escovado, VU meters com agulha,
  madeira). Envelhece mal e prejudica a legibilidade em tela pequena.
- Excesso de neon/glow. Cansa em sessão longa e prejudica contraste.

### Direção sugerida

Superfícies escuras e neutras (não pretas puras — cinzas com leve temperatura),
tipografia com boa legibilidade em tamanho pequeno, **uma** cor de acento forte
usada com parcimônia, e a informação de áudio (waveform, medidores) como o
elemento mais vívido da tela. O áudio é o protagonista; a interface é o
enquadramento.

### Tokens a definir — **entregável**

```
cor
  superfície   surface/base · raised · overlay · sunken
  borda        border/subtle · default · strong · focus
  texto        text/primary · secondary · tertiary · inverse
  acento       accent/default · hover · pressed · subtle
  semântica    success · warning · danger · info
  origem       param/ai · param/locked · param/default   ← específico deste produto
  áudio        waveform/fill · waveform/played · playhead · splice-marker

tipografia
  família      interface (sans) · numérica tabular (valores) · mono (trace_id, JSON)
  escala       display · title · body · label · caption
  → valores numéricos SEMPRE com dígitos tabulares (o número não pode
    "dançar" enquanto atualiza em tempo real)

espaçamento   escala de 4 px
raio          nó · card · botão · input
sombra        raised · overlay · focus
movimento     duração e curva para: pulsação de proposta, progresso,
              entrada/saída de nó, streaming de texto
```

### Ícones

Conjunto único e consistente. Categorias necessárias: tipos de nó (6), estados
(6), transporte de áudio (play, pause, loop, A/B), origem de parâmetro (IA,
cadeado), ações (aprovar, recusar, refazer, baixar, expandir).

---

## 6. Acessibilidade

| Requisito | Detalhe |
| --- | --- |
| Contraste | 4,5:1 em texto; 3:1 em elementos de interface |
| Estado sem cor | Todo estado de nó tem forma/ícone além de cor |
| Teclado | Canvas navegável por Tab; proposta aprovável por Enter, recusável por Esc |
| Foco visível | Anel de foco em todos os controles interativos |
| Movimento reduzido | `prefers-reduced-motion` desliga pulsações e animações de borda |
| Leitor de tela | Proposta anunciada via `aria-live="assertive"`; raciocínio via `aria-live="polite"` |
| Alvo de toque | Mínimo 44×44 px nos controles principais |

O canvas de nós é notoriamente ruim para leitor de tela. Mitigação mínima: uma
**visão em lista** do fluxo, navegável, como alternativa ao canvas. Não precisa
ser bonita; precisa existir.

---

## 7. Tom de voz (copy em pt-BR)

| Situação | Sim | Não |
| --- | --- | --- |
| IA sugerindo | "O assistente sugere adicionar…" | "A IA detectou que você precisa de…" |
| Erro do sistema | "Algo deu errado ao renderizar." | "Erro interno: NullPointerException" |
| Recusa | "Ok — vou tentar de outro jeito." | "Sugestão rejeitada." |
| Espera | "Analisando as batidas…" | "Carregando…" |
| Sucesso | "Pronto — 30,4 s" | "Operação concluída com êxito!" |
| Limite | "A transição vai até 3 s." | "Valor inválido: máximo 3000" |

Direto, sem exclamação, sem gíria de startup, sem antropomorfizar demais. O
assistente é competente e discreto, não animado.

---

## 8. Entregáveis esperados

### Fase 1 — antes da Sprint 3 (bloqueia o frontend)

1. **Fluxos** das 3 jornadas críticas: primeiro render · proposta aprovada ·
   proposta recusada.
2. **Anatomia do nó** com os 8 estados da matriz.
3. **Overlay de proposta** com todas as variações.
4. **Tokens** em JSON ou Figma Variables (o frontend consome direto).

### Fase 2 — durante a Sprint 3

5. Telas de biblioteca, resultado, lista de trabalhos.
6. Painel de propriedades com todos os tipos de controle (slider, enum, toggle,
   multi-select, campo numérico com unidade).
7. Estados de vazio, carregando e erro de cada tela.

### Fase 3 — Sprint 4

8. Recursos/escala, configurações de projeto, version freeze.
9. Banners de recuperação e erro.
10. Guia de componentes consolidado.

### Formato

- Figma com páginas: `Fundamentos` · `Componentes` · `Telas` · `Fluxos` ·
  `Protótipo`
- Componentes com variantes nomeadas **exatamente** como os estados do código
  (`idle`, `proposed`, `running`…) — isso elimina uma classe inteira de
  ambiguidade no handoff
- Auto-layout em tudo que o frontend vai reproduzir com flexbox
- Tokens exportáveis (Figma Variables ou JSON no formato do Style Dictionary)

---

## 9. Restrições técnicas que afetam o design

| # | Restrição | Consequência |
| --- | --- | --- |
| T1 | Canvas é React Flow | Zoom, pan, minimapa e conectores são padrão da biblioteca |
| T2 | Streaming de texto chega em fragmentos | Layout do painel de raciocínio precisa ser estável; sem "pulo" |
| T3 | Proposta expira em 120 s | Precisa de contador e de estado de expiração |
| T4 | Parâmetros têm limites rígidos do backend | Slider precisa mostrar o limite; valor fora é recusado |
| T5 | Limites vêm da API, não hardcoded | O componente de slider recebe min/max por props |
| T6 | Render leva de 20 s a alguns minutos | A espera precisa de conteúdo, não de spinner |
| T7 | Sem LLM disponível, o produto ainda funciona | Toda tela precisa de versão sem assistente |
| T8 | Waveform vem como picos pré-calculados | Resolução limitada; não desenhar amostra a amostra |
| T9 | Recuperação pós-queda é comum no modo laptop | Banner de recuperação não é caso raro |
| T10 | Cores devem funcionar em monitor não calibrado | Não depender de diferenças sutis de matiz |

---

## 10. Referências úteis (para linguagem, não para cópia)

- **Ableton Live / Bitwig** — hierarquia de informação densa em tema escuro,
  legível em sessão longa.
- **Figma / Linear** — sistema de componentes limpo e movimento contido.
- **n8n / Node-RED** — convenções de canvas de nós que as pessoas já conhecem.
- **iZotope Ozone** — como apresentar sugestão automática de processamento sem
  tirar o controle do usuário. É o produto conceitualmente mais próximo.

O que **não** copiar: a densidade de uma DAW profissional. Nosso usuário
primário não é engenheiro de áudio; ele precisa de menos controles à vista e
mais explicação por controle.
