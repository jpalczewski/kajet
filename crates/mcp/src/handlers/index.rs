use crate::errors::internal_error;
use crate::schema::ReindexRequest;
use crate::use_cases::index::{ReindexOutput, execute_index_status, execute_reindex};
use rmcp::{handler::server::wrapper::Parameters, model::*, tool, tool_router};

#[tool_router(router = tool_router_index, vis = "pub(crate)")]
impl crate::KajetMcp {
    #[tool(
        description = "Reindex the Obsidian vault. Without arguments, performs a full vault reindex. With a 'path' argument, reindexes that specific file or all files under that folder."
    )]
    async fn reindex(
        &self,
        params: Parameters<ReindexRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let req = params.0;
        let output = execute_reindex(self, req.path)
            .await
            .map_err(|e| internal_error(t!("reindex_failed", error = e.to_string()).to_string()))?;

        match output {
            ReindexOutput::SingleFile { summary, path } => {
                tracing::info!(path = %path, "MCP reindex path");
                Ok(CallToolResult::success(vec![Content::text(summary)]))
            }
            ReindexOutput::Full { summary } => {
                Ok(CallToolResult::success(vec![Content::text(summary)]))
            }
        }
    }

    #[tool(
        description = "Get the current index status: number of indexed documents, chunks, and last indexing time."
    )]
    async fn index_status(&self) -> Result<CallToolResult, ErrorData> {
        let text = execute_index_status(self)
            .await
            .map_err(|e| internal_error(e.to_string()))?;
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }
}
