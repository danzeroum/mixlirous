# 04 — Domínio e Motor DSP

Este documento é a especificação de implementação do `audio_core`. Cada
algoritmo traz: objetivo, referência no script Python original, fórmula,
equivalente em Rust e **invariante testável**.

---

## 1. Modelo de domínio

### Agregado raiz: `RemixJob`

Controla o ciclo de vida. É a única entidade cujo estado o banco persiste
diretamente para governar o fluxo.

```rust
pub struct RemixJob {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub track_id: Uuid,
    pub status: JobStatus,
    pub mode: JobMode,             // Manual | Assisted
    pub recipe: RemixRecipe,
    pub graph: Graph,
    pub artifact: Option<Artifact>,
    pub trace_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

### Entidade: `SourceTrack`

Não guarda áudio. Guarda o que a análise derivou.

```rust
pub struct SourceTrack {
    pub id: Uuid,
    pub object_key: String,
    pub duration_sec: f32,
    pub sample_rate: u32,
    pub channels: u16,
    pub bpm: Option<f32>,
    pub bpm_confidence: Option<f32>,
    pub beat_grid: Vec<BeatCandidate>,
    pub energy_profile: Option<EnergyProfile>,
    pub sections: Vec<Section>,
}
```

### Entidade: `BeatBlock`

Unidade atômica de composição. Já existe no kit (`domain/block.rs`).

```rust
pub struct BeatBlock {
    pub id: Uuid,
    pub start_sample: usize,
    pub end_sample: usize,
    pub start_time: f32,
    pub end_time: f32,
    pub duration: f32,
    pub rms_energy: f32,
    pub spectral_centroid: f32,
    pub chroma_vector: Option<Vec<f32>>,
    pub beat_index: usize,
    pub score: f32,
    pub starts_on_strong_beat: bool,   // ▲ adicionar
}
```

### Objetos de valor

```rust
pub struct TimeSpan { pub start_ms: u32, pub end_ms: u32 }
// Invariante: end_ms > start_ms. Construtor retorna Result.

pub struct TargetDuration { pub target_sec: f32, pub tolerance_sec: f32 }
// Invariante: target_sec > 0, tolerance_sec >= 0.

pub struct Parameter<T> { pub value: T, pub source: ParameterSource,
                          pub confidence: Option<f32> }
```

### Tipos com limite embutido (newtype pattern)

Esta é a barreira contra alucinação. Um `u32` qualquer não serve.

```rust
#[derive(Serialize)]
pub struct CrossfadeMs(u32);

impl CrossfadeMs {
    pub const MAX: u32 = 3000;
    pub fn new(v: u32) -> Result<Self, DomainError> {
        if v > Self::MAX { return Err(DomainError::OutOfBounds { field: "crossfade_ms", max: Self::MAX as f64, got: v as f64 }); }
        Ok(Self(v))
    }
    pub fn get(&self) -> u32 { self.0 }
}

impl<'de> Deserialize<'de> for CrossfadeMs {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = u32::deserialize(d)?;
        Self::new(v).map_err(serde::de::Error::custom)
    }
}
```

O ponto: a validação acontece na **desserialização**. Não existe um
`CrossfadeMs` inválido em memória, então nenhuma função DSP precisa checar de
novo. Um JSON alucinado com `50000` falha antes de tocar no áudio.

Aplicar o mesmo padrão em: `CompressionRatio`, `ThresholdDb`, `AttackMs`,
`ReleaseMs`, `EqGainDb`, `TimeStretchFactor`, `LufsTarget`, `BlockSizeBeats`,
`Percentile`.

### Serviços de domínio

| Serviço | Entrada | Saída | Regra principal |
| --- | --- | --- | --- |
| `BlockSegmentationService` | `SourceTrack`, `block_size_beats` | `Vec<BeatBlock>` | Cortes só em batidas da grade |
| `KnapsackSelectionService` | `Vec<BeatBlock>`, `TargetDuration` | `Vec<BeatBlock>` | Maximiza energia sem estourar; primeiro bloco em batida forte |
| `AudioStitchingPolicy` | dois blocos vizinhos | `CrossfadeParams` | Diferença de dBFS > limite → crossfade mais longo + aviso |
| `StructurePreservationService` | `Vec<BeatBlock>`, intro/outro ms | `Vec<BeatBlock>` | Pontas fixas fora da otimização |

---

## 2. Pipeline de processamento

```
[decode] → [analyze] → [segment] → [select] → [stitch] → [master] → [encode]
   A          B            C          D          E          F          G
