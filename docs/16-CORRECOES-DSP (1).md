# 16 — Correções de DSP: qualidade do áudio de saída

**Pacote de trabalho.** Para ser executado **depois** da demanda atual, com o
workspace compilando e o CI verde.

**Duração estimada:** 6 a 7 dias.
**Pré-requisitos:**

1. Sprint 0 concluída (`cargo build`, `cargo test`, `cargo clippy -- -D warnings`
   e `npm run build` passando).
2. **T0.0 concluído** — validação nos newtypes (I14). Metade das correções aqui
   pressupõe que os limites já estão garantidos no tipo. Sem isso, o pacote
   constrói sobre fundação que ele mesmo assume existir.

**Versão 2** — incorpora o I14 como pré-requisito e converte o Bloco 1 para
testes baseados em propriedade. Motivo das duas mudanças na §11.

---

## Errata — leia antes de qualquer coisa

O documento `15-AUDACITY-TRIAGEM.md`, §5, item 6, prescreve a ordem
*"medir → limitar picos → normalizar LUFS"*.

**Essa linha está errada e este documento a substitui.** Normalizar o LUFS depois
de limitar reintroduz exatamente o bug que a ordem pretendia corrigir: o ganho
positivo da normalização empurra os picos de volta acima do teto. A ordem correta
está na §4 aqui.

Se você recebeu documentos externos validando aquela ordem — ou uma variante
como *"Limiter → Medir LUFS → Make-up gain"* — ignore-os neste ponto. O erro
passou por três rodadas de revisão sem ser pego. É o motivo de a §2 deste
pacote vir antes da §3.

---

## 1. Bloco 0 — Antes de tocar em código (bloqueante)

### T0.0 — Validação nos newtypes, não só na camada (I14)

**Faça isto antes de tudo. É o único item deste pacote que muda fundação.**

Hoje os parâmetros são validados apenas na `ValidationLayer`. O `04-DOMINIO-DSP.md`
prescreve validação **na desserialização do newtype**. A diferença não é
estilística.

Enquanto a validação viver numa camada, todo caminho de construção que não passe
por ela produz objeto de domínio inválido:

- recovery lendo um job do banco depois de um crash,
- teste montando a struct direto,
- um provedor LLM futuro com formato próprio,
- qualquer rota nova que alguém escreva daqui a três meses.

O newtype existe para tornar o estado inválido **irrepresentável**. Se
`CrossfadeMs` só pode existir com valor entre 0 e 3000, nenhum código do sistema
precisa lembrar de checar — e nenhum código pode esquecer.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct CrossfadeMs(u32);

impl CrossfadeMs {
    pub const MIN: u32 = 0;
    pub const MAX: u32 = 3000;

    pub fn new(v: u32) -> Result<Self, DomainError> {
        (Self::MIN..=Self::MAX).contains(&v)
            .then_some(Self(v))
            .ok_or(DomainError::ForaDoLimite { campo: "crossfade_ms", valor: v.into() })
    }

    pub fn get(self) -> u32 { self.0 }
}

