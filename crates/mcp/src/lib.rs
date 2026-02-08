#[macro_use]
extern crate rust_i18n;

i18n!("../../locales", fallback = "en");

use anyhow::Result;
use kajet_core::search::SearchResult;
use kajet_core::types::{AppState, Document, QueryEvent};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars, tool, tool_handler, tool_router,
    transport::stdio,
    ServerHandler, ServiceExt,
};
use std::sync::Arc;
use tracing::instrument;

// ---------------------------------------------------------------------------
// Tool input schemas
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ReindexRequest {
    /// Optional relative path of a specific file to reindex. If omitted, full vault reindex.
    #[schemars(
        description = "Optional relative path of a specific file to reindex. Omit for full vault reindex."
    )]
    pub path: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ExamineRequest {
    /// Path to the document (full or partial, e.g. "myfile.md" or "subfolder/myfile")
    #[schemars(
        description = "Path to the document — full relative path or partial (filename). Fuzzy suffix matching is used if exact match fails."
    )]
    pub path: String,

    /// Content display mode: "summary" (first 500 chars, default), "full" (entire content), "slice" (use offset+length)
    #[schemars(
        description = "Content mode: 'summary' (first 500 chars, default), 'full' (entire content), 'slice' (use offset+length)"
    )]
    pub content: Option<String>,

    /// Character offset for 'slice' mode (default: 0)
    #[schemars(description = "Character offset for 'slice' mode (default: 0)")]
    pub offset: Option<usize>,

    /// Number of characters for 'slice' mode (default: 500)
    #[schemars(description = "Number of characters for 'slice' mode (default: 500)")]
    pub length: Option<usize>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SearchRequest {
    /// The search query
    #[schemars(description = "Search query for finding relevant documents in the Obsidian vault")]
    pub query: String,

    /// Max number of results
    #[schemars(description = "Maximum number of results to return (default: 5)")]
    pub limit: Option<usize>,

    /// Search mode: "hybrid" (default), "vector", "fts"
    #[schemars(
        description = "Search mode: 'hybrid' (vector + full-text, default), 'vector' (semantic only), 'fts' (keyword only)"
    )]
    pub mode: Option<String>,
}

// ---------------------------------------------------------------------------
// Result formatting
// ---------------------------------------------------------------------------

pub fn format_results(query: &str, results: &[SearchResult]) -> String {
    if results.is_empty() {
        return t!("no_results", query = query).to_string();
    }

    let formatted = results
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let mut parts = format!(
                "{}\n{}\n{}\n\n{}",
                t!(
                    "result_header",
                    index = i + 1,
                    score = format!("{:.4}", r.score)
                ),
                t!("result_path", path = &r.note_path),
                t!("result_section", breadcrumb = &r.breadcrumb),
                r.content,
            );
            if !r.links.is_empty() {
                let link_list: Vec<String> = r
                    .links
                    .iter()
                    .map(|l| match (&l.alias, &l.resolved_path) {
                        (Some(alias), Some(path)) => {
                            format!("  - {} → {} ({})", l.target, path, alias)
                        }
                        (None, Some(path)) => format!("  - {} → {}", l.target, path),
                        (Some(alias), None) => format!("  - {} ({})", l.target, alias),
                        (None, None) => format!("  - {}", l.target),
                    })
                    .collect();
                parts.push_str(&format!(
                    "\n\n{}\n{}",
                    t!("result_links"),
                    link_list.join("\n")
                ));
            }
            parts
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    format!(
        "{}\n\n{}",
        t!("found_results", count = results.len(), query = query),
        formatted
    )
}

pub fn format_examine_result(
    doc: &Document,
    content_mode: &str,
    offset: usize,
    length: usize,
) -> String {
    let mut parts = Vec::new();

    // Title
    parts.push(t!("examine_title", title = &doc.title).to_string());

    // Tags
    if doc.tags.is_empty() {
        parts.push(t!("examine_no_tags").to_string());
    } else {
        parts.push(t!("examine_tags", tags = doc.tags.join(", ")).to_string());
    }

    // Outgoing links
    let no_links = t!("examine_no_links").to_string();
    if doc.outgoing_links.is_empty() {
        parts.push(format!(
            "{}\n  {}",
            t!("examine_outgoing_links", count = 0),
            no_links
        ));
    } else {
        let links_list = doc
            .outgoing_links
            .iter()
            .map(|l| format!("  - {l}"))
            .collect::<Vec<_>>()
            .join("\n");
        parts.push(format!(
            "{}\n{}",
            t!("examine_outgoing_links", count = doc.outgoing_links.len()),
            links_list
        ));
    }

    // Backlinks
    if doc.backlinks.is_empty() {
        parts.push(format!(
            "{}\n  {}",
            t!("examine_backlinks", count = 0),
            no_links
        ));
    } else {
        let bl_list = doc
            .backlinks
            .iter()
            .map(|l| format!("  - {l}"))
            .collect::<Vec<_>>()
            .join("\n");
        parts.push(format!(
            "{}\n{}",
            t!("examine_backlinks", count = doc.backlinks.len()),
            bl_list
        ));
    }

    // Content
    let total = doc.full_text.len();
    let text_slice = match content_mode {
        "full" => doc.full_text.clone(),
        "slice" => {
            let start = offset.min(total);
            let end = (start + length).min(total);
            let start = doc.full_text.floor_char_boundary(start);
            let end = doc.full_text.floor_char_boundary(end);
            doc.full_text[start..end].to_string()
        }
        _ => {
            // summary: first 500 chars
            let end = total.min(500);
            let end = doc.full_text.floor_char_boundary(end);
            let slice = &doc.full_text[..end];
            if end < total {
                format!("{slice}...")
            } else {
                slice.to_string()
            }
        }
    };

    parts.push(
        t!(
            "examine_content_info",
            shown = text_slice.len(),
            total = total
        )
        .to_string(),
    );
    parts.push(text_slice);

    parts.join("\n")
}

