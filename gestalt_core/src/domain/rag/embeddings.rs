use async_trait::async_trait;

/// A trait representing a model capable of generating high-dimensional vector embeddings from text.
///
/// Implementations must be `Send` and `Sync` to allow concurrent utilization across threads.
#[async_trait]
pub trait EmbeddingModel: Send + Sync {
    /// Generates a dense vector embedding for the given input text.
    ///
    /// # Parameters
    /// - `text`: The string slice to embed.
    ///
    /// # Returns
    /// - `Ok(Vec<f32>)`: The generated high-dimensional embedding vector.
    /// - `Err`: If the embedding generation fails, e.g. due to model load issues or service timeout.
    async fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>>;
}

// Dummy implementation for when models are not available or for testing
pub struct DummyEmbeddingModel {
    dim: usize,
}

impl DummyEmbeddingModel {
    pub fn new(dim: usize) -> Self {
        Self { dim }
    }
}

#[async_trait]
impl EmbeddingModel for DummyEmbeddingModel {
    async fn embed(&self, _text: &str) -> anyhow::Result<Vec<f32>> {
        // Return a pseudo-random but deterministic vector based on text hash to simulate embeddings
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        _text.hash(&mut hasher);
        let hash = hasher.finish();

        let mut vec = vec![0.0; self.dim];
        for (i, item) in vec.iter_mut().enumerate().take(self.dim) {
            *item = ((hash.wrapping_add(i as u64) % 1000) as f32) / 1000.0;
        }
        Ok(vec)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dummy_embedding_model_dimensionality() {
        let model = DummyEmbeddingModel::new(128);
        let embedding = model.embed("Hello world").await.unwrap();
        assert_eq!(embedding.len(), 128);

        let model_large = DummyEmbeddingModel::new(768);
        let embedding_large = model_large.embed("Hello world").await.unwrap();
        assert_eq!(embedding_large.len(), 768);
    }

    #[tokio::test]
    async fn test_dummy_embedding_model_deterministic() {
        let model = DummyEmbeddingModel::new(64);
        let embed_1 = model.embed("constant text").await.unwrap();
        let embed_2 = model.embed("constant text").await.unwrap();
        assert_eq!(embed_1, embed_2);

        let embed_diff = model.embed("different text").await.unwrap();
        assert_ne!(embed_1, embed_diff);
    }

    #[tokio::test]
    async fn test_dummy_embedding_model_empty_text() {
        let model = DummyEmbeddingModel::new(16);
        let embed_empty = model.embed("").await.unwrap();
        assert_eq!(embed_empty.len(), 16);
    }
}
