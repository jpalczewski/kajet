use crate::engine::SearchResult;
use crate::{AppState, QueryEvent};
use anyhow::Result;
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars, tool, tool_handler, tool_router, ServerHandler, ServiceExt,
    transport::stdio,
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

    /// Max number of results (default 5)
    #[schemars(description = "Maximum number of results to return (default: 5)")]
    pub limit: Option<usize>,
}

// ---------------------------------------------------------------------------
// Result formatting
// ---------------------------------------------------------------------------

pub fn format_results(query: &str, results: &[SearchResult]) -> String {
    if results.is_empty() {
        return format!("No results found for: {}", query);
    }

    let formatted = results
        .iter()
        .enumerate()
        .map(|(i, r)| {
            format!(
                "--- Result {} (score: {:.4}) ---\nPath: {}\nSection: {}\n\n{}",
                i + 1,
                r.score,
                r.note_path,
                r.breadcrumb,
                r.content,
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    format!(
        "Found {} results for: \"{}\"\n\n{}",
        results.len(),
        query,
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
                title: Some("Kajet - Obsidian Semantic Search".into()),
                icons: None,
                website_url: None,
            },
            instructions: Some(
                "Semantic search tool for Obsidian vaults. Use the 'search' tool to find relevant notes."
                    .into(),
            ),
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

    #[tool(description = "Search the Obsidian vault using semantic search. Returns the most relevant note chunks with their breadcrumb paths and content.")]
    async fn search(
        &self,
        params: Parameters<SearchRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let req = params.0;
        let limit = req.limit.unwrap_or(5);

        let results = self
            .state
            .engine
            .search(&req.query, limit)
            .await
            .map_err(|e| ErrorData {
                code: ErrorCode::INTERNAL_ERROR,
                message: format!("Search failed: {}", e).into(),
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

    #[test]
    fn format_results_empty() {
        let result = format_results("test query", &[]);
        assert_eq!(result, "No results found for: test query");
    }

    #[test]
    fn format_results_single() {
        let results = vec![SearchResult {
            note_path: "note.md".into(),
            breadcrumb: "note.md > Intro".into(),
            content: "Hello world".into(),
            score: 0.1234,
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
        let results = vec![
            SearchResult {
                note_path: "a.md".into(),
                breadcrumb: "a.md".into(),
                content: "AAA".into(),
                score: 0.1,
            },
            SearchResult {
                note_path: "b.md".into(),
                breadcrumb: "b.md".into(),
                content: "BBB".into(),
                score: 0.5,
            },
        ];
        let result = format_results("query", &results);
        assert!(result.contains("Found 2 results"));
        assert!(result.contains("Result 1"));
        assert!(result.contains("Result 2"));
    }
}
