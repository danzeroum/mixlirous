#!/usr/bin/env python3
"""
Gerador de fixtures de áudio para o Mixlirous — versão estável (corrigida).

Uso:
    python scripts/generate_fixtures.py [--output-dir fixtures/audio] [--sample-rate 44100] [--duration 5.0]

Regras:
    - Semente fixa derivada do nome do arquivo → reprodutível.
    - Durações são proporcionais (não absolutas) → funciona com --duration.
    - Gera manifest.json com SHA-256 e valores esperados.
    - Não comitar os WAVs; comitar apenas o manifesto.

Dependências (versão fixa — ponto flutuante diverge entre versões e quebra o
sha256 do manifesto):
    pip install -r scripts/requirements-fixtures.txt
"""

import argparse
import hashlib
import json
import math
import os
import sys
import tempfile
import warnings
from datetime import datetime
from pathlib import Path
from typing import Dict, Any, Optional, Tuple, List

import numpy as np
import soundfile as sf

# stdout/stderr do Windows usa o codepage do console (cp1252 em runners de CI
# e em instalações padrão) em vez de UTF-8 — os emojis dos prints abaixo
# (🎵, ✅, 📁...) derrubam o processo com UnicodeEncodeError antes de gerar
# um único arquivo. Linux/macOS já são UTF-8 por padrão; reconfigure() aí é
# um no-op.
if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")

# Suprime avisos de divisão por zero em filtros
warnings.filterwarnings("ignore", category=RuntimeWarning)


# =============================================================================
# Utilitários
# =============================================================================

def build_rng(filename: str) -> np.random.Generator:
    """Cria um gerador determinístico a partir do nome do arquivo."""
    seed = int(hashlib.sha256(filename.encode()).hexdigest()[:16], 16) % 2**32
    return np.random.default_rng(seed)


def normalize(audio: np.ndarray, peak: float = 0.9) -> np.ndarray:
    """Normaliza o pico absoluto para o valor alvo, evitando clipping."""
    max_val = np.max(np.abs(audio))
    if max_val > 1e-12:
        return audio / max_val * peak
    return audio


def db_to_linear(db: float) -> float:
    """Converte dBFS para fator de amplitude linear."""
    return 10.0 ** (db / 20.0)


def linear_to_db(linear: float) -> float:
    """Converte fator de amplitude linear para dBFS."""
    if linear <= 0.0:
        return -float('inf')
    return 20.0 * math.log10(linear)


def write_wav(
    output_path: Path,
    audio: np.ndarray,
    sample_rate: int,
    manifest_entry: Dict[str, Any],
    subtype: str = "PCM_16",
) -> None:
    """Escreve WAV e preenche a entrada do manifesto."""
    output_path.parent.mkdir(parents=True, exist_ok=True)
    sf.write(str(output_path), audio, sample_rate, subtype=subtype)

    # Preenche metadados para o manifesto
    manifest_entry["sample_rate"] = sample_rate
    manifest_entry["channels"] = audio.shape[1] if audio.ndim > 1 else 1
    manifest_entry["duration_sec"] = audio.shape[0] / sample_rate
    manifest_entry["sha256"] = sha256_file(output_path)
    manifest_entry["seed"] = None  # Será preenchido pelo gerador específico


def sha256_file(path: Path) -> str:
    """Calcula SHA-256 de um arquivo."""
    with open(path, "rb") as f:
        return hashlib.sha256(f.read()).hexdigest()


def quantize_like(audio: np.ndarray, subtype: str) -> np.ndarray:
    """Simula a quantização que `sf.write` vai aplicar, para calcular valores
    esperados sobre o que realmente será decodificado — não sobre o float64
    pré-quantização.

    Importa para zero-crossing: numa senoide cujo período cabe um número
    inteiro de amostras (100 Hz a 44100 Hz = 441 amostras/ciclo), o cruzamento
    cai exatamente EM cima de uma amostra a cada período. Em float64 o ruído
    de arredondamento do `sin()` mantém essa amostra ligeiramente positiva ou
    negativa (nunca zero exato), então `np.sign` não vê um terceiro estado. Em
    PCM_16 (passo de quantização ~3e-5) esse resíduo desaparece e a amostra
    quantiza para exatamente 0 — `np.sign` passa a ver +/0/- em vez de só
    +/-, e cada cruzamento nesses pontos é contado duas vezes. Sem isso o
    manifesto reflete o sinal ideal, não o arquivo que o teste realmente lê.
    """
    bits = {"PCM_16": 16, "PCM_24": 24}.get(subtype)
    if bits is None:  # FLOAT ou outro subtype de ponto flutuante: sem quantização
        return audio
    full_scale = float(1 << (bits - 1))
    return np.round(np.clip(audio, -1.0, 1.0) * full_scale).clip(-full_scale, full_scale - 1) / full_scale


# =============================================================================
# Geradores de sinal
# =============================================================================