// O ponto: desserializar passa obrigatoriamente pelo construtor.
impl<'de> Deserialize<'de> for CrossfadeMs {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = u32::deserialize(d)?;
        Self::new(v).map_err(serde::de::Error::custom)
    }
}
```

Sem campo público, sem `From<u32>` infalível, sem construtor alternativo. Se
existir uma porta dos fundos, ela vai ser usada.

**Fonte dos limites:** o registry `audio_agent::limits` que você criou na Sprint
0. Os newtypes derivam dele — não redigitam os números. Isso fecha o terceiro
lugar da regra do `CONTRIBUTING.md`.

Aproveite para estender o teste de divergência do registry para validar também a
tabela canônica de `05-AGENTE-IA-HITL.md` §3, lendo o markdown. Aí os três
lugares passam a ser um só com duas projeções verificadas.

**A `ValidationLayer` continua existindo** — ela dá erro legível e agregado para
o agente, o que um erro de desserialização não dá. Mas deixa de ser a única
defesa.

### T0.1 — Criar ADR-0011: política de propriedade intelectual

`docs/adr/README.md`, status **aceito**, com dono.

Conteúdo mínimo:

- O Audacity é GPL (GPLv3 no projeto, GPLv2-or-later na maioria dos arquivos).
  Copiar ou **traduzir** qualquer arquivo de `au3/` para Rust cria obra derivada
  e propaga o copyleft para o Mixlirous inteiro, incluindo o binário distribuído.
- **Proibido:** abrir o repositório do Audacity durante a implementação de um
  módulo de DSP equivalente. Ler e reescrever "com as próprias palavras" logo
  em seguida conta como reprodução.
- **Permitido:** implementar a partir de literatura publicada (DAFX/Zölzer, EBU
  TECH 3341/3342, ITU-R BS.1770, papers) e usar crates com licença permissiva
  (MIT, BSD, Apache-2.0).
- **Obrigatório:** todo módulo de DSP novo cita a fonte no cabeçalho.

> **Sobre as citações.** A política inteira depende de as referências serem
> verdadeiras. Um cabeçalho apontando para uma fonte que não diz aquilo é pior
> que cabeçalho nenhum — é documentação falsa de proveniência. **Confira cada
> citação contra um exemplar antes do commit.**
>
> Já conferido: DAFX 2ª ed. **§4.2 "Dynamic Range Control"** cobre limiters e
> compressores com a arquitetura envelope follower → curva estática → filtro de
> suavização → multiplicador, com atraso opcional do sinal para compensar o side
> chain. É a referência correta para o T3.2.
>
> **Não confirmado:** a referência *"DAFX 2ª ed., Equal Power Crossfade, p. 46"*,
> que circulou em documentos externos, não parece existir — a página 46 cai no
> capítulo de filtros. Crossfade de potência constante é resultado elementar e
> pode ser citado de qualquer texto de processamento de sinais de áudio. **Não
> copie essa citação sem verificar.**

### T0.2 — Adicionar o invariante I15

Em `docs/10-TESTES-QUALIDADE.md`:

> **I15 — Finitude.** Nenhuma amostra de nenhum buffer entregue a uma etapa
> seguinte, nem do WAV final, pode ser `NaN` ou infinita.

Parece óbvio, e é justamente por isso que ninguém escreve. Este invariante
sozinho pegaria o B1 no primeiro render.

### T0.3 — Estender o invariante I4

> **I4.1 — Continuidade.** A emenda não apresenta descontinuidade de amplitude
> além do limiar já definido.
>
> **I4.2 — Potência na emenda.** O RMS em janela deslizante sobre a região de
> crossfade não cai mais de **1 dB** em relação à média das janelas adjacentes
> dos dois blocos.

---

## 2. Bloco 1 — Escrever os testes que falham (faça isto primeiro)

Este bloco não corrige nada. Ele produz falhas vermelhas que descrevem cada bug.
Só depois vêm as correções.

A razão é concreta: o erro de ordenação da cadeia (veja a Errata) sobreviveu a
três rodadas de revisão em texto. Um teste teria pego em segundos. **Revisão em
prosa não é verificação.**

### Por que baseado em propriedade, e não caso fixo

O levantamento de cobertura da Sprint 0 mostrou 5 de 14 invariantes com teste,
**todos de caso fixo**. Caso fixo cobre um ponto do espaço de entrada. Em DSP os
bugs não moram no meio do espaço — moram nas bordas: buffer de uma amostra, sinal
DC puro, silêncio absoluto, amplitude exatamente no limite, `alpha` exatamente
0,0 ou 1,0, comprimento de fade maior que o buffer.

Os bugs B1 a B6 ilustram: a divisão por zero da curva logarítmica dispara com
**qualquer** entrada, e mesmo assim ninguém escreveu o caso que a exercitava. Um
gerador teria achado na primeira execução.

Adicione ao `audio_core`:

```toml
[dev-dependencies]
proptest = "1"
```

**Fixe a versão exata das dependências de DSP** (`=x.y.z`), não faixas. O rubato
publicou quatro versões maiores em sete meses; a Sprint 0 já perdeu tempo com uma
API que mudou embaixo. Atualização de dependência de áudio é tarefa agendada,
com escuta, não efeito colateral de um `cargo update`.

### T1.1 — Geradores de sinal

`crates/audio_core/tests/generators.rs`. Gerados em código, nunca arquivos de
áudio — atendem a `09-MLOPS-GOLDEN-MASTER.md` sem questão de licença e são
reprodutíveis por semente.

| Gerador | Faixa | Para quê |
|---|---|---|
| `arb_pcm()` | comprimento 0..=192000, amostras −1,0..=1,0 | Caso geral; inclui vazio e 1 amostra |
| `arb_sine()` | 20 Hz..20 kHz, amplitude 0,0..1,0 | Verificação analítica de ganho e LUFS |
| `arb_noise()` | semente arbitrária | Material não correlacionado para emenda |
| `arb_degenerate()` | silêncio, DC, ±1,0 constante, uma amostra | As bordas onde os bugs moram |
| `arb_transient()` | cliques esparsos com fundo baixo | Pico alto e loudness baixa — força o conflito da §4 |

Mantenha `arb_degenerate()` como estratégia própria e sorteada com peso alto.
Geradores uniformes quase nunca produzem silêncio puro por acaso.

### T1.2 — Propriedades da cadeia de masterização

```rust
proptest! {
    #[test]
    fn cadeia_respeita_teto_e_alvo(pcm in arb_pcm(), sr in prop::sample::select(vec![44100u32, 48000])) {
        let r = masterizar(&pcm, sr, Lufs::new(-14.0)?, DbTp::new(-1.0)?);

        // I15 — vale sempre, sem exceção
        prop_assert!(r.pcm.iter().all(|s| s.is_finite()));

        // O teto é inviolável
        prop_assert!(r.true_peak_dbtp <= -1.0 + TOL_DBTP);

        // O alvo de loudness é atingido OU o conflito é reportado — nunca em silêncio
        prop_assert!(
            (r.lufs - (-14.0)).abs() <= 0.5 || r.avisos.contains(&Aviso::ConflitoDeAlvos)
        );
    }
}
```

**Esta propriedade deve falhar hoje.** A implementação atual não satisfaz as duas
últimas condições ao mesmo tempo.

Note a forma da terceira: ela não exige que o sistema acerte sempre — exige que
ele **nunca erre calado**. É a asserção que codifica a tese do produto.

### T1.3 — Propriedades de emenda

```rust
proptest! {
    #[test]
    fn emenda_preserva_potencia_e_fase(
        a in arb_noise(), b in arb_noise(),
        fade_ms in 1u32..=3000,
        curva in prop::sample::select(FadeCurve::todas()),
    ) {
        let r = crossfade(&a, &b, fade_ms, curva);

        prop_assert!(r.iter().all(|s| s.is_finite()));                 // I15 — pega B1
        prop_assert!(descontinuidade_max(&r) <= LIMIAR_I4);            // I4.1
        prop_assert!(queda_rms_db(&r, emenda) <= 1.0);                 // I4.2 — pega B3
        prop_assert!(correlacao(&r[..], &a[..]) > 0.0);                // pega B2 (fase)
    }
}
```

A última asserção é o detector de inversão de fase: se a curva inverte o sinal de
A, a correlação com o material original fica negativa.

**Propriedade separada, e a mais forte do conjunto:**

```rust
// Emendar um bloco a si mesmo, com qualquer curva de ganho constante,
// tem que devolver o próprio bloco. Se não devolve, a curva está errada.
prop_assert!(aprox_igual(&crossfade(&x, &x, fade, ConstantGain), &x, EPS));
```

Isso é uma identidade matemática, não um valor medido. Ela não precisa ser
recalibrada quando o motor mudar.

### T1.4 — Propriedades de zero-crossing

```rust
proptest! {
    #[test]
    fn zero_crossing_e_o_mais_proximo(pcm in arb_pcm(), alvo in any::<usize>(), janela in 1usize..=4410) {
        prop_assume!(!pcm.is_empty() && alvo < pcm.len());
        let r = find_zero_crossing(&pcm, alvo, janela);

        // Nunca faz panic — a asserção é o teste terminar
        if let Some(i) = r {
            prop_assert!(cruza_subindo(&pcm, i));                     // devolve cruzamento de verdade
            prop_assert!(i.abs_diff(alvo) <= janela);                 // dentro da janela
            // e nenhum cruzamento válido está mais perto que o devolvido
            prop_assert!(nenhum_cruzamento_mais_proximo(&pcm, alvo, i, janela));
        }
    }
}
```

A terceira asserção é a que pega o B6. As duas primeiras sozinhas passariam com a
implementação atual, que devolve um cruzamento legítimo — só que o errado.

### T1.5 — Casos fixos que continuam valendo

Propriedade não substitui tudo. Mantenha caso fixo onde existe **valor esperado
analítico**:

- RMS de seno unitário ≈ 0,7071 (I13, já existe)
- LUFS de seno a −20 dBFS, valor conhecido
- Um caso de regressão para cada bug encontrado, com o valor exato que falhava

Quando o `proptest` achar uma falha, ele minimiza o caso. **Congele esse caso
minimizado como teste fixo**, com o número da issue no nome. É assim que a suíte
fica mais forte a cada bug, em vez de só mais longa.

---

## 3. Bloco 2 — Emenda (`dsp/stitching/`)

### T2.1 — Remover as curvas quebradas (B1, B2)

`fades.rs` e `crossfade.rs`.

- `FadeCurve::Logarithmic` faz `.ln() / 1.0f32.ln()` → divisão por zero → toda a
  região vira `NaN`/`inf`.
- `FadeCurve::Exponential` usa `alpha.exp2()`, que vai de 1,0 a 2,0 em vez de 0 a
  1 — inverte a fase de A e adiciona 6 dB.

**Remova as duas variantes do enum.** Não as mantenha mapeando para o
comportamento linear num braço `_ =>`.

Isso é importante e não é preciosismo: um `_ =>` num match de domínio desliga a
checagem de exaustividade do compilador, que é metade do motivo de este projeto
ser em Rust. Removendo as variantes, o compilador aponta cada call site que
precisa de decisão. Mantendo-as com fallback, você troca um bug ruidoso por um
silencioso — o mesmo padrão que produziu B1 e B2.

Se as curvas voltarem depois, voltam com teste.

### T2.2 — Crossfade de potência constante (B3)

O crossfade linear mantém `gain_a + gain_b = 1`. Para sinais **não
correlacionados** — blocos de trechos diferentes, que é o caso do motor — o que
soma é a potência, não a amplitude. No meio da transição:

```
√(0,5² + 0,5²) = 0,707   →   queda de ~3 dB
```

Uma queda audível de volume em cada emenda. Correção:

```rust
//! Crossfade de potência constante: gain_a² + gain_b² = 1.
//! Referência: <preencher após verificar — ver T0.1>

