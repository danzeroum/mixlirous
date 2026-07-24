# Mixlirous — Relatório gerencial de funcionalidades

**O que o músico vai poder fazer, e o que vai poder ajustar.**

Versão de 24/07/2026 · Fonte: `docs/00`, `docs/03`, `docs/04`, `docs/05`, `docs/12`
e o modelo de domínio em código (`tools.rs`, `pipeline_config.rs`)

---

## ⚠️ Antes de ler: isto descreve o estado final, não o atual

Este documento é a especificação funcional do produto pronto. **Nada aqui
funciona hoje.**

O que existe em 24/07/2026: o projeto compila, a API responde, a interface
renderiza, os limites de todos os parâmetros estão implementados e validados. O
que **não** existe: execução do processamento de áudio, chamadas ao modelo de IA,
fila de trabalhos, persistência real. Nenhum arquivo de áudio foi processado
ainda.

Prazo realista para o conjunto abaixo: **9 a 10 semanas** de engenharia com uma
pessoa. Trate este documento como destino, não como inventário.

---

## 1. A ideia em uma frase

O músico sobe as próprias faixas, descreve em português o que quer, e o sistema
**recompõe o material dele** — não gera música nova. A IA analisa, propõe ajustes
e explica o motivo; o músico aceita, recusa ou corrige cada proposta.

Duas frases que governam todas as decisões de produto:

> **"Não gera música, recompõe a sua."**
> **"A IA propõe, você decide."**

A segunda é a que define o produto. Ferramentas concorrentes são ou totalmente
manuais (um editor de áudio tradicional) ou totalmente automáticas (um botão
"gerar"). O Mixlirous é a faixa do meio: a IA faz o trabalho pesado de análise e
sugestão, e o controle final nunca sai da mão do usuário.

---

## 2. Para quem

| Persona | Quem é | O que quer |
|---|---|---|
| **P1 — Músico solo** | Compõe em casa, um laptop, sem estúdio | Transformar um demo de 4 minutos num corte de 30 segundos para redes sociais, sem aprender uma DAW |
| **P2 — Banda** | Tem gravações de ensaio e shows | Extrair os melhores trechos de material longo e montar versões curtas |
| **P3 — Produtor de estúdio** | Trabalha com material de clientes | Processar dezenas de faixas com um critério só, e não deixar o áudio sair da máquina |

P1 é a persona primária e define a maior restrição do produto: **tem que rodar
num laptop comum, sem instalar banco de dados nem serviço nenhum.**

---

## 3. Como é uma sessão de trabalho

**1. Sobe a faixa.** WAV, AIFF, FLAC ou MP3. O sistema valida o arquivo de
verdade — não confia na extensão.

**2. O sistema analisa sozinho.** Sem o usuário pedir nada, ele extrai:

- andamento (BPM) e a grade de batidas
- onde estão as batidas fortes
- energia ao longo do tempo
- estrutura harmônica — o que permite agrupar trechos parecidos
- **blocos de batida**: o material picado em pedaços musicalmente coerentes, não
  em fatias de tempo arbitrárias

**3. Descreve o que quer, em português.** Duas formas:

- **Texto livre:** *"quero uma versão de 30 segundos com os refrões, mais
  agressiva, com a bateria mais presente"*
- **Receita pronta:** escolhe de um catálogo (ex.: *bossa nova*, *tiktok
  agressivo*). Receita é ponto de partida editável, não caixa preta.

**4. A IA trabalha à vista.** Um painel mostra o raciocínio dela conforme
acontece — qual ferramenta vai usar, com que valor, e por quê. Não é barra de
progresso: é o texto da decisão.

**5. Cada mudança relevante vira uma proposta.** A IA não aplica direto. Aparece
um cartão: *o que ela quer fazer, com que valor, e a justificativa em uma frase.*
Três botões: **aceitar**, **recusar**, **ajustar o valor**.

**6. Resultado com comparação A/B.** Ouve o original e o resultado lado a lado,
e volta a mexer se quiser.

**7. Exporta.** WAV, MP3, AAC ou FLAC. Cada arquivo sai com uma assinatura
verificável — dá para provar depois que aquele áudio veio daquela receita.

---

## 4. O que o músico pode ajustar — as 8 ferramentas

Estas são as operações que o sistema sabe fazer. **A IA pode propor qualquer uma
delas; o usuário pode ajustar qualquer uma delas manualmente.** Os dois usam a
mesma faixa de valores permitidos — a IA não tem privilégio.

### 4.1 Separação de instrumentos (`stem_separation`)

Isola bateria, baixo, vocal e o resto. É a ferramenta mais pedida — permite
coisas como *"comprima só a bateria"*.

O usuário escolhe **quais instrumentos isolar**.

> **Atenção gerencial:** depende de um programa externo instalado na máquina.
> Quando não está presente, a ferramenta aparece **indisponível com o motivo
> explicado**, não escondida nem quebrada. É a única funcionalidade que não vem
> no pacote de instalação única. Decisão registrada na ADR-0010.

### 4.2 Compressão (`compression`)

