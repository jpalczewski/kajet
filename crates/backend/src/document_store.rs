use anyhow::Result;
use arrow_array::{Float64Array, Int64Array, RecordBatch, RecordBatchIterator, StringArray};
use arrow_schema::{DataType, Field, Schema};
use async_trait::async_trait;
use futures::TryStreamExt;
use kajet_core::path_utils::normalize_folder_prefix;
use kajet_core::text_utils::truncate_with_ellipsis;
use kajet_core::traits::{DocumentStore, StoredFileInfo};
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
    pub async fn new(db_path: &str) -> Result<Self> {
        let db = lancedb::connect(db_path).execute().await?;
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
            Field::new("outgoing_links", DataType::Utf8, false), // JSON array
            Field::new("backlinks", DataType::Utf8, false),      // JSON array
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
        let outgoing_links = StringArray::from_iter_values(docs.iter().map(|d| {
            serde_json::to_string(&d.outgoing_links).unwrap_or_else(|_| "[]".to_string())
        }));
        let backlinks = StringArray::from_iter_values(
            docs.iter()
                .map(|d| serde_json::to_string(&d.backlinks).unwrap_or_else(|_| "[]".to_string())),
        );

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
                Arc::new(outgoing_links),
                Arc::new(backlinks),
            ],
        )?)
    }

    async fn table_exists(&self) -> Result<bool> {
        let names = self.db.table_names().execute().await?;
        Ok(names.contains(&TABLE_NAME.to_string()))
    }

    fn parse_document_batches(batches: &[RecordBatch]) -> Vec<Document> {
        let mut docs = Vec::new();
        for batch in batches {
            let paths = batch
                .column_by_name("source_file")
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
            let titles = batch
                .column_by_name("title")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let tags_col = batch
                .column_by_name("tags")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let hashes = batch
                .column_by_name("content_hash")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let mtimes = batch
                .column_by_name("last_modified")
                .unwrap()
                .as_any()
                .downcast_ref::<Float64Array>()
                .unwrap();
            let outgoing = batch
                .column_by_name("outgoing_links")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let backlinks_col = batch
                .column_by_name("backlinks")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();

            for i in 0..batch.num_rows() {
                docs.push(Document {
                    source_file: paths.value(i).to_string(),
                    full_text: texts.value(i).to_string(),
                    title: titles.value(i).to_string(),
                    tags: serde_json::from_str(tags_col.value(i)).unwrap_or_default(),
                    content_hash: hashes.value(i).to_string(),
                    last_modified: mtimes.value(i),
                    outgoing_links: serde_json::from_str(outgoing.value(i)).unwrap_or_default(),
                    backlinks: serde_json::from_str(backlinks_col.value(i)).unwrap_or_default(),
                });
            }
        }
        docs
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
            // Migrate: if table lacks outgoing_links column, drop and recreate
            let table = self.db.open_table(TABLE_NAME).execute().await?;
            let table_schema = table.schema().await?;
            if table_schema.field_with_name("outgoing_links").is_err() {
                tracing::warn!("Migrating documents table to new schema");
                drop(table);
                self.db.drop_table(TABLE_NAME, &[]).await?;
                let batch = Self::docs_to_batch(docs)?;
                let schema = Self::schema();
                let batch_iter = RecordBatchIterator::new(vec![Ok(batch)], schema);
                self.db
                    .create_table(TABLE_NAME, Box::new(batch_iter))
                    .execute()
                    .await?;
                return Ok(());
            }
            let table = self.db.open_table(TABLE_NAME).execute().await?;
            let mut merge = table.merge_insert(&["source_file"]);
            merge
                .when_matched_update_all(None)
                .when_not_matched_insert_all();
            merge.execute(Box::new(batch_iter)).await?;
        }

        Ok(())
    }

    async fn get_document_hashes(&self) -> Result<HashMap<String, StoredFileInfo>> {
        if !self.table_exists().await? {
            return Ok(HashMap::new());
        }

        let table = self.db.open_table(TABLE_NAME).execute().await?;
        let batches: Vec<RecordBatch> = table
            .query()
            .select(lancedb::query::Select::columns(&[
                "source_file",
                "content_hash",
                "last_modified",
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
            let mtime_col = batch
                .column_by_name("last_modified")
                .unwrap()
                .as_any()
                .downcast_ref::<Float64Array>()
                .unwrap();

            for i in 0..batch.num_rows() {
                hashes.insert(
                    paths.value(i).to_string(),
                    StoredFileInfo {
                        content_hash: hash_col.value(i).to_string(),
                        last_modified: mtime_col.value(i),
                    },
                );
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

        let start = std::time::Instant::now();
        let table = self.db.open_table(TABLE_NAME).execute().await?;
        let fts_query = FullTextSearchQuery::new(query.to_owned());

        let batches: Vec<RecordBatch> = table
            .query()
            .full_text_search(fts_query)
            .select(lancedb::query::Select::columns(&[
                "source_file",
                "title",
                "full_text",
                "_score",
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
                let snippet = truncate_with_ellipsis(full_text, 500);

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

        tracing::debug!(
            hits = results.len(),
            elapsed_ms = start.elapsed().as_millis() as u64,
            "fts search"
        );
        tracing::trace!(scores = ?results.iter().map(|r| r.score).collect::<Vec<_>>(), "fts scores");

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

    async fn get_all_documents(&self) -> Result<Vec<Document>> {
        if !self.table_exists().await? {
            return Ok(Vec::new());
        }

        let table = self.db.open_table(TABLE_NAME).execute().await?;
        let batches: Vec<RecordBatch> = table
            .query()
            .select(lancedb::query::Select::columns(&[
                "source_file",
                "full_text",
                "title",
                "tags",
                "content_hash",
                "last_modified",
                "outgoing_links",
                "backlinks",
            ]))
            .execute()
            .await?
            .try_collect()
            .await?;

        Ok(Self::parse_document_batches(&batches))
    }

    async fn get_document_by_path(&self, path: &str) -> Result<Option<Document>> {
        if !self.table_exists().await? {
            return Ok(None);
        }

        let table = self.db.open_table(TABLE_NAME).execute().await?;
        let escaped = path.replace('\'', "''");
        let predicate = format!("source_file = '{escaped}'");
        let batches: Vec<RecordBatch> = table
            .query()
            .only_if(predicate)
            .select(lancedb::query::Select::columns(&[
                "source_file",
                "full_text",
                "title",
                "tags",
                "content_hash",
                "last_modified",
                "outgoing_links",
                "backlinks",
            ]))
            .execute()
            .await?
            .try_collect()
            .await?;

        Ok(Self::parse_document_batches(&batches).into_iter().next())
    }

    async fn update_backlinks(&self, backlinks: &HashMap<String, Vec<String>>) -> Result<()> {
        if !self.table_exists().await? || backlinks.is_empty() {
            return Ok(());
        }

        // Read all docs, update backlinks, rewrite
        let mut docs = self.get_all_documents().await?;
        for doc in &mut docs {
            if let Some(bl) = backlinks.get(&doc.source_file) {
                doc.backlinks = bl.clone();
            } else {
                doc.backlinks = Vec::new();
            }
        }

        if !docs.is_empty() {
            let batch = Self::docs_to_batch(&docs)?;
            let schema = Self::schema();
            let batch_iter = RecordBatchIterator::new(vec![Ok(batch)], schema);

            let table = self.db.open_table(TABLE_NAME).execute().await?;
            let mut merge = table.merge_insert(&["source_file"]);
            merge
                .when_matched_update_all(None)
                .when_not_matched_insert_all();
            merge.execute(Box::new(batch_iter)).await?;
        }

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

    async fn query_documents(
        &self,
        from: Option<f64>,
        to: Option<f64>,
        folder: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Document>> {
        if !self.table_exists().await? {
            return Ok(Vec::new());
        }

        let table = self.db.open_table(TABLE_NAME).execute().await?;
        let mut query = table.query();

        // Build predicates
        let mut predicates = Vec::new();

        if let Some(from_ts) = from {
            predicates.push(format!("last_modified >= {}", from_ts));
        }

        if let Some(to_ts) = to {
            predicates.push(format!("last_modified <= {}", to_ts));
        }

        if let Some(folder_prefix) = folder {
            // Security: Escape single quotes for SQL LIKE predicate
            // Note: LanceDB's only_if() doesn't support parameterized queries,
            // so we use string escaping. The folder prefix comes from trusted
            // MCP tool parameters, not direct user input.
            let escaped = folder_prefix.replace('\'', "''");
            let prefix = normalize_folder_prefix(&escaped);
            predicates.push(format!("source_file LIKE '{}%'", prefix));
        }

        // Apply combined predicate
        if !predicates.is_empty() {
            let predicate = predicates.join(" AND ");
            query = query.only_if(predicate);
        }

        let batches: Vec<RecordBatch> = query
            .select(lancedb::query::Select::columns(&[
                "source_file",
                "full_text",
                "title",
                "tags",
                "content_hash",
                "last_modified",
                "outgoing_links",
                "backlinks",
            ]))
            .limit(limit) // Caller already applies FILTER_OVERFETCH_MULTIPLIER
            .execute()
            .await?
            .try_collect()
            .await?;

        let mut docs = Self::parse_document_batches(&batches);

        // Sort chronologically (oldest first)
        // Note: partial_cmp handles NaN gracefully by treating as equal
        docs.sort_by(|a, b| {
            a.last_modified
                .partial_cmp(&b.last_modified)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Note: We do NOT truncate here - caller (tools.rs) will apply limit
        // after post-filtering by tags to ensure correct result count.

        Ok(docs)
    }
}
