use crate::config::RemoteEmbedderConfig;
use crate::error::RemoteEmbedderError;
use crate::transport::{EmbedRequest, RemoteTransport, ReqwestTransport};
use crate::validator::validate_and_reorder;
use async_trait::async_trait;
use kajet_core::traits::Embedder;
use std::sync::Arc;

pub struct RemoteEmbedder {
    config: RemoteEmbedderConfig,
    transport: Arc<dyn RemoteTransport>,
    dim: Option<usize>,
}

impl RemoteEmbedder {
    pub fn new(config: RemoteEmbedderConfig, transport: Arc<dyn RemoteTransport>) -> Self {
        Self {
            config,
            transport,
            dim: None,
        }
    }

    pub async fn connect(
        base_url: &str,
        model: &str,
        api_key: Option<&str>,
    ) -> Result<Self, RemoteEmbedderError> {
        let cfg = RemoteEmbedderConfig {
            base_url: base_url.to_string(),
            model: model.to_string(),
            api_key: api_key.map(str::to_owned),
            ..RemoteEmbedderConfig::default()
        };
        let transport = Arc::new(ReqwestTransport::new(&cfg)?);
        let mut embedder = Self::new(cfg, transport);
        embedder.probe().await?;
        tracing::info!(
            base_url = %embedder.config.base_url,
            model = %embedder.config.model,
            dim = embedder.dim.unwrap_or_default(),
            "Remote embedder connected"
        );
        Ok(embedder)
    }

    pub async fn probe(&mut self) -> Result<(), RemoteEmbedderError> {
        let out = self
            .embed_with_retry(vec!["hello"])
            .await
            .map_err(|e| RemoteEmbedderError::ProbeFailed(e.to_string()))?;
        let dim = out
            .first()
            .map(std::vec::Vec::len)
            .ok_or_else(|| RemoteEmbedderError::ProbeFailed("empty probe response".into()))?;
        self.dim = Some(dim);
        Ok(())
    }

    async fn embed_with_retry(
        &self,
        texts: Vec<&str>,
    ) -> Result<Vec<Vec<f32>>, RemoteEmbedderError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let mut attempt = 0u32;
        let mut backoff = self.config.initial_backoff;
        loop {
            let req = EmbedRequest {
                model: &self.config.model,
                input: &texts,
            };
            match self.transport.embed(&req).await {
                Ok(resp) => {
                    let pairs = resp
                        .data
                        .into_iter()
                        .map(|d| (d.index, d.embedding))
                        .collect::<Vec<_>>();
                    return validate_and_reorder(texts.len(), pairs, self.dim);
                }
                Err(RemoteEmbedderError::HttpStatus { status, .. })
                    if (status == 429 || status >= 500) && attempt < self.config.max_retries =>
                {
                    tracing::debug!(attempt, status, "retrying remote embed request");
                }
                Err(RemoteEmbedderError::Transport(_)) if attempt < self.config.max_retries => {
                    tracing::debug!(attempt, "retrying remote embed request");
                }
                Err(e) => return Err(e),
            }

            attempt += 1;
            tokio::time::sleep(backoff).await;
            backoff = backoff.saturating_mul(2);
        }
    }
}

#[async_trait]
impl Embedder for RemoteEmbedder {
    async fn embed(&self, texts: Vec<&str>) -> anyhow::Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        if texts.len() <= self.config.max_batch_size {
            return self
                .embed_with_retry(texts)
                .await
                .map_err(anyhow::Error::new);
        }

        let mut out = Vec::with_capacity(texts.len());
        for batch in texts.chunks(self.config.max_batch_size) {
            out.extend(
                self.embed_with_retry(batch.to_vec())
                    .await
                    .map_err(anyhow::Error::new)?,
            );
        }
        Ok(out)
    }

    fn dimension(&self) -> usize {
        self.dim.unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::{EmbedData, EmbedResponse};
    use std::collections::VecDeque;
    use std::sync::Mutex;

    struct FakeTransport {
        responses: Mutex<VecDeque<Result<EmbedResponse, RemoteEmbedderError>>>,
        calls: Mutex<usize>,
    }

    #[async_trait]
    impl RemoteTransport for FakeTransport {
        async fn embed(
            &self,
            _request: &EmbedRequest<'_>,
        ) -> Result<EmbedResponse, RemoteEmbedderError> {
            *self.calls.lock().unwrap() += 1;
            self.responses
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| Err(RemoteEmbedderError::Transport("no response".into())))
        }
    }

    fn ok_response(data: Vec<(usize, Vec<f32>)>) -> Result<EmbedResponse, RemoteEmbedderError> {
        Ok(EmbedResponse {
            data: data
                .into_iter()
                .map(|(index, embedding)| EmbedData { index, embedding })
                .collect(),
        })
    }

    #[tokio::test]
    async fn probe_discovers_dimension() {
        let transport = Arc::new(FakeTransport {
            responses: Mutex::new(VecDeque::from([ok_response(vec![(
                0,
                vec![0.1, 0.2, 0.3],
            )])])),
            calls: Mutex::new(0),
        });
        let cfg = RemoteEmbedderConfig::default();
        let mut embedder = RemoteEmbedder::new(cfg, transport);
        embedder.probe().await.unwrap();
        assert_eq!(embedder.dimension(), 3);
    }

    #[tokio::test]
    async fn embed_retries_on_5xx_then_succeeds() {
        let transport = Arc::new(FakeTransport {
            responses: Mutex::new(VecDeque::from([
                Err(RemoteEmbedderError::HttpStatus {
                    status: 500,
                    body: "x".into(),
                }),
                ok_response(vec![(0, vec![0.1, 0.2])]),
            ])),
            calls: Mutex::new(0),
        });
        let mut cfg = RemoteEmbedderConfig::default();
        cfg.max_retries = 3;
        let mut embedder = RemoteEmbedder::new(cfg, transport.clone());
        embedder.dim = Some(2);
        let res = embedder.embed(vec!["a"]).await.unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(*transport.calls.lock().unwrap(), 2);
    }

    #[tokio::test]
    async fn embed_fails_on_missing_index() {
        let transport = Arc::new(FakeTransport {
            responses: Mutex::new(VecDeque::from([ok_response(vec![(1, vec![0.1, 0.2])])])),
            calls: Mutex::new(0),
        });
        let mut embedder = RemoteEmbedder::new(RemoteEmbedderConfig::default(), transport);
        embedder.dim = Some(2);
        let err = embedder.embed(vec!["a"]).await.unwrap_err().to_string();
        assert!(err.contains("invalid index sequence"));
    }
}
