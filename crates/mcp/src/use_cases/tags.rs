use crate::domain::tags_input::{EditTagsInput, ListTagsInput};
use crate::filters;
use crate::format::{format_edit_tags_result, format_list_tags};
use crate::ports::TagsPorts;

pub(crate) struct ListTagsOutput {
    pub folder: Option<String>,
    pub tag_count: usize,
    pub summary: String,
}

pub(crate) enum EditTagsError {
    ResolvePath(anyhow::Error),
    Other(anyhow::Error),
}

pub(crate) struct EditTagsOutput {
    pub relative_path: String,
    pub timestamp_updated: bool,
    pub added_count: usize,
    pub removed_count: usize,
    pub reindex_error: Option<String>,
    pub summary: String,
}

pub(crate) async fn execute_list_tags(
    ports: &impl TagsPorts,
    input: ListTagsInput,
) -> anyhow::Result<ListTagsOutput> {
    let all_docs = ports.get_all_documents().await?;
    let tag_stats = filters::aggregate_tags(&all_docs, input.folder.as_deref(), input.recursive);
    let sorted: Vec<(String, Vec<String>)> =
        tag_stats.into_iter().map(|ts| (ts.tag, ts.files)).collect();
    let tag_count = sorted.len();
    let summary = format_list_tags(&sorted, input.detail, input.folder.as_deref());

    Ok(ListTagsOutput {
        folder: input.folder,
        tag_count,
        summary,
    })
}

pub(crate) async fn execute_edit_tags(
    ports: &impl TagsPorts,
    input: EditTagsInput,
) -> Result<EditTagsOutput, EditTagsError> {
    let resolved = ports
        .resolve_note_path(&input.path)
        .await
        .map_err(EditTagsError::ResolvePath)?;

    let content = ports
        .read_file_to_string(&resolved.absolute)
        .await
        .map_err(EditTagsError::Other)?;

    let timestamp_update = ports.timestamp_update().map_err(EditTagsError::Other)?;
    let result = filters::apply_tag_edits(
        &content,
        input.add.as_deref(),
        input.remove.as_deref(),
        timestamp_update
            .as_ref()
            .map(|(field, value)| (field.as_str(), value.as_str())),
    )
    .map_err(EditTagsError::Other)?;

    ports
        .write_string_file(&resolved.absolute, &result.content)
        .await
        .map_err(EditTagsError::Other)?;

    let rel_paths = vec![resolved.relative.clone()];
    let reindex_error = ports
        .reindex_files(&rel_paths)
        .await
        .err()
        .map(|e| e.to_string());

    Ok(EditTagsOutput {
        relative_path: resolved.relative.clone(),
        timestamp_updated: result.timestamp_updated,
        added_count: result.tags_added,
        removed_count: result.tags_removed,
        reindex_error,
        summary: format_edit_tags_result(&resolved.relative, &input.add, &input.remove),
    })
}
