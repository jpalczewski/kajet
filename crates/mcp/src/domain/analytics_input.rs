use crate::schema::RecentContextRequest;
use chrono::NaiveDate;

const DEFAULT_DAYS: u32 = 7;
const MAX_DAYS: u32 = 90;
const DEFAULT_LIMIT: usize = 10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RecentContextInputError {
    InvalidDays(u32),
    InvalidLimit(usize),
}

#[derive(Debug, Clone)]
pub(crate) struct RecentContextInput {
    pub days: u32,
    pub limit: usize,
    pub from_ts: f64,
    pub to_ts: f64,
    pub prev_from_ts: f64,
    pub prev_to_ts: f64,
}

pub(crate) fn prepare_recent_context_input(
    req: RecentContextRequest,
    today: NaiveDate,
) -> Result<RecentContextInput, RecentContextInputError> {
    let days = req.days.unwrap_or(DEFAULT_DAYS);
    let limit = req.limit.unwrap_or(DEFAULT_LIMIT);

    if !(1..=MAX_DAYS).contains(&days) {
        return Err(RecentContextInputError::InvalidDays(days));
    }

    if limit == 0 {
        return Err(RecentContextInputError::InvalidLimit(limit));
    }

    let current_from_date = today - chrono::Duration::days((days as i64) - 1);
    let from_ts = current_from_date
        .and_hms_opt(0, 0, 0)
        .expect("00:00:00 is always valid")
        .and_utc()
        .timestamp() as f64;
    let to_ts = today
        .and_hms_opt(23, 59, 59)
        .expect("23:59:59 is always valid")
        .and_utc()
        .timestamp() as f64;

    let prev_from_date = current_from_date - chrono::Duration::days(days as i64);
    let prev_to_date = current_from_date - chrono::Duration::days(1);
    let prev_from_ts = prev_from_date
        .and_hms_opt(0, 0, 0)
        .expect("00:00:00 is always valid")
        .and_utc()
        .timestamp() as f64;
    let prev_to_ts = prev_to_date
        .and_hms_opt(23, 59, 59)
        .expect("23:59:59 is always valid")
        .and_utc()
        .timestamp() as f64;

    Ok(RecentContextInput {
        days,
        limit,
        from_ts,
        to_ts,
        prev_from_ts,
        prev_to_ts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_7_days_and_10_entries() {
        let input = prepare_recent_context_input(
            RecentContextRequest {
                days: None,
                limit: None,
            },
            NaiveDate::from_ymd_opt(2025, 1, 15).expect("valid date"),
        )
        .expect("input should be valid");

        assert_eq!(input.days, 7);
        assert_eq!(input.limit, 10);
        // current from = Jan 9 00:00:00 UTC
        assert_eq!(input.from_ts, 1736380800.0);
        // current to = Jan 15 23:59:59 UTC
        assert_eq!(input.to_ts, 1736985599.0);
        // previous from = Jan 2 00:00:00 UTC
        assert_eq!(input.prev_from_ts, 1735776000.0);
        // previous to = Jan 8 23:59:59 UTC
        assert_eq!(input.prev_to_ts, 1736380799.0);
    }

    #[test]
    fn custom_days_and_limit() {
        let input = prepare_recent_context_input(
            RecentContextRequest {
                days: Some(3),
                limit: Some(5),
            },
            NaiveDate::from_ymd_opt(2025, 1, 15).expect("valid date"),
        )
        .expect("input should be valid");

        assert_eq!(input.days, 3);
        assert_eq!(input.limit, 5);
        // current from = Jan 13 00:00:00 UTC
        assert_eq!(input.from_ts, 1736726400.0);
        // previous from = Jan 10 00:00:00 UTC
        assert_eq!(input.prev_from_ts, 1736467200.0);
        // previous to = Jan 12 23:59:59 UTC
        assert_eq!(input.prev_to_ts, 1736726399.0);
    }

    #[test]
    fn rejects_days_outside_supported_range() {
        let err = prepare_recent_context_input(
            RecentContextRequest {
                days: Some(0),
                limit: None,
            },
            NaiveDate::from_ymd_opt(2025, 1, 15).expect("valid date"),
        )
        .expect_err("should reject days=0");
        assert_eq!(err, RecentContextInputError::InvalidDays(0));

        let err = prepare_recent_context_input(
            RecentContextRequest {
                days: Some(91),
                limit: None,
            },
            NaiveDate::from_ymd_opt(2025, 1, 15).expect("valid date"),
        )
        .expect_err("should reject too large days");
        assert_eq!(err, RecentContextInputError::InvalidDays(91));
    }

    #[test]
    fn rejects_limit_zero() {
        let err = prepare_recent_context_input(
            RecentContextRequest {
                days: None,
                limit: Some(0),
            },
            NaiveDate::from_ymd_opt(2025, 1, 15).expect("valid date"),
        )
        .expect_err("should reject limit=0");
        assert_eq!(err, RecentContextInputError::InvalidLimit(0));
    }
}
