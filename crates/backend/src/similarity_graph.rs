use anyhow::{Context, Result, bail, ensure};
use arrow_array::{Float32Array, Int64Array, RecordBatch, StringArray};
use futures::TryStreamExt;
use kajet_core::similarity_graph::{CsrGraph, GraphBuilder};
use lancedb::query::{ExecutableQuery, QueryBase};
use rayon::prelude::*;
use std::cmp::Ordering;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ChunkEmbeddingRow {
    pub note_path: String,
    pub breadcrumb: String,
    pub chunk_index: u32,
    pub vector: Vec<f32>,
    pub content: String,
}

/// Load all chunk embeddings and metadata from the LanceDB `chunks` table.
pub async fn load_all_chunk_embeddings(db_path: &Path) -> Result<(u32, Vec<ChunkEmbeddingRow>)> {
    let db_path_str = db_path.to_string_lossy().to_string();
    let db = lancedb::connect(&db_path_str).execute().await?;

    if !db
        .table_names()
        .execute()
        .await?
        .contains(&"chunks".to_string())
    {
        bail!("Chunks table not found (index not built yet?)");
    }

    let table = db.open_table("chunks").execute().await?;
    let batches: Vec<RecordBatch> = table
        .query()
        .select(lancedb::query::Select::columns(&[
            "note_path",
            "breadcrumb",
            "chunk_index",
            "vector",
            "content",
        ]))
        .execute()
        .await?
        .try_collect()
        .await?;

    let mut rows = Vec::new();
    let mut dim: Option<u32> = None;

    for batch in &batches {
        let paths = batch
            .column_by_name("note_path")
            .context("Missing note_path column")?
            .as_any()
            .downcast_ref::<StringArray>()
            .context("Invalid note_path column type")?;
        let crumbs = batch
            .column_by_name("breadcrumb")
            .context("Missing breadcrumb column")?
            .as_any()
            .downcast_ref::<StringArray>()
            .context("Invalid breadcrumb column type")?;
        let chunk_indices = batch
            .column_by_name("chunk_index")
            .context("Missing chunk_index column")?
            .as_any()
            .downcast_ref::<Int64Array>()
            .context("Invalid chunk_index column type")?;
        let vectors = batch
            .column_by_name("vector")
            .context("Missing vector column")?
            .as_any()
            .downcast_ref::<arrow_array::FixedSizeListArray>()
            .context("Invalid vector column type")?;
        let contents = batch
            .column_by_name("content")
            .context("Missing content column")?
            .as_any()
            .downcast_ref::<StringArray>()
            .context("Invalid content column type")?;

        let value_length = vectors.value_length();
        ensure!(value_length > 0, "Vector dimension is zero");
        let batch_dim = value_length as u32;
        match dim {
            None => dim = Some(batch_dim),
            Some(existing) => ensure!(existing == batch_dim, "Vector dimension mismatch"),
        }

        let values = vectors
            .values()
            .as_any()
            .downcast_ref::<Float32Array>()
            .context("Invalid vector inner type (expected Float32)")?;
        let flat = values.values();
        let dim_usize = batch_dim as usize;

        for row_idx in 0..batch.num_rows() {
            let start = row_idx * dim_usize;
            let end = start + dim_usize;
            let vector = flat[start..end].to_vec();

            rows.push(ChunkEmbeddingRow {
                note_path: paths.value(row_idx).to_string(),
                breadcrumb: crumbs.value(row_idx).to_string(),
                chunk_index: chunk_indices.value(row_idx) as u32,
                vector,
                content: contents.value(row_idx).to_string(),
            });
        }
    }

    Ok((dim.unwrap_or(0), rows))
}

#[derive(Debug, Clone, Default)]
pub struct SgemmGraphBuilder;

