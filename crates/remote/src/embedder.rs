use crate::config::RemoteEmbedderConfig;
use crate::error::RemoteEmbedderError;
use crate::info::{TeiInfo, compute_optimal_params, fetch_tei_info};
use crate::transport::{EmbedRequest, RemoteTransport, ReqwestTransport};
use crate::validator::validate_and_reorder;
use async_trait::async_trait;
use kajet_core::traits::Embedder;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct RemoteEmbedder {
    config: RemoteEmbedderConfig,
    transport: Arc<dyn RemoteTransport>,
    dim: Option<usize>,
    effective_max_batch_size: AtomicUsize,
    effective_concurrent_requests: AtomicUsize,
    tei_info: Option<TeiInfo>,
}

impl RemoteEmbedder {
    pub fn new(config: RemoteEmbedderConfig, transport: Arc<dyn RemoteTransport>) -> Self {
        let initial_batch = config.max_batch_size;
        let initial_concurrent = config.max_concurrent_requests;
        Self {
            effective_max_batch_size: AtomicUsize::new(initial_batch),
            effective_concurrent_requests: AtomicUsize::new(initial_concurrent),
            config,
            transport,
            dim: None,
            tei_info: None,
        }
    }

    pub async fn connect(
        base_url: &str,
        model: &str,
        api_key: Option<&str>,
    ) -> Result<Self, RemoteEmbedderError> {
        Self::connect_with_config(RemoteEmbedderConfig {
            base_url: base_url.to_string(),
            model: model.to_string(),
            api_key: api_key.map(str::to_owned),
            ..RemoteEmbedderConfig::default()
        })
        .await
    }

    pub async fn connect_with_config(
        config: RemoteEmbedderConfig,
    ) -> Result<Self, RemoteEmbedderError> {
        let cfg = config;
        let transport = Arc::new(ReqwestTransport::new(&cfg)?);
        let mut embedder = Self::new(cfg, transport);

        // Best-effort /info probe for TEI servers
        if let Some(info) = fetch_tei_info(&embedder.config.base_url).await {
            let config_concurrent = embedder.config.max_concurrent_requests;
            let params = compute_optimal_params(&info, config_concurrent);
            embedder
                .effective_max_batch_size
                .store(params.batch_size, Ordering::Relaxed);
            embedder
                .effective_concurrent_requests
                .store(params.concurrent_requests, Ordering::Relaxed);
            tracing::info!(
                model_id = %info.model_id,
                computed_batch_size = params.batch_size,
                computed_concurrency = params.concurrent_requests,
                "TEI /info auto-detected"
            );
            embedder.tei_info = Some(info);
        } else {
            tracing::debug!(
                base_url = %embedder.config.base_url,
                "TEI /info not available, using config defaults"
            );
        }

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

    async fn embed_adaptive(&self, texts: Vec<&str>) -> Result<Vec<Vec<f32>>, RemoteEmbedderError> {
        let mut out = Vec::with_capacity(texts.len());

        // Build initial queue of owned batches at current batch_size
        let initial_batch = self.current_max_batch_size();
        let mut queue: std::collections::VecDeque<Vec<String>> = texts
            .chunks(initial_batch)
            .map(|chunk| chunk.iter().map(|s| s.to_string()).collect())
            .collect();

        while !queue.is_empty() {
            let concurrency = self.current_concurrent_requests();
            let drain_count = queue.len().min(concurrency);
            let batches: Vec<Vec<String>> = queue.drain(..drain_count).collect();

            if batches.len() == 1 {
                // Single batch: sequential with transparent 422 re-splitting
                let batch = batches.into_iter().next().unwrap();
                let refs: Vec<&str> = batch.iter().map(String::as_str).collect();
                match self.embed_with_retry(refs).await {
                    Ok(v) => out.extend(v),
                    Err(RemoteEmbedderError::HttpStatus { status, body }) if status == 422 => {
                        let Some(max_batch) = extract_max_batch_size(&body) else {
                            return Err(RemoteEmbedderError::HttpStatus { status, body });
                        };
                        if max_batch == 0 || batch.len() <= max_batch {
                            return Err(RemoteEmbedderError::HttpStatus { status, body });
                        }
                        self.reduce_max_batch_size(max_batch);
                        tracing::debug!(
                            sent = batch.len(),
                            max_batch,
                            "remote provider rejected batch size, splitting"
                        );
                        // Prepend splits in order
                        for split in batch.chunks(max_batch).rev() {
                            queue.push_front(split.to_vec());
                        }
                    }
                    Err(e) => return Err(e),
                }
            } else {
                // Multiple batches: concurrent dispatch
                let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency));
                let transport = Arc::clone(&self.transport);
                let model = self.config.model.clone();
                let max_retries = self.config.max_retries;
                let initial_backoff = self.config.initial_backoff;
                let expected_dim = self.dim;

                let mut handles = Vec::with_capacity(batches.len());
                for batch in batches {
                    let sem = Arc::clone(&semaphore);
                    let transport = Arc::clone(&transport);
                    let model = model.clone();
                    handles.push(tokio::spawn(async move {
                        let _permit = sem.acquire().await.expect("semaphore closed");
                        embed_batch_standalone(
                            transport,
                            &model,
                            batch,
                            max_retries,
                            initial_backoff,
                            expected_dim,
                        )
                        .await
                    }));
                }

                for handle in handles {
                    match handle.await.expect("embed task panicked") {
                        Ok(v) => out.extend(v),
                        Err(RemoteEmbedderError::HttpStatus { status, body }) if status == 422 => {
                            if let Some(max_batch) =
                                extract_max_batch_size(&body).filter(|&m| m > 0)
                            {
                                self.reduce_max_batch_size(max_batch);
                                tracing::debug!(
                                    max_batch,
                                    "remote provider rejected batch size, reduced for next call"
                                );
                            }
                            return Err(RemoteEmbedderError::HttpStatus { status, body });
                        }
                        Err(e) => return Err(e),
                    }
                }
            }
        }

