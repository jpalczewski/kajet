use anyhow::Result;
use candle_core::{Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config, DTYPE};
use hf_hub::{Repo, RepoType, api::sync::Api};
use kajet_core::traits::Embedder;
use std::sync::{Arc, Mutex};
use tokenizers::{PaddingParams, PaddingStrategy, Tokenizer};

pub struct CandleEmbedder {
    model: Arc<Mutex<BertModel>>,
    tokenizer: Arc<Mutex<Tokenizer>>,
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
            model: Arc::new(Mutex::new(model)),
            tokenizer: Arc::new(Mutex::new(tokenizer)),
            device,
            dim,
        })
    }

    fn embed_sync(
        model: &Mutex<BertModel>,
        tokenizer: &Mutex<Tokenizer>,
        device: &Device,
        dim: usize,
        texts: &[String],
    ) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let start = std::time::Instant::now();
        let text_count = texts.len();
        let text_refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();

        let tokenizer = tokenizer.lock().unwrap();
        let encodings = tokenizer
            .encode_batch(text_refs, true)
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
            Ok(Tensor::from_vec(flat, (data.len(), len), device)?)
        };

        let token_ids_t = to_tensor(&token_ids)?;
        let attention_mask_t = to_tensor(&attention_masks)?;
        let type_ids_t = to_tensor(&type_ids)?;

        let model = model.lock().unwrap();
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
        tracing::debug!(
            texts = text_count,
            dim,
            elapsed_ms = start.elapsed().as_millis() as u64,
            "embedding complete"
        );
        if let Some(first) = embeddings.first() {
            let preview_len = first.len().min(5);
            tracing::trace!(embedding_first_5 = ?first[..preview_len], "raw embedding sample");
        }
        Ok(embeddings)
    }
}

#[async_trait::async_trait]
impl Embedder for CandleEmbedder {
    async fn embed(&self, texts: Vec<&str>) -> Result<Vec<Vec<f32>>> {
        let owned: Vec<String> = texts.into_iter().map(str::to_owned).collect();
        let model = self.model.clone();
        let tokenizer = self.tokenizer.clone();
        let device = self.device.clone();
        let dim = self.dim;

        tokio::task::spawn_blocking(move || {
            Self::embed_sync(&model, &tokenizer, &device, dim, &owned)
        })
        .await?
    }

    fn dimension(&self) -> usize {
        self.dim
    }
}