def gen_click_train(
    duration: float,
    sample_rate: int,
    bpm: float,
    amplitude: float = 0.9,
    click_duration: float = 0.005,
    filename: str = "click",
) -> Tuple[np.ndarray, Dict[str, Any]]:
    """Trem de cliques em BPM conhecido."""
    rng = build_rng(filename)
    total_samples = int(sample_rate * duration)
    audio = np.zeros(total_samples)

    beat_interval_sec = 60.0 / bpm
    beat_interval_samples = int(sample_rate * beat_interval_sec)
    click_samples = int(sample_rate * click_duration)
    envelope = np.exp(-np.linspace(0, 4, click_samples))

    for i in range(0, total_samples, beat_interval_samples):
        end = min(i + click_samples, total_samples)
        if end > i:
            amp_var = 0.9 + 0.1 * rng.random()
            audio[i:end] += amplitude * amp_var * envelope[: end - i]

    audio = normalize(audio, amplitude)

    expected = {
        "bpm": bpm,
        "bpm_tolerance_pct": 2.0,
        "sample_peak_db": float(linear_to_db(np.max(np.abs(audio)))),
        "true_peak_dbtp": float(linear_to_db(np.max(np.abs(audio)))),
        "lufs_i": None,
        "zero_crossing_count": int(np.sum(np.diff(np.sign(audio)) != 0)),
    }
    return audio, expected


def gen_sine(
    duration: float,
    sample_rate: int,
    freq: float,
    amplitude: float = 0.8,
    filename: str = "sine",
) -> Tuple[np.ndarray, Dict[str, Any]]:
    """Tom senoidal puro."""
    t = np.linspace(0, duration, int(sample_rate * duration), endpoint=False)
    audio = amplitude * np.sin(2.0 * np.pi * freq * t)
    audio = normalize(audio, amplitude)

    expected = {
        "freq_hz": freq,
        "freq_tolerance_hz": 2.0,
        "sample_peak_db": float(linear_to_db(np.max(np.abs(audio)))),
        "true_peak_dbtp": float(linear_to_db(np.max(np.abs(audio)))),
    }
    return audio, expected


def gen_white_noise(
    duration: float,
    sample_rate: int,
    amplitude: float = 0.5,
    filename: str = "white_noise",
) -> Tuple[np.ndarray, Dict[str, Any]]:
    """Ruído branco (espectro plano)."""
    rng = build_rng(filename)
    total_samples = int(sample_rate * duration)
    audio = amplitude * rng.normal(0, 1, total_samples)
    audio = normalize(audio, amplitude)

    expected = {
        "sample_peak_db": float(linear_to_db(np.max(np.abs(audio)))),
        "true_peak_dbtp": float(linear_to_db(np.max(np.abs(audio)))),
    }
    return audio, expected


def gen_pink_noise(
    duration: float,
    sample_rate: int,
    amplitude: float = 0.5,
    filename: str = "pink_noise",
) -> Tuple[np.ndarray, Dict[str, Any]]:
    """Ruído rosa usando filtro IIR."""
    try:
        from scipy.signal import lfilter
    except ImportError:
        raise ImportError("scipy é necessário para pink noise. pip install scipy")

    rng = build_rng(filename)
    total_samples = int(sample_rate * duration)

    b = [0.049922035, -0.095993537, 0.050612699, -0.004408786]
    a = [1.0, -2.494956002, 2.017265875, -0.522189400]

    x = rng.normal(0, 1, total_samples)
    y = lfilter(b, a, x)
    audio = normalize(y, amplitude)

    expected = {
        "sample_peak_db": float(linear_to_db(np.max(np.abs(audio)))),
        "true_peak_dbtp": float(linear_to_db(np.max(np.abs(audio)))),
    }
    return audio, expected


