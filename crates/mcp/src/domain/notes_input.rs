use crate::schema::{CreateNoteRequest, EditNoteRequest};

#[derive(Debug, Clone)]
pub(crate) struct CreateNoteInput {
    pub target: String,
    pub content: String,
    pub tags: Vec<String>,
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct EditNoteInput {
    pub path: String,
    pub content: String,
    pub mode: kajet_writer::EditMode,
    pub target_heading: Option<String>,
    pub old_text: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) enum NotesInputError {
    InvalidEditMode { mode: String },
}

pub(crate) fn prepare_create_note_input(req: CreateNoteRequest) -> CreateNoteInput {
    CreateNoteInput {
        target: req.target,
        content: req.content,
        tags: req.tags.unwrap_or_default(),
        aliases: req.aliases.unwrap_or_default(),
    }
}

pub(crate) fn prepare_edit_note_input(
    req: EditNoteRequest,
) -> Result<EditNoteInput, NotesInputError> {
    let mode: kajet_writer::EditMode =
        req.mode
            .parse()
            .map_err(|_| NotesInputError::InvalidEditMode {
                mode: req.mode.clone(),
            })?;

    Ok(EditNoteInput {
        path: req.path,
        content: req.content,
        mode,
        target_heading: req.target_heading,
        old_text: req.old_text,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_input_defaults_tags_and_aliases() {
        let input = prepare_create_note_input(CreateNoteRequest {
            target: "a.md".to_string(),
            content: "x".to_string(),
            tags: None,
            aliases: None,
        });
        assert!(input.tags.is_empty());
        assert!(input.aliases.is_empty());
    }

    #[test]
    fn edit_input_rejects_invalid_mode() {
        let err = prepare_edit_note_input(EditNoteRequest {
            path: "a.md".to_string(),
            content: "x".to_string(),
            mode: "unknown".to_string(),
            target_heading: None,
            old_text: None,
        })
        .expect_err("invalid mode should fail");
        assert!(matches!(err, NotesInputError::InvalidEditMode { .. }));
    }
}
