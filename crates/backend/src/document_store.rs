use anyhow::Result;
use arrow_array::{Float64Array, Int64Array, RecordBatch, RecordBatchIterator, StringArray};
use arrow_schema::{DataType, Field, Schema};
use async_trait::async_trait;
use futures::TryStreamExt;
use kajet_core::traits::DocumentStore;
use kajet_core::types::{Document, FtsHit, IndexStats};
use lance_index::scalar::FullTextSearchQuery;
use lancedb::index::Index;
use lancedb::query::{ExecutableQuery, QueryBase};
use std::collections::HashMap;
use std::sync::Arc;

const TABLE_NAME: &str = "documents";

pub struct LanceDocumentStore {
    db: lancedb::Connection,
}

impl LanceDocumentStore {
    pub async fn new(vault_path: &str) -> Result<Self> {
        let db_path = format!("{}/.kajet", vault_path);
        let db = lancedb::connect(&db_path).execute().await?;
        Ok(Self { db })
    }

    fn schema() -> Arc<Schema> {
        Arc::new(Schema::new(vec![
            Field::new("id", DataType::Int64, false),
            Field::new("source_file", DataType::Utf8, false),
            Field::new("full_text", DataType::Utf8, false),
            Field::new("title", DataType::Utf8, false),
            Field::new("tags", DataType::Utf8, false), // JSON array as string
            Field::new("content_hash", DataType::Utf8, false),
            Field::new("last_modified", DataType::Float64, false),
        ]))
    }

    fn docs_to_batch(docs: &[Document]) -> Result<RecordBatch> {
        let schema = Self::schema();

        let ids = Int64Array::from_iter_values(0..docs.len() as i64);
        let source_files =
            StringArray::from_iter_values(docs.iter().map(|d| d.source_file.as_str()));
        let full_texts = StringArray::from_iter_values(docs.iter().map(|d| d.full_text.as_str()));
        let titles = StringArray::from_iter_values(docs.iter().map(|d| d.title.as_str()));
        let tags = StringArray::from_iter_values(
            docs.iter()
                .map(|d| serde_json::to_string(&d.tags).unwrap_or_else(|_| "[]".to_string())),
        );
        let hashes = StringArray::from_iter_values(docs.iter().map(|d| d.content_hash.as_str()));
        let last_modified = Float64Array::from_iter_values(docs.iter().map(|d| d.last_modified));

        Ok(RecordBatch::try_new(
            schema,
            vec![
                Arc::new(ids),
                Arc::new(source_files),
                Arc::new(full_texts),
                Arc::new(titles),
                Arc::new(tags),
                Arc::new(hashes),
                Arc::new(last_modified),
            ],
        )?)
    }

    async fn table_exists(&self) -> Result<bool> {
        let names = self.db.table_names().execute().await?;
        Ok(names.contains(&TABLE_NAME.to_string()))
    }
}

#[async_trait]
impl DocumentStore for LanceDocumentStore {
    async fn store_documents(&self, docs: &[Document]) -> Result<()> {
        if docs.is_empty() {
            return Ok(());
        }

        let batch = Self::docs_to_batch(docs)?;
        let schema = Self::schema();
        let batch_iter = RecordBatchIterator::new(vec![Ok(batch)], schema);

        if !self.table_exists().await? {
            self.db
                .create_table(TABLE_NAME, Box::new(batch_iter))
                .execute()
                .await?;
        } else {
            let table = self.db.open_table(TABLE_NAME).execute().await?;
            let mut merge = table.merge_insert(&["source_file"]);
            merge
                .when_matched_update_all(None)
                .when_not_matched_insert_all();
            merge.execute(Box::new(batch_iter)).await?;
        }

        Ok(())
    }