def gen_log_sweep(
    duration: float,
    sample_rate: int,
    freq_start: float,
    freq_end: float,
    amplitude: float = 0.8,
    filename: str = "sweep",
) -> Tuple[np.ndarray, Dict[str, Any]]:
    """Varredura logarítmica com solução analítica exata da fase.

    f(t) = freq_start * (freq_end/freq_start)^(t/duration), então
    phase(t) = 2*pi*freq_start*duration/L * (exp((t/duration)*L) - 1), com
    L = ln(freq_end/freq_start). O expoente tem que ser normalizado por
    `duration` — sem isso (bug encontrado ao tentar validar sinc/aliasing
    contra esta fixture, docs/17.1 §3.2) a frequência instantânea bate o alvo
    em t=1,0s **sempre**, não em t=duration: para duration=5.0 a varredura
    real termina em ~20 Hz a ~20 kHz por volta de t=1s e o resto (4 dos 5
    segundos) é `sin()` de uma fase da ordem de 1e16 radianos — silenciosamente
    ruído numérico, não uma varredura. Só passava despercebido porque
    duration=1.0s faz o fator de normalização virar 1 e mascarar o erro.
    """
    total_samples = int(sample_rate * duration)
    t = np.linspace(0, duration, total_samples, endpoint=False)

    if freq_start <= 0 or freq_end <= 0 or freq_start == freq_end:
        raise ValueError("freq_start e freq_end devem ser positivos e diferentes")
    if duration <= 0:
        raise ValueError("duration deve ser positiva")
    log_ratio = np.log(freq_end / freq_start)
    k = 2.0 * np.pi * freq_start * duration / log_ratio
    phase = k * (np.exp((t / duration) * log_ratio) - 1.0)

    audio = amplitude * np.sin(phase)
    audio = normalize(audio, amplitude)

    # Propriedade verificável do sinal em si, não só sha256 + valores de
    # saída — checkpoints de frequência instantânea, calculados da mesma
    # fórmula analítica usada para construir o sinal (não medidos com o
    # motor). O harness (fixtures_manifest.rs) mede o pico espectral perto de
    # cada `t_sec` e confere contra `freq_hz`. Teria pego o bug de
    # normalização por `duration` (ver docstring da função) sozinho, sem
    # depender de um teste específico de aliasing ter sido escrito primeiro.
    checkpoint_fracs = [0.1, 0.3, 0.5, 0.7, 0.9]
    checkpoints = [
        {
            "t_sec": float(duration * f),
            "freq_hz": float(freq_start * np.exp(f * log_ratio)),
        }
        for f in checkpoint_fracs
    ]

    expected = {
        "freq_start_hz": freq_start,
        "freq_end_hz": freq_end,
        "sample_peak_db": float(linear_to_db(np.max(np.abs(audio)))),
        "true_peak_dbtp": float(linear_to_db(np.max(np.abs(audio)))),
        "instantaneous_freq_checkpoints": checkpoints,
    }
    return audio, expected


def gen_rhythm_pattern(
    duration: float,
    sample_rate: int,
    bpm: float = 120.0,
    amplitude: float = 0.8,
    filename: str = "rhythm",
) -> Tuple[np.ndarray, Dict[str, Any]]:
    """Padrão rítmico com acentuação (simula bateria)."""
    rng = build_rng(filename)
    total_samples = int(sample_rate * duration)
    audio = np.zeros(total_samples)

    beat_interval_sec = 60.0 / bpm
    beat_interval_samples = int(sample_rate * beat_interval_sec)
    subdivs = 4
    subdiv_interval = beat_interval_samples // subdivs

    accent_pattern = [1.0, 0.4, 0.8, 0.3]

    for beat in range(int(duration / beat_interval_sec) + 1):
        for sub in range(subdivs):
            idx = beat * beat_interval_samples + sub * subdiv_interval
            if idx >= total_samples:
                break
            accent = accent_pattern[beat % len(accent_pattern)]
            if sub == 0:
                dur_samples = int(sample_rate * 0.02)
                amp = amplitude * accent * 0.9
            else:
                dur_samples = int(sample_rate * 0.01)
                amp = amplitude * accent * 0.3 * (0.5 + 0.5 * rng.random())

            end = min(idx + dur_samples, total_samples)
            if end > idx:
                env = np.exp(-np.linspace(0, 3, end - idx))
                audio[idx:end] += amp * env

    audio = normalize(audio, amplitude)

    expected = {
        "bpm": bpm,
        "bpm_tolerance_pct": 2.0,
        "sample_peak_db": float(linear_to_db(np.max(np.abs(audio)))),
        "true_peak_dbtp": float(linear_to_db(np.max(np.abs(audio)))),
        "zero_crossing_count": int(np.sum(np.diff(np.sign(audio)) != 0)),
    }
    return audio, expected


def gen_dynamic_complex(
    duration: float,
    sample_rate: int,
    amplitude: float = 0.9,
    filename: str = "dynamic_complex",
) -> Tuple[np.ndarray, Dict[str, Any]]:
    """Sinal com dinâmica variável (ataque, sustain com modulação, decaimento)."""
    total_samples = int(sample_rate * duration)
    t = np.linspace(0, duration, total_samples, endpoint=False)

    attack_dur = duration * 0.10
    sustain_dur = duration * 0.60
    decay_dur = duration * 0.30

    env = np.ones(total_samples)

    attack_samples = int(sample_rate * attack_dur)
    env[:attack_samples] = np.linspace(0.1, 1.0, attack_samples)

    sustain_start = attack_samples
    sustain_end = int(sample_rate * (attack_dur + sustain_dur))
    sustain_end = min(sustain_end, total_samples)
    sustain_len = sustain_end - sustain_start
    if sustain_len > 0:
        t_sustain = t[sustain_start:sustain_end]
        env[sustain_start:sustain_end] = 0.8 + 0.2 * np.sin(2 * np.pi * 1.5 * t_sustain)

    decay_start = sustain_end
    decay_len = total_samples - decay_start
    if decay_len > 0:
        env[decay_start:] = np.linspace(
            env[decay_start - 1] if decay_start > 0 else 1.0,
            0.05,
            decay_len
        )

    audio = amplitude * env * (
        0.6 * np.sin(2.0 * np.pi * 220 * t) +
        0.3 * np.sin(2.0 * np.pi * 440 * t) +
        0.1 * np.sin(2.0 * np.pi * 880 * t)
    )
    rng = build_rng(filename + "_noise")
    audio += 0.02 * rng.normal(0, 1, total_samples)

    audio = normalize(audio, amplitude)

    expected = {
        "sample_peak_db": float(linear_to_db(np.max(np.abs(audio)))),
        "true_peak_dbtp": float(linear_to_db(np.max(np.abs(audio)))),
        "lufs_i": None,
    }
    return audio, expected


