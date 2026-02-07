#[macro_use]
extern crate rust_i18n;

i18n!("../../locales", fallback = "en");

use anyhow::Result;
use kajet_core::search::SearchResult;
use kajet_core::types::{AppState, QueryEvent};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars, tool, tool_handler, tool_router,
    transport::stdio,
    ServerHandler, ServiceExt,
};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Tool input schemas
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SearchRequest {
    /// The search query to find relevant notes in the vault
    #[schemars(description = "Search query for semantic search over the Obsidian vault")]
    pub query: String,

    /// Max number of results (default from config)
    #[schemars(description = "Maximum number of results to return (default: 5)")]
    pub limit: Option<usize>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ReindexRequest {
    /// Optional relative path of a specific file to reindex. If omitted, full vault reindex.
    #[schemars(
        description = "Optional relative path of a specific file to reindex. Omit for full vault reindex."
    )]
    pub path: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SearchDocsRequest {
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
            format!(
                "{}\n{}\n{}\n\n{}",
                t!(
                    "result_header",
                    index = i + 1,
                    score = format!("{:.4}", r.score)
                ),
                t!("result_path", path = &r.note_path),
                t!("result_section", breadcrumb = &r.breadcrumb),
                r.content,
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    format!(
        "{}\n\n{}",
        t!("found_results", count = results.len(), query = query),
        formatted
    )
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
        description = "Search the Obsidian vault using semantic search. Returns the most relevant note chunks with their breadcrumb paths and content."
    )]
    async fn search(&self, params: Parameters<SearchRequest>) -> Result<CallToolResult, ErrorData> {
        let req = params.0;
        let limit = req
            .limit
            .unwrap_or(self.state.config.read().unwrap().default_limit);

        let results = self
            .state
            .search_engine
            .vector_search(&req.query, limit)
            .await
            .map_err(|e| ErrorData {
                code: ErrorCode::INTERNAL_ERROR,
                message: t!("search_failed", error = e.to_string()),
                data: None,
            })?;

        // Emit event to dashboard
        let _ = self.state.events.send(QueryEvent {
            query: req.query.clone(),
            num_results: results.len(),
            timestamp: chrono::Utc::now(),
        });

        let summary = format_results(&req.query, &results);

        Ok(CallToolResult::success(vec![Content::text(summary)]))
    }

    #[tool(
        description = "Search documents in the Obsidian vault using hybrid search (vector + full-text). Supports modes: 'hybrid' (default, best quality), 'vector' (semantic similarity), 'fts' (keyword matching)."
    )]
    async fn search_docs(
        &self,
        params: Parameters<SearchDocsRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let req = params.0;
        let limit = req
            .limit
            .unwrap_or(self.state.config.read().unwrap().default_limit);
        let mode = req.mode.as_deref().unwrap_or("hybrid");

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
                score: 0.1,
                search_type: kajet_core::types::SearchType::Vector,
            },
            SearchResult {
                note_path: "b.md".into(),
                breadcrumb: "b.md".into(),
                content: "BBB".into(),
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
