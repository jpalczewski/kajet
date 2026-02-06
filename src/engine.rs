use crate::parser::Chunk;
use crate::traits::{Embedder, SearchHit, StoredChunk, VectorStore};
use anyhow::Result;
use arrow_array::{Float32Array, Int64Array, RecordBatch, RecordBatchIterator, StringArray};
use arrow_schema::{DataType, Field, Schema};
use async_trait::async_trait;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use futures::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase};
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------
// Search result (public API)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchResult {
    pub note_path: String,
    pub breadcrumb: String,
    pub content: String,
    pub score: f32,
}

// ---------------------------------------------------------------------------
// FastEmbed implementation of Embedder
// ---------------------------------------------------------------------------

pub struct FastEmbedEmbedder {
    model: Mutex<TextEmbedding>,
    dim: usize,
}

impl FastEmbedEmbedder {
    pub fn new() -> Result<Self> {
        let model = TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::AllMiniLML6V2).with_show_download_progress(true),
        )?;
        // AllMiniLML6V2 produces 384-dim vectors
        let dim = 384;
        Ok(Self {
            model: Mutex::new(model),
            dim,
        })
    }
}

impl Embedder for FastEmbedEmbedder {
    fn embed(&self, texts: Vec<&str>) -> Result<Vec<Vec<f32>>> {
        let vecs = self.model.lock().unwrap().embed(texts, None)?;
        Ok(vecs)
    }

    fn dimension(&self) -> usize {
        self.dim
    }
}

// ---------------------------------------------------------------------------
// LanceDB implementation of VectorStore
// ---------------------------------------------------------------------------

pub struct LanceVectorStore {
    db: lancedb::Connection,
}

impl LanceVectorStore {
    pub async fn new(vault_path: &str) -> Result<Self> {
        let db_path = format!("{}/.kajet", vault_path);
        let db = lancedb::connect(&db_path).execute().await?;
        Ok(Self { db })
    }
}

#[async_trait]
impl VectorStore for LanceVectorStore {
    async fn store_chunks(&self, chunks: &[StoredChunk]) -> Result<()> {
        if chunks.is_empty() {
            return Ok(());
        }

        let dim = chunks[0].vector.len() as i32;

        let schema = Arc::new(Schema::new(vec![
            Field::new("id", DataType::Int64, false),
            Field::new("note_path", DataType::Utf8, false),
            Field::new("breadcrumb", DataType::Utf8, false),
            Field::new("content", DataType::Utf8, false),
            Field::new(
                "vector",
                DataType::FixedSizeList(
                    Arc::new(Field::new("item", DataType::Float32, true)),
                    dim,
                ),
                false,
            ),
        ]));

        let ids = Int64Array::from_iter_values(0..chunks.len() as i64);
        let paths = StringArray::from_iter_values(chunks.iter().map(|c| c.note_path.as_str()));
        let crumbs = StringArray::from_iter_values(chunks.iter().map(|c| c.breadcrumb.as_str()));
        let contents = StringArray::from_iter_values(chunks.iter().map(|c| c.content.as_str()));

        let flat: Vec<f32> = chunks.iter().flat_map(|c| c.vector.iter().copied()).collect();
        let values = Float32Array::from(flat);
        let field = Arc::new(Field::new("item", DataType::Float32, true));
        let vectors =
            arrow_array::FixedSizeListArray::try_new(field, dim, Arc::new(values), None)?;

        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(ids),
                Arc::new(paths),
                Arc::new(crumbs),
                Arc::new(contents),
                Arc::new(vectors),
            ],
        )?;

        let batches = RecordBatchIterator::new(vec![Ok(batch)], schema.clone());

        // Drop old table if exists, create new
        if self
            .db
            .table_names()
            .execute()
            .await?
            .contains(&"chunks".to_string())
        {
            self.db.drop_table("chunks", &[]).await?;
        }

        self.db
            .create_table("chunks", Box::new(batches))
            .execute()
            .await?;

        Ok(())
    }

    async fn search(&self, vector: &[f32], limit: usize) -> Result<Vec<SearchHit>> {
        let table = self.db.open_table("chunks").execute().await?;
        let batches: Vec<RecordBatch> = table
            .query()
            .nearest_to(vector)?
            .limit(limit)
            .execute()
            .await?
            .try_collect::<Vec<_>>()
            .await?;

        let mut results = Vec::new();
        for batch in &batches {
            let paths = batch
                .column_by_name("note_path")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let crumbs = batch
                .column_by_name("breadcrumb")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let contents = batch
                .column_by_name("content")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let distances = batch
                .column_by_name("_distance")
                .unwrap()
                .as_any()
                .downcast_ref::<Float32Array>()
                .unwrap();

            for i in 0..batch.num_rows() {
                results.push(SearchHit {
                    note_path: paths.value(i).to_string(),
                    breadcrumb: crumbs.value(i).to_string(),
                    content: contents.value(i).to_string(),
                    distance: distances.value(i),
                });
            }
        }

        Ok(results)
    }
}

// ---------------------------------------------------------------------------
// Engine — generic over Embedder + VectorStore
// ---------------------------------------------------------------------------

pub struct Engine {
    embedder: Box<dyn Embedder>,
    store: Box<dyn VectorStore>,
}