def gen_structural_test(
    duration: float,
    sample_rate: int,
    amplitude: float = 0.8,
    filename: str = "structure",
) -> Tuple[np.ndarray, Dict[str, Any]]:
    """Estrutura clara: intro, verso, refrão, outro (proporcional)."""
    total_samples = int(sample_rate * duration)
    t = np.linspace(0, duration, total_samples, endpoint=False)
    audio = np.zeros(total_samples)

    intro_end = duration * 0.10
    verse_end = duration * 0.40
    chorus_end = duration * 0.80
    outro_end = duration

    def add_section(start_sec: float, end_sec: float, freq: float, amp: float, rhythm: bool = False):
        start_idx = int(sample_rate * start_sec)
        end_idx = int(sample_rate * end_sec)
        if start_idx >= total_samples or end_idx <= 0:
            return
        end_idx = min(end_idx, total_samples)
        length = end_idx - start_idx
        if length <= 0:
            return
        t_sec = np.linspace(0, (end_sec - start_sec), length, endpoint=False)
        signal = amp * (
            0.5 * np.sin(2.0 * np.pi * freq * t_sec) +
            0.3 * np.sin(2.0 * np.pi * 2 * freq * t_sec) +
            0.15 * np.sin(2.0 * np.pi * 3 * freq * t_sec)
        )
        if rhythm:
            rhythm_env = 0.7 + 0.3 * np.sin(2 * np.pi * 2 * t_sec)
            signal *= rhythm_env
        audio[start_idx:end_idx] += signal

    add_section(0.0, intro_end, 110, 0.3, rhythm=False)
    add_section(intro_end, verse_end, 220, 0.6, rhythm=True)
    add_section(verse_end, chorus_end, 330, 0.9, rhythm=True)
    add_section(chorus_end, outro_end, 220, 0.5, rhythm=False)

    fade_start = int(sample_rate * (duration * 0.90))
    if fade_start < total_samples:
        fade_len = total_samples - fade_start
        audio[fade_start:] *= np.linspace(1.0, 0.0, fade_len)

    audio = normalize(audio, amplitude)

    expected = {
        "sample_peak_db": float(linear_to_db(np.max(np.abs(audio)))),
        "true_peak_dbtp": float(linear_to_db(np.max(np.abs(audio)))),
        "sections_detected_min": 3,
    }
    return audio, expected


def gen_inter_sample_peak(
    duration: float,
    sample_rate: int,
    true_peak_dbtp: float,
    filename: str = "inter_sample_peak",
) -> Tuple[np.ndarray, Dict[str, Any]]:
    """Senoide em fs/4 com fase de 45° para true peak."""
    if sample_rate % 4 != 0:
        raise ValueError(f"sample_rate ({sample_rate}) deve ser divisível por 4")
    freq = sample_rate / 4.0
    total_samples = int(sample_rate * duration)
    t = np.linspace(0, duration, total_samples, endpoint=False)

    amplitude = db_to_linear(true_peak_dbtp)
    phase = np.pi / 4.0
    audio = amplitude * np.sin(2.0 * np.pi * freq * t + phase)

    expected = {
        "freq_hz": freq,
        "sample_peak_dbfs": float(linear_to_db(np.max(np.abs(audio)))),
        "true_peak_dbtp": true_peak_dbtp,
        # 0,15, não 0,1: medido empiricamente contra o harness Rust
        # (docs/17 §5) um viés sistemático e determinístico de ~0,1037 dB
        # nesta construção — idêntico (à 4ª casa decimal) nos três níveis
        # (m10/p0/p15), ou seja, característico do filtro, não ruído. Causa:
        # a crate `ebur128` mede true peak sobreamostrando 4x com um FIR
        # polifásico de 12 taps por fase (`InterpF<12, 4, _>`, escolhido
        # porque sample_rate < 96000 — ver `ebur128::true_peak::
        # UpsamplingScanner::new`). fs/4 com fase 45° é o caso clássico de
        # "pico de amostra != pico real" justamente porque as amostras caem
        # exatamente nos zeros do padrão de ripple de um reconstrutor
        # sobreamostrado — um FIR de 12 taps diverge mais do ideal aí do que
        # em conteúdo genérico. Não é bug do meter nem do teste.
        "true_peak_dbtp_tolerance": 0.15,
    }
    return audio, expected


