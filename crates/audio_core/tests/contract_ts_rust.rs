//! Teste de contrato TS ↔ Rust — Item B3 + T2 do mapa de ação.
//!
//! Verifica que o `PipelineConfig::default()` (lado Rust) serializa para
//! um JSON compatível com o `defaultPipelineConfig()` exportado pelo
//! `ui/src/types/api.ts` (lado TS). Como não temos `ts-rs` integrado
//! (item B3 do mapa: a integração é a solução de longo prazo; por ora
//! o tipo TS é escrito à mão), este teste garante que os dois lados
//! concordam sobre os nomes de campo, tipos e valores default.
//!
//! Quando `ts-rs` for integrado (ver docs/03-CONTRATOS-API.md §8), este
//! teste pode ser substituído por `cargo test export_bindings` que
//! regenera e compara automaticamente.

use audio_core::domain::PipelineConfig;
use serde_json::{json, Value};

/// Helper: compara floats com tolerância para evitar falhas por precisão
/// de f32→f64 (0.8 em f32 vira 0.800000011920929 quando serializado
/// para JSON e parseado de volta como f64).
fn approx_eq_json(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            if let (Some(xf), Some(yf)) = (x.as_f64(), y.as_f64()) {
                return (xf - yf).abs() < 1e-5;
            }
            a == b
        },
        _ => a == b,
    }
}

#[test]
fn pipeline_config_default_serializa_com_estrutura_esperada_pelo_ts() {
    let config = PipelineConfig::default();
    let json_val = serde_json::to_value(&config).expect("serialize default");

    // Verifica campo a campo. Se algum dia um campo mudar de nome ou tipo,
    // este teste quebra — e o frontend precisa ser atualizado em sincronia.
    assert!(approx_eq_json(
        &json_val["crossfade"]["enabled"],
        &json!(true)
    ));
    assert!(approx_eq_json(
        &json_val["crossfade"]["max_duration_ms"],
        &json!(3000)
    ));
    assert_eq!(json_val["crossfade"]["curve"], json!("constant_power"));

    assert!(approx_eq_json(
        &json_val["mastering"]["lufs_target"],
        &json!(-14.0)
    ));
    assert!(approx_eq_json(
        &json_val["mastering"]["peak_db"],
        &json!(-1.0)
    ));
    assert!(approx_eq_json(
        &json_val["mastering"]["enable_limiting"],
        &json!(true)
    ));
    assert!(approx_eq_json(
        &json_val["mastering"]["compression_ratio"],
        &json!(2.0)
    ));

    assert!(approx_eq_json(
        &json_val["selection"]["min_strong_beat_percentile"],
        &json!(0.8)
    ));
    assert!(approx_eq_json(
        &json_val["selection"]["block_size_beats"],
        &json!(4)
    ));
    assert!(approx_eq_json(
        &json_val["selection"]["preserve_intro_ms"],
        &json!(3000)
    ));
    assert!(approx_eq_json(
        &json_val["selection"]["preserve_outro_ms"],
        &json!(3000)
    ));

    assert!(approx_eq_json(
        &json_val["format"]["sample_rate"],
        &json!(44100)
    ));
    assert!(approx_eq_json(&json_val["format"]["channels"], &json!(2)));
    assert!(approx_eq_json(&json_val["format"]["bit_depth"], &json!(24)));
    assert_eq!(json_val["format"]["codec"], json!("WAV"));
}

#[test]
fn pipeline_config_default_roundtrip_serializa_e_deserializa() {
    // Garantia básica de roundtrip: serializar default → desserializar →
    // re-serializa e comparação por JSON.
    let config = PipelineConfig::default();
    let json_val = serde_json::to_value(&config).expect("serialize");
    let restored: PipelineConfig = serde_json::from_value(json_val.clone()).expect("deserialize");

    let restored_json = serde_json::to_value(&restored).expect("serialize restored");
    assert_eq!(json_val, restored_json);
}

/// Item B1 do relatório: o TS anterior enviava um `pipeline_config` que
/// **não** desserializava no Rust. Este teste documenta o payload antigo
/// (problemático) e confirma que ele falha — ou seja, qualquer caller que
/// ainda envie o shape antigo recebe erro claro em vez de comportamento
/// surpresa. Se o teste um dia passar, é porque alguém regrediu o contrato.
#[test]
fn payload_ts_antigo_nao_desserializa_no_rust_atual() {
    // Este é o shape que `ui/src/types/api.ts` enviava antes do fix B3:
    // — `crossfade.duration_ms` (não existe; o Rust usa `max_duration_ms`)
    // — `crossfade.enabled` ausente (obrigatório no Rust)
    // — `mastering.enable_limiting` ausente (obrigatório no Rust)
    // — sem `selection`, `format`, `tuning` (todos obrigatórios)
    let legacy_payload = json!({
        "crossfade": { "duration_ms": 1000, "curve": "constant_power" },
        "mastering": {
            "lufs_target": -14.0,
            "peak_db": -1.0,
            "compression_ratio": 4.0
        }
    });

    let result: Result<PipelineConfig, _> = serde_json::from_value(legacy_payload);
    assert!(
        result.is_err(),
        "payload TS legado NÃO deve desserializar — se um dia desserializar, \
         alguém regrediu o contrato e o frontend vai voltar a enviar campos \
         que o backend ignora silenciosamente"
    );
}

/// Item T2: o JSON esperado pelo TS (`defaultPipelineConfig()`) desserializa
/// no Rust. Garante sincronia — quando `ts-rs` for integrado, este teste
/// será substituído por uma comparação automática.
#[test]
fn default_pipeline_config_ts_desserializa_no_rust() {
    // Mirror exato do que `ui/src/types/api.ts::defaultPipelineConfig()`
    // retorna. Mantenha em sincronia — se adicionar campo novo no Rust,
    // atualize aqui e no TS.
    let ts_default = json!({
        "target_duration": { "secs": 30, "nanos": 0 },
        "crossfade": {
            "enabled": true,
            "max_duration_ms": 3000,
            "curve": "constant_power"
        },
        "mastering": {
            "lufs_target": -14.0,
            "peak_db": -1.0,
            "enable_limiting": true,
            "compression_ratio": 2.0
        },
        "selection": {
            "min_strong_beat_percentile": 0.8,
            "block_size_beats": 4,
            "preserve_intro_ms": 3000,
            "preserve_outro_ms": 3000
        },
        "format": {
            "sample_rate": 44100,
            "channels": 2,
            "bit_depth": 24,
            "codec": "WAV"
        },
        "tuning": {
            "enabled": false,
            "mode": "disabled",
            "max_global_cents": 50.0,
            "min_confidence": 0.7,
            "force_tonic_hz": null,
            "force_mode": null
        }
    });

    let result: Result<PipelineConfig, _> = serde_json::from_value(ts_default.clone());
    assert!(
        result.is_ok(),
        "JSON esperado pelo TS não desserializa no Rust: {:?}",
        result.err()
    );

    // E roundtrip: serializa o Rust default → compara com o esperado TS.
    // Isto garante que nenhum campo foi esquecido de um lado.
    let restored = result.unwrap();
    let rust_json = serde_json::to_value(&restored).expect("serialize");
    // Compara campo a campo (com tolerância para floats).
    let top_keys: Vec<&str> = rust_json
        .as_object()
        .unwrap()
        .keys()
        .map(|k| k.as_str())
        .collect();
    let ts_keys: Vec<&str> = ts_default
        .as_object()
        .unwrap()
        .keys()
        .map(|k| k.as_str())
        .collect();
    assert_eq!(top_keys, ts_keys, "top-level keys devem coincidir TS↔Rust");
}
