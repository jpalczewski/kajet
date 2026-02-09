use crate::date_parser::{DateBound, parse_date};
use crate::format::{
    format_create_result, format_edit_result, format_edit_tags_result, format_entries,
    format_examine_result, format_list_tags, format_results,
};
use crate::schema::{
    CreateNoteRequest, EditNoteRequest, EditTagsRequest, ExamineRequest, ListTagsRequest,
    ReindexRequest, SearchRequest, TreeRequest,
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
        description = "Search or browse the Obsidian vault. With 'query': semantic/hybrid/FTS search with optional filters (from/to/tags/folder). Without 'query': browse mode with date/tag/folder filters. At least 'query' or one filter required."
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

        // Validation: at least query or one filter
        let has_query = req.query.is_some();
        let has_filters = req.from.is_some()
            || req.to.is_some()
            || req.tags.as_ref().map(|t| !t.is_empty()).unwrap_or(false)
            || req.folder.is_some();

        if !has_query && !has_filters {
            return Err(internal_error(t!("search_no_params").to_string()));
        }

        // Parse dates
        let from_date = if let Some(ref from_str) = req.from {
            Some(parse_date(from_str, DateBound::From).map_err(|e| {
                internal_error(
                    t!(
                        "search_date_parse_error",
                        input = &e.input,
                        error = e.to_string()
                    )
                    .to_string(),
                )
            })?)
        } else {
            None
        };

        let to_date = if let Some(ref to_str) = req.to {
            Some(parse_date(to_str, DateBound::To).map_err(|e| {
                internal_error(
                    t!(
                        "search_date_parse_error",
                        input = &e.input,
                        error = e.to_string()
                    )
                    .to_string(),
                )
            })?)
        } else if from_date.is_some() {
            // Default to end of today if from is set
            Some(chrono::Local::now().date_naive())
        } else {
            None
        };

        // Validate date range
        if let (Some(from), Some(to)) = (from_date, to_date)
            && from > to
        {
            return Err(internal_error(
                t!(
                    "search_date_range_error",
                    from = from.to_string(),
                    to = to.to_string()
                )
                .to_string(),
            ));
        }

        // Convert dates to timestamps
        let from_ts = from_date.map(|d| {
            d.and_hms_opt(0, 0, 0)
                .expect("00:00:00 is always a valid time")
                .and_utc()
                .timestamp() as f64
        });
        let to_ts = to_date.map(|d| {
            d.and_hms_opt(23, 59, 59)
                .expect("23:59:59 is always a valid time")
                .and_utc()
                .timestamp() as f64
        });

        let start = std::time::Instant::now();

        // Branch: search mode vs browse mode
        if let Some(ref query) = req.query {
            // SEARCH MODE: semantic/hybrid/fts with post-filtering
            let mode = req.mode.as_deref().unwrap_or("hybrid");
            let span = tracing::Span::current();
            span.record("query", query.as_str());
            span.record("mode", mode);
            span.record("limit", limit);

            // Over-fetch if filters are active
            let overfetch_multiplier = self
                .state
                .config
                .read()
                .unwrap()
                .filter_overfetch_multiplier;
            let fetch_limit = if has_filters {
                limit * overfetch_multiplier
            } else {
                limit
            };

            let mut results = match mode {
                "vector" => {
                    self.state
                        .search_engine
                        .vector_search(query, fetch_limit)
                        .await
                }
                "fts" => {
                    self.state
                        .search_engine
                        .fts_search(query, fetch_limit)
                        .await
                }
                _ => {
                    self.state
                        .search_engine
                        .hybrid_search(query, fetch_limit)
                        .await
                }
            }
            .map_err(|e| internal_error(t!("search_failed", error = e.to_string()).to_string()))?;

            // Post-filter results
            if has_filters {
                // Get all documents for metadata lookups
                let all_docs = self
                    .state
                    .search_engine
                    .doc_store()
                    .get_all_documents()
                    .await
                    .map_err(|e| internal_error(e.to_string()))?;

                let doc_map: HashMap<String, &kajet_core::types::Document> = all_docs
                    .iter()
                    .map(|d| (d.source_file.clone(), d))
                    .collect();

                results.retain(|r| {
                    // Filter by folder
                    if let Some(ref folder) = req.folder {
                        let prefix = if folder.ends_with('/') {
                            folder.clone()
                        } else {
                            format!("{}/", folder)
                        };
                        if !r.note_path.starts_with(&prefix) {
                            return false;
                        }
                    }

                    // Filter by date and tags (need document metadata)
                    if let Some(doc) = doc_map.get(&r.note_path) {
                        // Date filter
                        if let Some(from) = from_ts
                            && doc.last_modified < from
                        {
                            return false;
                        }
                        if let Some(to) = to_ts
                            && doc.last_modified > to
                        {
                            return false;
                        }

                        // Tag filter (all required)
                        if let Some(ref required_tags) = req.tags
                            && !tags_match(&doc.tags, required_tags)
                        {
                            return false;
                        }
                    } else {
                        // Document not found in metadata, filter out
                        return false;
                    }

                    true
                });

                // Apply limit after filtering
                results.truncate(limit);
            }

            span.record("results", results.len());

            tracing::info!(
                query = query.as_str(),
                mode,
                limit,
                results = results.len(),
                elapsed_ms = start.elapsed().as_millis() as u64,
                "MCP search"
            );

            let _ = self.state.events.send(QueryEvent {
                query: query.clone(),
                num_results: results.len(),
                timestamp: chrono::Utc::now(),
            });

            let summary = format_results(query, &results);
            Ok(CallToolResult::success(vec![Content::text(summary)]))
        } else {
            // BROWSE MODE: query documents by filters
            // When filtering by tags only (no date/folder constraints), we need to fetch
            // a large number of documents since tag filtering happens post-query.
            let (overfetch_multiplier, tags_only_limit) = {
                let config = self.state.config.read().unwrap();
                (
                    config.filter_overfetch_multiplier,
                    config.tags_only_fetch_limit,
                )
            };

            let has_date_or_folder = from_ts.is_some() || to_ts.is_some() || req.folder.is_some();
            let fetch_limit = if has_date_or_folder {
                limit * overfetch_multiplier
            } else {
                // Tags-only filtering: fetch up to configured limit to maximize chance of matches
                tags_only_limit.max(limit * overfetch_multiplier)
            };

            let mut docs = self
                .state
                .search_engine
                .doc_store()
                .query_documents(from_ts, to_ts, req.folder.as_deref(), fetch_limit)
                .await
                .map_err(|e| internal_error(e.to_string()))?;

            // Post-filter by tags
            if let Some(ref required_tags) = req.tags {
                docs.retain(|d| tags_match(&d.tags, required_tags));
            }

            // Apply limit
            docs.truncate(limit);

            tracing::info!(
                from = ?from_date,
                to = ?to_date,
                folder = req.folder.as_deref().unwrap_or("*"),
                tags = ?req.tags,
                results = docs.len(),
                elapsed_ms = start.elapsed().as_millis() as u64,
                "MCP browse"
            );

            // Send synthetic event for dashboard
            let query_display = format!(
                "[browse] from:{} to:{} folder:{} tags:{}",
                from_date
                    .map(|d| d.to_string())
                    .unwrap_or_else(|| "*".to_string()),
                to_date
                    .map(|d| d.to_string())
                    .unwrap_or_else(|| "*".to_string()),
                req.folder.as_deref().unwrap_or("*"),
                req.tags
                    .as_ref()
                    .map(|t| t.join(","))
                    .unwrap_or_else(|| "*".to_string())
            );
            let _ = self.state.events.send(QueryEvent {
                query: query_display,
                num_results: docs.len(),
                timestamp: chrono::Utc::now(),
            });

            let summary = format_entries(from_date, to_date, &docs);
            Ok(CallToolResult::success(vec![Content::text(summary)]))
        }
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

    #[tool(
        description = "Edit tags in a note's frontmatter. Adds/removes tags and optionally updates timestamp. Supports fuzzy path matching. At least one of 'add' or 'remove' must be provided."
    )]
    #[instrument(
        level = "debug",
        skip(self, params),
        fields(path, add_count, remove_count, timestamp_updated)
    )]
    async fn edit_tags(
        &self,
        params: Parameters<EditTagsRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let req = params.0;
        let span = tracing::Span::current();
        span.record("path", req.path.as_str());

        // Validation: at least one of add/remove must be provided
        let has_add = req.add.as_ref().map(|v| !v.is_empty()).unwrap_or(false);
        let has_remove = req.remove.as_ref().map(|v| !v.is_empty()).unwrap_or(false);

        if !has_add && !has_remove {
            return Err(internal_error(t!("edit_tags_empty_params").to_string()));
        }

        span.record("add_count", req.add.as_ref().map(|v| v.len()).unwrap_or(0));
        span.record(
            "remove_count",
            req.remove.as_ref().map(|v| v.len()).unwrap_or(0),
        );

        // Resolve path (fuzzy suffix matching)
        let vault_path = std::path::Path::new(&self.state.vault_path);
        let doc_store = self.state.search_engine.doc_store();
        let resolved =
            kajet_writer::resolve::resolve_note_path(&req.path, vault_path, Some(doc_store))
                .await
                .map_err(|e| {
                    internal_error(
                        t!(
                            "edit_tags_path_not_found",
                            path = &req.path,
                            error = e.to_string()
                        )
                        .to_string(),
                    )
                })?;

        // Read file
        let content = tokio::fs::read_to_string(&resolved.absolute)
            .await
            .map_err(|e| internal_error(e.to_string()))?;

        // Apply tag operations (remove first, then add)
        let mut modified = content;

        if let Some(remove) = &req.remove {
            modified = kajet_parser::remove_tags(&modified, remove)
                .map_err(|e| internal_error(e.to_string()))?;
        }

        if let Some(add) = &req.add {
            modified = kajet_parser::add_tags(&modified, add)
                .map_err(|e| internal_error(e.to_string()))?;
        }

        // Update timestamp if enabled
        let (timestamp_updated, modified_field) = {
            let config = self.state.config.read().unwrap();
            (
                config.writer.timestamps.enabled,
                config.writer.timestamps.modified_field.clone(),
            )
        };

        let timestamp_updated = if timestamp_updated {
            let config_timestamps = self.state.config.read().unwrap().writer.timestamps.clone();
            let ts = kajet_writer::timestamp::format_now(&config_timestamps)
                .map_err(|e| internal_error(e.to_string()))?;

            modified =
                kajet_parser::update_existing_frontmatter_field(&modified, &modified_field, &ts);
            true
        } else {
            false
        };

        span.record("timestamp_updated", timestamp_updated);

        // Write file back
        tokio::fs::write(&resolved.absolute, &modified)
            .await
            .map_err(|e| internal_error(e.to_string()))?;

        // Reindex the edited file
        if let Err(e) = self
            .state
            .indexer
            .reindex_files(vault_path, std::slice::from_ref(&resolved.relative))
            .await
        {
            tracing::warn!(
                path = %resolved.relative,
                error = %e,
                "Failed to reindex after edit_tags"
            );
        }

        tracing::info!(
            path = %resolved.relative,
            added = req.add.as_ref().map(|v| v.len()).unwrap_or(0),
            removed = req.remove.as_ref().map(|v| v.len()).unwrap_or(0),
            "Tags edited successfully"
        );

        Ok(CallToolResult::success(vec![Content::text(
            format_edit_tags_result(&resolved.relative, &req.add, &req.remove),
        )]))
    }

    #[tool(
        description = "Show vault folder structure as a tree. Returns folder names with note counts. Use 'path' to focus on a subfolder, 'show: files' to include filenames."
    )]
    #[instrument(level = "debug", skip(self, params), fields(path, depth, size, show))]
    async fn tree(&self, params: Parameters<TreeRequest>) -> Result<CallToolResult, ErrorData> {
        let req = params.0;

        let (depth, size, max_chars, exclude) = {
            let config = self.state.config.read().unwrap();
            let depth = req.depth.unwrap_or(config.tree.depth);
            let size = req.size.unwrap_or(config.tree.size);
            let max_chars = config.tree.max_chars;
            let exclude = config.exclude_folders.clone();
            (depth, size, max_chars, exclude)
        };

        // Validate parameters
        if depth == 0 {
            return Err(internal_error(t!("tree_depth_zero").to_string()));
        }
        if size == 0 {
            return Err(internal_error(t!("tree_size_zero").to_string()));
        }

        let show = match req.show.as_deref() {
            None | Some("folders") => kajet_parser::ShowMode::Folders,
            Some("files") => kajet_parser::ShowMode::Files,
            Some(invalid) => {
                return Err(internal_error(
                    t!("tree_invalid_show", mode = invalid).to_string(),
                ));
            }
        };

        let span = tracing::Span::current();
        span.record("path", req.path.as_deref().unwrap_or("*"));
        span.record("depth", depth);
        span.record("size", size);
        span.record("show", format!("{:?}", show).as_str());

        let vault_path = self.state.vault_path.clone();
        let path = req.path.clone();
        let options = kajet_parser::VaultTreeOptions {
            path,
            depth,
            size,
            show,
        };

        let tree = kajet_parser::vault_tree(&vault_path, &exclude, &options)
            .await
            .map_err(|e| internal_error(e.to_string()))?;

        let output = crate::format::format_vault_tree(&tree);

        if output.len() > max_chars {
            let has_files = show == kajet_parser::ShowMode::Files;
            return Ok(CallToolResult::success(vec![Content::text(
                crate::format::format_tree_too_large(output.len(), max_chars, depth, has_files),
            )]));
        }

        tracing::info!(
            path = req.path.as_deref().unwrap_or("*"),
            depth,
            size,
            entries = tree.subfolders.len(),
            output_chars = output.len(),
            "MCP tree"
        );

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }
}

/// Check if document tags contain all required tags (case-insensitive, # prefix ignored).
fn tags_match(doc_tags: &[String], required: &[String]) -> bool {
    let doc_tags_normalized: Vec<String> = doc_tags
        .iter()
        .map(|t| t.strip_prefix('#').unwrap_or(t).to_lowercase())
        .collect();

    required.iter().all(|req_tag| {
        let normalized = req_tag.strip_prefix('#').unwrap_or(req_tag).to_lowercase();
        doc_tags_normalized.contains(&normalized)
    })
}
