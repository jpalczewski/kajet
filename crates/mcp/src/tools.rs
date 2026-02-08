use crate::format::{
    format_create_result, format_edit_result, format_examine_result, format_list_tags,
    format_results,
};
use crate::schema::{
    CreateNoteRequest, EditNoteRequest, ExamineRequest, ListTagsRequest, ReindexRequest,
    SearchRequest,
};
use kajet_core::types::QueryEvent;
use rmcp::{handler::server::wrapper::Parameters, model::*, tool, tool_router};
use std::collections::HashMap;
use tracing::instrument;

fn internal_error(msg: impl Into<String>) -> ErrorData {
    ErrorData {
        code: ErrorCode::INTERNAL_ERROR,
        message: msg.into().into(),
        data: None,
    }
}

#[tool_router(router = tool_router, vis = "pub(crate)")]
impl crate::KajetMcp {
    #[tool(
        description = "Search the Obsidian vault using hybrid search (vector + full-text). Supports modes: 'hybrid' (default, best quality), 'vector' (semantic similarity), 'fts' (keyword matching)."
    )]
    #[instrument(
        level = "debug",
        skip(self, params),
        fields(query, mode, limit, results)
    )]
    async fn search(&self, params: Parameters<SearchRequest>) -> Result<CallToolResult, ErrorData> {
        let req = params.0;
        let limit = req
            .limit
            .unwrap_or(self.state.config.read().unwrap().default_limit);
        let mode = req.mode.as_deref().unwrap_or("hybrid");

        let span = tracing::Span::current();
        span.record("query", req.query.as_str());
        span.record("mode", mode);
        span.record("limit", limit);

        let start = std::time::Instant::now();

        let results = match mode {
            "vector" => {
                self.state
                    .search_engine
                    .vector_search(&req.query, limit)
                    .await
            }
            "fts" => self.state.search_engine.fts_search(&req.query, limit).await,
            _ => {
                self.state
                    .search_engine
                    .hybrid_search(&req.query, limit)
                    .await
            }
        }
        .map_err(|e| internal_error(t!("search_failed", error = e.to_string()).to_string()))?;

        span.record("results", results.len());

        tracing::info!(
            query = req.query.as_str(),
            mode,
            limit,
            results = results.len(),
            elapsed_ms = start.elapsed().as_millis() as u64,
            "MCP search"
        );

        let _ = self.state.events.send(QueryEvent {
            query: req.query.clone(),
            num_results: results.len(),
            timestamp: chrono::Utc::now(),
        });

        let summary = format_results(&req.query, &results);

        Ok(CallToolResult::success(vec![Content::text(summary)]))
    }

    #[tool(
        description = "Reindex the Obsidian vault. Without arguments, performs a full vault reindex. With a 'path' argument, reindexes only that specific file."
    )]
    async fn reindex(
        &self,
        params: Parameters<ReindexRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let req = params.0;
        let vault_path = std::path::Path::new(&self.state.vault_path);

        let map_err = |e: anyhow::Error| {
            internal_error(t!("reindex_failed", error = e.to_string()).to_string())
        };

        match req.path {
            Some(ref path) => {
                self.state
                    .indexer
                    .reindex_files(vault_path, std::slice::from_ref(path))
                    .await
                    .map_err(map_err)?;

                Ok(CallToolResult::success(vec![Content::text(
                    t!("reindex_file_complete", path = path).to_string(),
                )]))
            }
            None => {
                let exclude = self.state.config.read().unwrap().exclude_folders.clone();
                let stats = self
                    .state
                    .indexer
                    .full_reindex(vault_path, &exclude)
                    .await
                    .map_err(map_err)?;

                Ok(CallToolResult::success(vec![Content::text(
                    t!(
                        "reindex_complete",
                        documents = stats.total_documents,
                        chunks = stats.total_chunks
                    )
                    .to_string(),
                )]))
            }
        }
    }

    #[tool(
        description = "Examine an indexed document: view its metadata (title, tags, outgoing links, backlinks) and content. Accepts full or partial file paths with fuzzy matching."
    )]
    #[instrument(level = "debug", skip(self, params), fields(path))]
    async fn examine(
        &self,
        params: Parameters<ExamineRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let req = params.0;
        let span = tracing::Span::current();
        span.record("path", req.path.as_str());

        let content_mode = req.content.as_deref().unwrap_or("summary");
        let offset = req.offset.unwrap_or(0);
        let length = req.length.unwrap_or(500);

        let result = self
            .state
            .search_engine
            .examine(&req.path)
            .await
            .map_err(|e| internal_error(e.to_string()))?;

        let text = format_examine_result(&result.document, content_mode, offset, length);

        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    #[tool(
        description = "Get the current index status: number of indexed documents, chunks, and last indexing time."
    )]
    async fn index_status(&self) -> Result<CallToolResult, ErrorData> {
        let stats = self
            .state
            .indexer
            .get_index_stats()
            .await
            .map_err(|e| internal_error(e.to_string()))?;

        let last_indexed = match stats.last_indexed {
            Some(time) => t!("index_status_last_indexed", time = time.to_rfc3339()).to_string(),
            None => t!("index_status_never").to_string(),
        };

        let text = format!(
            "{}\n{}\n{}\n{}",
            t!("index_status_header"),
            t!("index_status_documents", count = stats.total_documents),
            t!("index_status_chunks", count = stats.total_chunks),
            last_indexed,
        );

        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    #[tool(
        description = "Create a new note in the Obsidian vault. Generates frontmatter with title, tags, and timestamps automatically. Fails if file already exists."
    )]
    #[instrument(level = "debug", skip(self, params), fields(target))]
    async fn create_note(
        &self,
        params: Parameters<CreateNoteRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let req = params.0;
        let span = tracing::Span::current();
        span.record("target", req.target.as_str());

        let config = self.state.config.read().unwrap().writer.clone();
        let vault_path = std::path::PathBuf::from(&self.state.vault_path);
        let doc_store = self.state.search_engine.doc_store().clone();

        let writer = kajet_writer::NoteWriter::new(
            vault_path,
            self.state.db_path.clone(),
            config,
            Some(doc_store),
        );

        let result = writer
            .create(kajet_writer::CreateNoteParams {
                target: req.target,
                content: req.content,
                tags: req.tags.unwrap_or_default(),
                aliases: req.aliases.unwrap_or_default(),
            })
            .await
            .map_err(|e| {
                internal_error(t!("create_note_failed", error = e.to_string()).to_string())
            })?;

        // Reindex the newly created file
        let vault = std::path::Path::new(&self.state.vault_path);
        if let Err(e) = self
            .state
            .indexer
            .reindex_files(vault, std::slice::from_ref(&result.path))
            .await
        {
            tracing::warn!(path = %result.path, error = %e, "Failed to reindex after create");
        }

        Ok(CallToolResult::success(vec![Content::text(
            format_create_result(&result),
        )]))
    }

    #[tool(
        description = "Edit an existing note in the Obsidian vault. Modes: 'append' (add to end), 'prepend' (add after frontmatter), 'overwrite' (replace body), 'replace_section' (replace heading section), 'replace_text' (exact string replacement), 'insert_after' (insert content after exact text anchor, uses old_text as anchor). Backups are created for destructive operations."
    )]
    #[instrument(level = "debug", skip(self, params), fields(path, mode))]
    async fn edit_note(
        &self,
        params: Parameters<EditNoteRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let req = params.0;
        let span = tracing::Span::current();
        span.record("path", req.path.as_str());
        span.record("mode", req.mode.as_str());

        let mode: kajet_writer::EditMode = req.mode.parse().map_err(|_| {
            internal_error(t!("edit_note_invalid_mode", mode = &req.mode).to_string())
        })?;

        let config = self.state.config.read().unwrap().writer.clone();
        let vault_path = std::path::PathBuf::from(&self.state.vault_path);
        let doc_store = self.state.search_engine.doc_store().clone();

        let writer = kajet_writer::NoteWriter::new(
            vault_path,
            self.state.db_path.clone(),
            config,
            Some(doc_store),
        );

        let result = writer
            .edit(kajet_writer::EditNoteParams {
                path: req.path,
                content: req.content,
                mode,
                target_heading: req.target_heading,
                old_text: req.old_text,
            })
            .await
            .map_err(|e| {
                internal_error(t!("edit_note_failed", error = e.to_string()).to_string())
            })?;

        // Reindex the edited file
        let vault = std::path::Path::new(&self.state.vault_path);
        if let Err(e) = self
            .state
            .indexer
            .reindex_files(vault, std::slice::from_ref(&result.path))
            .await
        {
            tracing::warn!(path = %result.path, error = %e, "Failed to reindex after edit");
        }

        Ok(CallToolResult::success(vec![Content::text(
            format_edit_result(&result),
        )]))
    }

    #[tool(
        description = "List all unique tags from the vault, optionally filtered by folder. Detail levels: 'names' (tag list), 'counts' (tag + document count, default), 'full' (tag + count + file paths)."
    )]
    #[instrument(level = "debug", skip(self, params), fields(folder, detail, tag_count))]
    async fn list_tags(
        &self,
        params: Parameters<ListTagsRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let req = params.0;
        let detail = req.detail.as_deref().unwrap_or("counts");
        let recursive = req.recursive.unwrap_or(true);

        let span = tracing::Span::current();
        span.record("folder", req.folder.as_deref().unwrap_or("*"));
        span.record("detail", detail);

        let all_docs = self
            .state
            .search_engine
            .doc_store()
            .get_all_documents()
            .await
            .map_err(|e| {
                internal_error(t!("list_tags_failed", error = e.to_string()).to_string())
            })?;

        let folder_prefix = req.folder.as_deref().map(|f| {
            let f = f.strip_suffix('/').unwrap_or(f);
            format!("{f}/")
        });

        let docs: Vec<&kajet_core::types::Document> = all_docs
            .iter()
            .filter(|d| {
                let Some(ref prefix) = folder_prefix else {
                    return true;
                };
                if !d.source_file.starts_with(prefix.as_str()) {
                    return false;
                }
                if !recursive {
                    let rest = &d.source_file[prefix.len()..];
                    return !rest.contains('/');
                }
                true
            })
            .collect();

        let mut tag_map: HashMap<String, Vec<String>> = HashMap::new();
        for doc in &docs {
            for tag in &doc.tags {
                tag_map
                    .entry(tag.clone())
                    .or_default()
                    .push(doc.source_file.clone());
            }
        }

        let mut sorted: Vec<(String, Vec<String>)> = tag_map.into_iter().collect();
        sorted.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(&b.0)));

        span.record("tag_count", sorted.len());
        tracing::info!(
            folder = req.folder.as_deref().unwrap_or("*"),
            tag_count = sorted.len(),
            "MCP list_tags"
        );

        let text = format_list_tags(&sorted, detail, req.folder.as_deref());
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }
}