        Ok(out)
    }

    fn current_max_batch_size(&self) -> usize {
        self.effective_max_batch_size.load(Ordering::Relaxed).max(1)
    }

    fn current_concurrent_requests(&self) -> usize {
        self.effective_concurrent_requests
            .load(Ordering::Relaxed)
            .max(1)
    }

    fn reduce_max_batch_size(&self, discovered_max: usize) {
        let discovered_max = discovered_max.max(1);
        let mut current = self.effective_max_batch_size.load(Ordering::Relaxed);
        while discovered_max < current {
            match self.effective_max_batch_size.compare_exchange(
                current,
                discovered_max,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(observed) => current = observed,
            }
        }
    }

    fn prepare_inputs(&self, texts: Vec<&str>) -> Vec<String> {
        let max_chars = self.config.max_input_chars;
        texts
            .into_iter()
            .map(|text| {
                if max_chars > 0 && text.len() > max_chars {
                    let end = text.floor_char_boundary(max_chars);
                    tracing::debug!(
                        original_len = text.len(),
                        truncated_len = end,
                        max_chars,
                        "truncating oversized embedding input"
                    );
                    text[..end].to_string()
                } else {
                    text.to_string()
                }
            })
            .collect()
    }
}

/// Standalone batch embed function that doesn't borrow `RemoteEmbedder`,
/// allowing it to be used inside `tokio::spawn`.
async fn embed_batch_standalone(
    transport: Arc<dyn RemoteTransport>,
    model: &str,
    texts: Vec<String>,
    max_retries: u32,
    initial_backoff: std::time::Duration,
    expected_dim: Option<usize>,
) -> Result<Vec<Vec<f32>>, RemoteEmbedderError> {
    if texts.is_empty() {
        return Ok(Vec::new());
    }

    let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
    let mut attempt = 0u32;
    let mut backoff = initial_backoff;
    loop {
        let req = EmbedRequest {
            model,
            input: &refs,
        };
        match transport.embed(&req).await {
            Ok(resp) => {
                let pairs = resp
                    .data
                    .into_iter()
                    .map(|d| (d.index, d.embedding))
                    .collect::<Vec<_>>();
                return validate_and_reorder(texts.len(), pairs, expected_dim);
            }
            Err(RemoteEmbedderError::HttpStatus { status, .. })
                if (status == 429 || status >= 500) && attempt < max_retries =>
            {
                tracing::debug!(attempt, status, "retrying remote embed request");
            }
            Err(RemoteEmbedderError::Transport(_)) if attempt < max_retries => {
                tracing::debug!(attempt, "retrying remote embed request");
            }
            Err(e) => return Err(e),
        }

        attempt += 1;
        tokio::time::sleep(backoff).await;
        backoff = backoff.saturating_mul(2);
    }
}

