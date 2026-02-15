use crate::date_parser::{DateBound, parse_date};
use crate::schema::FindSimilarRequest;
use chrono::NaiveDate;
use kajet_core::config::FindSimilarConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SimilarAggregation {
    Max,
    Avg,
}

impl SimilarAggregation {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Max => "max",
            Self::Avg => "avg",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SimilarSort {
    Similarity,
    Recent,
    Path,
}

impl SimilarSort {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Similarity => "similarity",
            Self::Recent => "recent",
            Self::Path => "path",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SimilarLinkExclusion {
    None,
    Outgoing,
    Both,
}

impl SimilarLinkExclusion {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Outgoing => "outgoing",
            Self::Both => "both",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct FindSimilarInput {
    pub path: String,
    pub limit: usize,
    pub threshold: f32,
    pub aggregation: SimilarAggregation,
    pub sort: SimilarSort,
    pub exclude_linked: SimilarLinkExclusion,
    pub from_ts: Option<f64>,
    pub to_ts: Option<f64>,
    pub folder: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub(crate) enum FindSimilarInputError {
    EmptyPath,
    LimitZero,
    InvalidThreshold(f32),
    InvalidAggregation(String),
    InvalidSort(String),
    InvalidExcludeLinked(String),
    DateParse { input: String, error: String },
    InvalidDateRange { from: NaiveDate, to: NaiveDate },
}

pub(crate) fn prepare_find_similar_input(
    req: FindSimilarRequest,
    defaults: &FindSimilarConfig,
    today: NaiveDate,
) -> Result<FindSimilarInput, FindSimilarInputError> {
    let path = normalize_find_similar_path(&req.path);
    if path.is_empty() {
        return Err(FindSimilarInputError::EmptyPath);
    }

    let limit = req.limit.unwrap_or(defaults.default_limit);
    if limit == 0 {
        return Err(FindSimilarInputError::LimitZero);
    }

    let threshold = req.threshold.unwrap_or(defaults.default_threshold);
    if !(0.0..=1.0).contains(&threshold) {
        return Err(FindSimilarInputError::InvalidThreshold(threshold));
    }

    let aggregation = match req
        .aggregation
        .as_deref()
        .unwrap_or(defaults.default_aggregation.as_str())
    {
        "max" => SimilarAggregation::Max,
        "avg" => SimilarAggregation::Avg,
        invalid => {
            return Err(FindSimilarInputError::InvalidAggregation(
                invalid.to_string(),
            ));
        }
    };

    let sort = match req.sort.as_deref().unwrap_or("similarity") {
        "similarity" => SimilarSort::Similarity,
        "recent" => SimilarSort::Recent,
        "path" => SimilarSort::Path,
        invalid => return Err(FindSimilarInputError::InvalidSort(invalid.to_string())),
    };

    let exclude_linked = match req
        .exclude_linked
        .as_deref()
        .unwrap_or(defaults.default_exclude_linked.as_str())
    {
        "none" => SimilarLinkExclusion::None,
        "outgoing" => SimilarLinkExclusion::Outgoing,
        "both" => SimilarLinkExclusion::Both,
        invalid => {
            return Err(FindSimilarInputError::InvalidExcludeLinked(
                invalid.to_string(),
            ));
        }
    };

    let from_date = if let Some(ref from_str) = req.from {
        Some(parse_date(from_str, DateBound::From).map_err(|e| {
            let input = e.input.clone();
            FindSimilarInputError::DateParse {
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
            FindSimilarInputError::DateParse {
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
        return Err(FindSimilarInputError::InvalidDateRange { from, to });
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

    Ok(FindSimilarInput {
        path,
        limit,
        threshold,
        aggregation,
        sort,
        exclude_linked,
        from_ts,
        to_ts,
        folder: req.folder,
        tags: req.tags,
    })
}

fn normalize_find_similar_path(path: &str) -> String {
    let trimmed = path.trim();
    if let Some(inner) = trimmed
        .strip_prefix("[[")
        .and_then(|v| v.strip_suffix("]]"))
    {
        return inner.split('|').next().unwrap_or(inner).trim().to_string();
    }
    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_defaults_when_optional_fields_missing() {
        let input = prepare_find_similar_input(
            FindSimilarRequest {
                path: "note.md".to_string(),
                limit: None,
                threshold: None,
                aggregation: None,
                sort: None,
                exclude_linked: None,
                tags: None,
                from: None,
                to: None,
                folder: None,
            },
            &FindSimilarConfig::default(),
            NaiveDate::from_ymd_opt(2026, 2, 15).expect("valid date"),
        )
        .expect("valid input");

        assert_eq!(input.limit, 10);
        assert!((input.threshold - 0.6).abs() < f32::EPSILON);
        assert_eq!(input.aggregation, SimilarAggregation::Max);
        assert_eq!(input.sort, SimilarSort::Similarity);
        assert_eq!(input.exclude_linked, SimilarLinkExclusion::Outgoing);
    }

    #[test]
    fn rejects_invalid_threshold() {
        let err = prepare_find_similar_input(
            FindSimilarRequest {
                path: "note.md".to_string(),
                limit: None,
                threshold: Some(1.5),
                aggregation: None,
                sort: None,
                exclude_linked: None,
                tags: None,
                from: None,
                to: None,
                folder: None,
            },
            &FindSimilarConfig::default(),
            NaiveDate::from_ymd_opt(2026, 2, 15).expect("valid date"),
        )
        .expect_err("should fail");

        assert!(matches!(err, FindSimilarInputError::InvalidThreshold(_)));
    }

    #[test]
    fn normalizes_wikilink_path() {
        let input = prepare_find_similar_input(
            FindSimilarRequest {
                path: "[[folder/note|Alias]]".to_string(),
                limit: None,
                threshold: None,
                aggregation: None,
                sort: None,
                exclude_linked: None,
                tags: None,
                from: None,
                to: None,
                folder: None,
            },
            &FindSimilarConfig::default(),
            NaiveDate::from_ymd_opt(2026, 2, 15).expect("valid date"),
        )
        .expect("valid input");

        assert_eq!(input.path, "folder/note");
    }

    #[test]
    fn parses_exclude_linked_mode() {
        let input = prepare_find_similar_input(
            FindSimilarRequest {
                path: "note.md".to_string(),
                limit: None,
                threshold: None,
                aggregation: None,
                sort: None,
                exclude_linked: Some("both".to_string()),
                tags: None,
                from: None,
                to: None,
                folder: None,
            },
            &FindSimilarConfig::default(),
            NaiveDate::from_ymd_opt(2026, 2, 15).expect("valid date"),
        )
        .expect("valid input");

        assert_eq!(input.exclude_linked, SimilarLinkExclusion::Both);
    }
}
