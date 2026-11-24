use async_trait::async_trait;

use crate::EmbedError;

/// Turns texts into vectors. One vector per text, in the same order.
#[async_trait]
pub trait Embedder: Send + Sync {
    /// Size of each vector, e.g. 384. Qdrant only accepts vectors of its collection's size.
    fn dimensions(&self) -> usize;

    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedError>;
}
