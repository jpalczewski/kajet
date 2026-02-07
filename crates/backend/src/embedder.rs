use anyhow::Result;
use candle_core::{Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config, DTYPE};
use hf_hub::{api::sync::Api, Repo, RepoType};
use kajet_core::traits::Embedder;
use std::sync::Mutex;
use tokenizers::{PaddingParams, PaddingStrategy, Tokenizer};

pub struct CandleEmbedder {
    model: Mutex<BertModel>,
    tokenizer: Mutex<Tokenizer>,
    device: Device,
    dim: usize,
}

impl CandleEmbedder {
    fn device() -> Result<Device> {
        #[cfg(target_os = "macos")]
        {
            Ok(Device::new_metal(0)?)
        }
        #[cfg(not(target_os = "macos"))]
        {
            Ok(Device::Cpu)
        }
    }

    pub fn new(model_id: &str) -> Result<Self> {
        let device = Self::device()?;
        let repo = Repo::with_revision(model_id.to_string(), RepoType::Model, "main".to_string());

        let api = Api::new()?;
        let api_repo = api.repo(repo);
        let config_path = api_repo.get("config.json")?;
        let tokenizer_path = api_repo.get("tokenizer.json")?;
        let weights_path = api_repo.get("model.safetensors")?;

        let config: Config = serde_json::from_str(&std::fs::read_to_string(config_path)?)?;
        let dim = config.hidden_size;

        let mut tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {}", e))?;
        tokenizer.with_padding(Some(PaddingParams {
            strategy: PaddingStrategy::BatchLongest,
            ..Default::default()
        }));

        let vb = unsafe { VarBuilder::from_mmaped_safetensors(&[weights_path], DTYPE, &device)? };
        let model = BertModel::load(vb, &config)?;

        Ok(Self {
            model: Mutex::new(model),
            tokenizer: Mutex::new(tokenizer),
            device,
            dim,
        })
    }
}

impl Embedder for CandleEmbedder {
    fn embed(&self, texts: Vec<&str>) -> Result<Vec<Vec<f32>>> {
        let start = std::time::Instant::now();
        let text_count = texts.len();
        let tokenizer = self.tokenizer.lock().unwrap();
        let encodings = tokenizer
            .encode_batch(texts.to_vec(), true)
            .map_err(|e| anyhow::anyhow!("Tokenization failed: {}", e))?;

        let token_ids: Vec<&[u32]> = encodings.iter().map(|e| e.get_ids()).collect();
        let attention_masks: Vec<&[u32]> =
            encodings.iter().map(|e| e.get_attention_mask()).collect();
        let type_ids: Vec<&[u32]> = encodings.iter().map(|e| e.get_type_ids()).collect();

        let to_tensor = |data: &[&[u32]]| -> Result<Tensor> {
            let len = data[0].len();
            let flat: Vec<i64> = data
                .iter()
                .flat_map(|s| s.iter().map(|&v| v as i64))
                .collect();
            Ok(Tensor::from_vec(flat, (data.len(), len), &self.device)?)
        };

        let token_ids_t = to_tensor(&token_ids)?;
        let attention_mask_t = to_tensor(&attention_masks)?;
        let type_ids_t = to_tensor(&type_ids)?;

        let model = self.model.lock().unwrap();
        let embeddings = model.forward(&token_ids_t, &type_ids_t, Some(&attention_mask_t))?;

        // Mean pooling with attention mask
        let attention_mask_f = attention_mask_t.to_dtype(candle_core::DType::F32)?;
        let mask = attention_mask_f
            .unsqueeze(2)?
            .broadcast_as(embeddings.shape())?;
        let masked = (embeddings * mask)?;
        let sum = masked.sum(1)?;
        let count = attention_mask_f.sum(1)?.unsqueeze(1)?;
        let pooled = sum.broadcast_div(&count)?;

        // L2 normalization
        let norm = pooled.sqr()?.sum(1)?.sqrt()?.unsqueeze(1)?;
        let normalized = pooled.broadcast_div(&norm)?;

        let embeddings = normalized.to_vec2()?;
        let dim = self.dim;
        tracing::debug!(
            texts = text_count,
            dim,
            elapsed_ms = start.elapsed().as_millis() as u64,
            "embedding complete"
        );
        tracing::trace!(embedding_first_5 = ?embeddings[0][..5], "raw embedding sample");
        Ok(embeddings)
    }

    fn dimension(&self) -> usize {
        self.dim
    }
}
