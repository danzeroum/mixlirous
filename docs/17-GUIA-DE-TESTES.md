# 17 — Guia de testes

**Ponto de entrada único.** Se você só vai ler um documento sobre testes, é este.

Responde três perguntas: **o que existe**, **como rodar tudo de uma vez**, e
**o que nenhum teste automatizado consegue provar**.

> **Mudou desde a versão anterior:** os caminhos eram `tests/` e agora são
> `fixtures/audio/`. As fixtures **não são comitadas** — são geradas por
> script. O harness da §5 existe agora (antes não existia nenhum) e as 35
> fixtures deixaram de ser arquivos inertes. Leia a §2 antes de procurar os
> arquivos no repositório.

---

## 1. Um comando

Hoje a verificação está espalhada em vários comandos que ninguém roda na
mesma ordem duas vezes. Criar `scripts/test-all.sh` e fazer dele o único
caminho ainda está pendente (§6, item 4) — o que existe hoje:

```bash
pip install -r scripts/requirements-fixtures.txt
python scripts/generate_fixtures.py --duration 5.0
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
cargo test --workspace
```

As fixtures vêm **primeiro** — sem elas, os testes de áudio não têm o que
ler. O `ci-rust.yml` já roda esses passos nessa ordem (job `build_and_test`,
todos os três SO da matriz).

---

## 2. Onde estão as fixtures — leia antes de procurar

**Os arquivos `.wav` não estão no repositório.** Isso é deliberado.

```
scripts/generate_fixtures.py      ← versionado
fixtures/audio/manifest.json      ← versionado
fixtures/audio/**/*.wav           ← no .gitignore, gerado localmente
```

Depois de clonar ou dar `pull`:

```bash
pip install -r scripts/requirements-fixtures.txt
python scripts/generate_fixtures.py --duration 5.0
```

35 arquivos, ~14 MB, alguns segundos.

**Por que não são comitados.** 14 MB entrariam no histórico do git para sempre
— todo clone baixa, inclusive quem só mexe em documentação, e tirar depois exige
reescrever histórico. Além disso, binário no git pode ser trocado sem ninguém
perceber: diff de WAV é ilegível.

**Por que isso é seguro.** O gerador usa **semente fixa** — os arquivos saem byte
a byte idênticos em qualquer máquina. E o `manifest.json`, que **é** versionado,
guarda o sha256 de cada um. Se a sua geração divergir, o teste falha na hora e
diz qual arquivo. A prova de igualdade está no repositório mesmo que os arquivos
não estejam.

É a mesma relação entre `package.json` e `node_modules/`.

> **Fixe as versões de `numpy`, `soundfile` e `scipy`.** Versões diferentes podem
> produzir resultados de ponto flutuante ligeiramente distintos, e aí o sha256
> falha. Falha em voz alta, que é o comportamento certo — mas é evitável.
> `scipy` entrou na lista porque `pink_noise` e o fundo de `conflict_targets`
> dependem dele sem fallback (ver §2.4) — instalar só numpy+soundfile faz o
> gerador quebrar no meio, não gerar algo diferente. As três versões só
> existem num único lugar, `scripts/requirements-fixtures.txt` — `ci-rust.yml`
> e este guia apontam para ele, em vez de repetir a string de versão em três
> lugares que podem divergir entre si.

### 2.1 Inventário — o que cada fixture prova

**Análise**

| Arquivo | Prova |
|---|---|
| `click_tracks/click_{60,90,120,128,140}bpm_mono.wav` | Detecção de BPM com resposta conhecida por construção |
| `tones/sine_{100hz,440hz,1khz,2khz,8khz}_mono.wav` | Pico espectral na frequência certa |
| `noise/white_noise_mono.wav` | Espectro plano |
| `noise/pink_noise_mono.wav` | Queda de ~3 dB por oitava |
| `sweeps/sweep_{20_20k,200_2k}_mono.wav` | Resposta em frequência, resolução do espectrograma |
| `rhythm/rhythm_{120,140}bpm_mono.wav` | Grade rítmica com acentuação — mais realista que clique puro |
| `structure/structure_intro_verse_chorus.wav` | Segmentação estrutural detecta ao menos 3 seções |
| `dynamics/dynamic_complex_mono.wav` | Envelope de compressão |

