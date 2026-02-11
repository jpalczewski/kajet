use crate::domain::analytics_input::{RecentContextInputError, prepare_recent_context_input};
use crate::errors::internal_error;
use crate::schema::RecentContextRequest;
use crate::use_cases::analytics::execute_recent_context;
use rmcp::{handler::server::wrapper::Parameters, model::*, tool, tool_router};
use tracing::instrument;

fn map_input_error(err: RecentContextInputError) -> ErrorData {
    match err {
        RecentContextInputError::InvalidDays(value) => {
            internal_error(t!("recent_context_invalid_days", value = value).to_string())
        }
        RecentContextInputError::InvalidLimit(value) => {
            internal_error(t!("recent_context_invalid_limit", value = value).to_string())
        }
    }
}

#[tool_router(router = tool_router_analytics, vis = "pub(crate)")]
impl crate::KajetMcp {
    #[tool(
        description = "Get a compact overview of recent journaling activity: recent entries, top tags, writing frequency, gaps, and newly appearing tags."
    )]
    #[instrument(
        level = "debug",
        skip(self, params),
        fields(days, limit, entries, tags)
    )]
    async fn recent_context(
        &self,
        params: Parameters<RecentContextRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let input = prepare_recent_context_input(params.0, chrono::Local::now().date_naive())
            .map_err(map_input_error)?;
        let start = std::time::Instant::now();
        tracing::info!(
            days = input.days,
            limit = input.limit,
            "MCP recent_context requested"
        );
        tracing::debug!(
            from_ts = input.from_ts,
            to_ts = input.to_ts,
            prev_from_ts = input.prev_from_ts,
            prev_to_ts = input.prev_to_ts,
            "MCP recent_context prepared input"
        );

        let span = tracing::Span::current();
        span.record("days", input.days);
        span.record("limit", input.limit);

        let out = execute_recent_context(self, input).await.map_err(|e| {
            internal_error(t!("recent_context_failed", error = e.to_string()).to_string())
        })?;

        span.record("entries", out.entry_count);
        span.record("tags", out.tag_count);

        tracing::info!(
            days = out.days,
            entries = out.entry_count,
            tags = out.tag_count,
            elapsed_ms = start.elapsed().as_millis() as u64,
            "MCP recent_context"
        );

        Ok(CallToolResult::success(vec![Content::text(out.summary)]))
    }
}