Deixa o material mais uniforme e mais "presente". Cinco controles:

| Controle | O que faz |
|---|---|
| Razão | Quão forte é a compressão |
| Limiar (`threshold`) | A partir de que volume ela começa a agir — **−60 a 0 dB** |
| Ataque | Quão rápido reage a um som forte |
| Alívio (`release`) | Quão rápido solta depois |
| Ganho de compensação | Recupera o volume perdido no processo |

### 4.3 Equalização dinâmica (`dynamic_eq`)

Ajusta o equilíbrio de frequências — mais graves, menos agudo estridente,
destacar a voz. O usuário define **quantas bandas quiser**, e em cada uma:
frequência, ganho, largura (Q) e tipo de filtro.

É a ferramenta mais expressiva e a mais fácil de estragar o som. A IA propõe
bandas com justificativa acústica (*"há uma ressonância em 250 Hz mascarando a
caixa"*) em vez de números soltos.

### 4.4 Transição entre trechos (`crossfade`)

Como um bloco emenda no próximo. Dois controles:

| Controle | Faixa |
|---|---|
| Duração | **0 a 3000 ms** |
| Tipo de curva | Potência constante (padrão) ou ganho constante |

> A escolha de curva não é estética: **potência constante** para trechos de
> origens diferentes — que é o caso normal — porque a curva simples causa uma
> queda audível de volume no meio da transição. **Ganho constante** só quando o
> mesmo trecho se sobrepõe a si mesmo. O padrão está certo para 95% dos casos, e
> o usuário avançado pode trocar.

### 4.5 Ajuste de duração (`time_stretch`)

Encaixa o resultado numa duração exata sem mudar o tom. Um controle: o fator de
esticamento, limitado a **0,90 a 1,10** — ou seja, ±10%.

> **Por que o limite é tão apertado:** além de ±10% o material começa a soar
> artificial. O limite protege o usuário de estragar o som ao tentar forçar 45
> segundos de música em 30. Se a duração alvo não couber, o sistema **avisa** em
> vez de entregar algo distorcido.

### 4.6 Normalização de volume (`lufs_normalization`)

Coloca o material no volume padrão das plataformas de streaming. Dois controles:

| Controle | Padrão | Para quê |
|---|---|---|
| Volume percebido alvo | **−14 LUFS** | Padrão de Spotify, YouTube e Apple Music |
| Teto de pico | **−1 dBTP** | Evita distorção depois da conversão da plataforma |

> **Comportamento importante para o produto:** existe material dinâmico demais
> para atingir os dois alvos ao mesmo tempo. Nesse caso o sistema **prioriza o
> teto de pico e avisa o usuário do desvio de volume** — nunca escolhe em
> silêncio. Um motor que entrega volume errado sem falar está mentindo para o
> usuário, e isso é decisão de produto, não detalhe técnico.

### 4.7 e 4.8 Entrada e saída suaves (`fade_in`, `fade_out`)

Duração e tipo de curva, para começo e fim do resultado.

---

## 5. Configuração global da receita

Além das 8 ferramentas, o usuário controla parâmetros que valem para o trabalho
inteiro:

### Duração e montagem

| Ajuste | Padrão | O que significa para o usuário |
|---|---|---|
| Duração alvo | **30 s** | O tamanho do resultado |
| Tamanho do bloco | **4 batidas** | A granularidade do corte — blocos maiores preservam mais o gesto musical, menores dão mais liberdade de montagem |
| Seletividade de batida forte | **percentil 80** | Quão exigente é na escolha dos trechos. Mais alto = só o melhor material, resultado mais curto |
| Preservar introdução | **3000 ms** | Protege o começo original de ser picado |
| Preservar final | **3000 ms** | Protege o final original |

### Masterização

| Ajuste | Padrão |
|---|---|
| Volume alvo | −14 LUFS |
| Teto de pico | −1 dBTP |
| Limitação ligada | sim |
| Razão de compressão geral | 2:1 |

### Formato de saída

| Ajuste | Opções | Padrão |
|---|---|---|
| Codec | WAV · MP3 · AAC · FLAC | WAV |
| Taxa de amostragem | — | 44100 Hz |
| Canais | mono · estéreo | estéreo |
| Profundidade | — | 24 bits |

---

## 6. O recurso que diferencia o produto: a trava manual

Este é o item mais importante do relatório e o mais fácil de perder de vista.

**Todo parâmetro do sistema carrega três informações, não uma:** o valor, **de
onde veio** (usuário ou IA), e — quando vem da IA — **o grau de confiança dela**.

A consequência prática:

> **Se o usuário definiu um valor à mão, a IA não sobrescreve. Nunca.**
>
> Ela pode sugerir outro valor e explicar por quê. O usuário decide se solta a
> trava.

Isso é o que permite o fluxo de trabalho real: o músico fixa o que já sabe que
quer (*"a transição é 800 ms, não discuta"*) e deixa a IA trabalhar em cima do
resto. Sem essa garantia, a ferramenta seria um gerador com ilusão de controle.

Na interface, a origem do valor é visível — o usuário sempre sabe se está olhando
uma escolha dele ou um palpite da máquina.

---

## 7. O ciclo de proposta

Como a IA e o usuário negociam, em regras:

| Regra | Efeito para o usuário |
|---|---|
| Nada é aplicado sem aceite | Nenhuma surpresa no áudio |
| Cada proposta tem justificativa | Ele entende antes de decidir, e aprende no processo |
| Proposta expira em 2 minutos | Não fica uma decisão velha pendurada esperando aceite |
| Recusa é registrada | A IA não insiste na mesma sugestão recusada |
| Toda proposta respeita os limites | Uma proposta fora de faixa é rejeitada antes de chegar ao usuário |
| Histórico completo | Dá para reconstruir qualquer decisão depois |

No canvas, cada bloco de processamento tem estado visível. O estado **"proposta
pendente"** é o mais importante da interface — é onde o usuário decide, e é onde
o produto se distingue.

---

## 8. Processamento em lote

O usuário aplica uma receita a **um conjunto de faixas de uma vez**. É o caso da
persona P3.

O detalhe que importa: **os parâmetros se adaptam a cada faixa.** A receita
carrega a intenção (*"mais agressivo, 30 segundos"*), não números fixos — cada
faixa é analisada e otimizada individualmente. Aplicar o mesmo limiar de
compressão a 40 faixas de volumes diferentes daria 40 resultados inconsistentes.

---

## 9. Privacidade: o usuário escolhe onde a IA roda

Dois modos, com consequências claras:

| | **IA local** | **IA em serviço externo** |
|---|---|---|
| Custo | zero | por uso |
| O que sai da máquina | nada | o texto do pedido e dados técnicos da faixa |
| Precisa de internet | não | sim |
| Instalação | programa extra + ~5 GB | só uma chave de acesso |

**Em nenhum dos dois modos o áudio sai da máquina do usuário.** Só o texto do
pedido e os metadados de análise. Isso é garantia de arquitetura, não promessa de
marketing — e precisa estar escrito na tela antes do primeiro uso do modo
assistido, nomeando o provedor.

O sistema detecta sozinho se há IA local instalada e usa; se não houver, pede a
chave. O usuário não precisa entender a diferença antes de começar.

---

## 10. O que **não** entra na primeira versão

Fica de fora deliberadamente, para o produto não virar um editor de áudio
genérico:

| Fora | Por quê |
|---|---|
| Edição multipista | O produto recompõe uma faixa, não monta um arranjo |
| Efeitos em tempo real com preview instantâneo | Exige um segundo motor inteiro; invalidaria as garantias de recuperação após queda |
| Plugins de terceiros (VST) | Licenciamento e escopo de DAW |
| MIDI | Público diferente |
| Colaboração simultânea | Depois |
| Desenho livre de curvas de volume | Depois |
| Reparo espectral (remover cliques, zumbido) | Depois |
| Automação de parâmetros ao longo do tempo | Depois |

A tentação recorrente é medir o produto pela régua de completude de uma DAW.
Cada item de paridade dilui o diferencial e consome tempo que ele não tem.

---

## 11. Duas decisões de produto ainda abertas

| Decisão | Impacto no usuário | Quando decidir |
|---|---|---|
| **Espectrograma no cartão de proposta** | Mostraria visualmente *o que a IA está vendo* no áudio antes de o usuário decidir. É o argumento mais forte de transparência que o produto pode ter. Implementação é caro; a decisão de layout é barata agora e cara depois. | **Antes de o designer fechar as telas** |
| **Rótulos de seção editáveis** | O usuário renomearia "Refrão" para "Drop" e pediria *"só as seções marcadas assim"*. Transforma a análise estrutural em ação. | Antes do desenho da API de seções |

---

## 12. Como medir se funcionou

| Métrica | O que ela responde |
|---|---|
| Taxa de aceite de propostas | A IA está sugerindo coisa útil, ou o usuário recusa tudo? |
| Quantos parâmetros o usuário trava | Ele confia na IA, ou está fazendo tudo à mão? |
| Tempo do upload ao primeiro resultado | O produto é rápido o suficiente para iterar? |
| Quantas iterações até exportar | Ele chega onde quer, ou desiste? |
| Uso de receita pronta vs texto livre | Qual das duas portas de entrada realmente funciona? |

A segunda métrica é a mais reveladora. Se ninguém trava parâmetro, a promessa de
controle é decoração. Se todos travam tudo, a IA não está entregando valor.

---

## 13. Resumo para decisão

**O que o usuário controla:** 8 operações de áudio, ~20 parâmetros individuais, 5
ajustes de montagem, 4 de masterização, 4 de formato de saída — e a origem de
cada valor.

**O que o sistema faz sozinho:** analisa a faixa, corta em blocos musicais,
seleciona o melhor material, propõe parâmetros com justificativa, monta,
masteriza e exporta.

**O que nunca acontece:** o sistema não sobrescreve escolha do usuário, não aplica
mudança sem aceite, não manda áudio para fora da máquina, e não entrega resultado
fora do alvo sem avisar.

**O que ainda não existe:** tudo isso. Ver o aviso no topo.
