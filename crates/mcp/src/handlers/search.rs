use crate::domain::search_input::{SearchInputError, prepare_search_input};
use crate::errors::internal_error;
use crate::ports::SearchPorts;
use crate::schema::SearchRequest;
use crate::use_cases::search::{SearchUseCaseOutput, execute_search};
use rmcp::{handler::server::wrapper::Parameters, model::*, tool, tool_router};
use tracing::instrument;

fn map_input_error(err: SearchInputError) -> ErrorData {
    match err {
        SearchInputError::NoQueryOrFilters => internal_error(t!("search_no_params").to_string()),
        SearchInputError::DateParse { input, error } => {
            internal_error(t!("search_date_parse_error", input = input, error = error).to_string())
        }
        SearchInputError::InvalidDateRange { from, to } => internal_error(
            t!(
                "search_date_range_error",
                from = from.to_string(),
                to = to.to_string()
            )
            .to_string(),
        ),
    }
}

#[tool_router(router = tool_router_search, vis = "pub(crate)")]
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
        let input =
            prepare_search_input(req, self.default_limit(), chrono::Local::now().date_naive())
                .map_err(map_input_error)?;
        let start = std::time::Instant::now();

        match execute_search(self, input)
            .await
            .map_err(|e| internal_error(t!("search_failed", error = e.to_string()).to_string()))?
        {
            SearchUseCaseOutput::Search {
                query,
                mode,
                limit,
                results,
                summary,
            } => {
                let span = tracing::Span::current();
                span.record("query", query.as_str());
                span.record("mode", mode.as_str());
                span.record("limit", limit);
                span.record("results", results);

                tracing::info!(
                    query = query.as_str(),
                    mode = mode.as_str(),
                    limit,
                    results,
                    response_bytes = summary.len(),
                    elapsed_ms = start.elapsed().as_millis() as u64,
                    "MCP search"
                );
                Ok(CallToolResult::success(vec![Content::text(summary)]))
            }
            SearchUseCaseOutput::Browse {
                from,
                to,
                folder,
                tags,
                limit,
                results,
                summary,
            } => {
                tracing::info!(
                    from = ?from,
                    to = ?to,
                    folder = folder.as_deref().unwrap_or("*"),
                    tags = ?tags,
                    limit,
                    results,
                    elapsed_ms = start.elapsed().as_millis() as u64,
                    "MCP browse"
                );
                Ok(CallToolResult::success(vec![Content::text(summary)]))
            }
        }
    }
}
