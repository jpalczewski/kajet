use crate::domain::search_input::SearchMode;
use kajet_core::actions::{ActionEvent, SearchResultSummary};
use kajet_core::search::{ExamineResult, SearchResult};
use kajet_core::types::{Document, IndexStats};
use std::path::{Path, PathBuf};

pub(crate) trait SearchPorts {
    fn default_limit(&self) -> usize;
    fn filter_overfetch_multiplier(&self) -> usize;
    fn tags_only_fetch_limit(&self) -> usize;
    async fn run_search(
        &self,
        query: &str,
        mode: SearchMode,
        limit: usize,
    ) -> anyhow::Result<Vec<SearchResult>>;
    async fn get_all_documents(&self) -> anyhow::Result<Vec<Document>>;
    async fn query_documents(
        &self,
        from_ts: Option<f64>,
        to_ts: Option<f64>,
        folder: Option<&str>,
        limit: usize,
    ) -> anyhow::Result<Vec<Document>>;
    fn send_query_event(
        &self,
        query: String,
        results: Vec<SearchResultSummary>,
        duration_ms: u64,
        timestamp: String,
    );
}

impl SearchPorts for crate::KajetMcp {
    fn default_limit(&self) -> usize {
        self.state.config.read().unwrap().default_limit
    }

    fn filter_overfetch_multiplier(&self) -> usize {
        self.state
            .config
            .read()
            .unwrap()
            .filter_overfetch_multiplier
    }

    fn tags_only_fetch_limit(&self) -> usize {
        self.state.config.read().unwrap().tags_only_fetch_limit
    }

    async fn run_search(
        &self,
        query: &str,
        mode: SearchMode,
        limit: usize,
    ) -> anyhow::Result<Vec<SearchResult>> {
        match mode {
            SearchMode::Vector => self.state.search_engine.vector_search(query, limit).await,
            SearchMode::Fts => self.state.search_engine.fts_search(query, limit).await,
            SearchMode::Hybrid => self.state.search_engine.hybrid_search(query, limit).await,
        }
    }

    async fn get_all_documents(&self) -> anyhow::Result<Vec<Document>> {
        self.state
            .search_engine
            .doc_store()
            .get_all_documents()
            .await
    }

    async fn query_documents(
        &self,
        from_ts: Option<f64>,
        to_ts: Option<f64>,
        folder: Option<&str>,
        limit: usize,
    ) -> anyhow::Result<Vec<Document>> {
        self.state
            .search_engine
            .doc_store()
            .query_documents(from_ts, to_ts, folder, limit)
            .await
    }

    fn send_query_event(
        &self,
        query: String,
        results: Vec<SearchResultSummary>,
        duration_ms: u64,
        timestamp: String,
    ) {
        let _ = self.state.action_bus.send(ActionEvent::QueryExecuted {
            query,
            results,
            duration_ms,
            timestamp,
        });
    }
}

pub(crate) struct ResolvedNotePath {
    pub relative: String,
    pub absolute: PathBuf,
}

pub(crate) trait TagsPorts {
    async fn get_all_documents(&self) -> anyhow::Result<Vec<Document>>;
    async fn resolve_note_path(&self, path: &str) -> anyhow::Result<ResolvedNotePath>;
    async fn read_file_to_string(&self, path: &Path) -> anyhow::Result<String>;
    async fn write_string_file(&self, path: &Path, content: &str) -> anyhow::Result<()>;
    fn timestamp_update(&self) -> anyhow::Result<Option<(String, String)>>;
    async fn reindex_files(&self, rel_paths: &[String]) -> anyhow::Result<()>;
}

impl TagsPorts for crate::KajetMcp {
    async fn get_all_documents(&self) -> anyhow::Result<Vec<Document>> {
        self.state
            .search_engine
            .doc_store()
            .get_all_documents()
            .await
    }

    async fn resolve_note_path(&self, path: &str) -> anyhow::Result<ResolvedNotePath> {
        let vault_path = std::path::Path::new(&self.state.vault_path);
        let doc_store = self.state.search_engine.doc_store();
        let resolved =
            kajet_writer::resolve::resolve_note_path(path, vault_path, Some(doc_store)).await?;
        Ok(ResolvedNotePath {
            relative: resolved.relative,
            absolute: resolved.absolute,
        })
    }

    async fn read_file_to_string(&self, path: &Path) -> anyhow::Result<String> {
        Ok(tokio::fs::read_to_string(path).await?)
    }

    async fn write_string_file(&self, path: &Path, content: &str) -> anyhow::Result<()> {
        tokio::fs::write(path, content).await?;
        Ok(())
    }

    fn timestamp_update(&self) -> anyhow::Result<Option<(String, String)>> {
        let config = self.state.config.read().unwrap();
        if config.writer.timestamps.enabled {
            let ts = kajet_writer::timestamp::format_now(&config.writer.timestamps)?;
            let field = config.writer.timestamps.modified_field.clone();
            Ok(Some((field, ts)))
        } else {
            Ok(None)
        }
    }