use std::f32::consts::FRAC_PI_2;

pub enum FadeCurve {
    /// Ganho constante. Para material correlacionado (mesmo trecho sobreposto
    /// a si mesmo), onde a soma linear é a correta.
    ConstantGain,
    /// Potência constante. Padrão: os blocos vêm de trechos diferentes.
    ConstantPower,
}

pub fn compute_gains(alpha: f32, curve: &FadeCurve) -> (f32, f32) {
    let a = alpha.clamp(0.0, 1.0);
    match curve {
        FadeCurve::ConstantGain => (1.0 - a, a),
        FadeCurve::ConstantPower => {
            let angle = a * FRAC_PI_2;
            (angle.cos(), angle.sin())
        }
    }
}
```

Sem braço `_ =>`. Duas variantes, duas linhas, exaustivo.

> **Regra do `CONTRIBUTING.md`:** expor a curva como parâmetro exige atualizar
> **três lugares no mesmo PR** — a tabela canônica em `05-AGENTE-IA-HITL.md` §3,
> o validador em Rust, e o schema exposto à UI. O padrão é `ConstantPower`.

### T2.3 — Zero-crossing a partir do alvo (B6)

O docstring promete "o mais próximo do alvo". O código varre da borda esquerda
da janela e devolve o **primeiro** que encontrar — com janela de 50 ms, desloca o
corte em até 25 ms mesmo havendo cruzamento a 1 amostra do alvo. Isso é erro
rítmico audível.

Corrija buscando **do alvo para fora**, alternando os lados.

Duas armadilhas numa correção que circulou:

1. **Não clampe a janela simetricamente.** `max_offset.min(alvo).min(len - 1 -
   alvo)` reduz a busca dos dois lados ao menor espaço disponível — perto do
   início do buffer a janela colapsa mesmo havendo milhões de amostras à direita.
   Os limites são independentes por lado.
2. **Se mudar o retorno para `Option<usize>`,** defina a política de fallback em
   **um lugar só** (helper que devolve o alvo e sinaliza), não em cada call site.

Trate buffer vazio e alvo na borda: `pcm.len() - 1` estoura hoje.

---

## 4. Bloco 3 — Cadeia de masterização (`dsp/mastering/`)

### O problema

`apply_lufs_gain()` normaliza para −14 LUFS. Em seguida `brickwall_limiter()`
acha o pico global e multiplica **o buffer inteiro** por um ganho único para
levar o pico a −1 dBFS — o que derruba o LUFS junto. Os dois alvos que o produto
promete são inatingíveis simultaneamente.

Além disso, `limiter.rs` não é um limiter: é um normalizador de pico, sem
envelope, sem attack, sem release. E mede **pico de amostra**, não **pico real**.
Picos entre amostras podem exceder o pico de amostra em vários dB depois da
reconstrução D/A ou de uma codificação com perdas — que é exatamente o que o
Spotify faz. O teto de −1 dBTP prometido não é cumprido.

### T3.1 — Medição de true peak

A crate `ebur128` já é dependência e expõe `Mode::TRUE_PEAK`. Hoje `lufs.rs`
instancia só `Mode::I`. Custo da correção: uma linha.

Corrija também, no mesmo arquivo: o fallback silencioso para 44100 Hz quando a
construção falha produz medição errada em vez de erro. Deve retornar `Result`.

### T3.2 — Limiter com look-ahead

Referência: **DAFX 2ª ed. §4.2**, arquitetura envelope follower → curva estática
→ filtro de suavização → multiplicador, com linha de atraso no caminho do sinal
para compensar o side chain.

Estrutura:

```
                    ┌──────────────────────────────────┐
  entrada ─────────►│ side chain: sobreamostrar (4x),  │
      │             │ detectar pico, calcular ganho    │
      │             │ necessário, suavizar envelope    │
      │             └──────────────┬───────────────────┘
      │                            │ ganho(t)
      ▼                            ▼
  ┌────────────┐              ┌─────────┐
  │ atraso de  │─────────────►│    ×    │──────► saída
  │ look-ahead │              └─────────┘
  └────────────┘