def gen_conflict_targets(
    duration: float,
    sample_rate: int,
    amplitude: float = 0.9,
    filename: str = "conflict_targets",
) -> Tuple[np.ndarray, Dict[str, Any]]:
    """Fundo baixo (-30 LUFS) com transientes esparsos próximos de 0 dBFS."""
    rng = build_rng(filename)
    total_samples = int(sample_rate * duration)
    t = np.linspace(0, duration, total_samples, endpoint=False)

    from scipy.signal import lfilter
    b = [0.049922035, -0.095993537, 0.050612699, -0.004408786]
    a = [1.0, -2.494956002, 2.017265875, -0.522189400]
    bg = lfilter(b, a, rng.normal(0, 1, total_samples))
    bg = bg * db_to_linear(-30.0)

    audio = bg.copy()

    num_transients = max(2, int(duration / 1.5))
    for _ in range(num_transients):
        pos = int(rng.random() * 0.8 * total_samples + 0.1 * total_samples)
        length = int(sample_rate * 0.05)
        amp = 0.9 + 0.1 * rng.random()
        env = np.exp(-np.linspace(0, 2, length))
        audio[pos:min(pos + length, total_samples)] += amp * env * amplitude

    audio = np.clip(audio, -1.0, 1.0)

    expected = {
        "sample_peak_db": float(linear_to_db(np.max(np.abs(audio)))),
        "true_peak_dbtp": float(linear_to_db(np.max(np.abs(audio)))),
        "lufs_i": -30.0,
        "conflict_expected": True,
    }
    return audio, expected


def gen_crossfade_pair(
    duration: float,
    sample_rate: int,
    filename_prefix: str = "crossfade_pair",
) -> List[Tuple[Path, np.ndarray, Dict[str, Any]]]:
    """Dois arquivos não-correlacionados para testar crossfade."""
    audio_a, exp_a = gen_pink_noise(duration, sample_rate, 0.8, filename=f"{filename_prefix}_A")
    exp_a["expected_crossfade"] = "constant_power"

    audio_b, exp_b = gen_pink_noise(duration, sample_rate, 0.8, filename=f"{filename_prefix}_B")
    exp_b["expected_crossfade"] = "constant_power"

    return [
        (Path(f"{filename_prefix}_A.wav"), audio_a, exp_a),
        (Path(f"{filename_prefix}_B.wav"), audio_b, exp_b),
    ]


def gen_zero_crossing_cases(
    duration: float,
    sample_rate: int,
    filename: str = "zero_crossing",
    subtype: str = "PCM_16",
) -> List[Tuple[Path, np.ndarray, Dict[str, Any]]]:
    """Casos específicos de zero-crossing."""
    results = []
    total_samples = int(sample_rate * duration)
    t = np.linspace(0, duration, total_samples, endpoint=False)

    # Offset DC (nunca cruza)
    audio_dc = 0.5 + 0.4 * np.sin(2.0 * np.pi * 100 * t)
    exp_dc = {
        "zero_crossing_count": 0,
        "expected_behavior": "none",
        "sample_peak_db": float(linear_to_db(np.max(np.abs(audio_dc)))),
    }
    results.append((Path(f"{filename}_dc_offset.wav"), audio_dc, exp_dc))

    # Senoide 100 Hz (cruzamentos conhecidos)
    freq = 100.0
    audio_sine = 0.8 * np.sin(2.0 * np.pi * freq * t)
    crossings = np.where(np.diff(np.sign(quantize_like(audio_sine, subtype))) != 0)[0]
    exp_sine = {
        "zero_crossing_count": int(len(crossings)),
        "zero_crossing_indices": crossings.tolist(),
        "expected_behavior": "known_indices",
        "sample_peak_db": float(linear_to_db(np.max(np.abs(audio_sine)))),
    }
    results.append((Path(f"{filename}_sine_100hz.wav"), audio_sine, exp_sine))

    return results


def gen_pure_tone_stretch(
    duration: float,
    sample_rate: int,
    freq: float = 440.0,
    amplitude: float = 0.8,
    filename: str = "pure_tone_440hz",
) -> Tuple[np.ndarray, Dict[str, Any]]:
    """Tom puro para testar time-stretch (tom não deve mudar)."""
    audio, exp = gen_sine(duration, sample_rate, freq, amplitude, filename)
    exp["expected_freq_after_stretch"] = freq
    exp["freq_tolerance_hz"] = 2.0
    return audio, exp


def gen_degenerate_cases(
    duration: float,
    sample_rate: int,
    filename: str = "degenerate",
) -> List[Tuple[Path, np.ndarray, Dict[str, Any]]]:
    """Casos degenerados para testar robustez do decodificador."""
    results = []
    total_samples = int(sample_rate * duration)

    audio = np.zeros(total_samples)
    exp = {"expected_behavior": "silence", "sample_peak_db": None}  # -inf não é JSON válido
    results.append((Path(f"{filename}_silence.wav"), audio, exp))

    audio = np.ones(total_samples) * 0.5
    exp = {"expected_behavior": "dc_constant", "sample_peak_db": float(linear_to_db(0.5))}
    results.append((Path(f"{filename}_dc_constant.wav"), audio, exp))

    # Buffer de exatamente uma amostra — expõe `len() - 1` estourando.
    audio = np.array([0.9], dtype=np.float32)
    exp = {"expected_behavior": "single_sample", "sample_peak_db": float(linear_to_db(0.9))}
    results.append((Path(f"{filename}_single_sample.wav"), audio, exp))

    # Buffer vazio — decoder tem que recusar, não estourar.
    audio = np.array([], dtype=np.float32)
    exp = {"expected_behavior": "zero_duration", "sample_peak_db": None}
    results.append((Path(f"{filename}_zero_duration.wav"), audio, exp))

    audio = np.ones(total_samples) * 1.0
    exp = {"expected_behavior": "full_scale", "sample_peak_db": 0.0}
    results.append((Path(f"{filename}_full_scale.wav"), audio, exp))

    audio = np.array([1.0 if i % 2 == 0 else -1.0 for i in range(total_samples)], dtype=np.float32)
    exp = {"expected_behavior": "nyquist_toggle", "sample_peak_db": 0.0}
    results.append((Path(f"{filename}_nyquist_toggle.wav"), audio, exp))

    return results


