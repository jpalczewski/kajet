use anyhow::{Context, Result, bail, ensure};
use kajet_backend::similarity_graph::{
    ChunkEmbeddingRow, SgemmGraphBuilder, load_all_chunk_embeddings,
};
use kajet_backend::{CandleEmbedder, create_production_search_engine};
use kajet_core::config::{KajetConfig, SimilarityGraphConfig, load_config};
use kajet_core::db_path::resolve_db_path;
use kajet_core::similarity_graph::{
    CsrGraph, GraphBuilder, HeadingRegexFilter, SimilarityGraph, heading_from_breadcrumb,
};
use kajet_indexer::Indexer;
use std::path::PathBuf;
use std::sync::Arc;

fn l2_normalize_in_place(embeddings: &mut [f32], n: usize, d: usize) -> Result<()> {
    ensure!(
        embeddings.len() == n.saturating_mul(d),
        "Invalid embeddings buffer length"
    );

    for row_idx in 0..n {
        let start = row_idx * d;
        let end = start + d;
        let row = &mut embeddings[start..end];
        let norm_sq: f32 = row.iter().map(|v| v * v).sum();
        if norm_sq == 0.0 {
            bail!("Zero-norm embedding at row {row_idx}");
        }
        let inv_norm = 1.0 / norm_sq.sqrt();
        for v in row {
            *v *= inv_norm;
        }
    }

    Ok(())
}

fn percentiles(mut values: Vec<f32>) -> (f32, f32, f32, f32, f32) {
    // Returns (min, median, mean, p95, max).
    if values.is_empty() {
        return (0.0, 0.0, 0.0, 0.0, 0.0);
    }

    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let min = values[0];
    let max = values[values.len() - 1];
    let mean = values.iter().copied().sum::<f32>() / values.len() as f32;

    let mid = values.len() / 2;
    let median = if values.len() % 2 == 0 {
        (values[mid - 1] + values[mid]) / 2.0
    } else {
        values[mid]
    };

    let p95_idx = ((values.len() - 1) as f64 * 0.95).round() as usize;
    let p95 = values[p95_idx.min(values.len() - 1)];
    (min, median, mean, p95, max)
}

fn pick_sample_chunk(rows: &[ChunkEmbeddingRow]) -> usize {
    // Prefer the journal chunk in `zacznij_tutaj_zadanie.md` for the Polish example vault.
    rows.iter()
        .position(|r| {
            r.note_path.ends_with("zacznij_tutaj_zadanie.md")
                && r.breadcrumb.to_lowercase().contains("dziennik")
        })
        .unwrap_or(0)
}

fn print_chunk(label: &str, dense_idx: u32, row: &ChunkEmbeddingRow) {
    let heading = heading_from_breadcrumb(&row.breadcrumb).unwrap_or("");
    println!(
        "{label} dense_idx={dense_idx} path={} heading={}",
        row.note_path, heading
    );
}

