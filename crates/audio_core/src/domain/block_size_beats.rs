use crate::error::Error;
use serde::{Deserialize, Serialize};

/// Tamanho da janela de agrupamento de batidas em blocos, em número de
/// batidas — validado na desserialização, não numa camada por cima
/// (`docs/16-CORRECOES-DSP` T0.0, I14). Mesmo padrão de [`crate::CrossfadeMs`]
/// (ver o comentário lá para o racional completo).
///
/// **`MIN`/`MAX` aqui são uma escolha minha, não um número de `docs/04` ou
/// `docs/05`.** `docs/04-DOMINIO-DSP.md` §5 só dá exemplos de uso (`4, 8 ou
/// 16`), sem declarar limite formal — diferente dos outros 8 newtypes deste
/// lote, que têm faixa documentada em algum lugar do repositório. `MIN = 1`
/// evita o caso degenerado de janela zero (`build_beat_blocks` já trata
/// `block_size_beats == 0` como entrada inválida, retornando vazio — o
/// newtype fecha isso um passo antes). `MAX = 64` dá bastante margem acima
/// dos exemplos documentados sem abrir a porta para um valor absurdo (ex.:
/// milhões) que faria sentido rejeitar mas que nenhum documento hoje proíbe
/// explicitamente. Se esse teto se mostrar errado em uso real, é só ajustar
/// a constante — o ponto do newtype é ter *um* lugar para isso, não acertar
/// o número de primeira.
///
/// Sem entrada em `audio_agent::limits`/`validator`: `block_size_beats` é
/// campo de [`crate::SelectionConfig`], não parâmetro de tool call do agente
/// — `docs/05-AGENTE-IA-HITL.md` §3 exclui `block_selection` do escopo do
/// registry de propósito (ainda não tem representação própria lá).
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
