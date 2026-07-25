use crate::error::Error;
use serde::{Deserialize, Serialize};

/// Percentil de força de batida usado para separar batidas fortes de fracas
/// (`docs/04-DOMINIO-DSP.md` §B.3, `strong_beat_threshold`) — expresso como
/// fração `0.0..=1.0`, não `0..=100` (`percentile = 0.8` no exemplo
/// documentado). Validado na desserialização, não numa camada por cima
/// (`docs/16-CORRECOES-DSP` T0.0, I14). Mesmo padrão de [`crate::CrossfadeMs`]
/// (ver o comentário lá para o racional completo).
///
/// `MIN = 0.0`/`MAX = 1.0` vêm diretamente da semântica de "percentil como
/// fração" que `docs/04` já usa (`np.percentile(beat_strength, 80)` vira
/// `percentile = 0.8`) — não é uma escolha minha como em
/// [`crate::BlockSizeBeats`].
///
/// Sem entrada em `audio_agent::limits`/`validator`: é campo de
/// [`crate::SelectionConfig`], não parâmetro de tool call do agente —
/// `docs/05-AGENTE-IA-HITL.md` §3 exclui `block_selection` do escopo do
/// registry de propósito.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "f32")]
pub struct Percentile(f32);

impl Percentile {
    pub const MIN: f32 = 0.0;
    pub const MAX: f32 = 1.0;

    pub fn get(self) -> f32 {
        self.0
    }
}

impl TryFrom<f32> for Percentile {
    type Error = Error;

    fn try_from(v: f32) -> Result<Self, Self::Error> {
        (Self::MIN..=Self::MAX)
            .contains(&v)
            .then_some(Self(v))
            .ok_or_else(|| {
                Error::Validation(format!(
                    "percentile deve estar entre {} e {}; recebido {v}",
                    Self::MIN,
                    Self::MAX
                ))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_accepts_min_and_max_boundaries() {
        assert!(Percentile::try_from(Percentile::MIN).is_ok());
        assert!(Percentile::try_from(Percentile::MAX).is_ok());
    }

    #[test]
    fn test_rejects_outside_bounds() {
        assert!(Percentile::try_from(Percentile::MIN - 0.01).is_err());
        assert!(Percentile::try_from(Percentile::MAX + 0.01).is_err());
    }

    #[test]
    fn test_rejects_nan() {
        assert!(Percentile::try_from(f32::NAN).is_err());
    }

    #[test]
    fn test_get_roundtrips_the_value() {
        let v = Percentile::try_from(0.8).unwrap();
        assert_eq!(v.get(), 0.8);
    }

    #[test]
    fn test_deserialize_rejects_out_of_range_value() {
        let err = serde_json::from_str::<Percentile>("80.0").unwrap_err();
        assert!(err.to_string().contains("percentile"));
    }

    #[test]
    fn test_deserialize_accepts_in_range_value() {
        let v: Percentile = serde_json::from_str("0.8").unwrap();
        assert_eq!(v.get(), 0.8);
    }

    #[test]
    fn test_serialize_is_transparent_as_the_inner_value() {
        let v = Percentile::try_from(0.8).unwrap();
        assert_eq!(serde_json::to_string(&v).unwrap(), "0.8");
    }
}
