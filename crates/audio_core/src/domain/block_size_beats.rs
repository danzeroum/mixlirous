use crate::error::Error;
use serde::{Deserialize, Serialize};

/// Tamanho da janela de agrupamento de batidas em blocos, em n├║mero de
/// batidas ÔÇö validado na desserializa├º├úo, n├úo numa camada por cima
/// (`docs/16-CORRECOES-DSP` T0.0, I14). Mesmo padr├úo de [`crate::CrossfadeMs`]
/// (ver o coment├írio l├í para o racional completo).
///
/// **`MIN`/`MAX` aqui s├úo uma escolha minha, n├úo um n├║mero de `docs/04` ou
/// `docs/05`.** `docs/04-DOMINIO-DSP.md` ┬º5 s├│ d├í exemplos de uso (`4, 8 ou
/// 16`), sem declarar limite formal ÔÇö diferente dos outros 8 newtypes deste
/// lote, que t├¬m faixa documentada em algum lugar do reposit├│rio. `MIN = 1`
/// evita o caso degenerado de janela zero (`build_beat_blocks` j├í trata
/// `block_size_beats == 0` como entrada inv├ílida, retornando vazio ÔÇö o
/// newtype fecha isso um passo antes). `MAX = 64` d├í bastante margem acima
/// dos exemplos documentados sem abrir a porta para um valor absurdo (ex.:
/// milh├Áes) que faria sentido rejeitar mas que nenhum documento hoje pro├¡be
/// explicitamente. Se esse teto se mostrar errado em uso real, ├® s├│ ajustar
/// a constante ÔÇö o ponto do newtype ├® ter *um* lugar para isso, n├úo acertar
/// o n├║mero de primeira.
///
/// Sem entrada em `audio_agent::limits`/`validator`: `block_size_beats` ├®
/// campo de [`crate::SelectionConfig`], n├úo par├ómetro de tool call do agente
/// ÔÇö `docs/05-AGENTE-IA-HITL.md` ┬º3 exclui `block_selection` do escopo do
/// registry de prop├│sito (ainda n├úo tem representa├º├úo pr├│pria l├í).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "usize")]
pub struct BlockSizeBeats(usize);

impl BlockSizeBeats {
    pub const MIN: usize = 1;
    pub const MAX: usize = 64;

    pub fn get(self) -> usize {
        self.0
    }
}

impl TryFrom<usize> for BlockSizeBeats {
    type Error = Error;

    fn try_from(v: usize) -> Result<Self, Self::Error> {
        (Self::MIN..=Self::MAX)
            .contains(&v)
            .then_some(Self(v))
            .ok_or_else(|| {
                Error::Validation(format!(
                    "block_size_beats deve estar entre {} e {}; recebido {v}",
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
        assert!(BlockSizeBeats::try_from(BlockSizeBeats::MIN).is_ok());
        assert!(BlockSizeBeats::try_from(BlockSizeBeats::MAX).is_ok());
    }

    #[test]
    fn test_rejects_zero_and_above_max() {
        assert!(BlockSizeBeats::try_from(0).is_err());
        assert!(BlockSizeBeats::try_from(BlockSizeBeats::MAX + 1).is_err());
    }

    #[test]
    fn test_accepts_documented_examples() {
        for exemplo in [4, 8, 16] {
            assert!(BlockSizeBeats::try_from(exemplo).is_ok());
        }
    }

    #[test]
    fn test_get_roundtrips_the_value() {
        let v = BlockSizeBeats::try_from(4).unwrap();
        assert_eq!(v.get(), 4);
    }

    #[test]
    fn test_deserialize_rejects_out_of_range_value() {
        let err = serde_json::from_str::<BlockSizeBeats>("0").unwrap_err();
        assert!(err.to_string().contains("block_size_beats"));
    }

    #[test]
    fn test_deserialize_accepts_in_range_value() {
        let v: BlockSizeBeats = serde_json::from_str("4").unwrap();
        assert_eq!(v.get(), 4);
    }

    #[test]
    fn test_serialize_is_transparent_as_the_inner_value() {
        let v = BlockSizeBeats::try_from(4).unwrap();
        assert_eq!(serde_json::to_string(&v).unwrap(), "4");
    }
}
