use anyhow::Result;
use kajet_core::config::EmbeddingConfig;
use kajet_core::traits::Embedder;
use std::sync::Arc;

/// Create an embedder from config.
/// Used both at startup and for hot-swapping when config changes.
pub async fn create_embedder(config: &EmbeddingConfig) -> Result<Arc<dyn Embedder>> {
    match config.backend {
        kajet_core::config::EmbeddingBackend::Candle => {
            Ok(Arc::new(kajet_backend::CandleEmbedder::new(&config.model)?))
        }
        kajet_core::config::EmbeddingBackend::Remote => {
            let mut remote_cfg = kajet_remote::RemoteEmbedderConfig {
                base_url: config.base_url.clone(),
                model: config.model.clone(),
                api_key: if config.api_key.is_empty() {
                    None
                } else {
                    Some(config.api_key.clone())
                },
                ..kajet_remote::RemoteEmbedderConfig::default()
            };
            remote_cfg.max_batch_size = config.remote_max_batch_size.max(1);
            remote_cfg.max_input_chars = config.remote_max_input_chars.max(128);
            Ok(Arc::new(
                kajet_remote::RemoteEmbedder::connect_with_config(remote_cfg)
                    .await
                    .map_err(anyhow::Error::new)?,
            ))
        }
    }
}
