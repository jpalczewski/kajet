use kajet_writer::{CreateNoteResult, EditNoteResult};

pub fn format_create_result(result: &CreateNoteResult) -> String {
    t!(
        "create_note_success",
        path = &result.path,
        bytes = result.bytes_written
    )
    .to_string()
}

pub fn format_edit_result(result: &EditNoteResult) -> String {
    let mut text = t!(
        "edit_note_success",
        path = &result.path,
        mode = &result.mode,
        bytes = result.bytes_written
    )
    .to_string();

    if result.backup_path.is_some() {
        text.push_str(&format!("\n{}", t!("edit_note_backup_created")));
    }

    text
}

pub fn format_edit_tags_result(
    path: &str,
    added: &Option<Vec<String>>,
    removed: &Option<Vec<String>>,
) -> String {
    let mut text = t!("edit_tags_success", path = path).to_string();

    if let Some(tags) = added
        && !tags.is_empty()
    {
        text.push_str(&format!(
            "\n  {}",
            t!("edit_tags_added", tags = tags.join(", "))
        ));
    }

    if let Some(tags) = removed
        && !tags.is_empty()
    {
        text.push_str(&format!(
            "\n  {}",
            t!("edit_tags_removed", tags = tags.join(", "))
        ));
    }

    text
}
