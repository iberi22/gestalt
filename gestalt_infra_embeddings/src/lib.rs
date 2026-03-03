#[cfg(feature = "bert")]
use async_trait::async_trait;
#[cfg(feature = "bert")]
use gestalt_core::domain::rag::embeddings::EmbeddingModel;

#[cfg(feature = "bert")]
use candle_core::{Device, Tensor};
#[cfg(feature = "bert")]
use candle_nn::VarBuilder;
#[cfg(feature = "bert")]
use candle_transformers::models::bert::{BertModel, Config, DTYPE};
#[cfg(feature = "bert")]
use std::path::Path;
#[cfg(feature = "bert")]
use tokenizers::Tokenizer;

#[cfg(feature = "bert")]
pub struct BertEmbeddingModel {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
}

#[cfg(feature = "bert")]
impl BertEmbeddingModel {
    pub fn new(
        config_path: impl AsRef<Path>,
        tokenizer_path: impl AsRef<Path>,
        weights_path: impl AsRef<Path>,
    ) -> anyhow::Result<Self> {
        let device = Device::Cpu;

        let config: Config = serde_json::from_str(&std::fs::read_to_string(config_path)?)?;
        let tokenizer = Tokenizer::from_file(tokenizer_path).map_err(anyhow::Error::msg)?;

        // SAFETY: `weights_path` points to immutable safetensors weights owned by the runtime,
        // `DTYPE` matches the model tensor dtype, and the returned VarBuilder is used read-only.
        // Callers are responsible for providing valid model artifacts and compatible device setup.
        let vb = unsafe { VarBuilder::from_mmaped_safetensors(&[weights_path], DTYPE, &device)? };
        let model = BertModel::load(vb, &config)?;

        Ok(Self {
            model,
            tokenizer,
            device,
        })
    }
}

#[cfg(feature = "bert")]
#[async_trait]
impl EmbeddingModel for BertEmbeddingModel {
    async fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        let tokens = self
            .tokenizer
            .encode(text, true)
            .map_err(anyhow::Error::msg)?;
        let token_ids = tokens.get_ids().to_vec();
        let token_ids = Tensor::new(token_ids.as_slice(), &self.device)?.unsqueeze(0)?;
        let token_type_ids = token_ids.zeros_like()?;

        let embeddings = self.model.forward(&token_ids, &token_type_ids, None)?;

        let (_n_batch, n_tokens, _hidden_size) = embeddings.dims3()?;
        let embeddings = (embeddings.sum(1)? / (n_tokens as f64))?;
        let embeddings = embeddings.get(0)?;

        Ok(embeddings.to_vec1::<f32>()?)
    }
}