// ---------------------------------------------------------------------------
// MCP Handler
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct KajetMcp {
    state: Arc<AppState>,
    tool_router: ToolRouter<Self>,
}

#[tool_handler]
impl ServerHandler for KajetMcp {
    fn get_info(&self) -> rmcp::model::ServerInfo {
        rmcp::model::ServerInfo {
            protocol_version: rmcp::model::ProtocolVersion::LATEST,
            capabilities: rmcp::model::ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: rmcp::model::Implementation {
                name: "kajet".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                title: Some(t!("server_title").into()),
                icons: None,
                website_url: None,
            },
            instructions: Some(t!("server_instructions").into()),
        }
    }
}

#[tool_router]
impl KajetMcp {
    pub fn new(state: Arc<AppState>) -> Self {
        Self {
            state,
            tool_router: Self::tool_router(),
        }
    }

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
        .map_err(|e| ErrorData {
            code: ErrorCode::INTERNAL_ERROR,
            message: t!("search_failed", error = e.to_string()),
            data: None,
        })?;

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

        let map_err = |e: anyhow::Error| ErrorData {
            code: ErrorCode::INTERNAL_ERROR,
            message: t!("reindex_failed", error = e.to_string()),
            data: None,
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
            .map_err(|e| ErrorData {
                code: ErrorCode::INTERNAL_ERROR,
                message: e.to_string().into(),
                data: None,
            })?;

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
            .map_err(|e| ErrorData {
                code: ErrorCode::INTERNAL_ERROR,
                message: e.to_string().into(),
                data: None,
            })?;

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
}

// ---------------------------------------------------------------------------
// Serve MCP over stdio
// ---------------------------------------------------------------------------

pub async fn serve(state: Arc<AppState>) -> Result<()> {
    let handler = KajetMcp::new(state);
    let service = handler.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_locale() {
        rust_i18n::set_locale("en");
    }

    #[test]
    fn format_results_empty() {
        setup_locale();
        let result = format_results("test query", &[]);
        assert_eq!(result, "No results found for: test query");
    }

    #[test]
    fn format_results_single() {
        setup_locale();
        let results = vec![SearchResult {
            note_path: "note.md".into(),
            breadcrumb: "note.md > Intro".into(),
            content: "Hello world".into(),
            raw_content: "Hello world".into(),
            links: vec![],
            score: 0.1234,
            search_type: kajet_core::types::SearchType::Vector,
        }];
        let result = format_results("hello", &results);
        assert!(result.contains("Found 1 results for: \"hello\""));
        assert!(result.contains("Result 1 (score: 0.1234)"));
        assert!(result.contains("Path: note.md"));
        assert!(result.contains("Section: note.md > Intro"));
        assert!(result.contains("Hello world"));
    }

    #[test]
    fn format_results_multiple() {
        setup_locale();
        let results = vec![
            SearchResult {
                note_path: "a.md".into(),
                breadcrumb: "a.md".into(),
                content: "AAA".into(),
                raw_content: "AAA".into(),
                links: vec![],
                score: 0.1,
                search_type: kajet_core::types::SearchType::Vector,
            },
            SearchResult {
                note_path: "b.md".into(),
                breadcrumb: "b.md".into(),
                content: "BBB".into(),
                raw_content: "BBB".into(),
                links: vec![],
                score: 0.5,
                search_type: kajet_core::types::SearchType::Fts,
            },
        ];
        let result = format_results("query", &results);
        assert!(result.contains("Found 2 results"));
        assert!(result.contains("Result 1"));
        assert!(result.contains("Result 2"));
    }

    #[test]
    fn format_results_polish_content() {
        setup_locale();
        let results = vec![SearchResult {
            note_path: "łódź.md".into(),
            breadcrumb: "łódź.md > Główne zabytki".into(),
            content: "Pałac Izraela Poznańskiego — największy pałac przemysłowca w Europie. Zażółć gęślą jaźń.".into(),
            raw_content: "Pałac Izraela Poznańskiego — największy pałac przemysłowca w Europie. Zażółć gęślą jaźń.".into(),
            links: vec![],
            score: 0.8765,
            search_type: kajet_core::types::SearchType::Vector,
        }];
        let result = format_results("pałac", &results);
        assert!(result.contains("Path: łódź.md"));
        assert!(result.contains("Główne zabytki"));
        assert!(result.contains("Poznańskiego"));
        assert!(result.contains("jaźń"));
    }

    #[test]
    fn format_results_polish_query() {
        setup_locale();
        let result = format_results("zażółć gęślą jaźń", &[]);
        assert!(result.contains("zażółć gęślą jaźń"));
    }
}