    async fn reindex_files(&self, rel_paths: &[String]) -> anyhow::Result<()> {
        let vault = std::path::Path::new(&self.state.vault_path);
        let indexer = self.state.indexer.read().await.clone();
        indexer.reindex_files(vault, rel_paths).await
    }
}

pub(crate) trait NotesPorts {
    async fn create_note(
        &self,
        params: kajet_writer::CreateNoteParams,
    ) -> anyhow::Result<kajet_writer::CreateNoteResult>;
    async fn edit_note(
        &self,
        params: kajet_writer::EditNoteParams,
    ) -> anyhow::Result<kajet_writer::EditNoteResult>;
    async fn reindex_files(&self, rel_paths: &[String]) -> anyhow::Result<()>;
}

impl NotesPorts for crate::KajetMcp {
    async fn create_note(
        &self,
        params: kajet_writer::CreateNoteParams,
    ) -> anyhow::Result<kajet_writer::CreateNoteResult> {
        let config = self.state.config.read().unwrap().writer.clone();
        let vault_path = std::path::PathBuf::from(&self.state.vault_path);
        let doc_store = self.state.search_engine.doc_store().clone();

        let writer = kajet_writer::NoteWriter::new(
            vault_path,
            self.state.db_path.clone(),
            config,
            Some(doc_store),
        );

        writer.create(params).await
    }

    async fn edit_note(
        &self,
        params: kajet_writer::EditNoteParams,
    ) -> anyhow::Result<kajet_writer::EditNoteResult> {
        let config = self.state.config.read().unwrap().writer.clone();
        let vault_path = std::path::PathBuf::from(&self.state.vault_path);
        let doc_store = self.state.search_engine.doc_store().clone();

        let writer = kajet_writer::NoteWriter::new(
            vault_path,
            self.state.db_path.clone(),
            config,
            Some(doc_store),
        );

        writer.edit(params).await
    }

    async fn reindex_files(&self, rel_paths: &[String]) -> anyhow::Result<()> {
        let vault = std::path::Path::new(&self.state.vault_path);
        let indexer = self.state.indexer.read().await.clone();
        indexer.reindex_files(vault, rel_paths).await
    }
}

pub(crate) trait IndexPorts {
    fn exclude_folders(&self) -> Vec<String>;
    async fn reindex_files(&self, rel_paths: &[String]) -> anyhow::Result<()>;
    async fn full_reindex(&self, exclude_folders: &[String]) -> anyhow::Result<IndexStats>;
    async fn get_index_stats(&self) -> anyhow::Result<IndexStats>;
}

impl IndexPorts for crate::KajetMcp {
    fn exclude_folders(&self) -> Vec<String> {
        self.state.config.read().unwrap().exclude_folders.clone()
    }

    async fn reindex_files(&self, rel_paths: &[String]) -> anyhow::Result<()> {
        let vault = std::path::Path::new(&self.state.vault_path);
        let indexer = self.state.indexer.read().await.clone();
        indexer.reindex_files(vault, rel_paths).await
    }

    async fn full_reindex(&self, exclude_folders: &[String]) -> anyhow::Result<IndexStats> {
        let vault = std::path::Path::new(&self.state.vault_path);
        let indexer = self.state.indexer.read().await.clone();
        indexer.full_reindex(vault, exclude_folders).await
    }

    async fn get_index_stats(&self) -> anyhow::Result<IndexStats> {
        let indexer = self.state.indexer.read().await.clone();
        indexer.get_index_stats().await
    }
}

pub(crate) trait DocumentsPorts {
    async fn examine(&self, path: &str) -> anyhow::Result<ExamineResult>;
}

impl DocumentsPorts for crate::KajetMcp {
    async fn examine(&self, path: &str) -> anyhow::Result<ExamineResult> {
        self.state.search_engine.examine(path).await
    }
}

pub(crate) trait AnalyticsPorts {
    async fn query_documents(
        &self,
        from_ts: Option<f64>,
        to_ts: Option<f64>,
        folder: Option<&str>,
        limit: usize,
    ) -> anyhow::Result<Vec<Document>>;

    async fn get_all_documents(&self) -> anyhow::Result<Vec<Document>>;
}

impl AnalyticsPorts for crate::KajetMcp {
    async fn query_documents(
        &self,
        from_ts: Option<f64>,
        to_ts: Option<f64>,
        folder: Option<&str>,
        limit: usize,
    ) -> anyhow::Result<Vec<Document>> {
        self.state
            .search_engine
            .doc_store()
            .query_documents(from_ts, to_ts, folder, limit)
            .await
    }

    async fn get_all_documents(&self) -> anyhow::Result<Vec<Document>> {
        self.state
            .search_engine
            .doc_store()
            .get_all_documents()
            .await
    }
}
