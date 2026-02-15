use crate::schema::ExamineRequest;
use anyhow::ensure;

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
    pub paths: Vec<String>,
    pub content_mode: ExamineContentMode,
    pub offset: usize,
    pub length: usize,
}

pub(crate) fn prepare_examine_input(req: ExamineRequest) -> anyhow::Result<ExamineInput> {
    ensure!(
        !req.paths.is_empty(),
        "At least one path is required in 'paths'"
    );
    let mut paths = Vec::with_capacity(req.paths.len());
    for path in req.paths {
        let trimmed = path.trim();
        ensure!(
            !trimmed.is_empty(),
            "Paths cannot contain empty values in 'paths'"
        );
        paths.push(trimmed.to_string());
    }

    Ok(ExamineInput {
        paths,
        content_mode: ExamineContentMode::from_request(req.content.as_deref()),
        offset: req.offset.unwrap_or(0),
        length: req.length.unwrap_or(500),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_summary_mode_and_default_slice_params() {
        let input = prepare_examine_input(ExamineRequest {
            paths: vec!["a.md".to_string()],
            content: None,
            offset: None,
            length: None,
        })
        .expect("valid request");

        assert_eq!(input.content_mode, ExamineContentMode::Summary);
        assert_eq!(input.offset, 0);
        assert_eq!(input.length, 500);
    }

    #[test]
    fn parses_full_and_slice_modes() {
        let full = prepare_examine_input(ExamineRequest {
            paths: vec!["a.md".to_string()],
            content: Some("full".to_string()),
            offset: None,
            length: None,
        })
        .expect("valid request");
        let slice = prepare_examine_input(ExamineRequest {
            paths: vec!["a.md".to_string()],
            content: Some("slice".to_string()),
            offset: Some(7),
            length: Some(22),
        })
        .expect("valid request");

        assert_eq!(full.content_mode, ExamineContentMode::Full);
        assert_eq!(slice.content_mode, ExamineContentMode::Slice);
        assert_eq!(slice.offset, 7);
        assert_eq!(slice.length, 22);
    }

    #[test]
    fn unknown_mode_falls_back_to_summary_for_forward_compatibility() {
        let input = prepare_examine_input(ExamineRequest {
            paths: vec!["a.md".to_string()],
            content: Some("custom".to_string()),
            offset: None,
            length: None,
        })
        .expect("valid request");

        assert_eq!(input.content_mode, ExamineContentMode::Summary);
    }

    #[test]
    fn empty_paths_are_rejected() {
        let err = prepare_examine_input(ExamineRequest {
            paths: vec![],
            content: None,
            offset: None,
            length: None,
        })
        .unwrap_err();

        assert!(err.to_string().contains("At least one path"));
    }
}