fn usage() -> &'static str {
    "Usage: cargo run -p kajet-indexer --example validate_similarity_graph -- <vault_path>\n\
     Example: cargo run -p kajet-indexer --example validate_similarity_graph -- example-vault/pl\n"
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let vault_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("example-vault/pl"));

    if !vault_path.exists() {
        bail!("Vault path does not exist: {}", vault_path.display());
    }

    let db_path = resolve_db_path(&vault_path);
    std::fs::create_dir_all(&db_path)?;

    let cfg: KajetConfig = load_config(&db_path, 3579, Some("pl".into()))?;
    let SimilarityGraphConfig {
        enabled,
        k,
        boilerplate_patterns,
    } = cfg.similarity_graph.clone();

    println!("vault_path={}", vault_path.display());
    println!("db_path={}", db_path.display());
    println!(
        "similarity_graph.enabled={enabled} k={k} patterns={:?}",
        boilerplate_patterns
    );

    // Indexing: run full reindex but skip graph build here (we measure build separately below).
    let embedder = Arc::new(CandleEmbedder::new(&cfg.embedding.model).with_context(|| {
        format!(
            "Failed to init CandleEmbedder for model '{}'",
            cfg.embedding.model
        )
    })?);
    let db_path_str = db_path.to_string_lossy().to_string();
    let search_engine = create_production_search_engine(&db_path_str, embedder.clone()).await?;

    let mut indexer = Indexer::new(
        embedder,
        search_engine.store().clone(),
        search_engine.doc_store().clone(),
    )
    .with_db_path(db_path.clone())
    .with_concurrency(cfg.max_concurrent_files, cfg.pipeline_buffer_size)
    .with_progress_step(cfg.logging.progress_percent_step)
    .with_date_fields(
        cfg.writer.frontmatter.created_date_field.clone(),
        cfg.writer.frontmatter.modified_date_field.clone(),
    )
    .with_document_prefix(cfg.embedding.document_prefix.clone());

    // Force-disable graph build during indexing so we can time it precisely afterwards.
    let mut sg_disabled = cfg.similarity_graph.clone();
    sg_disabled.enabled = false;
    indexer = indexer.with_similarity_graph_config(sg_disabled);

    let index_start = std::time::Instant::now();
    let stats = indexer
        .full_reindex(&vault_path, &cfg.exclude_folders)
        .await
        .context("Full reindex failed")?;
    println!(
        "indexing: documents={} chunks={} elapsed_ms={}",
        stats.total_documents,
        stats.total_chunks,
        index_start.elapsed().as_millis()
    );

    // Load all embeddings from LanceDB.
    let load_start = std::time::Instant::now();
    let (dim, mut all_rows) = load_all_chunk_embeddings(&db_path).await?;
    let load_ms = load_start.elapsed().as_millis();

    let total_chunks = all_rows.len();
    println!("lancedb: dim={dim} total_chunks={total_chunks} load_ms={load_ms}");

    if all_rows.is_empty() {
        bail!("No chunks found in LanceDB after indexing");
    }

    let filter = HeadingRegexFilter::new(&boilerplate_patterns)?;
    let filter_start = std::time::Instant::now();
    let before = all_rows.len();
    all_rows.retain(|row| filter.include_breadcrumb(&row.breadcrumb));
    let after = all_rows.len();
    let excluded = before.saturating_sub(after);
    let filter_ms = filter_start.elapsed().as_millis();

    println!(
        "filter: included={} excluded={} filter_ms={filter_ms}",
        after, excluded
    );

    if all_rows.is_empty() {
        bail!("No chunks left after boilerplate filtering");
    }

    // Dense order must match the graph build (sort by note_path, then chunk_index).
    let sort_start = std::time::Instant::now();
    all_rows.sort_by(|a, b| match a.note_path.cmp(&b.note_path) {
        std::cmp::Ordering::Equal => a.chunk_index.cmp(&b.chunk_index),
        other => other,
    });
    let sort_ms = sort_start.elapsed().as_millis();

    // Build flat embeddings + chunk_to_doc mapping (u16 doc indices).
    let build_inputs_start = std::time::Instant::now();
    let n = all_rows.len();
    let d = dim as usize;

    let mut embeddings: Vec<f32> = Vec::with_capacity(n * d);
    let mut chunk_to_doc: Vec<u16> = Vec::with_capacity(n);

    let mut current_doc: Option<&str> = None;
    let mut current_doc_idx: u16 = 0;

    for row in &all_rows {
        match current_doc {
            None => current_doc = Some(row.note_path.as_str()),
            Some(doc) if doc != row.note_path => {
                current_doc = Some(row.note_path.as_str());
                current_doc_idx = current_doc_idx
                    .checked_add(1)
                    .context("Too many documents for u16 doc index")?;
            }
            _ => {}
        }

        ensure!(
            row.vector.len() == d,
            "Embedding dimension mismatch for chunk {}:{} (expected {d}, got {})",
            row.note_path,
            row.chunk_index,
            row.vector.len()
        );
        chunk_to_doc.push(current_doc_idx);
        embeddings.extend_from_slice(&row.vector);
    }

    l2_normalize_in_place(&mut embeddings, n, d)?;
    let build_inputs_ms = build_inputs_start.elapsed().as_millis();

    // Build graph and measure SGEMM+topK time separately from serialization.
    let builder = SgemmGraphBuilder;
    let build_start = std::time::Instant::now();
    let graph = builder.build(&embeddings, n as u32, dim, k, &chunk_to_doc)?;
    let build_ms = build_start.elapsed().as_millis();

    let graph_path = db_path.join("similarity_graph.kjsg");
    let save_start = std::time::Instant::now();
    graph.save_to_path(&graph_path)?;
    let save_ms = save_start.elapsed().as_millis();

    let file_bytes = std::fs::metadata(&graph_path)?.len();
    let loaded = CsrGraph::load_from_path(&graph_path)?;
    ensure!(
        loaded.n_chunks == graph.n_chunks && loaded.k == graph.k && loaded.dim == graph.dim,
        "Graph load mismatch vs saved graph"
    );

    println!(
        "graph: n_chunks={} dim={} k_effective={} sort_ms={sort_ms} build_inputs_ms={build_inputs_ms} build_ms={build_ms} save_ms={save_ms} file_bytes={file_bytes}",
        graph.n_chunks, graph.dim, graph.k
    );

    // Sanity check: pick a journal-ish chunk and print its nearest neighbors.
    let sample_idx = pick_sample_chunk(&all_rows) as u32;
    let sample_row = &all_rows[sample_idx as usize];
    print_chunk("sample", sample_idx, sample_row);

    let neighbors = graph.neighbors(sample_idx);
    println!("neighbors_top5:");
    for (rank, (nbr_idx, sim)) in neighbors.iter().take(5).enumerate() {
        let row = all_rows
            .get(*nbr_idx as usize)
            .context("Neighbor idx OOB")?;
        let heading = heading_from_breadcrumb(&row.breadcrumb).unwrap_or("");
        println!(
            "  {}: dense_idx={} sim={:.4} path={} heading={}",
            rank + 1,
            nbr_idx,
            sim,
            row.note_path,
            heading
        );
    }

    // Similarity distribution across all edges.
    let mut sims: Vec<f32> = Vec::with_capacity((graph.n_chunks as usize) * (graph.k as usize));
    for i in 0..graph.len() {
        for &(_, sim) in graph.neighbors(i).iter() {
            sims.push(sim);
        }
    }
    let (min, median, mean, p95, max) = percentiles(sims);
    println!(
        "similarity_stats: min={min:.4} median={median:.4} mean={mean:.4} p95={p95:.4} max={max:.4}"
    );

    Ok(())
}