```

Cada etapa emite `job.progress` com o `stage` correspondente.

---

## 3. Etapa A — Decodificação e normalização

**Objetivo:** transformar qualquer formato em `Array1<f32>` mono ou estéreo
intercalado, a uma taxa conhecida.

**Origem Python:** `ffmpeg` via subprocess + `librosa.load`.
**Rust:** `symphonia` (decode) + `rubato` (resample se necessário).

```rust
pub fn decode_to_pcm(bytes: &[u8], target_sr: u32)
    -> Result<(Array1<f32>, AudioMeta), Error>
```

Requisitos:

1. Validar **magic bytes** antes de decodificar (`RIFF`, `ID3`, `fLaC`, `OggS`).
   Não confiar na extensão nem no `Content-Type`.
2. Recusar arquivos acima do limite configurado (`audio.max_input_mb`).
3. Para análise, converter para **mono** (média dos canais) — reduz custo pela
   metade e não prejudica detecção de batida.
4. Para render, preservar os canais originais.
5. Faixas longas: processar por *chunks*, nunca `read_to_end` de 500 MB.

**Invariantes:**

- `decode_to_pcm` nunca entra em pânico com entrada arbitrária → validar com
  `cargo-fuzz` sobre bytes aleatórios.
- Amostras sempre em `[-1.0, 1.0]` após normalização de bit depth.

---

## 4. Etapa B — Análise

### B.1 Onset strength

**Origem:** `librosa.onset.onset_strength`.
**Kit:** `dsp/analysis/beat_tracking.rs::onset_strength` — já implementado com
derivada de RMS. Funciona, mas é a versão fraca.

Fórmula atual (energia):

$$\text{onset}[n] = \max(0,\ \text{RMS}[n] - \text{RMS}[n-1])$$

**Recomendação de upgrade — flux espectral:**

$$\text{onset}[n] = \sum_{k=0}^{N/2} \max\big(0,\ |X_n[k]| - |X_{n-1}[k]|\big)$$

Por que trocar: RMS detecta mudança de volume; flux espectral detecta mudança de
*conteúdo*. Em jam com bateria constante e riff entrando, o RMS quase não mexe e
o flux acusa. Custa uma FFT por quadro, que já será calculada para o chroma.

**Parâmetros padrão:** `frame_size = 2048`, `hop_size = 512`, janela de Hann.

### B.2 Detecção de BPM e grade de batidas

**Origem:** `librosa.beat.beat_track`.

Algoritmo:

1. Autocorrelação (ou FFT) do envelope de onset → período dominante entre
   `min_bpm = 60` e `max_bpm = 200`.
2. Programação dinâmica sobre os picos, penalizando desvio do período estimado:

   $$\text{score}(n) = \text{onset}[n] + \max_m \big[\text{score}(m) - \lambda \cdot \big(\log \tfrac{n-m}{P}\big)^2\big]$$

3. Backtracking do melhor caminho → `beat_grid`.

**Saída:** `Vec<BeatCandidate { sample_idx, time_sec, onset_strength }>` + `bpm`
+ `bpm_confidence` (razão entre o pico da autocorrelação e a média).

**Invariantes:**

- `beat_grid` estritamente crescente em `time_sec`.
- Intervalo entre batidas consecutivas dentro de ±35% do período mediano
  (tolerância para variação humana de tempo).
- Com `bpm_confidence < 0.5`, o sistema **avisa** que a faixa tem tempo instável
  e sugere modo contínuo (§6.2) em vez de colagem por blocos.

### B.3 Batidas fortes (percentil)

**Origem:** `np.percentile(beat_strength, 80)`.

```rust
pub fn strong_beat_threshold(beats: &[BeatCandidate], percentile: f32) -> f32
```

Usar interpolação linear entre posições, não índice truncado — o resultado
precisa ser estável com poucas batidas.

**Invariante:** com `percentile = 0.8`, entre 15% e 25% das batidas são
classificadas como fortes em material real.

### B.4 Perfil de energia (RMS)

$$\text{RMS} = \sqrt{\frac{1}{N}\sum_{i=0}^{N-1} x_i^2}$$

**Kit:** `dsp/analysis/rms.rs` — já existe.

**Invariante:** `rms(sinal_silencioso) == 0.0`;
`rms(onda_quadrada_amplitude_1) ≈ 1.0`; `rms(seno_amplitude_1) ≈ 0.7071`.

### B.5 Chroma (perfil harmônico) e seções

**Origem:** `librosa.feature.chroma_cqt` + `AgglomerativeClustering`.

Implementação pragmática em Rust (não precisa de CQT completa):

1. FFT por quadro → magnitude.
2. Mapear cada bin de frequência para classe de pitch:
   $$c = \Big\lfloor 12 \log_2\big(\tfrac{f}{440}\big) \Big\rfloor \bmod 12$$
3. Somar magnitudes por classe → vetor de 12 valores, normalizado (L2).
4. Matriz de similaridade: produto escalar entre vetores normalizados.
5. Segmentação: detectar blocos diagonais na matriz (novelty curve + picos), em
   vez de clustering aglomerativo completo — mais simples e suficiente.

**Uso no produto:** rotular seções (`intro`, `A`, `chorus`, `outro`) e detectar
repetições, para o agente poder dizer "priorize o refrão".

**Invariante:** vetor de chroma de um seno puro em A4 (440 Hz) concentra > 80%
da energia na classe 9 (A).

---

## 5. Etapa C — Segmentação em blocos

**Kit:** `domain/block.rs::build_beat_blocks` — implementado, precisa de ajuste.

Regras:

1. Agrupa batidas em janelas de `block_size_beats` (4, 8 ou 16).
2. Bloco só é válido se `starts_on_strong_beat == true` **quando** for candidato a
   primeiro bloco do remix.
3. Calcula `rms_energy`, `spectral_centroid` e `chroma_vector` por bloco.
4. `score` inicial = `rms_energy` normalizado; refinado pela prioridade escolhida.

**Prioridades de score** (vindas da receita):

| `priority` | Fórmula do score |
| --- | --- |
| `energy` (padrão) | `rms_norm` |
| `onset` | `0.4 · rms_norm + 0.6 · onset_norm` |
| `brightness` | `0.5 · rms_norm + 0.5 · centroid_norm` |
| `chorus` | `rms_norm · (1 + 0.5 · repeat_count)` |

Normalização sempre min-max dentro da faixa, para o score ficar em `[0,1]`.

**Invariantes:**

- Blocos não se sobrepõem: `blocks[i].end_sample <= blocks[i+1].start_sample`.
- Nenhum bloco tem `duration <= 0`.
- Somatório das durações ≤ duração da faixa.

---

## 6. Etapa D — Seleção

### 6.1 Modo colagem (knapsack)

**Origem:** ordenação por score + acumulação até `DUR_ALVO`.

O problema é uma mochila 0/1 com peso = duração e valor = score. Duração alvo é
pequena (15–60 s) e blocos são poucos (dezenas a centenas), então:

- **≤ 500 blocos:** programação dinâmica exata com peso discretizado em passos
  de 10 ms. Custo trivial.
- **> 500 blocos:** heurística gulosa por razão `score/duração` + busca local.

Restrições adicionais, aplicadas **antes** da otimização:

1. Reservar `preserve_intro_ms` e `preserve_outro_ms` do orçamento de tempo.
2. O primeiro bloco do corpo **precisa** ter `starts_on_strong_beat == true`.
3. Blocos selecionados são remontados em **ordem cronológica original**, não em
   ordem de score. Isso preserva a progressão harmônica.
4. Opcional (`allow_repeats: false` por padrão): não repetir o mesmo bloco.

**Invariantes:**

- `|duração_final − target_sec| <= tolerance_sec`, ou a função retorna
  `Err(SelectionError::CannotMeetTarget)` — nunca entrega silenciosamente fora.
- Seleção é **determinística**: mesma entrada, mesma saída. Empate de score
  resolve pelo `beat_index` menor.

### 6.2 Modo contínuo (melhor janela única)

**Origem:** janela deslizante somando RMS médio.

Para faixas com tempo instável ou quando o usuário quer um trecho sem cortes:
varre janelas de duração alvo e escolhe a de maior energia média, ajustando as
bordas para a batida forte mais próxima.

Custo: O(n) com soma acumulada (*prefix sum*), não O(n·m).

---

## 7. Etapa E — Emenda (stitching)

Esta é a etapa onde erro é **audível**. Prioridade máxima de teste.

### 7.1 Ajuste para cruzamento por zero

**Origem:** varredura procurando `x[i-1] <= 0 < x[i]`.
**Kit:** `dsp/stitching/zero_cross.rs` — existe.

```rust
pub fn snap_to_zero_crossing(pcm: &[f32], idx: usize, window: usize,
                             dir: SearchDir) -> usize