def gen_corrupted_wav(
    duration: float,
    sample_rate: int,
    filename: str = "corrupted",
) -> Tuple[Path, bytes, Dict[str, Any]]:
    """Gera um WAV com cabeçalho válido, mas dados truncados."""
    total_samples = int(sample_rate * duration)
    rng = build_rng(filename)
    audio = rng.normal(0, 0.5, total_samples).astype(np.float32)

    # `/tmp` é caminho absoluto só no POSIX — no Windows resolve para
    # `\tmp` na raiz da unidade atual, que não existe (`LibsndfileError:
    # Error opening '\\tmp\\corrupted_tmp.wav'`). `tempfile` resolve o
    # diretório temporário certo em qualquer SO.
    fd, tmp_name = tempfile.mkstemp(suffix=".wav")
    os.close(fd)
    tmp_path = Path(tmp_name)
    try:
        sf.write(str(tmp_path), audio, sample_rate, subtype="PCM_16")
        with open(tmp_path, "rb") as f:
            full_data = f.read()
    finally:
        tmp_path.unlink(missing_ok=True)

    header_size = 44
    data_start = header_size
    truncate_pos = data_start + int((len(full_data) - data_start) * 0.4)
    corrupted_data = full_data[:truncate_pos]

    exp = {
        "expected_behavior": "decode_error",
        "error_type": "truncated_data",
    }
    return Path(f"{filename}_truncated.wav"), corrupted_data, exp


# =============================================================================
# Main
# =============================================================================

