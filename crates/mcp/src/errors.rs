use rmcp::model::{ErrorCode, ErrorData};

pub(crate) fn internal_error(msg: impl Into<String>) -> ErrorData {
    ErrorData {
        code: ErrorCode::INTERNAL_ERROR,
        message: msg.into().into(),
        data: None,
    }
}

pub(crate) fn translate_path_error(e: anyhow::Error) -> ErrorData {
    use kajet_parser::path_validation::PathValidationError;

    // Try to downcast to PathValidationError for i18n
    if let Some(path_err) = e.downcast_ref::<PathValidationError>() {
        let msg = match path_err {
            PathValidationError::Traversal { path } => t!("path_traversal", path = path),
            PathValidationError::Absolute { path } => t!("path_absolute", path = path),
            PathValidationError::SymlinkEscape { path, vault } => {
                t!("path_symlink_escape", path = path, vault = vault)
            }
            PathValidationError::CannotResolve { path } => t!("path_cannot_resolve", path = path),
            PathValidationError::Io(_) => return internal_error(e.to_string()),
        };
        return internal_error(msg.to_string());
    }

    internal_error(e.to_string())
}