```

Busca o cruzamento mais próximo dentro de `window` (padrão: 10 ms). Se não
encontrar, retorna `idx` original — e o crossfade cobre o problema.

**Invariante:** o índice retornado está dentro de `[idx - window, idx + window]` e
sempre dentro dos limites do buffer.

### 7.2 Curvas de fade

**Kit:** `dsp/stitching/fades.rs` — existe.

| Curva | Ganho em `t ∈ [0,1]` | Uso |
| --- | --- | --- |
| Linear | `t` | Emendas muito curtas (< 50 ms) |
| Logarítmica (padrão) | `t^{1/2.2}` ou `10^{(1-t)·(-40)/20}` | Transições percebidas como suaves |
| Exponencial | `t²` | Fade-out de final de faixa |

Por que logarítmica é o padrão: a audição percebe ganho em escala aproximadamente
logarítmica. Fade linear soa como "sumiu rápido demais no fim".

### 7.3 Crossfade

**Kit:** `dsp/stitching/crossfade.rs` — existe.

$$y[n] = a[n]\cdot g_{\text{out}}(t) + b[n]\cdot g_{\text{in}}(t),\quad t = \frac{n}{L}$$

Com curvas complementares: $g_{\text{out}}(t) = g_{\text{in}}(1-t)$.

**Política de emenda (`AudioStitchingPolicy`):**

```
diff_db = |dBFS(fim de A) − dBFS(início de B)|