    async fn get_document_hashes(&self) -> Result<HashMap<String, String>> {
        if !self.table_exists().await? {
            return Ok(HashMap::new());
        }

        let table = self.db.open_table(TABLE_NAME).execute().await?;
        let batches: Vec<RecordBatch> = table
            .query()
            .select(lancedb::query::Select::columns(&[
                "source_file",
                "content_hash",
            ]))
            .execute()
            .await?
            .try_collect()
            .await?;

        let mut hashes = HashMap::new();
        for batch in &batches {
            let paths = batch
                .column_by_name("source_file")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let hash_col = batch
                .column_by_name("content_hash")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();

            for i in 0..batch.num_rows() {
                hashes.insert(paths.value(i).to_string(), hash_col.value(i).to_string());
            }
        }

        Ok(hashes)
    }

    async fn delete_by_paths(&self, paths: &[String]) -> Result<()> {
        if paths.is_empty() || !self.table_exists().await? {
            return Ok(());
        }

        let table = self.db.open_table(TABLE_NAME).execute().await?;
        let escaped: Vec<String> = paths
            .iter()
            .map(|p| format!("'{}'", p.replace('\'', "''")))
            .collect();
        let predicate = format!("source_file IN ({})", escaped.join(", "));
        table.delete(&predicate).await?;

        Ok(())
    }

    async fn fts_search(&self, query: &str, limit: usize) -> Result<Vec<FtsHit>> {
        if !self.table_exists().await? {
            return Ok(Vec::new());
        }

        let table = self.db.open_table(TABLE_NAME).execute().await?;
        let fts_query = FullTextSearchQuery::new(query.to_owned());

        let batches: Vec<RecordBatch> = table
            .query()
            .full_text_search(fts_query)
            .select(lancedb::query::Select::columns(&[
                "source_file",
                "title",
                "full_text",
            ]))
            .limit(limit)
            .execute()
            .await?
            .try_collect()
            .await?;

        let mut results = Vec::new();
        for batch in &batches {
            let paths = batch
                .column_by_name("source_file")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let titles = batch
                .column_by_name("title")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let texts = batch
                .column_by_name("full_text")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();

            // FTS returns a _score column (BM25)
            let scores = batch
                .column_by_name("_score")
                .and_then(|c| c.as_any().downcast_ref::<Float64Array>().cloned());

            for i in 0..batch.num_rows() {
                let full_text = texts.value(i);
                // Take first 500 chars as snippet
                let snippet = if full_text.len() > 500 {
                    format!("{}...", &full_text[..500])
                } else {
                    full_text.to_string()
                };

                results.push(FtsHit {
                    source_file: paths.value(i).to_string(),
                    title: titles.value(i).to_string(),
                    content_snippet: snippet,
                    score: scores
                        .as_ref()
                        .map(|s| s.value(i) as f32)
                        .unwrap_or(1.0 - (i as f32 / limit as f32)),
                });
            }
        }

        Ok(results)
    }

    async fn create_fts_index(&self) -> Result<()> {
        if !self.table_exists().await? {
            return Ok(());
        }

        let table = self.db.open_table(TABLE_NAME).execute().await?;
        table
            .create_index(&["full_text"], Index::FTS(Default::default()))
            .execute()
            .await?;

        Ok(())
    }

    async fn get_index_stats(&self) -> Result<IndexStats> {
        if !self.table_exists().await? {
            return Ok(IndexStats {
                total_documents: 0,
                total_chunks: 0,
                last_indexed: None,
            });
        }

        let table = self.db.open_table(TABLE_NAME).execute().await?;
        let total_documents = table.count_rows(None).await?;

        // Count chunks from the chunks table if it exists
        let total_chunks = if self
            .db
            .table_names()
            .execute()
            .await?
            .contains(&"chunks".to_string())
        {
            let chunks_table = self.db.open_table("chunks").execute().await?;
            chunks_table.count_rows(None).await?
        } else {
            0
        };

        Ok(IndexStats {
            total_documents,
            total_chunks,
            last_indexed: Some(chrono::Utc::now()),
        })
    }
}