```

Pontos que decidem se funciona:

- **O tempo de attack tem que ser ≤ o tempo de look-ahead.** Se for maior, o pico
  chega antes de a redução de ganho estar completa e o limiter não limita. É o
  erro mais comum nesta implementação.
- **Sobreamostre só o side chain**, não o sinal principal. Sobreamostrar o buffer
  inteiro estoura o orçamento de performance de `04-DOMINIO-DSP.md`.
- Release longo demais bombeia; curto demais distorce graves. Comece por
  valores conservadores e ajuste com escuta — não com número.

Meça o custo. O orçamento total é < 20 s sem LLM.

### T3.3 — Ordem correta e reporte de conflito (B4)

```
1. Medir LUFS integrado
2. Ganho estático  G = -14 - LUFS_medido      →  LUFS agora é exatamente -14
                                                 (ganho constante desloca o LUFS
                                                  exatamente pelo mesmo valor em dB)
3. Medir true peak
4. Se TP <= -1 dBTP  →  pronto. Não faça mais nada.
5. Se TP >  -1 dBTP  →  limiter com look-ahead, threshold em -1 dBTP.
                        Reduz só os picos; a média quase não se move.
6. Re-medir LUFS.
   Se |LUFS - (-14)| <= 0,5 LU  →  ok.
   Senão                        →  CONFLITO DE ALVOS.
