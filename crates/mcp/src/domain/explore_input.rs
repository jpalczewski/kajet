use crate::date_parser::{DateBound, parse_date};
use crate::schema::ExploreConnectionsRequest;
use chrono::NaiveDate;
use kajet_core::config::ExploreConnectionsConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExploreFilterMode {
    Display,
    Traverse,
}

impl ExploreFilterMode {
    pub(crate) fn from_request(mode: Option<&str>, default_mode: &str) -> Self {
        match mode.unwrap_or(default_mode) {
            "traverse" => Self::Traverse,
            _ => Self::Display,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Display => "display",
            Self::Traverse => "traverse",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ExploreConnectionsInput {
    pub path: String,
    pub depth: usize,
    pub limit: usize,
    pub dedup: bool,
    pub include_context: bool,
    pub filter_mode: ExploreFilterMode,
    pub from_ts: Option<f64>,
    pub to_ts: Option<f64>,
    pub folder: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub(crate) enum ExploreInputError {
    EmptyPath,
    DepthZero,
    LimitZero,
    DateParse { input: String, error: String },
    InvalidDateRange { from: NaiveDate, to: NaiveDate },
}

pub(crate) fn prepare_explore_connections_input(
    req: ExploreConnectionsRequest,
    defaults: &ExploreConnectionsConfig,
    today: NaiveDate,
) -> Result<ExploreConnectionsInput, ExploreInputError> {
    let path = req.path.trim().to_string();
    if path.is_empty() {
        return Err(ExploreInputError::EmptyPath);
    }

    let depth = req.depth.unwrap_or(defaults.default_depth);
    if depth == 0 {
        return Err(ExploreInputError::DepthZero);
    }

    let limit = req.limit.unwrap_or(defaults.default_limit);
    if limit == 0 {
        return Err(ExploreInputError::LimitZero);
    }

    let from_date = if let Some(ref from_str) = req.from {
        Some(parse_date(from_str, DateBound::From).map_err(|e| {
            let input = e.input.clone();
            ExploreInputError::DateParse {
                input,
                error: e.to_string(),
            }
        })?)
    } else {
        None
    };

    let to_date = if let Some(ref to_str) = req.to {
        Some(parse_date(to_str, DateBound::To).map_err(|e| {
            let input = e.input.clone();
            ExploreInputError::DateParse {
                input,
                error: e.to_string(),
            }
        })?)
    } else if from_date.is_some() {
        Some(today)
    } else {
        None
    };

    if let (Some(from), Some(to)) = (from_date, to_date)
        && from > to
    {
        return Err(ExploreInputError::InvalidDateRange { from, to });
    }

    let from_ts = from_date.map(|d| {
        d.and_hms_opt(0, 0, 0)
            .expect("00:00:00 is always valid")
            .and_utc()
            .timestamp() as f64
    });
    let to_ts = to_date.map(|d| {
        d.and_hms_opt(23, 59, 59)
            .expect("23:59:59 is always valid")
            .and_utc()
            .timestamp() as f64
    });

    Ok(ExploreConnectionsInput {
        path,
        depth,
        limit,
        dedup: req.dedup.unwrap_or(defaults.default_dedup),
        include_context: req
            .include_context
            .unwrap_or(defaults.default_include_context),
        filter_mode: ExploreFilterMode::from_request(
            req.filter_mode.as_deref(),
            &defaults.default_filter_mode,
        ),
        from_ts,
        to_ts,
        folder: req.folder,
        tags: req.tags,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> ExploreConnectionsConfig {
        ExploreConnectionsConfig::default()
    }

    #[test]
    fn applies_defaults_when_optional_fields_missing() {
        let input = prepare_explore_connections_input(
            ExploreConnectionsRequest {
                path: "note.md".to_string(),
                depth: None,
                limit: None,
                dedup: None,
                include_context: None,
                filter_mode: None,
                tags: None,
                from: None,
                to: None,
                folder: None,
            },
            &defaults(),
            NaiveDate::from_ymd_opt(2026, 2, 15).expect("valid date"),
        )
        .expect("valid request");

        assert_eq!(input.depth, 3);
        assert_eq!(input.limit, 100);
        assert!(input.dedup);
        assert!(!input.include_context);
        assert_eq!(input.filter_mode, ExploreFilterMode::Display);
    }

    #[test]
    fn parses_traverse_mode_and_date_defaults() {
        let input = prepare_explore_connections_input(
            ExploreConnectionsRequest {
                path: "note.md".to_string(),
                depth: Some(2),
                limit: Some(50),
                dedup: Some(false),
                include_context: Some(true),
                filter_mode: Some("traverse".to_string()),
                tags: Some(vec!["research".to_string()]),
                from: Some("2026-02-10".to_string()),
                to: None,
                folder: Some("journal".to_string()),
            },
            &defaults(),
            NaiveDate::from_ymd_opt(2026, 2, 15).expect("valid date"),
        )
        .expect("valid request");

        assert_eq!(input.filter_mode, ExploreFilterMode::Traverse);
        assert_eq!(input.depth, 2);
        assert_eq!(input.limit, 50);
        assert!(!input.dedup);
        assert!(input.include_context);
        assert!(input.from_ts.is_some());
        assert!(input.to_ts.is_some());
    }

    #[test]
    fn rejects_empty_path() {
        let err = prepare_explore_connections_input(
            ExploreConnectionsRequest {
                path: "   ".to_string(),
                depth: None,
                limit: None,
                dedup: None,
                include_context: None,
                filter_mode: None,
                tags: None,
                from: None,
                to: None,
                folder: None,
            },
            &defaults(),
            NaiveDate::from_ymd_opt(2026, 2, 15).expect("valid date"),
        )
        .expect_err("must fail");

        assert!(matches!(err, ExploreInputError::EmptyPath));
    }
}
