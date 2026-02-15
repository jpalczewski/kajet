use crate::domain::documents_input::prepare_examine_input;
use crate::domain::explore_input::{ExploreInputError, prepare_explore_connections_input};
use crate::domain::find_similar_input::{FindSimilarInputError, prepare_find_similar_input};
use crate::errors::internal_error;
use crate::schema::{ExamineRequest, ExploreConnectionsRequest, FindSimilarRequest};
use crate::use_cases::documents::execute_examine;
use crate::use_cases::explore_connections::execute_explore_connections;
use crate::use_cases::find_similar::execute_find_similar;
use rmcp::{handler::server::wrapper::Parameters, model::*, tool, tool_router};
use tracing::instrument;

fn map_explore_input_error(err: ExploreInputError) -> ErrorData {
    match err {
        ExploreInputError::EmptyPath => internal_error(t!("explore_path_required").to_string()),
        ExploreInputError::DepthZero => internal_error(t!("explore_depth_zero").to_string()),
        ExploreInputError::LimitZero => internal_error(t!("explore_limit_zero").to_string()),
        ExploreInputError::DateParse { input, error } => {
            internal_error(t!("explore_date_parse_error", input = input, error = error).to_string())
        }
        ExploreInputError::InvalidDateRange { from, to } => internal_error(
            t!(
                "explore_date_range_error",
                from = from.to_string(),
                to = to.to_string()
            )
            .to_string(),
        ),
    }
}

fn map_find_similar_input_error(err: FindSimilarInputError) -> ErrorData {
    match err {
        FindSimilarInputError::EmptyPath => {
            internal_error(t!("find_similar_path_required").to_string())
        }
        FindSimilarInputError::LimitZero => {
            internal_error(t!("find_similar_limit_zero").to_string())
        }
        FindSimilarInputError::InvalidThreshold(value) => {
            internal_error(t!("find_similar_invalid_threshold", value = value).to_string())
        }
        FindSimilarInputError::InvalidAggregation(value) => {
            internal_error(t!("find_similar_invalid_aggregation", value = value).to_string())
        }
        FindSimilarInputError::InvalidSort(value) => {
            internal_error(t!("find_similar_invalid_sort", value = value).to_string())
        }
        FindSimilarInputError::InvalidExcludeLinked(value) => {
            internal_error(t!("find_similar_invalid_exclude_linked", value = value).to_string())
        }
        FindSimilarInputError::DateParse { input, error } => internal_error(
            t!(
                "find_similar_date_parse_error",
                input = input,
                error = error
            )
            .to_string(),
        ),
        FindSimilarInputError::InvalidDateRange { from, to } => internal_error(
            t!(
                "find_similar_date_range_error",
                from = from.to_string(),
                to = to.to_string()
            )
            .to_string(),
        ),
    }
}

#[tool_router(router = tool_router_documents, vis = "pub(crate)")]
impl crate::KajetMcp {
    #[tool(
        description = "Examine indexed documents: view metadata (title, tags, outgoing links, backlinks) and content. Accepts one or more full/partial file paths with fuzzy matching."
    )]
    #[instrument(level = "debug", skip(self, params), fields(path_count))]
    async fn examine(
        &self,
        params: Parameters<ExamineRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let input = prepare_examine_input(params.0).map_err(|e| internal_error(e.to_string()))?;
        let span = tracing::Span::current();
        span.record("path_count", input.paths.len());

        let out = execute_examine(
            self,
            &input.paths,
            input.content_mode,
            input.offset,
            input.length,
        )
        .await
        .map_err(|e| internal_error(e.to_string()))?;

        Ok(CallToolResult::success(vec![Content::text(out.summary)]))
    }

    #[tool(
        description = "Explore note connections as a traversal tree. Supports depth/limit, deduplication, optional section context, and filter modes ('display' or 'traverse')."
    )]
    #[instrument(
        level = "debug",
        skip(self, params),
        fields(path, depth, limit, dedup, include_context, filter_mode)
    )]
    async fn explore_connections(
        &self,
        params: Parameters<ExploreConnectionsRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let req = params.0;
        let defaults = {
            let config = self.state.config.read().unwrap();
            config.mcp.explore_connections.clone()
        };

        let input =
            prepare_explore_connections_input(req, &defaults, chrono::Local::now().date_naive())
                .map_err(map_explore_input_error)?;

        let span = tracing::Span::current();
        span.record("path", input.path.as_str());
        span.record("depth", input.depth);
        span.record("limit", input.limit);
        span.record("dedup", input.dedup);
        span.record("include_context", input.include_context);
        span.record("filter_mode", input.filter_mode.as_str());

        let start = std::time::Instant::now();
        let out = execute_explore_connections(self, input)
            .await
            .map_err(|e| internal_error(t!("explore_failed", error = e.to_string()).to_string()))?;

        tracing::info!(
            elapsed_ms = start.elapsed().as_millis() as u64,
            "MCP explore_connections"
        );

        Ok(CallToolResult::success(vec![Content::text(out.summary)]))
    }

    #[tool(
        description = "Find semantically similar notes to a source note using multi-vector chunk matching. Supports threshold, aggregation mode (max/avg), sort mode, optional exclude_linked mode (none/outgoing/both), and filters (from/to/tags/folder)."
    )]
    #[instrument(
        level = "debug",
        skip(self, params),
        fields(path, limit, threshold, aggregation, sort, exclude_linked)
    )]
    async fn find_similar(
        &self,
        params: Parameters<FindSimilarRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let defaults = {
            let config = self.state.config.read().unwrap();
            config.mcp.find_similar.clone()
        };
        let input =
            prepare_find_similar_input(params.0, &defaults, chrono::Local::now().date_naive())
                .map_err(map_find_similar_input_error)?;

        let span = tracing::Span::current();
        span.record("path", input.path.as_str());
        span.record("limit", input.limit);
        span.record("threshold", input.threshold);
        span.record("aggregation", input.aggregation.as_str());
        span.record("sort", input.sort.as_str());
        span.record("exclude_linked", input.exclude_linked.as_str());

        let start = std::time::Instant::now();
        let out = execute_find_similar(self, input).await.map_err(|e| {
            internal_error(t!("find_similar_failed", error = e.to_string()).to_string())
        })?;

        tracing::info!(
            elapsed_ms = start.elapsed().as_millis() as u64,
            "MCP find_similar"
        );

        Ok(CallToolResult::success(vec![Content::text(out.summary)]))
    }
}
