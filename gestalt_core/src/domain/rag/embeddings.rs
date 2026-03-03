use async_trait::async_trait;

#[async_trait]
pub trait EmbeddingModel: Send + Sync {
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
