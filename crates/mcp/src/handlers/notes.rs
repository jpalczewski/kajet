use crate::domain::notes_input::{
    NotesInputError, prepare_create_note_input, prepare_edit_note_input,
};
use crate::errors::internal_error;
use crate::schema::{CreateNoteRequest, EditNoteRequest};
use crate::use_cases::notes::{execute_create_note, execute_edit_note};
use rmcp::{handler::server::wrapper::Parameters, model::*, tool, tool_router};
use tracing::instrument;

fn map_notes_input_error(err: NotesInputError) -> ErrorData {
    match err {
        NotesInputError::InvalidEditMode { mode } => {
            internal_error(t!("edit_note_invalid_mode", mode = mode).to_string())
        }
    }
}

#[tool_router(router = tool_router_notes, vis = "pub(crate)")]
impl crate::KajetMcp {
    #[tool(
        description = "Create a new note in the Obsidian vault. Generates frontmatter with title, tags, and timestamps automatically. Fails if file already exists."
    )]
    #[instrument(level = "debug", skip(self, params), fields(target))]
    async fn create_note(
        &self,
        params: Parameters<CreateNoteRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let input = prepare_create_note_input(params.0);
        let span = tracing::Span::current();
        span.record("target", input.target.as_str());

        let out = execute_create_note(self, input).await.map_err(|e| {
            internal_error(t!("create_note_failed", error = e.to_string()).to_string())
        })?;

        if let Some(error) = out.reindex_error {
            tracing::warn!(path = %out.path, error = %error, "Failed to reindex after create");
        }

        Ok(CallToolResult::success(vec![Content::text(out.summary)]))
    }

    #[tool(
        description = "Edit an existing note in the Obsidian vault. Modes: 'append' (add to end), 'prepend' (add after frontmatter), 'overwrite' (replace body), 'replace_section' (replace heading section), 'replace_text' (exact string replacement), 'insert_after' (insert content after exact text anchor, uses old_text as anchor). Backups are created for destructive operations."
    )]
    #[instrument(level = "debug", skip(self, params), fields(path, mode))]
    async fn edit_note(
        &self,
        params: Parameters<EditNoteRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let input = prepare_edit_note_input(params.0).map_err(map_notes_input_error)?;
        let span = tracing::Span::current();
        span.record("path", input.path.as_str());
        span.record("mode", input.mode.as_str());

        let out = execute_edit_note(self, input).await.map_err(|e| {
            internal_error(t!("edit_note_failed", error = e.to_string()).to_string())
        })?;

        if let Some(error) = out.reindex_error {
            tracing::warn!(path = %out.path, error = %error, "Failed to reindex after edit");
        }

        Ok(CallToolResult::success(vec![Content::text(out.summary)]))
    }
}
