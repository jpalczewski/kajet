use anyhow::Result;
use kajet_backend::{LanceDocumentStore, LanceVectorStore};
use kajet_core::config::SimilarityGraphConfig;
use kajet_core::similarity_graph::{CsrGraph, HeadingRegexFilter, SimilarityGraph};
use kajet_core::traits::{DocumentStore, Embedder, VectorStore};
use kajet_indexer::Indexer;
use std::sync::Arc;

struct DeterministicEmbedder {
    dim: usize,
}

#[async_trait::async_trait]
impl Embedder for DeterministicEmbedder {
    async fn embed(&self, texts: Vec<&str>) -> Result<Vec<Vec<f32>>> {
        Ok(texts
            .into_iter()
            .map(|text| embed_text(text, self.dim))
            .collect())
    }

    fn dimension(&self) -> usize {
        self.dim
    }
}

fn embed_text(text: &str, dim: usize) -> Vec<f32> {
    let mut vec = vec![0.0_f32; dim.max(1)];
    if text.is_empty() {
        vec[0] = 1.0;
        return vec;
    }

    for (idx, byte) in text.as_bytes().iter().enumerate() {
        let slot1 = idx % dim;
        let slot2 = (idx * 3 + 1) % dim;
        vec[slot1] += *byte as f32;
        vec[slot2] += (*byte as f32) * 0.25;
    }

    let norm_sq: f32 = vec.iter().map(|v| v * v).sum();
    let norm = norm_sq.sqrt();
    if norm > 1e-12 {
        for v in &mut vec {
            *v /= norm;
        }
    } else {
        vec[0] = 1.0;
    }

    vec
}

fn long_text(seed: &str) -> String {
    // Ensure we exceed parser min_content_chars threshold.
    format!("{seed} — Martinaise, Revachol. {}", "RCM notes ".repeat(20))
}

#[tokio::test]
async fn full_reindex_builds_similarity_graph_and_partial_reindex_invalidates() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let vault_path = dir.path().join("vault");
    let db_path = dir.path().join("db");
    std::fs::create_dir_all(&vault_path)?;

    // Two notes, one contains a boilerplate-like section heading.
    std::fs::write(
        vault_path.join("a.md"),
        format!(
            "# A\n\n{}\n\n## Historia zmian\n\n{}\n\n## Wnioski\n\n{}\n",
            long_text("Preamble A"),
            long_text("To be filtered"),
            long_text("Keep me")
        ),
    )?;
    std::fs::write(
        vault_path.join("b.md"),
        format!(
            "# B\n\n{}\n\n## Martinaise\n\n{}\n",
            long_text("Preamble B"),
            long_text("Body B")
        ),
    )?;

    let embedder: Arc<dyn Embedder> = Arc::new(DeterministicEmbedder { dim: 8 });
    let db_path_str = db_path.to_string_lossy().to_string();
    let store: Arc<dyn VectorStore> = Arc::new(LanceVectorStore::new(&db_path_str).await?);
    let doc_store: Arc<dyn DocumentStore> = Arc::new(LanceDocumentStore::new(&db_path_str).await?);

    let config = SimilarityGraphConfig {
        enabled: true,
        k: 32, // intentionally larger than N-1 to test clamping
        boilerplate_patterns: vec![r"^Historia zmian$".to_string()],
    };

    let indexer = Indexer::new(embedder, store, doc_store)
        .with_db_path(db_path.clone())
        .with_similarity_graph_config(config.clone());

    indexer.full_reindex(&vault_path, &[]).await?;

    let graph_path = db_path.join("similarity_graph.kjsg");
    assert!(
        graph_path.exists(),
        "Expected similarity graph to be written"
    );

    // Compute expected included chunk count by applying the same filter to raw rows.
    let (dim, rows) = kajet_backend::similarity_graph::load_all_chunk_embeddings(&db_path).await?;
    assert_eq!(dim, 8);
    let filter = HeadingRegexFilter::new(&config.boilerplate_patterns)?;
    let included = rows
        .iter()
        .filter(|r| filter.include_breadcrumb(&r.breadcrumb))
        .count() as u32;
    assert!(included > 0);

    let graph = CsrGraph::load_from_path(&graph_path)?;
    assert_eq!(graph.n_chunks, included);
    assert_eq!(graph.dim, 8);
    assert_eq!(graph.k, included.saturating_sub(1).min(config.k));

    // Basic sanity: neighbors within bounds, similarities in [-1, 1].
    if graph.n_chunks > 0 {
        for idx in 0..graph.n_chunks {
            for (n_idx, sim) in graph.neighbors(idx) {
                assert!(*n_idx < graph.n_chunks);
                assert!(*sim >= -1.0001 && *sim <= 1.0001);
            }
        }
    }

    // Partial reindex should invalidate the graph file (full-only rebuild policy).
    std::fs::write(
        vault_path.join("b.md"),
        format!("# B\n\n{}\n", long_text("Changed")),
    )?;
    indexer
        .reindex_files(&vault_path, &["b.md".to_string()])
        .await?;
    assert!(
        !graph_path.exists(),
        "Expected similarity graph to be invalidated after partial reindex"
    );

    Ok(())
}
