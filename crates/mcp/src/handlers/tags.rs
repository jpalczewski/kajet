use crate::domain::tags_input::{TagsInputError, prepare_edit_tags_input, prepare_list_tags_input};
use crate::errors::internal_error;
use crate::schema::{EditTagsRequest, ListTagsRequest};
use crate::use_cases::tags::{EditTagsError, execute_edit_tags, execute_list_tags};
use rmcp::{handler::server::wrapper::Parameters, model::*, tool, tool_router};
use tracing::instrument;

fn map_tags_input_error(err: TagsInputError) -> ErrorData {
    match err {
        TagsInputError::EmptyEditOperations => {
            internal_error(t!("edit_tags_empty_params").to_string())
        }
    }
}

#[tool_router(router = tool_router_tags, vis = "pub(crate)")]
impl crate::KajetMcp {
    #[tool(
        description = "List all unique tags from the vault, optionally filtered by folder. Detail levels: 'names' (tag list), 'counts' (tag + document count, default), 'full' (tag + count + file paths)."
    )]
    #[instrument(level = "debug", skip(self, params), fields(folder, detail, tag_count))]
    async fn list_tags(
        &self,
        params: Parameters<ListTagsRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let input = prepare_list_tags_input(params.0);

        let span = tracing::Span::current();
        span.record("folder", input.folder.as_deref().unwrap_or("*"));
        span.record("detail", input.detail.as_str());

        let out = execute_list_tags(self, input).await.map_err(|e| {
            internal_error(t!("list_tags_failed", error = e.to_string()).to_string())
        })?;

        span.record("tag_count", out.tag_count);
        tracing::info!(
            folder = out.folder.as_deref().unwrap_or("*"),
            tag_count = out.tag_count,
            "MCP list_tags"
        );

        Ok(CallToolResult::success(vec![Content::text(out.summary)]))
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
        let input = prepare_edit_tags_input(params.0).map_err(map_tags_input_error)?;
        let span = tracing::Span::current();
        span.record("path", input.path.as_str());
        span.record(
            "add_count",
            input.add.as_ref().map(|v| v.len()).unwrap_or(0),
        );
        span.record(
            "remove_count",
            input.remove.as_ref().map(|v| v.len()).unwrap_or(0),
        );

        let out = execute_edit_tags(self, input.clone())
            .await
            .map_err(|e| match e {
                EditTagsError::ResolvePath(err) => internal_error(
                    t!(
                        "edit_tags_path_not_found",
                        path = &input.path,
                        error = err.to_string()
                    )
                    .to_string(),
                ),
                EditTagsError::Other(err) => internal_error(err.to_string()),
            })?;

        span.record("timestamp_updated", out.timestamp_updated);
        if let Some(error) = &out.reindex_error {
            tracing::warn!(
                path = %out.relative_path,
                error = %error,
                "Failed to reindex after edit_tags"
            );
        }

        tracing::info!(
            path = %out.relative_path,
            added = out.added_count,
            removed = out.removed_count,
            "Tags edited successfully"
        );

        Ok(CallToolResult::success(vec![Content::text(out.summary)]))
    }
}