def main():
    parser = argparse.ArgumentParser(
        description="Gera fixtures de áudio estáveis para o Mixlirous."
    )
    parser.add_argument(
        "--output-dir",
        default="./fixtures/audio",
        help="Diretório de saída (padrão: ./fixtures/audio)",
    )
    parser.add_argument(
        "--sample-rate",
        type=int,
        default=44100,
        help="Taxa de amostragem (Hz)",
    )
    parser.add_argument(
        "--duration",
        type=float,
        default=5.0,
        help="Duração padrão em segundos (mínimo 2.0)",
    )
    parser.add_argument(
        "--bit-depth",
        choices=["16", "24", "32"],
        default="16",
        help="Profundidade de bits (16, 24, 32 float)",
    )
    args = parser.parse_args()

    if args.duration < 2.0:
        print("❌ A duração mínima é 2.0 segundos.")
        return

    output_dir = Path(args.output_dir)
    sample_rate = args.sample_rate
    duration = args.duration
    subtype = {"16": "PCM_16", "24": "PCM_24", "32": "FLOAT"}[args.bit_depth]

    print(f"🎵 Gerando fixtures de áudio para o Mixlirous")
    print(f"   Diretório: {output_dir}")
    print(f"   Sample rate: {sample_rate} Hz")
    print(f"   Duração: {duration}s")
    print(f"   Profundidade: {args.bit_depth}-bit\n")

    manifest: Dict[str, Any] = {
        "generator_version": "2.0.2",
        "generated_at": datetime.utcnow().isoformat() + "Z",
        "files": {},
    }

    # ------------------------------------------------------------------------
    # 1. Cliques
    # ------------------------------------------------------------------------
    print("📁 click_tracks/")
    for bpm in [60, 90, 120, 128, 140]:
        fname = f"click_{bpm}bpm_mono.wav"
        audio, exp = gen_click_train(duration, sample_rate, bpm, filename=fname)
        path = output_dir / "click_tracks" / fname
        entry = {}
        write_wav(path, audio, sample_rate, entry, subtype)
        entry["expected"] = exp
        manifest["files"][path.relative_to(output_dir).as_posix()] = entry
        print(f"  ✅ {path.name} ({entry['duration_sec']:.1f}s, {entry['channels']}ch)")

    # ------------------------------------------------------------------------
    # 2. Tons
    # ------------------------------------------------------------------------
    print("📁 tones/")
    for freq, label in [(100, "100hz"), (440, "440hz"), (1000, "1khz"), (2000, "2khz"), (8000, "8khz")]:
        fname = f"sine_{label}_mono.wav"
        audio, exp = gen_sine(duration, sample_rate, freq, filename=fname)
        path = output_dir / "tones" / fname
        entry = {}
        write_wav(path, audio, sample_rate, entry, subtype)
        entry["expected"] = exp
        manifest["files"][path.relative_to(output_dir).as_posix()] = entry
        print(f"  ✅ {path.name} ({entry['duration_sec']:.1f}s)")

    # ------------------------------------------------------------------------
    # 3. Ruído
    # ------------------------------------------------------------------------
    print("📁 noise/")
    for noise_type, gen_func in [("white", gen_white_noise), ("pink", gen_pink_noise)]:
        fname = f"{noise_type}_noise_mono.wav"
        audio, exp = gen_func(duration, sample_rate, filename=fname)
        path = output_dir / "noise" / fname
        entry = {}
        write_wav(path, audio, sample_rate, entry, subtype)
        entry["expected"] = exp
        manifest["files"][path.relative_to(output_dir).as_posix()] = entry
        print(f"  ✅ {path.name} ({entry['duration_sec']:.1f}s)")

    # ------------------------------------------------------------------------
    # 4. Varreduras
    # ------------------------------------------------------------------------
    print("📁 sweeps/")
    for start, end, label in [(20, 20000, "20_20k"), (200, 2000, "200_2k")]:
        fname = f"sweep_{label}_mono.wav"
        audio, exp = gen_log_sweep(duration, sample_rate, start, end, filename=fname)
        path = output_dir / "sweeps" / fname
        entry = {}
        write_wav(path, audio, sample_rate, entry, subtype)
        entry["expected"] = exp
        manifest["files"][path.relative_to(output_dir).as_posix()] = entry
        print(f"  ✅ {path.name} ({entry['duration_sec']:.1f}s)")

    # ------------------------------------------------------------------------
    # 5. Ritmo
    # ------------------------------------------------------------------------
    print("📁 rhythm/")
    for bpm in [120, 140]:
        fname = f"rhythm_{bpm}bpm_mono.wav"
        audio, exp = gen_rhythm_pattern(duration, sample_rate, bpm, filename=fname)
        path = output_dir / "rhythm" / fname
        entry = {}
        write_wav(path, audio, sample_rate, entry, subtype)
        entry["expected"] = exp
        manifest["files"][path.relative_to(output_dir).as_posix()] = entry
        print(f"  ✅ {path.name} ({entry['duration_sec']:.1f}s)")

    # ------------------------------------------------------------------------
    # 6. Dinâmica
    # ------------------------------------------------------------------------
    print("📁 dynamics/")
    fname = "dynamic_complex_mono.wav"
    audio, exp = gen_dynamic_complex(duration, sample_rate, filename=fname)
    path = output_dir / "dynamics" / fname
    entry = {}
    write_wav(path, audio, sample_rate, entry, subtype)
    entry["expected"] = exp
    manifest["files"][path.relative_to(output_dir).as_posix()] = entry
    print(f"  ✅ {path.name} ({entry['duration_sec']:.1f}s)")

    # ------------------------------------------------------------------------
    # 7. Estrutura
    # ------------------------------------------------------------------------
    print("📁 structure/")
    fname = "structure_intro_verse_chorus.wav"
    audio, exp = gen_structural_test(duration, sample_rate, filename=fname)
    path = output_dir / "structure" / fname
    entry = {}
    write_wav(path, audio, sample_rate, entry, subtype)
    entry["expected"] = exp
    manifest["files"][path.relative_to(output_dir).as_posix()] = entry
    print(f"  ✅ {path.name} ({entry['duration_sec']:.1f}s)")

    # ------------------------------------------------------------------------
    # 8. True peak (F1)
    # ------------------------------------------------------------------------
    print("📁 true_peak/")
    for tp_db in [-1.0, 0.0, 1.5]:
        label = f"{'m' if tp_db < 0 else 'p'}{abs(int(tp_db*10))}"
        fname = f"true_peak_{label}.wav"
        audio, exp = gen_inter_sample_peak(duration, sample_rate, tp_db, filename=fname)
        path = output_dir / "true_peak" / fname
        entry = {}
        write_wav(path, audio, sample_rate, entry, subtype)
        entry["expected"] = exp
        manifest["files"][path.relative_to(output_dir).as_posix()] = entry
        print(f"  ✅ {path.name} (true peak {tp_db:.1f} dBTP)")

    # ------------------------------------------------------------------------
    # 9. Conflito de alvos (F2)
    # ------------------------------------------------------------------------
    print("📁 conflicts/")
    fname = "conflict_targets.wav"
    audio, exp = gen_conflict_targets(duration, sample_rate, filename=fname)
    path = output_dir / "conflicts" / fname
    entry = {}
    write_wav(path, audio, sample_rate, entry, subtype)
    entry["expected"] = exp
    manifest["files"][path.relative_to(output_dir).as_posix()] = entry
    print(f"  ✅ {path.name} ({entry['duration_sec']:.1f}s)")

    # ------------------------------------------------------------------------
    # 10. Par para crossfade (F3)
    # ------------------------------------------------------------------------
    print("📁 crossfade_pair/")
    pair = gen_crossfade_pair(duration, sample_rate, "crossfade_pair")
    for path_rel, audio, exp in pair:
        path = output_dir / "crossfade_pair" / path_rel
        entry = {}
        write_wav(path, audio, sample_rate, entry, subtype)
        entry["expected"] = exp
        manifest["files"][path.relative_to(output_dir).as_posix()] = entry
        print(f"  ✅ {path.name} ({entry['duration_sec']:.1f}s)")

    # ------------------------------------------------------------------------
    # 11. Zero-crossing (F4)
    # ------------------------------------------------------------------------
    print("📁 zero_crossing/")
    zcases = gen_zero_crossing_cases(duration, sample_rate, "zero_crossing", subtype)
    for path_rel, audio, exp in zcases:
        path = output_dir / "zero_crossing" / path_rel
        entry = {}
        write_wav(path, audio, sample_rate, entry, subtype)
        entry["expected"] = exp
        manifest["files"][path.relative_to(output_dir).as_posix()] = entry
        print(f"  ✅ {path.name} ({entry['duration_sec']:.1f}s)")

    # ------------------------------------------------------------------------
    # 12. Time-stretch (F5)
    # ------------------------------------------------------------------------
    print("📁 time_stretch/")
    fname = "pure_tone_440hz.wav"
    audio, exp = gen_pure_tone_stretch(duration, sample_rate, filename=fname)
    path = output_dir / "time_stretch" / fname
    entry = {}
    write_wav(path, audio, sample_rate, entry, subtype)
    entry["expected"] = exp
    manifest["files"][path.relative_to(output_dir).as_posix()] = entry
    print(f"  ✅ {path.name} ({entry['duration_sec']:.1f}s)")

    # ------------------------------------------------------------------------
    # 13. Degenerados (F7)
    # ------------------------------------------------------------------------
    print("📁 degenerate/")
    degen = gen_degenerate_cases(duration, sample_rate, "degenerate")
    for path_rel, audio, exp in degen:
        path = output_dir / "degenerate" / path_rel
        entry = {}
        write_wav(path, audio, sample_rate, entry, subtype)
        entry["expected"] = exp
        manifest["files"][path.relative_to(output_dir).as_posix()] = entry
        print(f"  ✅ {path.name} ({entry['duration_sec']:.1f}s)")

    # ------------------------------------------------------------------------
    # 14. Corrompido (F6)
    # ------------------------------------------------------------------------
    print("📁 corrupted/")
    path_rel, data, exp = gen_corrupted_wav(duration, sample_rate, "corrupted")
    path = output_dir / "corrupted" / path_rel
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "wb") as f:
        f.write(data)
    entry = {
        "sample_rate": sample_rate,
        "channels": 1,
        "duration_sec": None,
        "sha256": hashlib.sha256(data).hexdigest(),
        "expected": exp,
    }
    manifest["files"][path.relative_to(output_dir).as_posix()] = entry
    print(f"  ✅ {path.name} (corrompido, {len(data)} bytes)")

    # ------------------------------------------------------------------------
    # 15. Estéreo
    # ------------------------------------------------------------------------
    print("📁 stereo/")
    t = np.linspace(0, duration, int(sample_rate * duration), endpoint=False)
    left = 0.8 * np.sin(2.0 * np.pi * 440 * t)
    right = 0.8 * np.sin(2.0 * np.pi * 440 * t + np.pi / 3)
    audio_stereo = np.column_stack((left, right))
    fname = "stereo_440hz.wav"
    path = output_dir / "stereo" / fname
    entry = {}
    write_wav(path, audio_stereo, sample_rate, entry, subtype)
    entry["expected"] = {
        "channels": 2,
        "sample_peak_db": float(linear_to_db(np.max(np.abs(audio_stereo)))),
        "true_peak_dbtp": float(linear_to_db(np.max(np.abs(audio_stereo)))),
    }
    manifest["files"][path.relative_to(output_dir).as_posix()] = entry
    print(f"  ✅ {path.name} (estéreo, {entry['duration_sec']:.1f}s)")

    # ------------------------------------------------------------------------
    # Manifesto
    # ------------------------------------------------------------------------
    manifest_path = output_dir / "manifest.json"
    with open(manifest_path, "w") as f:
        json.dump(manifest, f, indent=2)

    print(f"\n✅ Manifesto gerado: {manifest_path}")

    total_files = len(manifest["files"])
    total_size = sum(
        Path(output_dir / k).stat().st_size
        for k in manifest["files"].keys()
        if (output_dir / k).exists()
    )
    print(f"\n📊 Resumo:")
    print(f"   Arquivos gerados: {total_files}")
    print(f"   Tamanho total: {total_size / (1024*1024):.1f} MB")
    print(f"\n📂 {output_dir.absolute()}")
    print("\n💡 Lembre-se:")
    print("   - Este diretório está no .gitignore (gerar, não comitar).")
    print("   - Apenas o manifest.json deve ser commitado.")
    print("   - Para regenerar: python scripts/generate_fixtures.py")


if __name__ == "__main__":
    main()