```

**O passo 6 é o mais importante e é novo.** Existe material dinâmico demais para
atingir −14 LUFS e −1 dBTP ao mesmo tempo sem limitação audível. Nesse caso o
motor **não escolhe sozinho**. Ele:

- entrega o áudio priorizando o teto de pico (nunca estoure o teto),
- registra o desvio de loudness no resultado do job,
- emite um evento SSE de aviso,
- e passa o fato ao agente como observação, para que ele possa propor
  alternativa ao usuário.

Isso não é enfeite: é a tese do produto aplicada à masterização. *A IA propõe,
você decide* vale aqui também. Um motor que silenciosamente entrega −17 LUFS
quando prometeu −14 está mentindo para o usuário.

O contrato do evento e do campo de resultado precisa entrar em
`03-CONTRATOS-API.md` — código de erro/aviso novo, catálogo de eventos SSE
atualizado.

### T3.4 — Renomear

`brickwall_limiter()` → o nome deve descrever o que a função faz. Se ela for
substituída pelo limiter de verdade, o nome fica; se algum caminho ainda precisar
de normalização de pico estática, ela vira `normalize_peak()`.

Arquivo chamado `limiter.rs` contendo um normalizador é o tipo de divergência que
sobrevive a review porque ninguém abre para conferir.

---

## 5. Bloco 4 — Verificação

1. Todos os testes do Bloco 1 passam.
2. `cargo test --workspace` e `cargo clippy -- -D warnings` verdes.
3. **Escuta humana obrigatória** antes do merge, conforme
   `09-MLOPS-GOLDEN-MASTER.md`: renderize o mesmo material antes e depois e ouça
   as emendas e a masterização. Um teste de RMS não detecta bombeamento de
   limiter.
4. Gere novos Golden Masters — os antigos, se existirem, foram produzidos por uma
   cadeia com bugs e não servem de referência.

---

## 6. Escopo — o que **não** entra

Estes itens foram propostos por análises externas e estão descartados. Estão
listados para você não perder tempo reavaliando:

| Item | Motivo |
|---|---|
| Portar `EBUR128.cpp` do Audacity | Redundante (`ebur128` MIT já é dependência) e GPL |
| Grafo de áudio com ring buffers lock-free | Erro de categoria: somos renderizador offline em lote, não há callback de tempo real |
| Efeitos em tempo real não-destrutivos | Invalidaria fila, escrita atômica, recovery e Golden Master |
| Hospedar VST3 / LV2 | Licença do SDK + escopo de DAW |
| Marketplace de scripts WASM, DSL estilo Nyquist | Produto diferente |
| Multi-track, MIDI, colaboração CRDT | Não-metas explícitas em `00-VISAO-ESCOPO.md` |
| Envelope desenhável, automação, reparo espectral | Legítimos, mas depois de o motor renderizar o primeiro WAV correto |

---

## 7. Decisões de design pendentes (não são suas, mas te afetam)

| Decisão | Impacto em você | Prazo |
|---|---|---|
| **Espectrograma no overlay de proposta** | Se aprovado, o backend precisa serializar STFT — muda o contrato e o orçamento de performance | Antes de o designer fechar o layout |
| **Rótulos de seção editáveis** | Muda o contrato de `/sections`: renomear e filtrar blocos por seção | Antes do desenho da API de seções |
| **ADR-0009** — provedor LLM padrão | Bloqueia a Sprint 2 | Já vencido |
| **ADR-0010** — separação de stems | Bloqueia a Sprint 3 | Antes da Sprint 3 |

---

## 8. Issues para o backlog

| Tarefa | Labels | Milestone |
|---|---|---|
| **T0.0 Validação nos newtypes (I14)** | `area/domain`, `type/fix`, `prio/p0`, `pillar/validation` | S2 Motor DSP |
| T0.1 ADR-0011 política de PI | `area/docs`, `type/chore`, `prio/p0` | S2 Motor DSP |
| T0.2/T0.3 Invariantes I15 e I4.2 | `area/docs`, `type/test`, `prio/p0` | S2 Motor DSP |
| T1.1 Geradores de sinal (proptest) | `area/dsp`, `type/test`, `prio/p0` | S2 Motor DSP |
| T1.2 Propriedades da cadeia de masterização | `area/dsp`, `type/test`, `prio/p0` | S2 Motor DSP |
| T1.3 Propriedades de emenda | `area/dsp`, `type/test`, `prio/p0` | S2 Motor DSP |
| T1.4 Propriedades de zero-crossing | `area/dsp`, `type/test`, `prio/p1` | S2 Motor DSP |
| T1.5 Casos fixos com valor analítico | `area/dsp`, `type/test`, `prio/p1` | S2 Motor DSP |
| T2.1 Remover curvas quebradas | `area/dsp`, `type/fix`, `prio/p0` | S2 Motor DSP |
| T2.2 Crossfade de potência constante | `area/dsp`, `type/feat`, `prio/p0` | S2 Motor DSP |
| T2.3 Zero-crossing a partir do alvo | `area/dsp`, `type/fix`, `prio/p1` | S2 Motor DSP |
| T3.1 Medição de true peak | `area/dsp`, `type/fix`, `prio/p0` | S2 Motor DSP |
| T3.2 Limiter com look-ahead | `area/dsp`, `type/feat`, `prio/p0` | S2 Motor DSP |
| T3.3 Ordem da cadeia e conflito de alvos | `area/dsp`, `area/api`, `type/fix`, `prio/p0` | S2 Motor DSP |
| T3.4 Renomear limiter | `area/dsp`, `type/chore`, `prio/p2` | S2 Motor DSP |

---

## 9. Definição de pronto

- [ ] ADR-0011 aceito, com dono, e toda citação de fonte verificada contra exemplar
- [ ] T0.0 concluído: limites garantidos no newtype, derivados do registry; nenhum construtor alternativo público
- [ ] Teste de divergência do registry cobrindo os **três** lugares (validador, API, tabela de `docs/05` §3)
- [ ] Propriedades do Bloco 1 escritas **antes** das correções e falhando na primeira execução
- [ ] Cada falha minimizada pelo `proptest` congelada como caso fixo, nomeada com a issue
- [ ] Dependências de DSP fixadas em versão exata (`=x.y.z`)
- [ ] B1–B6 corrigidos; testes verdes
- [ ] Nenhum braço `_ =>` novo em match de tipo de domínio
- [ ] `03-CONTRATOS-API.md` atualizado com o aviso de conflito de alvos
- [ ] `05-AGENTE-IA-HITL.md` §3, validador e schema da UI alinhados quanto à curva de crossfade
- [ ] Escuta humana registrada no PR
- [ ] Golden Masters regenerados

---

## 10. Uma observação sobre método

Este pacote existe porque uma cadeia de análises produziu um erro que sobreviveu
a três rodadas de revisão — inclusive a rodadas que declararam o texto anterior
"impecável" e "100% correto".

Concordância não é verificação. Se você discordar de qualquer item aqui,
principalmente da §4, **diga**. Uma revisão que não produz nenhuma discordância
não revisou nada.

---

## 11. Por que esta versão mudou

Duas coisas motivaram a v2, e as duas vieram de verificação, não de revisão.

**O I14 virou pré-requisito** porque o levantamento de cobertura da Sprint 0
mostrou que a validação vive só na `ValidationLayer`. Metade das correções deste
pacote pressupõe limites garantidos no tipo. Construir o pacote antes seria
apoiá-lo numa suposição falsa.

**O Bloco 1 virou baseado em propriedade** porque o mesmo levantamento mostrou 5
de 14 invariantes cobertos, todos de caso fixo. Caso fixo teria deixado B1
passar: a divisão por zero disparava com qualquer entrada, e mesmo assim o teste
que a exercitava nunca foi escrito.

**E a política de fixar versão exata** veio de um erro de fato numa revisão
anterior: uma versão de crate foi afirmada com base em página de listagem
desatualizada e corrigida por quem foi ao registry ao vivo. O número errado era
sintoma; a causa era a crate publicar versões maiores a cada seis semanas.
