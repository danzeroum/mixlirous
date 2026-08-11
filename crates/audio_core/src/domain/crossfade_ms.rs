use crate::error::Error;
use serde::{Deserialize, Serialize};

/// Dura├º├úo de crossfade, em milissegundos ÔÇö validada na desserializa├º├úo, n├úo
/// numa camada por cima (`docs/16-CORRECOES-DSP` T0.0, I14).
///
/// O ponto n├úo ├® validar ÔÇö ├® tornar `CrossfadeMs` fora da faixa
/// **irrepresent├ível**. Isso s├│ vale enquanto n├úo houver porta dos fundos:
/// sem `Default` (qual seria o crossfade "padr├úo" de ningu├®m ter pedido um?),
/// sem `From<u32>` infal├¡vel, sem operador aritm├®tico (`a + b` fora da faixa
/// n├úo erraria alto, erraria calado), sem construtor de teste que pule
/// `TryFrom`. Se uma dessas for adicionada depois "s├│ para um teste", a
/// garantia inteira volta a valer nada ÔÇö ├® exatamente o tipo de furo que n├úo
/// aparece em `git diff` a menos que algu├®m procure por ele.
///
/// `MIN`/`MAX` s├úo a fonte can├┤nica: `audio_agent::limits` deriva os limites
/// do registry a partir daqui (n├úo o contr├írio), e um teste de deriva em
/// `audio_agent` prende os dois n├║meros juntos ÔÇö fecha o terceiro lugar da
/// regra do `CONTRIBUTING.md` (docs/05 ┬º3, validador, schema da UI) sem
/// precisar redigitar `0`/`3000` numa quarta vez.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u32")]
pub struct CrossfadeMs(u32);

impl CrossfadeMs {
    pub const MIN: u32 = 0;
    pub const MAX: u32 = 3000;

    pub fn get(self) -> u32 {
        self.0
    }
}

impl TryFrom<u32> for CrossfadeMs {
    type Error = Error;

    fn try_from(v: u32) -> Result<Self, Self::Error> {
        (Self::MIN..=Self::MAX)
            .contains(&v)
            .then_some(Self(v))
            .ok_or_else(|| {
                Error::Validation(format!(
                    "crossfade_ms deve estar entre {} e {}; recebido {v}",
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
        assert!(CrossfadeMs::try_from(CrossfadeMs::MIN).is_ok());
        assert!(CrossfadeMs::try_from(CrossfadeMs::MAX).is_ok());
    }

    #[test]
    fn test_rejects_below_min() {
        // MIN ├® 0 (u32 n├úo tem valor abaixo), ent├úo a borda real que importa
        // testar ├® a constru├º├úo com u32::MAX ÔÇö a maior entrada poss├¡vel do
        // tipo de origem, para provar que o teto realmente barra.
        assert!(CrossfadeMs::try_from(CrossfadeMs::MAX + 1).is_err());
        assert!(CrossfadeMs::try_from(u32::MAX).is_err());
    }

    #[test]
    fn test_get_roundtrips_the_value() {
        let v = CrossfadeMs::try_from(1500).unwrap();
        assert_eq!(v.get(), 1500);
    }

    #[test]
    fn test_deserialize_rejects_out_of_range_value() {
        // A garantia que T0.0 existe para dar: um valor inv├ílido n├úo
        // sobrevive nem ├á desserializa├º├úo, muito antes de qualquer camada de
        // valida├º├úo rodar por cima.
        let err = serde_json::from_str::<CrossfadeMs>("50000").unwrap_err();
        assert!(err.to_string().contains("crossfade_ms"));
    }

    #[test]
    fn test_deserialize_accepts_in_range_value() {
        let v: CrossfadeMs = serde_json::from_str("1000").unwrap();
        assert_eq!(v.get(), 1000);
    }

    #[test]
    fn test_serialize_is_transparent_as_the_inner_value() {
        let v = CrossfadeMs::try_from(1000).unwrap();
        assert_eq!(serde_json::to_string(&v).unwrap(), "1000");
    }
}
