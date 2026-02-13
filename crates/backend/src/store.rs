use anyhow::Result;
use arrow_array::{Float32Array, Int64Array, RecordBatch, RecordBatchIterator, StringArray};
use arrow_schema::{DataType, Field, Schema};
use async_trait::async_trait;
use futures::TryStreamExt;
use kajet_core::traits::{SearchHit, StoredChunk, VectorStore};
use lancedb::query::{ExecutableQuery, QueryBase};
use std::sync::Arc;

pub struct LanceVectorStore {
    db: lancedb::Connection,
}

impl LanceVectorStore {
    pub async fn new(db_path: &str) -> Result<Self> {
        let db = lancedb::connect(db_path).execute().await?;
        Ok(Self { db })
    }

    fn build_batches(chunks: &[StoredChunk]) -> Result<(Arc<Schema>, Vec<RecordBatch>)> {
        let dim = chunks[0].vector.len() as i32;

        let schema = Arc::new(Schema::new(vec![
            Field::new("id", DataType::Int64, false),
            Field::new("note_path", DataType::Utf8, false),
            Field::new("breadcrumb", DataType::Utf8, false),
            Field::new("content", DataType::Utf8, false),
            Field::new("raw_content", DataType::Utf8, false),
            Field::new("links", DataType::Utf8, false),
            Field::new("chunk_index", DataType::Int64, false),
            Field::new("content_hash", DataType::Utf8, false),
            Field::new(
                "vector",
                DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Float32, true)), dim),
                false,
            ),
        ]));

        let ids = Int64Array::from_iter_values(0..chunks.len() as i64);
        let paths = StringArray::from_iter_values(chunks.iter().map(|c| c.note_path.as_str()));
        let crumbs = StringArray::from_iter_values(chunks.iter().map(|c| c.breadcrumb.as_str()));
        let contents = StringArray::from_iter_values(chunks.iter().map(|c| c.content.as_str()));
        let raw_contents =
            StringArray::from_iter_values(chunks.iter().map(|c| c.raw_content.as_str()));
        let links_json = StringArray::from_iter_values(
            chunks
                .iter()
                .map(|c| serde_json::to_string(&c.links).unwrap_or_else(|_| "[]".to_string())),
        );
        let chunk_indices =
            Int64Array::from_iter_values(chunks.iter().map(|c| c.chunk_index as i64));
        let hashes = StringArray::from_iter_values(chunks.iter().map(|c| c.content_hash.as_str()));

        let flat: Vec<f32> = chunks
            .iter()
            .flat_map(|c| c.vector.iter().copied())
            .collect();
        let values = Float32Array::from(flat);
        let field = Arc::new(Field::new("item", DataType::Float32, true));
        let vectors = arrow_array::FixedSizeListArray::try_new(field, dim, Arc::new(values), None)?;

        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(ids),
                Arc::new(paths),
                Arc::new(crumbs),
                Arc::new(contents),
                Arc::new(raw_contents),
                Arc::new(links_json),
                Arc::new(chunk_indices),
                Arc::new(hashes),
                Arc::new(vectors),
            ],
        )?;

        Ok((schema, vec![batch]))
    }

    fn vector_dim_from_schema(schema: &Schema) -> Option<i32> {
        schema
            .field_with_name("vector")
            .ok()
            .and_then(|field| match field.data_type() {
                DataType::FixedSizeList(_, dim) => Some(*dim),
                _ => None,
            })
    }
}

#[async_trait]
impl VectorStore for LanceVectorStore {
    async fn store_chunks(&self, chunks: &[StoredChunk]) -> Result<()> {
        if chunks.is_empty() {
            return Ok(());
        }

        let (schema, batches) = Self::build_batches(chunks)?;
        let batch_iter = RecordBatchIterator::new(batches.into_iter().map(Ok), schema.clone());

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
            .create_table("chunks", Box::new(batch_iter))
            .execute()
            .await?;

        Ok(())
    }

