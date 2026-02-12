use crate::config::RemoteEmbedderConfig;
use crate::error::RemoteEmbedderError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct EmbedRequest<'a> {
    pub model: &'a str,
    pub input: &'a [&'a str],
}

#[derive(Debug, Clone, Deserialize)]
pub struct EmbedResponse {
    pub data: Vec<EmbedData>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EmbedData {
    pub embedding: Vec<f32>,
    pub index: usize,
}

#[async_trait]
pub trait RemoteTransport: Send + Sync {
    async fn embed(&self, request: &EmbedRequest<'_>)
    -> Result<EmbedResponse, RemoteEmbedderError>;
}

#[derive(Clone)]
pub struct ReqwestTransport {
    client: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
}

impl ReqwestTransport {
    pub fn new(cfg: &RemoteEmbedderConfig) -> Result<Self, RemoteEmbedderError> {
        let client = reqwest::Client::builder()
            .connect_timeout(cfg.connect_timeout)
            .timeout(cfg.request_timeout)
            .pool_max_idle_per_host(4)
            .build()
            .map_err(|e| RemoteEmbedderError::Transport(e.to_string()))?;

        Ok(Self {
            client,
            base_url: cfg.base_url.trim_end_matches('/').to_string(),
            api_key: cfg.api_key.clone(),
        })
    }
}

#[async_trait]
impl RemoteTransport for ReqwestTransport {
    async fn embed(
        &self,
        request: &EmbedRequest<'_>,
    ) -> Result<EmbedResponse, RemoteEmbedderError> {
        let url = format!("{}/v1/embeddings", self.base_url);
        let mut req = self.client.post(url).json(request);
        if let Some(key) = &self.api_key {
            req = req.bearer_auth(key);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| RemoteEmbedderError::Transport(e.to_string()))?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(RemoteEmbedderError::HttpStatus {
                status: status.as_u16(),
                body,
            });
        }

        resp.json::<EmbedResponse>()
            .await
            .map_err(|e| RemoteEmbedderError::Transport(e.to_string()))
    }
}
