//! Library target do `audio_api` — expõe módulos internos para testes de
//! integração em `crates/audio_api/tests/`.
//!
//! O binário principal (`main.rs`) fica como está. Esta lib só existe
//! porque integration tests (em `tests/`) precisam de uma lib crate
//! para importar — o binário sozinho não expõe API pública.

pub mod adapters;
pub mod atomic;
pub mod audit;
pub mod cleanup;
pub mod config;
pub mod instrument;
pub mod metrics;
pub mod middleware;
pub mod recovery;
pub mod routes;
pub mod sse;
pub mod state;
pub mod storage;
pub mod worker;