impl GraphBuilder for SgemmGraphBuilder {
    fn build(
        &self,
        embeddings: &[f32],
        n_chunks: u32,
        dim: u32,
        k: u32,
        chunk_to_doc: &[u16],
    ) -> Result<CsrGraph> {
        if n_chunks == 0 {
            bail!("Cannot build similarity graph: no chunks provided");
        }
        ensure!(
            dim > 0,
            "Cannot build similarity graph: embedding dimension is zero"
        );
        ensure!(
            embeddings.len() == (n_chunks as usize) * (dim as usize),
            "Invalid embeddings length: expected {} floats, got {}",
            (n_chunks as usize) * (dim as usize),
            embeddings.len()
        );
        ensure!(
            chunk_to_doc.len() == n_chunks as usize,
            "Invalid chunk_to_doc length"
        );
        ensure!(
            n_chunks <= 10_000,
            "Similarity graph build refused: n_chunks={n_chunks} exceeds brute-force limit (10_000)"
        );

        let effective_k = k.min(n_chunks.saturating_sub(1));
        let n = n_chunks as usize;
        let d = dim as usize;
        let k_usize = effective_k as usize;

        // Compute similarity matrix S = E × Eᵀ (row-major).
        let mut sims = vec![0.0_f32; n * n];
        unsafe {
            matrixmultiply::sgemm(
                n,
                d,
                n,
                1.0,
                embeddings.as_ptr(),
                d as isize,
                1,
                // B is Eᵀ view: rs=1, cs=d
                embeddings.as_ptr(),
                1,
                d as isize,
                0.0,
                sims.as_mut_ptr(),
                n as isize,
                1,
            );
        }

        // Fixed-K CSR offsets.
        let mut offsets = Vec::with_capacity(n + 1);
        for i in 0..=n {
            offsets.push((i * k_usize) as u32);
        }

        // Flattened adjacency list (N * K).
        let mut adj = vec![(0_u32, 0.0_f32); n * k_usize];

        if k_usize > 0 {
            adj.par_chunks_mut(k_usize)
                .enumerate()
                .for_each(|(row_idx, out)| {
                    let row = &sims[row_idx * n..(row_idx + 1) * n];

                    let mut candidates: Vec<u32> = (0..n as u32).collect();
                    candidates.swap_remove(row_idx);

                    let k_row = k_usize.min(candidates.len());
                    if k_row == 0 {
                        return;
                    }

                    let cmp_desc = |a: &u32, b: &u32| {
                        let sa = row[*a as usize];
                        let sb = row[*b as usize];
                        sb.partial_cmp(&sa).unwrap_or(Ordering::Equal)
                    };

                    candidates.select_nth_unstable_by(k_row - 1, cmp_desc);
                    let top = &mut candidates[..k_row];
                    top.sort_unstable_by(cmp_desc);

                    for (slot, &neighbor_idx) in top.iter().enumerate() {
                        out[slot] = (neighbor_idx, row[neighbor_idx as usize]);
                    }
                });
        }

        CsrGraph::new_fixed_k(
            effective_k,
            n_chunks,
            dim,
            offsets,
            adj,
            chunk_to_doc.to_vec(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kajet_core::similarity_graph::SimilarityGraph;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn sgemm_graph_builder_correctness_small() {
        // 4 chunks, 3D normalized embeddings
        // e0 = [1,0,0]
        // e1 = [0.8,0.6,0]
        // e2 = [0.6,0.8,0]
        // e3 = [-1,0,0]
        let embeddings: Vec<f32> = vec![
            1.0, 0.0, 0.0, //
            0.8, 0.6, 0.0, //
            0.6, 0.8, 0.0, //
            -1.0, 0.0, 0.0, //
        ];

        let builder = SgemmGraphBuilder::default();
        let chunk_to_doc = vec![0_u16; 4];
        let graph = builder.build(&embeddings, 4, 3, 2, &chunk_to_doc).unwrap();

        assert_eq!(graph.k, 2);
        assert_eq!(graph.n_chunks, 4);

        // Chunk 0: top neighbors should be 1 (0.8), then 2 (0.6)
        let n0 = graph.neighbors(0);
        assert_eq!(n0.len(), 2);
        assert_eq!(n0[0].0, 1);
        assert!(approx_eq(n0[0].1, 0.8));
        assert_eq!(n0[1].0, 2);
        assert!(approx_eq(n0[1].1, 0.6));
        assert!(n0[0].1 >= n0[1].1);

        // Chunk 1: top neighbors 2 (0.96), then 0 (0.8)
        let n1 = graph.neighbors(1);
        assert_eq!(n1.len(), 2);
        assert_eq!(n1[0].0, 2);
        assert!(approx_eq(n1[0].1, 0.96));
        assert_eq!(n1[1].0, 0);
        assert!(approx_eq(n1[1].1, 0.8));
        assert!(n1[0].1 >= n1[1].1);

        // Chunk 2: top neighbors 1 (0.96), then 0 (0.6)
        let n2 = graph.neighbors(2);
        assert_eq!(n2.len(), 2);
        assert_eq!(n2[0].0, 1);
        assert!(approx_eq(n2[0].1, 0.96));
        assert_eq!(n2[1].0, 0);
        assert!(approx_eq(n2[1].1, 0.6));
        assert!(n2[0].1 >= n2[1].1);

        // Chunk 3: top neighbors 2 (-0.6), then 1 (-0.8)
        let n3 = graph.neighbors(3);
        assert_eq!(n3.len(), 2);
        assert_eq!(n3[0].0, 2);
        assert!(approx_eq(n3[0].1, -0.6));
        assert_eq!(n3[1].0, 1);
        assert!(approx_eq(n3[1].1, -0.8));
        assert!(n3[0].1 >= n3[1].1);

        // Self should never appear.
        for i in 0..4_u32 {
            assert!(!graph.neighbors(i).iter().any(|(idx, _)| *idx == i));
        }
    }

    #[test]
    fn sgemm_graph_builder_clamps_k_to_n_minus_1() {
        let embeddings: Vec<f32> = vec![
            1.0, 0.0, //
            0.0, 1.0, //
            -1.0, 0.0, //
        ];
        let builder = SgemmGraphBuilder::default();
        let chunk_to_doc = vec![0_u16; 3];
        let graph = builder.build(&embeddings, 3, 2, 32, &chunk_to_doc).unwrap();
        assert_eq!(graph.k, 2);
        assert_eq!(graph.neighbors(0).len(), 2);
    }

    #[test]
    fn sgemm_graph_builder_single_chunk_has_no_neighbors() {
        let embeddings: Vec<f32> = vec![1.0, 0.0];
        let builder = SgemmGraphBuilder::default();
        let chunk_to_doc = vec![0_u16; 1];
        let graph = builder.build(&embeddings, 1, 2, 10, &chunk_to_doc).unwrap();
        assert_eq!(graph.k, 0);
        assert_eq!(graph.neighbors(0).len(), 0);
    }

    #[test]
    fn sgemm_graph_builder_zero_chunks_errors() {
        let builder = SgemmGraphBuilder::default();
        let err = builder.build(&[], 0, 2, 10, &[]).unwrap_err().to_string();
        assert!(err.contains("no chunks"));
    }
}