diff_db <= 3   →  crossfade padrão da receita
3 < diff <= 8  →  crossfade × 1,5 (limitado ao máximo)
diff_db > 8    →  crossfade máximo + emite warning "encaixe brusco"
```

O warning vai para o job como `warnings: [...]` e aparece na UI. Não bloqueia.

**Invariantes (testes obrigatórios):**

1. **Sem clipping:** `peak(crossfade(a,b)) <= max(peak(a), peak(b)) + 1e-6`.
2. **Continuidade:** a diferença máxima entre amostras consecutivas na região da
   emenda não excede o máximo observado dentro de cada bloco isolado × 1,5.
   *(É o teste que pega estalo.)*
3. **Preservação de duração:** `len(out) == len(a) + len(b) − L`.
4. `crossfade` com `L = 0` é concatenação simples.

### 7.4 Corte de cauda (tail trimming)

**Origem:** `encontrar_ponto_de_corte` — mede dBFS em blocos de 100 ms nos
últimos segundos e escolhe o mínimo.

Aplicado no final do render para evitar cortar em meio a um acorde sustentado.

---

## 8. Etapa F — Masterização

Ordem fixa da cadeia. Não é negociável:

```
compressor → (opcional) EQ dinâmico → limiter → normalização LUFS
```

Normalizar antes de limitar produz resultado imprevisível.

### 8.1 Compressor

**Kit:** não existe ainda (`mastering/compressor.rs` a criar).

```rust
pub struct CompressorParams {
    pub threshold_db: ThresholdDb,   // -60..0
    pub ratio: CompressionRatio,     // 1.0..10.0
    pub attack_ms: AttackMs,         // 0..500
    pub release_ms: ReleaseMs,       // 10..5000
    pub makeup_gain_db: f32,         // -12..+12
    pub knee_db: f32,                // 0..12, padrão 6 (soft knee)
}
```

Cálculo de redução de ganho por amostra, com detector RMS e envelope suavizado:

```
level_db = 20·log10(|x| + ε)
if level_db > threshold:
    reduction = (level_db − threshold) · (1 − 1/ratio)
