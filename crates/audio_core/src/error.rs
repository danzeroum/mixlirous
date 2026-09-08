/// Erro do domínio + DSP (`audio_core`)
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Decoding error: {0}")]
    Decode(#[from] symphonia::core::errors::Error),
    #[error("Resampling error: {0}")]
    Resample(String),
    #[error("Validation failed: {0}")]
    Validation(String),
}
