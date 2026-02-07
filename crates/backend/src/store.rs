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
                Arc::new(chunk_indices),
                Arc::new(hashes),
                Arc::new(vectors),
            ],
        )?;

        Ok((schema, vec![batch]))
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

        // Migrate: if table lacks chunk_index column, drop and recreate
        let table = self.db.open_table("chunks").execute().await?;
        let schema = table.schema().await?;
        if schema.field_with_name("chunk_index").is_err() {
            tracing::warn!(
                "Migrating chunks table to new schema (adding chunk_index, content_hash)"
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
}