else:
    reduction = 0
envelope = smooth(reduction, attack, release)   // one-pole
y = x · 10^(−envelope/20) · 10^(makeup/20)
```

**Invariante crítica (property test com 10.000 casos):**

$$\text{peak}(\text{comprimido}) \le \text{peak}(\text{original}) + \varepsilon$$

quando `makeup_gain_db <= 0`. É o teste que impede o motor de estourar o ouvido
de alguém.

Invariante adicional: `ratio = 1.0` é identidade (a menos do makeup gain).

### 8.2 Limiter

**Kit:** `mastering/limiter.rs` — existe, verificar implementação.

Brickwall com *lookahead* de 5 ms. Teto padrão −1,0 dBTP.

**Invariante:** nenhuma amostra de saída excede o teto configurado. Testar com
sinal deliberadamente acima de 0 dBFS.

### 8.3 Normalização LUFS

**Kit:** `mastering/lufs.rs` — existe, usar `ebur128`.

1. Medir loudness integrado (ITU-R BS.1770-4).
2. Calcular ganho: `gain_db = target_lufs − measured_lufs`.
3. Aplicar ganho.
4. Se o pico resultante estourar o teto, o limiter absorve; se a redução
   necessária for > 3 dB, emitir warning (sinal muito comprimido na origem).

Alvos por plataforma (a UI oferece como preset):

| Preset | LUFS | True peak |
| --- | --- | --- |
| Streaming / Reels / TikTok | −14 | −1,0 |
| YouTube | −14 | −1,0 |
| Rádio / broadcast | −23 (EBU R128) | −1,0 |
| Clube / DJ | −9 | −0,3 |

**Invariante:** após normalização, `|lufs_medido − target| <= 0.5 LU`.

### 8.4 Time-stretch

**Kit:** `mastering/stretch.rs` — existe, usar `rubato`.

Usado apenas para o ajuste final fino de duração, quando a colagem não fecha
exatamente no alvo. Fator limitado a `[0.90, 1.10]` — acima disso o artefato é
audível e é melhor recolar os blocos.

**Invariante:** `|duração_saída − duração_alvo| < 20 ms`.

---

## 9. Etapa G — Fingerprint acústico

**Kit:** `domain/fingerprint.rs` — estrutura pronta, MFCC é placeholder
(`vec![0.0; 13]`). **Precisa de implementação real** ou o Golden Master não
detecta nada.

MFCC, passo a passo:

1. FFT por quadro → espectro de potência.
2. Banco de 26 filtros triangulares em escala Mel:
   $$m = 2595 \log_{10}\Big(1 + \tfrac{f}{700}\Big)$$
3. `log` da energia de cada filtro.
4. DCT-II → manter os coeficientes 1–13 (descartar o 0, que é energia global).

Demais features: `spectral_centroid`, `rms_energy`, `spectral_contrast` (7
bandas), `peak_ratio`, `lufs`.

Distância ponderada (já no kit):

$$d = \frac{2\,d_{\text{MFCC}} + 1{,}5\,d_{\text{centroid}} + d_{\text{RMS}}}{4{,}5}$$

**Atenção:** `d_centroid` está em Hz e `d_rms` em amplitude — escalas
incomparáveis. **Normalizar cada componente antes de somar**, senão o centroide
domina a distância inteira. Corrigir na Sprint 4:

```
d_centroid_norm = |c1 − c2| / max(c1, c2, 1.0)
d_rms_norm      = |r1 − r2| / max(r1, r2, 1e-6)
d_mfcc_norm     = euclidean(m1, m2) / sqrt(13)
```

**Invariantes:** `d(x, x) == 0`; `d` simétrica; `d` no intervalo `[0, ~2]`.

---

## 10. Mapa: script Python → módulo Rust

| Script original | Lógica extraída | Destino em `audio_core` |
| --- | --- | --- |
| `cortarMusica.py` | Similaridade cromática, detecção de refrão | `dsp/analysis/chroma.rs` |
| `Remix Turbo Beat-Locked.py` | Beat tracking, percentil P80, grid | `dsp/analysis/beat_tracking.rs` |
| `remix_colagem.py` | Fatiamento por N batidas, score RMS | `domain/block.rs` + `dsp/selection/` |
| `remix_colagem2.py` / `_v4.py` | Ajuste fino de duração, corte parcial | `dsp/selection/knapsack.rs` |
| `remixador_gui_s1.py` | Zero-crossing, fades logarítmicos | `dsp/stitching/` |
| `remixador_gui_s2.py` | Segmentação estrutural, intro/outro fixos | `dsp/analysis/sections.rs` |
| todos | LUFS, compressão, limiter | `dsp/mastering/` |

### Substituição de dependências

| Python | Rust | Observação |
| --- | --- | --- |
| `librosa.load` / `pydub` / `ffmpeg` | `symphonia` | Decode nativo, sem subprocess |
| `numpy` / `scipy` | `ndarray` | Vetorizado, sem alocação extra |
| `librosa.stft` | `realfft` | FFT real, ~2× mais rápida que complexa |
| `pyloudnorm` | `ebur128` | Implementa BS.1770-4 |
| `pyrubberband` | `rubato` | Resample e stretch em Rust puro |
| `sklearn.cluster` | implementação própria | Novelty curve + picos; evita dependência pesada |

> Não existe equivalente maduro em Rust puro para separação de stems
> (`demucs`/`spleeter`). Ver ADR-0010.

---

## 11. Orçamento de performance

Faixa de 5 minutos, 44,1 kHz estéreo, laptop de 4 vCPU:

| Etapa | Alvo | Observação |
| --- | --- | --- |
| Decode | < 2 s | I/O-bound |
| Onset + beat grid | < 4 s | Paraleliza por chunk com Rayon |
| Chroma + seções | < 6 s | Mais caro; cachear no `SourceTrack` |
| Blocos + energia | < 1 s | |
| Seleção (knapsack) | < 100 ms | |
| Stitching | < 1 s | |
| Masterização | < 2 s | |
| Encode + escrita | < 2 s | |
| **Total (sem LLM)** | **< 20 s** | |
| Chamada LLM | 2–8 s | Fora do controle; streaming disfarça |

A análise (etapas B) é **cacheada por faixa**. O segundo remix da mesma faixa
custa < 6 s no total. Isso é o que torna viável processar as 200 faixas.

Benchmarks com `criterion` na Sprint 2, com regressão de performance no CI:
falha se p95 piorar mais de 20% em relação à baseline.
