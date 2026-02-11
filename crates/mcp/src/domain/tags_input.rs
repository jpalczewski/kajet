use crate::schema::{EditTagsRequest, ListTagsRequest};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TagListDetail {
    Names,
    Counts,
    Full,
}

impl TagListDetail {
    pub(crate) fn from_request(detail: Option<&str>) -> Self {
        match detail {
            Some("names") => Self::Names,
            Some("full") => Self::Full,
            _ => Self::Counts,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Names => "names",
            Self::Counts => "counts",
            Self::Full => "full",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ListTagsInput {
    pub folder: Option<String>,
    pub detail: TagListDetail,
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
        detail: TagListDetail::from_request(req.detail.as_deref()),
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
        assert_eq!(input.detail, TagListDetail::Counts);
        assert!(input.recursive);
    }

    #[test]
    fn list_tags_parses_all_detail_modes() {
        let names = prepare_list_tags_input(ListTagsRequest {
            folder: None,
            recursive: None,
            detail: Some("names".to_string()),
        });
        let full = prepare_list_tags_input(ListTagsRequest {
            folder: None,
            recursive: None,
            detail: Some("full".to_string()),
        });
        let unknown = prepare_list_tags_input(ListTagsRequest {
            folder: None,
            recursive: None,
            detail: Some("something-new".to_string()),
        });

        assert_eq!(names.detail, TagListDetail::Names);
        assert_eq!(full.detail, TagListDetail::Full);
        assert_eq!(unknown.detail, TagListDetail::Counts);
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