fn extract_max_batch_size(body: &str) -> Option<usize> {
    let marker = "maximum allowed batch size";
    let idx = body.find(marker)?;
    let tail = &body[idx + marker.len()..];
    let digits: String = tail
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    if digits.is_empty() {
        return None;
    }
    digits.parse::<usize>().ok()
}

#[async_trait]
impl Embedder for RemoteEmbedder {
    async fn embed(&self, texts: Vec<&str>) -> anyhow::Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let prepared = self.prepare_inputs(texts);
        let prepared_refs: Vec<&str> = prepared.iter().map(String::as_str).collect();
        self.embed_adaptive(prepared_refs)
            .await
            .map_err(anyhow::Error::new)
    }

    fn dimension(&self) -> usize {
        self.dim.unwrap_or(0)
    }

    fn max_batch_size_hint(&self) -> Option<usize> {
        Some(self.current_max_batch_size())
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

    struct BatchLimitTransport {
        max_batch: usize,
        calls: Mutex<usize>,
    }

    #[async_trait]
    impl RemoteTransport for BatchLimitTransport {
        async fn embed(
            &self,
            request: &EmbedRequest<'_>,
        ) -> Result<EmbedResponse, RemoteEmbedderError> {
            *self.calls.lock().unwrap() += 1;
            if request.input.len() > self.max_batch {
                return Err(RemoteEmbedderError::HttpStatus {
                    status: 422,
                    body: format!(
                        "{{\"message\":\"batch size {} > maximum allowed batch size {}\"}}",
                        request.input.len(),
                        self.max_batch
                    ),
                });
            }
            Ok(EmbedResponse {
                data: request
                    .input
                    .iter()
                    .enumerate()
                    .map(|(index, _)| EmbedData {
                        index,
                        embedding: vec![0.1, 0.2],
                    })
                    .collect(),
            })
        }
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
        let cfg = RemoteEmbedderConfig {
            max_retries: 3,
            ..RemoteEmbedderConfig::default()
        };
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

    #[tokio::test]
    async fn embed_splits_after_422_batch_limit_error() {
        let transport = Arc::new(BatchLimitTransport {
            max_batch: 32,
            calls: Mutex::new(0),
        });
        let cfg = RemoteEmbedderConfig {
            max_batch_size: 256,
            ..RemoteEmbedderConfig::default()
        };
        let mut embedder = RemoteEmbedder::new(cfg, transport.clone());
        embedder.dim = Some(2);

        let texts: Vec<String> = (0..41).map(|i| format!("t{i}")).collect();
        let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
        let out = embedder.embed(refs).await.unwrap();

        assert_eq!(out.len(), 41);
        assert_eq!(*transport.calls.lock().unwrap(), 3);
    }

    /// Concurrent transport: tracks call count; each call returns sequential embeddings.
    struct ConcurrentTransport {
        calls: Mutex<usize>,
    }

    #[async_trait]
    impl RemoteTransport for ConcurrentTransport {
        async fn embed(
            &self,
            request: &EmbedRequest<'_>,
        ) -> Result<EmbedResponse, RemoteEmbedderError> {
            *self.calls.lock().unwrap() += 1;
            Ok(EmbedResponse {
                data: request
                    .input
                    .iter()
                    .enumerate()
                    .map(|(index, _)| EmbedData {
                        index,
                        embedding: vec![0.1, 0.2],
                    })
                    .collect(),
            })
        }
    }

    #[tokio::test]
    async fn concurrent_embed_preserves_order() {
        // 100 texts, batch_size=8, concurrency=4 → 13 batches dispatched concurrently
        let transport = Arc::new(ConcurrentTransport {
            calls: Mutex::new(0),
        });
        let cfg = RemoteEmbedderConfig {
            max_batch_size: 8,
            max_concurrent_requests: 4,
            ..RemoteEmbedderConfig::default()
        };
        let mut embedder = RemoteEmbedder::new(cfg, transport.clone());
        embedder.dim = Some(2);

        let texts: Vec<String> = (0..100).map(|i| format!("text_{i}")).collect();
        let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
        let out = embedder.embed(refs).await.unwrap();

        assert_eq!(out.len(), 100);
        // Each embedding should be [0.1, 0.2]
        for emb in &out {
            assert_eq!(emb, &[0.1f32, 0.2f32]);
        }
    }

    #[tokio::test]
    async fn fast_path_single_batch_makes_one_transport_call() {
        let transport = Arc::new(ConcurrentTransport {
            calls: Mutex::new(0),
        });
        let cfg = RemoteEmbedderConfig {
            max_batch_size: 32,
            max_concurrent_requests: 4,
            ..RemoteEmbedderConfig::default()
        };
        let mut embedder = RemoteEmbedder::new(cfg, transport.clone());
        embedder.dim = Some(2);

        // 5 texts < batch_size=32 → single batch path
        let texts: Vec<String> = (0..5).map(|i| format!("t{i}")).collect();
        let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
        let out = embedder.embed(refs).await.unwrap();

        assert_eq!(out.len(), 5);
        assert_eq!(*transport.calls.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn concurrent_error_propagates() {
        let transport = Arc::new(FakeTransport {
            responses: Mutex::new(VecDeque::from([
                ok_response(vec![(0, vec![0.1, 0.2]), (1, vec![0.3, 0.4])]),
                Err(RemoteEmbedderError::Transport("network error".into())),
            ])),
            calls: Mutex::new(0),
        });
        let cfg = RemoteEmbedderConfig {
            max_batch_size: 2,
            max_concurrent_requests: 1, // sequential to make ordering deterministic
            ..RemoteEmbedderConfig::default()
        };
        let mut embedder = RemoteEmbedder::new(cfg, transport.clone());
        embedder.dim = Some(2);

        let texts: Vec<String> = (0..4).map(|i| format!("t{i}")).collect();
        let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
        let err = embedder.embed(refs).await.unwrap_err().to_string();
        assert!(err.contains("transport error") || err.contains("network error"));
    }

    #[tokio::test]
    async fn connect_without_tei_info_uses_config_defaults() {
        // Without /info endpoint, embedder should fall back to config defaults
        let transport = Arc::new(FakeTransport {
            responses: Mutex::new(VecDeque::from([ok_response(vec![(0, vec![0.1, 0.2])])])),
            calls: Mutex::new(0),
        });
        let cfg = RemoteEmbedderConfig {
            max_batch_size: 16,
            max_concurrent_requests: 2,
            ..RemoteEmbedderConfig::default()
        };
        let embedder = RemoteEmbedder::new(cfg, transport);
        // No /info was probed, so effective values should match config
        assert_eq!(embedder.current_max_batch_size(), 16);
        assert_eq!(embedder.current_concurrent_requests(), 2);
        assert!(embedder.tei_info.is_none());
    }

    #[tokio::test]
    async fn embed_learns_provider_batch_size_for_next_calls() {
        let transport = Arc::new(BatchLimitTransport {
            max_batch: 32,
            calls: Mutex::new(0),
        });
        let cfg = RemoteEmbedderConfig {
            max_batch_size: 256,
            ..RemoteEmbedderConfig::default()
        };
        let mut embedder = RemoteEmbedder::new(cfg, transport.clone());
        embedder.dim = Some(2);

        let texts: Vec<String> = (0..41).map(|i| format!("t{i}")).collect();
        let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
        let _ = embedder.embed(refs.clone()).await.unwrap();
        let calls_after_first = *transport.calls.lock().unwrap();
        assert_eq!(calls_after_first, 3);

        let _ = embedder.embed(refs).await.unwrap();
        let calls_after_second = *transport.calls.lock().unwrap();
        assert_eq!(calls_after_second, 5);
    }
}
