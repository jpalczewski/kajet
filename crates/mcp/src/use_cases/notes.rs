use crate::domain::notes_input::{CreateNoteInput, EditNoteInput};
use crate::format::{format_create_result, format_edit_result};
use crate::ports::NotesPorts;

pub(crate) struct CreateNoteOutput {
    pub path: String,
    pub reindex_error: Option<String>,
    pub summary: String,
}

pub(crate) struct EditNoteOutput {
    pub path: String,
    pub reindex_error: Option<String>,
    pub summary: String,
}

pub(crate) async fn execute_create_note(
    ports: &impl NotesPorts,
    input: CreateNoteInput,
) -> anyhow::Result<CreateNoteOutput> {
    let result = ports
        .create_note(kajet_writer::CreateNoteParams {
            target: input.target,
            content: input.content,
            tags: input.tags,
            aliases: input.aliases,
        })
        .await?;

    let path = result.path.clone();
    let summary = format_create_result(&result);
    let rel_paths = vec![path.clone()];
    let reindex_error = ports
        .reindex_files(&rel_paths)
        .await
        .err()
        .map(|e| e.to_string());

    Ok(CreateNoteOutput {
        path,
        reindex_error,
        summary,
    })
}

pub(crate) async fn execute_edit_note(
    ports: &impl NotesPorts,
    input: EditNoteInput,
) -> anyhow::Result<EditNoteOutput> {
    let result = ports
        .edit_note(kajet_writer::EditNoteParams {
            path: input.path,
            content: input.content,
            mode: input.mode,
            target_heading: input.target_heading,
            old_text: input.old_text,
        })
        .await?;

    let path = result.path.clone();
    let summary = format_edit_result(&result);
    let rel_paths = vec![path.clone()];
    let reindex_error = ports
        .reindex_files(&rel_paths)
        .await
        .err()
        .map(|e| e.to_string());

    Ok(EditNoteOutput {
        path,
        reindex_error,
        summary,
    })
}