**Bugs conhecidos** — estas são as que importam agora

| Arquivo | Prova | Bug |
|---|---|---|
| `true_peak/true_peak_{m10,p0,p15}.wav` | **Pico entre amostras.** Senoide em `fs/4` com fase 45°: o pico real fica exatamente 3,01 dB acima do pico de amostra | **B5** — o limiter mede pico de amostra, não pico real |
| `conflicts/conflict_targets.wav` | Leito contínuo baixo com transientes esparsos perto de 0 dBFS | **B4** — o aviso `loudness_target_conflict` tem que disparar |
| `crossfade_pair/crossfade_pair_{A,B}.wav` | Dois trechos não correlacionados. Com ganho constante a queda de ~3 dB é medível; com potência constante não | **B3** — valida o T2.2 sobre arquivo, não só buffer |
| `zero_crossing/zero_crossing_dc_offset.wav` | **Nunca cruza zero.** Tem que devolver `None` ou o fallback | **B6** — caso de borda |
| `zero_crossing/zero_crossing_sine_100hz.wav` | Cruzamentos em índices exatos e afirmáveis (ver §2.4 — o número real é 1499, não 1000) | **B6** — devolve o mais próximo, não o primeiro da janela |
| `time_stretch/pure_tone_440hz.wav` | Esticar por 1,10 e o pico continuar em 440 Hz (±2 Hz) | Estiramento alterando o tom — erro típico de usar reamostragem |

**Bordas e robustez**

| Arquivo | Prova |
|---|---|
| `degenerate/degenerate_silence.wav` | Silêncio absoluto — divisão por zero em normalização |
| `degenerate/degenerate_dc_constant.wav` | DC puro — sem cruzamento, sem transiente |
| `degenerate/degenerate_single_sample.wav` | Buffer de uma amostra — `len() - 1` estoura |
| `degenerate/degenerate_zero_duration.wav` | Buffer vazio (zero amostras) |
| `degenerate/degenerate_full_scale.wav` | Já no teto — normalizar não pode aumentar |
| `degenerate/degenerate_nyquist_toggle.wav` | Alternância `+1,−1` por amostra — pior caso de reamostragem |
| `corrupted/corrupted_truncated.wav` | Cabeçalho válido, dados truncados — **erro tratado, nunca panic** |
| `stereo/stereo_440hz.wav` | Caminho estéreo e soma mono |

### 2.2 Os dois pontos que motivaram esta seção — verificados e corrigidos

O rascunho anterior deste guia pedia para conferir dois pontos antes de
confiar no manifesto. Os dois foram checados contra o áudio real (harness da
§5 + inspeção manual), com resultado diferente em cada um:

1. **Pico de amostra 3,01 dB abaixo do pico real, nos três `true_peak/*`.**
   Verificado — **correto, sem correção necessária**. Em `true_peak_p15.wav`
   o pico de amostra medido é −1,51 dBFS (não 0,0 dBFS — não está clipado).
   A construção (`fs/4`, fase 45°) garante essa relação exata por
   trigonometria: toda amostra cai em `sin(45° + n·90°) = ±0,7071`,
   `20·log10(0,7071) ≈ −3,01 dB` abaixo do pico contínuo. O `measure_true_peak`
   do harness, que sobreamostra 4x com FIR de 12 taps (`ebur128`), mede um
   viés sistemático de ~0,1037 dB nessa construção — idêntico à 4ª casa
   decimal nos três níveis, ou seja, característico do filtro, não ruído
   (ver §2.4/§5 para a causa exata). A tolerância da asserção é 0,15 dB, não
   0,01 — cobre o viés real do meter, não abre margem para regressão.

2. **`degenerate_single_sample.wav` imprimiu "5.0s" na geração.** Verificado
   — **era bug real, corrigido**. `gen_degenerate_cases` tinha os dois casos
   trocados: `single_sample` recebia um buffer de 220.500 amostras (5 s) com
   um impulso no índice 0, e `zero_duration` recebia um buffer de exatamente
   uma amostra. Agora `single_sample` tem uma amostra e `zero_duration` tem
   zero amostras — como os nomes dizem.

