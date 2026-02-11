use crate::domain::documents_input::prepare_examine_input;
use crate::errors::internal_error;
use crate::schema::ExamineRequest;
use crate::use_cases::documents::execute_examine;
use rmcp::{handler::server::wrapper::Parameters, model::*, tool, tool_router};
use tracing::instrument;

#[tool_router(router = tool_router_documents, vis = "pub(crate)")]
impl crate::KajetMcp {
    #[tool(
        description = "Examine an indexed document: view its metadata (title, tags, outgoing links, backlinks) and content. Accepts full or partial file paths with fuzzy matching."
    )]
    #[instrument(level = "debug", skip(self, params), fields(path))]
    async fn examine(
        &self,
        params: Parameters<ExamineRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let input = prepare_examine_input(params.0);
        let span = tracing::Span::current();
        span.record("path", input.path.as_str());

        let out = execute_examine(
            self,
            &input.path,
            input.content_mode,
            input.offset,
            input.length,
        )
        .await
        .map_err(|e| internal_error(e.to_string()))?;

        Ok(CallToolResult::success(vec![Content::text(out.summary)]))
    }
}