impl Engine {
    pub fn new(embedder: Box<dyn Embedder>, store: Box<dyn VectorStore>) -> Self {
        Self { embedder, store }
    }

    /// Production constructor — connects to LanceDB and loads FastEmbed model.
    pub async fn new_production(vault_path: &str) -> Result<Self> {
        let embedder = FastEmbedEmbedder::new()?;
        let store = LanceVectorStore::new(vault_path).await?;
        Ok(Self {
            embedder: Box::new(embedder),
            store: Box::new(store),
        })
    }

    /// Embed and store chunks in the vector database. Returns count of indexed chunks.
    pub async fn index(&self, chunks: Vec<Chunk>) -> Result<usize> {
        if chunks.is_empty() {
            tracing::warn!("No chunks to index");
            return Ok(0);
        }

        let total = chunks.len();
        tracing::info!("Embedding {} chunks...", total);

        let texts: Vec<String> = chunks.iter().map(|c| c.embed_text()).collect();
        let text_refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
        let embeddings = self.embedder.embed(text_refs)?;

        let stored: Vec<StoredChunk> = chunks
            .into_iter()
            .zip(embeddings)
            .map(|(chunk, vector)| StoredChunk {
                note_path: chunk.note_path,
                breadcrumb: chunk.breadcrumb,
                content: chunk.content,
                vector,
            })
            .collect();

        self.store.store_chunks(&stored).await?;

        tracing::info!("Indexed {} chunks", total);
        Ok(total)
    }

    /// Search the vault for chunks similar to the query.
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let query_emb = self.embedder.embed(vec![query])?;

        let hits = self.store.search(&query_emb[0], limit).await?;

        let results = hits
            .into_iter()
            .map(|hit| SearchResult {
                note_path: hit.note_path,
                breadcrumb: hit.breadcrumb,
                content: hit.content,
                score: hit.distance,
            })
            .collect();

        Ok(results)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::mocks::{MockEmbedder, MockVectorStore};
    use std::sync::Arc;

    fn make_test_chunks() -> Vec<Chunk> {
        vec![
            Chunk {
                note_path: "note.md".into(),
                breadcrumb: "note.md > Intro".into(),
                content: "Hello world".into(),
            },
            Chunk {
                note_path: "note.md".into(),
                breadcrumb: "note.md > Body".into(),
                content: "Some body text".into(),
            },
        ]
    }

    #[tokio::test]
    async fn index_embeds_and_stores_all_chunks() {
        let embedder = Arc::new(MockEmbedder::new(4));
        let store = Arc::new(MockVectorStore::new());
        let store_clone = store.clone();

        let engine = Engine::new(Box::new(embedder), Box::new(store));
        let chunks = make_test_chunks();
        let count = engine.index(chunks).await.unwrap();

        assert_eq!(count, 2);

        let stored = store_clone.stored.lock().unwrap();
        assert_eq!(stored.len(), 2);
        assert_eq!(stored[0].note_path, "note.md");
        assert_eq!(stored[0].vector.len(), 4);
    }

    #[tokio::test]
    async fn index_empty_chunks_returns_zero() {
        let embedder = Arc::new(MockEmbedder::new(4));
        let store = Arc::new(MockVectorStore::new());
        let engine = Engine::new(Box::new(embedder), Box::new(store));

        let count = engine.index(vec![]).await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn search_returns_mapped_results() {
        let embedder = Arc::new(MockEmbedder::new(4));
        let store = Arc::new(MockVectorStore::with_search_results(vec![
            SearchHit {
                note_path: "note.md".into(),
                breadcrumb: "note.md > Intro".into(),
                content: "Hello".into(),
                distance: 0.1,
            },
            SearchHit {
                note_path: "other.md".into(),
                breadcrumb: "other.md".into(),
                content: "World".into(),
                distance: 0.5,
            },
        ]));

        let engine = Engine::new(Box::new(embedder), Box::new(store));
        let results = engine.search("hello", 5).await.unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].note_path, "note.md");
        assert_eq!(results[0].score, 0.1);
        assert_eq!(results[1].content, "World");
    }

    #[tokio::test]
    async fn search_respects_limit() {
        let embedder = Arc::new(MockEmbedder::new(4));
        let store = Arc::new(MockVectorStore::with_search_results(vec![
            SearchHit {
                note_path: "a.md".into(),
                breadcrumb: "a.md".into(),
                content: "A".into(),
                distance: 0.1,
            },
            SearchHit {
                note_path: "b.md".into(),
                breadcrumb: "b.md".into(),
                content: "B".into(),
                distance: 0.2,
            },
        ]));

        let engine = Engine::new(Box::new(embedder), Box::new(store));
        let results = engine.search("query", 1).await.unwrap();

        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn index_sends_correct_texts_to_embedder() {
        let embedder = Arc::new(MockEmbedder::new(4));
        let embedder_clone = embedder.clone();
        let store = Arc::new(MockVectorStore::new());

        let engine = Engine::new(Box::new(embedder), Box::new(store));
        let chunks = make_test_chunks();
        engine.index(chunks).await.unwrap();

        let calls = embedder_clone.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].len(), 2);
        assert!(calls[0][0].contains("Hello world"));
        assert!(calls[0][1].contains("Some body text"));
    }
}
