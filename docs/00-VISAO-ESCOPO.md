# 00 — Visão, Personas e Escopo

## 1. O problema

Um músico com 200+ gravações de ensaio (jams, sessões, takes) tem material bruto
demais e tempo de edição de menos. Extrair 30 segundos aproveitáveis de uma jam
de 12 minutos exige: ouvir tudo, achar o trecho de energia alta, cortar no tempo
certo do compasso, emendar sem estalo, e masterizar para a plataforma de destino.
Isso é meia hora de trabalho por faixa, no mínimo — e ninguém faz isso 200 vezes.

Ferramentas existentes resolvem partes: DAWs cortam com precisão mas exigem
operação manual; geradores de IA criam áudio novo em vez de reaproveitar o
material do artista.

## 2. A proposta

O Mixlirous automatiza o recorte e a remontagem **do material que o artista já
gravou**, guiado por intenção em linguagem natural, com controle manual completo
por cima.

Duas frases que definem o produto:

- **"Não gera música, recompõe a sua."** O output é sempre derivado do áudio de
  entrada. Nada é sintetizado.
- **"A IA propõe, você decide."** Nenhuma decisão da IA é aplicada de forma
  silenciosa e irreversível.

## 3. Personas

### P1 — Músico solo / produtor caseiro (persona primária do MVP)

- Tem entre 50 e 300 arquivos de ensaio em disco. Nomes tipo `jam_04_final2.wav`.
- Publica em Reels, TikTok e YouTube Shorts. Precisa de recortes de 15–60s.
- Conhecimento técnico de áudio: intermediário. Sabe o que é compressão, não sabe
  o que é *threshold* em dB.
- **Roda o sistema no próprio laptop.** Não tem VPS, não quer configurar Postgres.
- Métrica de sucesso: "gerei 10 versões aproveitáveis em 20 minutos".

### P2 — Banda com acervo compartilhado

- Instala em uma VPS barata (2–4 vCPU) para que os 4 integrantes acessem.
- Precisa de contas separadas e histórico de quem gerou o quê.
- Processa lotes: "gera versão curta de todas as 40 faixas do ensaio de março".
- Métrica de sucesso: processar um lote overnight sem babá.

### P3 — Produtor profissional / estúdio (persona de validação, não do MVP)

- Exige que o áudio **nunca** saia da máquina (stems de artistas contratados).
- Exige reprodutibilidade: reabrir o projeto em 6 meses e obter o mesmo som.
- Não confia em ferramenta que muda de comportamento com atualização de modelo.
- Métrica de sucesso: assinatura sonora congelada e auditável (`version freeze`).

## 4. Jornada principal (MVP)

```
1. Instala        → baixa binário + roda. SQLite embutido, sem config.
2. Importa        → aponta uma pasta. Sistema analisa BPM/grid em background.
3. Descreve       → "versão de 30s pra Reels, agressiva, foco na bateria"
4. Acompanha      → vê o raciocínio da IA em tempo real; sliders se preenchem
5. Ajusta         → trava a duração em 45s; a IA respeita a trava
6. Aprova         → aceita ou recusa a proposta de adicionar um nó novo
7. Renderiza      → WAV masterizado, download direto
8. Repete         → mesma receita aplicada a outras 39 faixas
```

O passo 5 é o diferencial competitivo. O passo 6 é o compromisso ético.

## 5. Escopo do MVP (V1 em produção)

### Dentro

| # | Entrega | Por que é núcleo |
| --- | --- | --- |
| 1 | Ingestão e análise (BPM, grid de batidas, RMS, chroma) | Sem grid não existe corte seguro |
| 2 | Remontagem por blocos com seleção heurística (knapsack) | É o produto |
| 3 | Emenda sem artefato (zero-crossing + crossfade logarítmico) | Um estalo destrói a credibilidade |
| 4 | Masterização (compressão, limiter, normalização LUFS) | Output publicável sem retrabalho |
| 5 | Canvas DAG visual (React Flow) | Torna o pipeline inspecionável e editável |
| 6 | Agente ReAct com budget de ferramentas | Traduz intenção em parâmetros |
| 7 | Validation Layer com limites tipados | Barreira contra alucinação |
| 8 | Proposta com consentimento explícito (HITL) | Compromisso de produto |
| 9 | Streaming de raciocínio via SSE | Transparência e percepção de velocidade |
| 10 | Persistência dual SQLite/PostgreSQL | Mesmo binário no laptop e na VPS |
| 11 | Escrita atômica + recovery loop no boot | Laptop desliga; job não se perde |
| 12 | Propagação de `trace_id` ponta a ponta | Suporte em minutos, não em horas |
| 13 | Golden Master acústico no CI | Atualização de modelo não muda o som |
| 14 | Version freeze na UI | Confiança do produtor profissional |
| 15 | Botão de escala local via Docker | Usa o hardware que a pessoa tem |

### Adiável (corta primeiro se o prazo apertar)

| Item | Impacto de adiar |
| --- | --- |
| Separação de stems | Alto valor, custo alto — depende de modelo externo (ADR-0010) |
| Autopilot (auto-escala por CPU) | Baixo — slider manual atende as personas P1/P2 |
| WASM para validação no cliente | Baixo — validação no servidor já protege |
| RabbitMQ / KEDA | Nulo no MVP — fila em Postgres atende até ~1000 jobs |
| Sandbox seccomp | Médio — relevante só quando houver upload de terceiros |
| Canary por tenant | Nulo antes de existir tenant pagante |

### Fora de escopo (não-metas explícitas)

- Não gera áudio sintético, não faz text-to-music, não clona voz.
- Não é DAW. Sem edição de sample individual, sem automação de envelope por
  ponto, sem gravação multipista.
- Não faz transcrição para partitura ou MIDI.
- Não sugere nem gerencia licenciamento/direitos autorais.
- Não tem loja, marketplace ou feed social.
- Não integra com Spotify/DistroKid no MVP.

## 6. Restrições que moldam a arquitetura

| Restrição | Consequência de projeto |
| --- | --- |
| Roda em laptop de 4 vCPU / 8 GB | Monolito modular, um binário, fila em memória/SQLite |
| Instalação sem conhecimento de infra | Zero-config por padrão; Docker é opcional |
| Áudio não pode vazar (P3) | LLM local suportado; nada de upload obrigatório |
| Deve virar SaaS sem reescrita | Ports & Adapters; `tenant_id` desde o dia 1 |
| WAV descompactado é grande | Streaming/`memmap`, nunca carregar 200 faixas em RAM |
| LLM é não-determinístico | Contrato tipado + Golden Master + version freeze |

## 7. Métricas de sucesso da V1

| Métrica | Alvo |
| --- | --- |
| Tempo do prompt ao WAV (faixa de 5 min, laptop 4 vCPU) | < 90 s (p95) |
| Renders com artefato audível (estalo, clipping) | 0 |
| Jobs perdidos após desligamento abrupto | 0 |
| Alucinações que chegam ao arquivo final | 0 |
| Distância de fingerprint entre renders da mesma receita | < 0,05 |
| Instalação até primeiro render (usuário novo) | < 10 min |
