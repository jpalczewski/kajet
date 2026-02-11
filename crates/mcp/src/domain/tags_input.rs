use crate::schema::{EditTagsRequest, ListTagsRequest};

#[derive(Debug, Clone)]
pub(crate) struct ListTagsInput {
    pub folder: Option<String>,
    pub detail: String,
    pub recursive: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct EditTagsInput {
    pub path: String,
    pub add: Option<Vec<String>>,
    pub remove: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub(crate) enum TagsInputError {
    EmptyEditOperations,
}

pub(crate) fn prepare_list_tags_input(req: ListTagsRequest) -> ListTagsInput {
    ListTagsInput {
        folder: req.folder,
        detail: req.detail.unwrap_or_else(|| "counts".to_string()),
        recursive: req.recursive.unwrap_or(true),
    }
}

pub(crate) fn prepare_edit_tags_input(
    req: EditTagsRequest,
) -> Result<EditTagsInput, TagsInputError> {
    let has_add = req.add.as_ref().map(|v| !v.is_empty()).unwrap_or(false);
    let has_remove = req.remove.as_ref().map(|v| !v.is_empty()).unwrap_or(false);
    if !has_add && !has_remove {
        return Err(TagsInputError::EmptyEditOperations);
    }

    Ok(EditTagsInput {
        path: req.path,
        add: req.add,
        remove: req.remove,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_tags_defaults() {
        let input = prepare_list_tags_input(ListTagsRequest {
            folder: None,
            recursive: None,
            detail: None,
        });
        assert_eq!(input.detail, "counts");
        assert!(input.recursive);
    }

    #[test]
    fn edit_tags_requires_add_or_remove() {
        let result = prepare_edit_tags_input(EditTagsRequest {
            path: "a.md".to_string(),
            add: None,
            remove: None,
        });
        assert!(matches!(result, Err(TagsInputError::EmptyEditOperations)));
    }
}
