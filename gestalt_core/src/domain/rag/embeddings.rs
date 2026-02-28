use async_trait::async_trait;
use candle_core::{Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config, DTYPE};
use tokenizers::Tokenizer;
use std::path::Path;

#[async_trait]
pub trait EmbeddingModel: Send + Sync {
    async fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>>;
}

pub struct BertEmbeddingModel {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
}

impl BertEmbeddingModel {
    pub fn new(
        config_path: impl AsRef<Path>,
        tokenizer_path: impl AsRef<Path>,
        weights_path: impl AsRef<Path>,
    ) -> anyhow::Result<Self> {
        let device = Device::Cpu;

        let config: Config = serde_json::from_str(&std::fs::read_to_string(config_path)?)?;
        let tokenizer = Tokenizer::from_file(tokenizer_path).map_err(anyhow::Error::msg)?;

        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[weights_path], DTYPE, &device)?
        };
        let model = BertModel::load(vb, &config)?;

        Ok(Self {
            model,
            tokenizer,
            device,
        })
    }
}

#[async_trait]
impl EmbeddingModel for BertEmbeddingModel {
    async fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        let tokens = self.tokenizer.encode(text, true).map_err(anyhow::Error::msg)?;
        let token_ids = tokens.get_ids().to_vec();
        let token_ids = Tensor::new(token_ids.as_slice(), &self.device)?.unsqueeze(0)?;
        let token_type_ids = token_ids.zeros_like()?;

        let embeddings = self.model.forward(&token_ids, &token_type_ids, None)?;

        // Mean pooling
        let (_n_batch, n_tokens, _hidden_size) = embeddings.dims3()?;
        let embeddings = (embeddings.sum(1)? / (n_tokens as f64))?;
        let embeddings = embeddings.get(0)?;

        Ok(embeddings.to_vec1::<f32>()?)
    }
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
