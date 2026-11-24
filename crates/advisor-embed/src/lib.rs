//! Text embeddings: turning text into the vectors Qdrant searches with.
//! The vectors must come from the same model that embedded the stored news,
//! so this crate calls the same embedding service the ingestion used.

mod embed_error;
mod embedder;
mod http_embedder;

pub use embed_error::EmbedError;
pub use embedder::Embedder;
pub use http_embedder::{HttpEmbedder, HttpEmbedderConfig};
