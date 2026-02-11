use crate::date_parser::{DateBound, parse_date};
use crate::schema::SearchRequest;
use chrono::NaiveDate;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SearchMode {
    Hybrid,
    Vector,
    Fts,
}

impl SearchMode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Hybrid => "hybrid",
            Self::Vector => "vector",
            Self::Fts => "fts",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum SearchKind {
    Query { query: String, mode: SearchMode },
    Browse,
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedSearchInput {
    pub kind: SearchKind,
    pub limit: usize,
    pub has_filters: bool,
    pub from_date: Option<NaiveDate>,
    pub to_date: Option<NaiveDate>,
    pub from_ts: Option<f64>,
    pub to_ts: Option<f64>,
    pub folder: Option<String>,
    pub tags: Option<Vec<String>>,
}

impl PreparedSearchInput {
    pub(crate) fn browse_event_query(&self) -> String {
        format!(
            "[browse] from:{} to:{} folder:{} tags:{}",
            self.from_date
                .map(|d| d.to_string())
                .unwrap_or_else(|| "*".to_string()),
            self.to_date
                .map(|d| d.to_string())
                .unwrap_or_else(|| "*".to_string()),
            self.folder.as_deref().unwrap_or("*"),
            self.tags
                .as_ref()
                .map(|t| t.join(","))
                .unwrap_or_else(|| "*".to_string())
        )
    }
}

#[derive(Debug, Clone)]
pub(crate) enum SearchInputError {
    NoQueryOrFilters,
    DateParse { input: String, error: String },
    InvalidDateRange { from: NaiveDate, to: NaiveDate },
}

pub(crate) fn prepare_search_input(
    req: SearchRequest,
    default_limit: usize,
    today: NaiveDate,
) -> Result<PreparedSearchInput, SearchInputError> {
    let limit = req.limit.unwrap_or(default_limit);
    let has_query = req.query.is_some();
    let has_filters = req.from.is_some()
        || req.to.is_some()
        || req.tags.as_ref().map(|t| !t.is_empty()).unwrap_or(false)
        || req.folder.is_some();

    if !has_query && !has_filters {
        return Err(SearchInputError::NoQueryOrFilters);
    }

    let from_date = if let Some(ref from_str) = req.from {
        Some(parse_date(from_str, DateBound::From).map_err(|e| {
            let input = e.input.clone();
            SearchInputError::DateParse {
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
            SearchInputError::DateParse {
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
        return Err(SearchInputError::InvalidDateRange { from, to });
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

    let kind = if let Some(query) = req.query {
        let mode = match req.mode.as_deref() {
            Some("vector") => SearchMode::Vector,
            Some("fts") => SearchMode::Fts,
            _ => SearchMode::Hybrid,
        };
        SearchKind::Query { query, mode }
    } else {
        SearchKind::Browse
    };

    Ok(PreparedSearchInput {
        kind,
        limit,
        has_filters,
        from_date,
        to_date,
        from_ts,
        to_ts,
        folder: req.folder,
        tags: req.tags,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> SearchRequest {
        SearchRequest {
            query: None,
            limit: None,
            mode: None,
            from: None,
            to: None,
            tags: None,
            folder: None,
        }
    }

    #[test]
    fn fails_without_query_and_filters() {
        let err = prepare_search_input(
            request(),
            5,
            NaiveDate::from_ymd_opt(2026, 2, 11).expect("valid date"),
        )
        .expect_err("should fail");

        assert!(matches!(err, SearchInputError::NoQueryOrFilters));
    }

    #[test]
    fn defaults_to_today_when_from_without_to() {
        let mut req = request();
        req.from = Some("2026-02-01".to_string());

        let out = prepare_search_input(
            req,
            5,
            NaiveDate::from_ymd_opt(2026, 2, 11).expect("valid date"),
        )
        .expect("should parse");

        assert_eq!(
            out.to_date,
            Some(NaiveDate::from_ymd_opt(2026, 2, 11).expect("valid date"))
        );
    }

    #[test]
    fn parses_mode_and_query() {
        let mut req = request();
        req.query = Some("rust".to_string());
        req.mode = Some("vector".to_string());

        let out = prepare_search_input(
            req,
            7,
            NaiveDate::from_ymd_opt(2026, 2, 11).expect("valid date"),
        )
        .expect("should parse");

        assert_eq!(out.limit, 7);
        match out.kind {
            SearchKind::Query { mode, .. } => assert_eq!(mode, SearchMode::Vector),
            SearchKind::Browse => panic!("expected query mode"),
        }
    }

    #[test]
    fn fails_on_invalid_date_range() {
        let mut req = request();
        req.from = Some("2026-02-10".to_string());
        req.to = Some("2026-02-01".to_string());

        let err = prepare_search_input(
            req,
            5,
            NaiveDate::from_ymd_opt(2026, 2, 11).expect("valid date"),
        )
        .expect_err("should fail");

        assert!(matches!(err, SearchInputError::InvalidDateRange { .. }));
    }
}