### 2.3 Um terceiro achado, fora do escopo original: `zero_crossings` não era um campo, eram três

Construir o harness da §5 expôs que `"zero_crossings"` no manifesto assumia
três formatos diferentes conforme o arquivo — um inteiro (contagem total, em
`click_tracks`/`rhythm`), `null` (em `zero_crossing_dc_offset`), ou uma lista
de índices (em `zero_crossing_sine_100hz`). Um campo, três tipos JSON — não dá
para deserializar isso num único struct Rust tipado.

Corrigido dividindo em dois campos sem ambiguidade:

- `zero_crossing_count` (inteiro, sempre presente quando aplicável — `0` para
  o caso que nunca cruza, em vez de `null`)
- `zero_crossing_indices` (lista, só em `zero_crossing_sine_100hz`)

### 2.4 Correções aplicadas ao gerador ao construir o harness

Nenhuma delas estava nos dois pontos pedidos originalmente em §2.2, mas todas
seriam invisíveis até quebrar o harness ou o CI — por isso ficam registradas
aqui, não só na mensagem do commit:

- **Caminhos com `\` no manifesto.** `str(path.relative_to(output_dir))` no
  Windows produz `click_tracks\click_60bpm_mono.wav`; no `manifest.json`
  versionado isso quebra a busca do arquivo em qualquer runner Linux/macOS
  (a matriz de CI roda os três). Trocado por `.as_posix()`.
- **`-Infinity` no JSON.** `degenerate_silence.wav` gravava
  `"sample_peak_db": -Infinity` — token que `serde_json` não aceita (não é
  JSON válido, é uma extensão do `json` do Python). Vira `null`.
- **`corrupted_truncated.wav` não era determinístico.** `gen_corrupted_wav`
  chamava `np.random.normal` (estado global do NumPy, não a semente por
  arquivo de `build_rng`) — o sha256 mudava a cada execução, contradizendo a
  premissa central da §2 ("byte a byte idênticos em qualquer máquina").
  Trocado para usar `build_rng(filename)` como todo o resto do gerador.
- **`zero_crossing_sine_100hz.wav`: 1000 cruzamentos no float64, 1499 no
  arquivo real.** 100 Hz a 44100 Hz dá período de exatamente 441 amostras —
  a cada período inteiro, o cruzamento cai *exatamente* em cima de uma
  amostra. Em float64 o ruído de arredondamento do `sin()` mantém essa
  amostra ligeiramente não-nula; quantizado para PCM_16 (passo ~3×10⁻⁵) esse
  resíduo desaparece e a amostra vira exatamente `0.0`, criando um terceiro
  estado de sinal onde só havia dois — e cada cruzamento nesses pontos passa
  a ser contado duas vezes. O manifesto agora calcula os índices sobre o
  sinal *já quantizado* (`quantize_like`), não sobre o float64
  pré-quantização — reflete o arquivo que o teste realmente lê, não o ideal
  matemático que nunca é gravado.
- **`scipy` sem fallback consistente.** `gen_pink_noise` falhava alto sem
  `scipy`; `gen_conflict_targets` tinha um `try/except` que, na ausência de
  `scipy`, gerava áudio silenciosamente diferente (mesma semente, resultado
  diferente — quebra a garantia de reprodutibilidade sem avisar). Removido o
  fallback; `scipy` é dependência obrigatória e está na lista de instalação
  (§2).
- **Tolerância de true peak alargada sem causa registrada (0,1 → 0,2), depois
  apertada com a causa certa (0,2 → 0,15).** A primeira revisão só constatou
  que o delta medido (~0,104 dB) cabia numa tolerância maior e parou aí — sem
  explicar o porquê, indistinguível de "aumentei até passar". A causa real:
  `ebur128::Mode::TRUE_PEAK` sobreamostra 4x com um FIR polifásico de 12 taps
  por fase (`InterpF<12, 4, _>`, escolhido porque `sample_rate < 96_000` — ver
  `ebur128::true_peak::UpsamplingScanner::new`). `fs/4` com fase 45° é
  justamente o caso didático clássico de "pico de amostra ≠ pico real": as
  amostras caem exatamente nos zeros do padrão de ripple de um reconstrutor
  ideal, o que também o torna o pior caso para um FIR curto de 12 taps
  divergir do ideal. O delta é idêntico à 4ª casa decimal nos três níveis
  (m10/p0/p15) — viés determinístico do filtro, não ruído de medição — então
  0,15 (margem pequena acima de 0,1037) é apertado o bastante para pegar
  regressão real e largo o bastante para não quebrar por causa da própria
  precisão do meter.
- **`gen_log_sweep` normalizava o expoente da fase por segundos, não por
  `duration`.** A frequência instantânea batia o alvo em `t=1,0s` sempre,
  nunca em `t=duration` — para `duration=5.0` (o valor real de toda fixture
  gerada), os últimos 4 dos 5 segundos de `sweeps/sweep_20_20k_mono.wav` e
  `sweep_200_2k_mono.wav` eram `sin()` de uma fase da ordem de 1e16 radianos:
  ruído numérico, não uma varredura. Só passava despercebido porque
  `duration=1.0s` faz o fator de normalização virar 1 e mascarar o erro, e
  porque nenhum teste tinha usado essas fixtures para medir frequência antes
  de `docs/17.1` §3.2 (aliasing) — `sample_peak_db`/`true_peak_dbtp` não
  distinguem uma varredura de ruído de alta frequência com o mesmo pico.

Mais duas, encontradas só quando o CI de fato rodou nas três plataformas da
matriz pela primeira vez — a razão de gerar fixtures nos três SOs em vez de só
Linux, mesmo custando tempo de execução:

- **`UnicodeEncodeError` no Windows.** stdout do Python usa o codepage do
  console (`cp1252`) por padrão, não UTF-8; os emojis dos prints de progresso
  (🎵, ✅, 📁...) derrubavam o processo antes de gerar um único arquivo.
  Corrigido com `sys.stdout.reconfigure(encoding="utf-8")` no início do
  script (no-op em Linux/macOS, que já são UTF-8).
- **`/tmp` hardcoded quebra no Windows.** `gen_corrupted_wav` escrevia num
  arquivo temporário em `Path("/tmp/corrupted_tmp.wav")` — caminho absoluto
  só faz sentido no POSIX; no Windows resolve para `\tmp` na raiz da unidade
  atual, que não existe (`LibsndfileError: Error opening
  '\tmp\corrupted_tmp.wav'`). Trocado por `tempfile.mkstemp()`, que resolve
  o diretório temporário certo em qualquer SO.

### 2.5 Fixture com defeito é pior que teste ausente

O bug de `gen_log_sweep` (§2.4) é o mais instrutivo dos encontrados
construindo esta suíte, por um motivo que não é sobre o bug em si: durante
quatro dos cinco segundos de duração, a fixture não era uma varredura —
era ruído numérico. E isso passou despercebido por múltiplas rodadas de
revisão, incluindo uma que apontou a integração de fase como imprecisa (uma
observação real, mas que classificou o problema como questão de precisão
quando na verdade era ausência total de normalização por `duration` —
destrói o sinal inteiro depois do primeiro segundo, não arredonda mal).

**Um teste ausente é uma lacuna conhecida — algo que se sabe que falta.
Uma fixture defeituosa produz um verde que ninguém questiona.** Antes do
harness de fixtures existir (PR #19), nada usava essa varredura para nada;
depois, o harness verificava sha256 e um pico de amostra que não distingue
uma varredura de ruído de alta frequência com o mesmo pico — o defeito
sobreviveu ao próprio processo criado para pegar exatamente esse tipo de
erro, porque a verificação existente nunca perguntava "este sinal é o que
diz ser?", só "este arquivo é o mesmo de sempre?" e "o resultado processado
está dentro da faixa?".

**A correção estrutural, não pontual:** o `manifest.json` agora carrega,
para as fixtures de varredura, `instantaneous_freq_checkpoints` — pontos
`(t_sec, freq_hz)` calculados da mesma fórmula analítica usada para
construir o sinal, verificados pelo harness via pico espectral (§5). Isso
não depende de nenhum teste específico (aliasing, THD, o que for) ter sido
escrito antes — é uma propriedade do sinal em si, junto do sha256, não uma
consequência de alguém ter lembrado de testar. Verificado revertendo a
correção temporariamente: a asserção falha sozinha, sem precisar do teste
de aliasing de `docs/17.1` §3.2.

**O princípio generaliza.** Fixtures merecem o mesmo rigor do motor que
elas testam — sha256 mais um valor de saída esperado prova que o arquivo
não mudou e que ele produz um número dentro da faixa, não que o sinal
tem a estrutura que o nome promete. Toda fixture cuja construção tem uma
propriedade analiticamente verificável (não só um valor de pico ou uma
contagem) deveria carregá-la no manifesto — é o mesmo raciocínio que já
motiva o sha256, aplicado um nível abaixo: à validade do sinal, não só à
sua imutabilidade.

---

## 3. Duas armadilhas de caminho

### 3.1 `fixtures/` não é diretório de testes do Cargo

O Cargo só descobre testes de integração em `<crate>/tests/`. `fixtures/` na raiz
do workspace é **diretório de dados** — nada ali é compilado ou executado.

E caminho relativo resolve a partir do diretório da **crate**, não do workspace:

```rust
fn fixtures_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR = <repo>/crates/audio_core
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/audio")
}
```

Sem isso o teste passa na sua máquina e falha no CI, ou o contrário, dependendo
de onde foi invocado. É exatamente assim que `crates/audio_core/tests/fixtures_manifest.rs`
resolve o diretório hoje.

### 3.2 Mudança em `fixtures/` ou `scripts/` não disparava CI

`ci-rust.yml` filtrava por `paths: ["crates/**", "Cargo.*", "config/**"]`.
Nenhum dos dois diretórios estava na lista — trocar, adicionar ou corromper
uma fixture não rodava nada. Corrigido: o filtro agora inclui
`"fixtures/**"` e `"scripts/**"`, e o job `build_and_test` instala Python +
as três dependências pinadas e roda `generate_fixtures.py` antes de
`cargo test`, nos três sistemas operacionais da matriz.

Correção definitiva (um único context agregador em vez de nove `paths:`
filtros para manter sincronizados): issue #5.

---

## 4. As camadas — o que cada uma prova

| Camada | Como roda | Prova | **Não** prova |
|---|---|---|---|
| Compilação | `cargo build` | Tipos coerentes | Nada sobre comportamento |
| Unitário | `cargo test` | Funções isoladas | Interação entre elas |
| Propriedade (`proptest`) | `cargo test` | Invariantes numa faixa ampla de entrada, inclusive bordas | Qualidade musical |
| Deriva | `cargo test` | Registry, validador e tabela de `docs/05` §3 coerentes entre si | Que os valores estejam **certos** |
| Newtype | compilação + `serde` | Estado inválido irrepresentável | Que os limites façam sentido |
| **Fixtures de áudio** | `cargo test --test fixtures_manifest` | Comportamento sobre arquivo real com valor esperado conhecido | Qualidade musical |
| Golden Master | não existe ainda | Regressão acústica entre versões | A primeira execução |
| Fatia vertical | não existe ainda | Cadeia ponta a ponta produz WAV | Se o WAV presta |
| **Escuta humana** | **você** | Qualidade | Nada disso é automatizável |

Duas linhas merecem atenção.

**Deriva prova coerência, não correção.** Se o limite de crossfade estiver errado
no `limits.rs`, os três lugares concordam e o teste passa. Ele impede que
divirjam, não que estejam errados juntos.

**Golden Master não valida a primeira execução.** Ele congela um resultado e
detecta mudança. Se o primeiro render estiver ruim, ele congela o ruim. Por isso
o `docs/09` exige escuta humana ao gerar ou atualizar um Golden Master — não é
formalidade, é o único momento em que alguém decide se aquilo é bom.

---

## 5. O harness de fixtures

**Uma função de teste, N casos, dirigida pelo manifesto** —
`crates/audio_core/tests/fixtures_manifest.rs`. Adicionar fixture passa a ser
acrescentar uma entrada no JSON, sem tocar em Rust.

```rust
#[test]
fn fixtures_conformam_ao_manifesto() {
    let manifest = load_manifest();

    for (caminho, spec) in &manifest.files {
        let arquivo = fixtures_dir().join(caminho);

        // 1. A fixture não mudou desde que os valores foram calculados
        assert_eq!(sha256_of(&arquivo), spec.sha256, "{caminho}: regenere ou atualize o manifesto");

        // corrupted_truncated.wav é o único caso cuja decodificação deve
        // falhar por construção — coberto por outro teste, não este loop.
        // degenerate_zero_duration.wav decodifica normalmente (buffer vazio,
        // sem erro do hound) e segue pelo caminho comum.
        if spec.expected.expected_behavior.as_deref() == Some("decode_error") {
            continue;
        }

        let audio = decodificar(&arquivo).unwrap_or_else(|e| panic!("{caminho}: {e}"));

        // 2. Vale para toda fixture, sempre
        assert!(audio.interleaved.iter().all(|s| s.is_finite()));   // I15
        assert_eq!(audio.sample_rate, spec.sample_rate);
        assert_eq!(audio.channels, spec.channels);

        // 3. Condicional ao que o manifesto declara
        if let (Some(bpm), Some(tol)) = (spec.expected.bpm, spec.expected.bpm_tolerance_pct) {
            let medido = estimate_bpm(&onset_strength(&audio.mono, 2048, 512), audio.sample_rate, 512) as f64;
            // As 3 fixtures em BPM_METADE_CONHECIDA_ISSUE_18 são cobradas
            // contra bpm/2 (o valor ERRADO que produzem hoje), não contra
            // bpm — pin explícito do bug, não tolerância de oitava. Quando
            // o #18 for corrigido esta asserção passa a falhar; é para
            // falhar. As outras 4 fixtures de BPM não têm essa complacência:
            // um trem de cliques uniforme não tem ambiguidade musical entre
            // tempo e metade de tempo, então medir metade ali é falha de
            // detecção, não leitura alternativa válida.
            let alvo = if BPM_METADE_CONHECIDA_ISSUE_18.contains(&caminho.as_str()) { bpm / 2.0 } else { bpm };
            let erro_pct = (medido - alvo).abs() / alvo * 100.0;
            assert!(erro_pct <= tol, "{caminho}: BPM {medido:.1}, esperado {alvo:.1}");
        }

        // só afirmado quando o gerador declara tolerância explícita — hoje
        // só true_peak/*; nas demais fixtures o campo é cópia informativa
        // do pico de amostra, não uma medição de true peak validada
        if let (Some(tp), Some(tol)) = (spec.expected.true_peak_dbtp, spec.expected.true_peak_dbtp_tolerance) {
            assert!((medir_true_peak(&audio) - tp).abs() <= tol);
        }

        if let Some(lufs) = spec.expected.lufs_i {
            assert!((medir_lufs(&audio) - lufs).abs() <= 3.5); // ver §2 sobre por que largo
        }

        if let Some(n) = spec.expected.zero_crossing_count {
            assert_eq!(contar_zero_crossings(&audio), n);
        }
        if let Some(indices) = &spec.expected.zero_crossing_indices {
            assert_eq!(&zero_crossing_indices(&audio), indices);
        }
    }
}
```

**A asserção do sha256 é a mais importante e a menos óbvia.** Ela amarra os
valores esperados ao arquivo que os produziu. Sem ela, alguém regenera as
fixtures com um parâmetro diferente, o áudio muda, os valores esperados
continuam os antigos, e os testes passam medindo a coisa errada.

**`corrupted_truncated.wav` tem teste separado**, porque nele a asserção é
que a decodificação **falha de forma tratada** — `degenerate_zero_duration.wav`
não entra nesse teste porque, verificado contra `hound`, um WAV com cabeçalho
válido e zero amostras de dados decodifica normalmente (buffer vazio, sem
erro); não há decoder de produção ainda que rejeite isso por regra de
negócio, então o harness testa o que existe de fato, não o que um rascunho
anterior presumia que aconteceria:

```rust
#[test]
fn arquivos_invalidos_falham_sem_panic() {
    let path = fixtures_dir().join("corrupted/corrupted_truncated.wav");
    let r = std::panic::catch_unwind(|| decodificar(&path));
    assert!(r.is_ok(), "causou panic em vez de Err");
    assert!(r.unwrap().is_err(), "deveria falhar e não falhou");
}
```

> **De onde vêm os valores esperados.** Da construção matemática do sinal,
> **não de medição com o nosso próprio motor**. Um trem de cliques a 128 BPM
> tem 128 BPM por construção. Se fossem medidos com o motor, o teste
> provaria apenas que o motor concorda consigo mesmo. Exceção documentada:
> LUFS de `conflict_targets.wav` não é derivável à mão a partir da amplitude
> linear usada na construção (o gating da BS.1770 pesa desproporcionalmente
> os transientes esparsos sobre o leito contínuo) — por isso a tolerância ali
> é larga (±3,5 LU): pega quebra grosseira, não valida ao décimo.

---

## 6. O que falta, em ordem

| # | Item | Status |
|---|---|---|
| 1 | `fixtures/**` e `scripts/**` no filtro do `ci-rust.yml` + geração no CI | **Feito** |
| 2 | Harness da §5 | **Feito** — `crates/audio_core/tests/fixtures_manifest.rs` |
| 3 | Conferir os dois pontos da §2.2 | **Feito** — um confirmado correto, um era bug e foi corrigido (§2.2, §2.4) |
| 4 | `scripts/test-all.sh`, com o CI chamando ele | Pendente |
| 5 | I4.2 — RMS em janela deslizante na emenda | Pendente — issue #16 |
| 6 | `estimate_bpm` confunde tempo com metade/dobro em alguns casos | Pendente — issue #18 (achado construindo o harness) |
| 7 | Fatia vertical | Pendente — bloqueada por fixtures de áudio real (não sintético) |
| 8 | Golden Master usando as fixtures | Pendente |
| 9 | Teste schema ↔ DSP | Pendente — issue #8 |

---

## 7. O que nenhum teste vai provar

A lista é curta e vale conhecer de cor:

- **Se a emenda soa bem.** O I4.2 mede queda de RMS. Não mede se o corte caiu
  no lugar musicalmente certo.
- **Se a seleção de blocos faz sentido.** Nenhuma métrica distingue "escolheu os
  refrões" de "escolheu quatro trechos com energia alta".
- **Se a masterização soa transparente.** LUFS e true peak dentro do alvo com
  limiter bombeando passam em todos os testes.
- **Se a proposta da IA é útil.** Formato válido e parâmetro dentro da faixa não
  dizem nada sobre a sugestão ser boa.

Todas as quatro precisam de uma pessoa ouvindo. É por isso que a fatia vertical
está no roteiro e por isso que o `docs/09` exige escuta humana registrada no PR
sempre que um Golden Master é criado ou atualizado.

**Um teste verde nunca significa que o áudio presta. Significa que ele não
regrediu de forma detectável.**

---

## 8. Quando um teste falha

1. **Não relaxe a tolerância.** Se um teste de LUFS falha por 0,3 LU, a pergunta
   é por que mudou, não se 0,5 vira 0,8. (Tolerâncias largas por design —
   como a de LUFS em `conflict_targets.wav`, §5 — são decisão de autoria,
   documentada inline; isso é diferente de afrouxar uma tolerância existente
   porque um teste passou a falhar.)
2. **Falha do `proptest` vem minimizada.** Congele o caso mínimo como teste fixo
   nomeado com o número da issue. A suíte fica mais forte a cada bug em vez de
   só mais longa.
3. **Falha de deriva significa que você mudou o registry.** Regenere a tabela;
   não edite os dois lados à mão.
4. **Falha de sha256 de fixture** significa que o áudio mudou. Ou você regenerou
   sem querer, ou o gerador mudou. Nos dois casos, os Golden Masters existentes
   estão inválidos.
5. **Panic em fixture degenerada é sempre bug**, nunca entrada inválida. Silêncio,
   DC e buffer de uma amostra são áudio legítimo.
