use anyhow::Result;
use kajet_core::traits::Embedder;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

/// Request to embed a batch of texts.
pub struct EmbedRequest {
    pub texts: Vec<String>,
    pub response_tx: oneshot::Sender<Result<Vec<Vec<f32>>>>,
}

/// Worker that owns the embedding model and processes requests in batches.
///
/// This design eliminates contention by having a single owner that batches requests.
pub struct EmbeddingWorker {
    request_rx: mpsc::Receiver<EmbedRequest>,
    embedder: Arc<dyn Embedder>,
}

impl EmbeddingWorker {
    /// Create a new embedding worker and return a handle for sending requests.
    ///
    /// Uses a bounded channel (capacity 256) to provide backpressure if embedding
    /// requests arrive faster than they can be processed.
    pub fn spawn(embedder: Arc<dyn Embedder>) -> EmbeddingHandle {
        let (request_tx, request_rx) = mpsc::channel(256);

        let mut worker = Self {
            request_rx,
            embedder,
        };

        // Spawn the worker in a separate task and store the JoinHandle
        let join_handle = tokio::spawn(async move {
            worker.run().await;
        });

        EmbeddingHandle {
            request_tx,
            _join_handle: Arc::new(join_handle),
        }
    }

    /// Main worker loop: drain channel and process batched requests.
    async fn run(&mut self) {
        while let Some(request) = self.request_rx.recv().await {
            // Natural batching: try to drain additional pending requests
            let mut batch = vec![request];
            while let Ok(req) = self.request_rx.try_recv() {
                batch.push(req);
                // Limit batch size to avoid excessive memory usage
                if batch.len() >= 32 {
                    break;
                }
            }

            tracing::trace!(
                batch_size = batch.len(),
                "embedding worker: processing batch"
            );

            // Flatten all texts from the batch
            let all_texts: Vec<String> = batch
                .iter()
                .flat_map(|req| req.texts.iter().cloned())
                .collect();

            let mut all_embeddings = Vec::with_capacity(all_texts.len());
            let mut embed_error = None;
            let max_texts_per_embed_call = self
                .embedder
                .max_batch_size_hint()
                .unwrap_or(usize::MAX)
                .max(1);

            for text_chunk in all_texts.chunks(max_texts_per_embed_call) {
                let text_refs: Vec<&str> = text_chunk.iter().map(|s| s.as_str()).collect();
                match self.embedder.embed(text_refs).await {
                    Ok(emb) => all_embeddings.extend(emb),
                    Err(e) => {
                        embed_error = Some(e);
                        break;
                    }
                }
            }

            match embed_error {
                None => {
                    let embeddings = all_embeddings;
                    // Split embeddings back to individual requests
                    let mut offset = 0;
                    for req in batch {
                        let count = req.texts.len();
                        let chunk_embeddings = embeddings[offset..offset + count].to_vec();
                        offset += count;
                        let _ = req.response_tx.send(Ok(chunk_embeddings));
                    }
                    tracing::trace!(
                        batch_size = all_texts.len(),
                        "embedding worker: batch complete"
                    );
                }
                Some(e) => {
                    tracing::error!("Embedding batch failed: {e}");
                    for req in batch {
                        let _ = req.response_tx.send(Err(anyhow::anyhow!("{e}")));
                    }
                }
            }
        }
    }
}

/// Handle for sending embedding requests to the worker.
///
/// The worker task will be automatically shut down when all handles are dropped,
/// as the channel will close and the worker loop will exit.
#[derive(Clone)]
pub struct EmbeddingHandle {
    request_tx: mpsc::Sender<EmbedRequest>,
    // Keep the JoinHandle alive to prevent orphaned tasks.
    // Wrapped in Arc so it can be cloned with the handle.
    _join_handle: Arc<JoinHandle<()>>,
}

impl EmbeddingHandle {
    /// Embed a batch of texts, returns embeddings in the same order.
    ///
    /// If the channel is full, this will wait (backpressure) until the worker
    /// processes some requests.
    pub async fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        let (response_tx, response_rx) = oneshot::channel();

        self.request_tx
            .send(EmbedRequest { texts, response_tx })
            .await
            .map_err(|_| anyhow::anyhow!("Embedding worker closed"))?;

        response_rx
            .await
            .map_err(|_| anyhow::anyhow!("Embedding response channel closed"))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kajet_core::traits::mocks::MockEmbedder;

    #[tokio::test]
    async fn worker_processes_single_request() {
        let embedder = Arc::new(MockEmbedder::new(4));
        let handle = EmbeddingWorker::spawn(embedder.clone());

        let texts = vec!["hello".to_string(), "world".to_string()];
        let result = handle.embed(texts).await.unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].len(), 4);
    }

    #[tokio::test]
    async fn worker_processes_multiple_requests() {
        let embedder = Arc::new(MockEmbedder::new(4));
        let handle = EmbeddingWorker::spawn(embedder.clone());

        // Send multiple requests concurrently
        let handles: Vec<_> = (0..5)
            .map(|i| {
                let h = handle.clone();
                tokio::spawn(async move { h.embed(vec![format!("text {i}")]).await })
            })
            .collect();

        // All should complete successfully
        for h in handles {
            let result = h.await.unwrap().unwrap();
            assert_eq!(result.len(), 1);
            assert_eq!(result[0].len(), 4);
        }
    }

    #[tokio::test]
    async fn worker_batches_requests() {
        let embedder = Arc::new(MockEmbedder::new(4));
        let handle = EmbeddingWorker::spawn(embedder.clone());

        // Send many requests quickly — worker should batch them
        let mut handles = vec![];
        for i in 0..10 {
            let h = handle.clone();
            handles.push(tokio::spawn(async move {
                h.embed(vec![format!("text {i}")]).await
            }));
        }

        // All should complete
        for h in handles {
            h.await.unwrap().unwrap();
        }

        // Check that batching happened (fewer calls than requests)
        let calls = embedder.calls.lock().unwrap();
        assert!(
            calls.len() < 10,
            "Expected batching, got {} calls",
            calls.len()
        );
    }

    #[tokio::test]
    async fn worker_does_not_hard_cap_single_embed_call_to_32() {
        let embedder = Arc::new(MockEmbedder::new(4));
        let handle = EmbeddingWorker::spawn(embedder.clone());

        let texts: Vec<String> = (0..64).map(|i| format!("text {i}")).collect();
        let result = handle.embed(texts).await.unwrap();
        assert_eq!(result.len(), 64);

        let calls = embedder.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].len(), 64);
    }
}