    async fn search(&self, vector: &[f32], limit: usize) -> Result<Vec<SearchHit>> {
        // Return empty results if table doesn't exist yet (indexing in progress)
        if !self
            .db
            .table_names()
            .execute()
            .await?
            .contains(&"chunks".to_string())
        {
            tracing::warn!("Search skipped: chunks table not found (indexing in progress?)");
            return Ok(Vec::new());
        }

        let start = std::time::Instant::now();
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
            let raw_contents = batch
                .column_by_name("raw_content")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>().cloned());
            let links_col = batch
                .column_by_name("links")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>().cloned());
            let distances = batch
                .column_by_name("_distance")
                .unwrap()
                .as_any()
                .downcast_ref::<Float32Array>()
                .unwrap();

            for i in 0..batch.num_rows() {
                let content = contents.value(i).to_string();
                let raw_content = raw_contents
                    .as_ref()
                    .map(|rc| rc.value(i).to_string())
                    .unwrap_or_else(|| content.clone());
                let links: Vec<kajet_core::parser::Link> = links_col
                    .as_ref()
                    .and_then(|lc| serde_json::from_str(lc.value(i)).ok())
                    .unwrap_or_default();
                results.push(SearchHit {
                    note_path: paths.value(i).to_string(),
                    breadcrumb: crumbs.value(i).to_string(),
                    content,
                    raw_content,
                    links,
                    distance: distances.value(i),
                });
            }
        }

        tracing::debug!(
            hits = results.len(),
            elapsed_ms = start.elapsed().as_millis() as u64,
            "vector store search"
        );
        tracing::trace!(scores = ?results.iter().map(|r| r.distance).collect::<Vec<_>>(), "hit distances");

        Ok(results)
    }

    async fn upsert_chunks(&self, chunks: &[StoredChunk]) -> Result<()> {
        if chunks.is_empty() {
            return Ok(());
        }

        let table_names = self.db.table_names().execute().await?;
        if !table_names.contains(&"chunks".to_string()) {
            return self.store_chunks(chunks).await;
        }

        // Migrate: if table lacks chunk_index or raw_content column, drop and recreate
        let table = self.db.open_table("chunks").execute().await?;
        let schema = table.schema().await?;
        let expected_dim = chunks[0].vector.len() as i32;
        let existing_dim = Self::vector_dim_from_schema(&schema);
        let vector_dim_mismatch = existing_dim != Some(expected_dim);
        if schema.field_with_name("chunk_index").is_err()
            || schema.field_with_name("raw_content").is_err()
            || vector_dim_mismatch
        {
            tracing::warn!(
                expected_dim,
                existing_dim,
                "Migrating chunks table to new schema"
            );
            drop(table);
            self.db.drop_table("chunks", &[]).await?;
            return self.store_chunks(chunks).await;
        }

        let (batch_schema, batches) = Self::build_batches(chunks)?;
        let batch_iter =
            RecordBatchIterator::new(batches.into_iter().map(Ok), batch_schema.clone());

        let mut merge = table.merge_insert(&["note_path", "chunk_index"]);
        merge
            .when_matched_update_all(None)
            .when_not_matched_insert_all();
        merge.execute(Box::new(batch_iter)).await?;

        Ok(())
    }

    async fn delete_chunks_by_paths(&self, paths: &[String]) -> Result<()> {
        if paths.is_empty() {
            return Ok(());
        }

        let table_names = self.db.table_names().execute().await?;
        if !table_names.contains(&"chunks".to_string()) {
            return Ok(());
        }

        let table = self.db.open_table("chunks").execute().await?;
        let escaped: Vec<String> = paths
            .iter()
            .map(|p| format!("'{}'", p.replace('\'', "''")))
            .collect();
        let predicate = format!("note_path IN ({})", escaped.join(", "));
        table.delete(&predicate).await?;

        Ok(())
    }

    async fn get_chunks_by_path(&self, note_path: &str) -> Result<Vec<StoredChunk>> {
        // Return empty results if table doesn't exist yet
        if !self
            .db
            .table_names()
            .execute()
            .await?
            .contains(&"chunks".to_string())
        {
            return Ok(Vec::new());
        }

        let table = self.db.open_table("chunks").execute().await?;
        let escaped_path = note_path.replace('\'', "''");
        let predicate = format!("note_path = '{}'", escaped_path);

        let batches: Vec<RecordBatch> = table
            .query()
            .only_if(predicate)
            .execute()
            .await?
            .try_collect()
            .await?;

        let mut chunks = Vec::new();
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
            let raw_contents = batch
                .column_by_name("raw_content")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let links_col = batch
                .column_by_name("links")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let chunk_indices = batch
                .column_by_name("chunk_index")
                .unwrap()
                .as_any()
                .downcast_ref::<Int64Array>()
                .unwrap();
            let hashes = batch
                .column_by_name("content_hash")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let vectors = batch
                .column_by_name("vector")
                .unwrap()
                .as_any()
                .downcast_ref::<arrow_array::FixedSizeListArray>()
                .unwrap();

            for i in 0..batch.num_rows() {
                let links: Vec<kajet_core::parser::Link> =
                    serde_json::from_str(links_col.value(i)).unwrap_or_default();

                // Extract vector from FixedSizeListArray
                let vector_slice = vectors.value(i);
                let vector_floats = vector_slice
                    .as_any()
                    .downcast_ref::<Float32Array>()
                    .unwrap();
                let vector: Vec<f32> = (0..vector_floats.len())
                    .map(|j| vector_floats.value(j))
                    .collect();

                chunks.push(StoredChunk {
                    note_path: paths.value(i).to_string(),
                    breadcrumb: crumbs.value(i).to_string(),
                    content: contents.value(i).to_string(),
                    raw_content: raw_contents.value(i).to_string(),
                    vector,
                    chunk_index: chunk_indices.value(i) as u32,
                    content_hash: hashes.value(i).to_string(),
                    links,
                });
            }
        }

        // Sort by chunk_index (ascending) for correct document order
        chunks.sort_by_key(|c| c.chunk_index);

        Ok(chunks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kajet_core::traits::StoredChunk;

    fn sample_chunk(path: &str, dim: usize) -> StoredChunk {
        StoredChunk {
            note_path: path.to_string(),
            breadcrumb: String::new(),
            content: "content".to_string(),
            raw_content: "content".to_string(),
            vector: vec![0.1; dim],
            chunk_index: 0,
            content_hash: "hash".to_string(),
            links: Vec::new(),
        }
    }

    #[test]
    fn upsert_recreates_table_when_vector_dimension_changes() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            let dir = tempfile::tempdir().unwrap();
            let db_path = dir.path().to_string_lossy().to_string();
            let store = LanceVectorStore::new(&db_path).await.unwrap();

            store
                .upsert_chunks(&[sample_chunk("note.md", 4)])
                .await
                .unwrap();
            store
                .upsert_chunks(&[sample_chunk("note.md", 8)])
                .await
                .unwrap();

            let table = store.db.open_table("chunks").execute().await.unwrap();
            let schema = table.schema().await.unwrap();
            assert_eq!(LanceVectorStore::vector_dim_from_schema(&schema), Some(8));
        });
    }

    #[test]
    fn get_chunks_by_path_returns_sorted_chunks() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            let dir = tempfile::tempdir().unwrap();
            let db_path = dir.path().to_string_lossy().to_string();
            let store = LanceVectorStore::new(&db_path).await.unwrap();

            // Insert chunks in reverse order to verify sorting
            let chunks = vec![
                StoredChunk {
                    note_path: "test.md".into(),
                    breadcrumb: "test.md".into(),
                    content: "Third chunk".into(),
                    raw_content: "Third chunk".into(),
                    vector: vec![0.3; 384],
                    chunk_index: 2,
                    content_hash: "hash3".into(),
                    links: vec![],
                },
                StoredChunk {
                    note_path: "test.md".into(),
                    breadcrumb: "test.md".into(),
                    content: "First chunk".into(),
                    raw_content: "First chunk".into(),
                    vector: vec![0.1; 384],
                    chunk_index: 0,
                    content_hash: "hash1".into(),
                    links: vec![],
                },
                StoredChunk {
                    note_path: "test.md".into(),
                    breadcrumb: "test.md".into(),
                    content: "Second chunk".into(),
                    raw_content: "Second chunk".into(),
                    vector: vec![0.2; 384],
                    chunk_index: 1,
                    content_hash: "hash2".into(),
                    links: vec![],
                },
            ];

            store.upsert_chunks(&chunks).await.unwrap();

            let retrieved = store.get_chunks_by_path("test.md").await.unwrap();
            assert_eq!(retrieved.len(), 3);
            assert_eq!(retrieved[0].chunk_index, 0);
            assert_eq!(retrieved[1].chunk_index, 1);
            assert_eq!(retrieved[2].chunk_index, 2);
            assert_eq!(retrieved[0].content, "First chunk");
            assert_eq!(retrieved[1].content, "Second chunk");
            assert_eq!(retrieved[2].content, "Third chunk");
        });
    }
}
