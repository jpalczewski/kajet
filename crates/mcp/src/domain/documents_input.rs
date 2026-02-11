use crate::schema::ExamineRequest;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExamineContentMode {
    Summary,
    Full,
    Slice,
}

impl ExamineContentMode {
    pub(crate) fn from_request(mode: Option<&str>) -> Self {
        match mode {
            Some("full") => Self::Full,
            Some("slice") => Self::Slice,
            _ => Self::Summary,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ExamineInput {
    pub path: String,
    pub content_mode: ExamineContentMode,
    pub offset: usize,
    pub length: usize,
}

pub(crate) fn prepare_examine_input(req: ExamineRequest) -> ExamineInput {
    ExamineInput {
        path: req.path,
        content_mode: ExamineContentMode::from_request(req.content.as_deref()),
        offset: req.offset.unwrap_or(0),
        length: req.length.unwrap_or(500),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_summary_mode_and_default_slice_params() {
        let input = prepare_examine_input(ExamineRequest {
            path: "a.md".to_string(),
            content: None,
            offset: None,
            length: None,
        });

        assert_eq!(input.content_mode, ExamineContentMode::Summary);
        assert_eq!(input.offset, 0);
        assert_eq!(input.length, 500);
    }

    #[test]
    fn parses_full_and_slice_modes() {
        let full = prepare_examine_input(ExamineRequest {
            path: "a.md".to_string(),
            content: Some("full".to_string()),
            offset: None,
            length: None,
        });
        let slice = prepare_examine_input(ExamineRequest {
            path: "a.md".to_string(),
            content: Some("slice".to_string()),
            offset: Some(7),
            length: Some(22),
        });

        assert_eq!(full.content_mode, ExamineContentMode::Full);
        assert_eq!(slice.content_mode, ExamineContentMode::Slice);
        assert_eq!(slice.offset, 7);
        assert_eq!(slice.length, 22);
    }

    #[test]
    fn unknown_mode_falls_back_to_summary_for_forward_compatibility() {
        let input = prepare_examine_input(ExamineRequest {
            path: "a.md".to_string(),
            content: Some("custom".to_string()),
            offset: None,
            length: None,
        });

        assert_eq!(input.content_mode, ExamineContentMode::Summary);
    }
}
