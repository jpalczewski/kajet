use chrono::{Datelike, NaiveDate};

/// Context for date parsing - whether the date represents the start or end of a range.
///
/// This affects how ambiguous dates like "2025-01" (month only) or "last week" are interpreted:
/// - `From`: Uses the earliest moment (e.g., first day of month, Monday of week)
/// - `To`: Uses the latest moment (e.g., last day of month, Sunday of week)
#[derive(Debug, Clone, Copy)]
pub enum DateBound {
    /// Start of a date range (earliest moment)
    From,
    /// End of a date range (latest moment)
    To,
}

/// Error returned when a date string cannot be parsed.
#[derive(Debug)]
pub struct DateParseError {
    /// The original input string that failed to parse
    pub input: String,
}

impl std::fmt::Display for DateParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Unable to parse date: '{}'", self.input)
    }
}

impl std::error::Error for DateParseError {}

/// Parse a date string with context-sensitive interpretation based on bound.
///
/// # Supported Formats
///
/// Formats are tried in the following order:
///
/// 1. **ISO 8601:** `"2025-01-15"`, `"2025-01"` (month only)
/// 2. **Polish keywords:**
///    - Absolute: `"dzisiaj"`, `"wczoraj"`
///    - Relative: `"zeszły tydzień"`, `"zeszły miesiąc"`, `"zeszły rok"`
///    - Months: `"w styczniu"`, `"w lutym"`, ..., `"w grudniu"` (also `"we wrześniu"`)
/// 3. **English keywords:**
///    - Absolute: `"today"`, `"yesterday"`
///    - Relative: `"last week"`, `"last month"`, `"last year"`
///    - Months: `"in january"`, `"in february"`, ..., `"in december"`
///
/// # Context-Sensitive Interpretation
///
/// The `bound` parameter affects ambiguous dates:
/// - `"2025-01"` with `From` → `2025-01-01`, with `To` → `2025-01-31`
/// - `"last week"` with `From` → Monday, with `To` → Sunday
/// - `"last month"` with `From` → 1st day, with `To` → last day
///
/// # Examples
///
/// ```
/// use kajet_mcp::{parse_date, DateBound};
///
/// // ISO date
/// let date = parse_date("2025-01-15", DateBound::From).unwrap();
/// assert_eq!(date.to_string(), "2025-01-15");
///
/// // Month with different bounds
/// let from = parse_date("2025-01", DateBound::From).unwrap();
/// let to = parse_date("2025-01", DateBound::To).unwrap();
/// assert_eq!(from.to_string(), "2025-01-01");
/// assert_eq!(to.to_string(), "2025-01-31");
///
/// // Polish keywords (case-insensitive)
/// let date = parse_date("DZISIAJ", DateBound::From).unwrap();
/// // Returns today's date
/// ```
///
/// # Errors
///
/// Returns `DateParseError` if the input doesn't match any supported format.
pub fn parse_date(input: &str, bound: DateBound) -> Result<NaiveDate, DateParseError> {
    let input = input.trim();

    // Try ISO 8601 first
    if let Ok(date) = NaiveDate::parse_from_str(input, "%Y-%m-%d") {
        return Ok(date);
    }

    // ISO month only: "2025-01"
    if let Ok(ym) = chrono::NaiveDate::parse_from_str(&format!("{}-01", input), "%Y-%m-%d") {
        return Ok(match bound {
            DateBound::From => ym,
            DateBound::To => last_day_of_month(ym.year(), ym.month()),
        });
    }

    let lower = input.to_lowercase();
    let today = chrono::Local::now().date_naive();

    // Polish keywords
    if lower == "dzisiaj" || lower == "today" {
        return Ok(today);
    }

    if lower == "wczoraj" || lower == "yesterday" {
        return Ok(today - chrono::Duration::days(1));
    }

    // Polish: "zeszły tydzień"
    if lower == "zeszły tydzień" || lower == "last week" {
        let weekday = today.weekday().num_days_from_monday();
        let last_monday = today - chrono::Duration::days((weekday + 7) as i64);
        return Ok(match bound {
            DateBound::From => last_monday,
            DateBound::To => last_monday + chrono::Duration::days(6),
        });
    }

    // Polish: "zeszły miesiąc"
    if lower == "zeszły miesiąc" || lower == "last month" {
        let last_month = if today.month() == 1 {
            NaiveDate::from_ymd_opt(today.year() - 1, 12, 1).unwrap()
        } else {
            NaiveDate::from_ymd_opt(today.year(), today.month() - 1, 1).unwrap()
        };
        return Ok(match bound {
            DateBound::From => last_month,
            DateBound::To => last_day_of_month(last_month.year(), last_month.month()),
        });
    }

    // Polish: "zeszły rok"
    if lower == "zeszły rok" || lower == "last year" {
        let last_year = today.year() - 1;
        return Ok(match bound {
            DateBound::From => NaiveDate::from_ymd_opt(last_year, 1, 1).unwrap(),
            DateBound::To => NaiveDate::from_ymd_opt(last_year, 12, 31).unwrap(),
        });
    }

    // Month names with "w"/"in" prefix
    let month_names_pl = [
        ("w styczniu", 1),
        ("w lutym", 2),
        ("w marcu", 3),
        ("w kwietniu", 4),
        ("w maju", 5),
        ("w czerwcu", 6),
        ("w lipcu", 7),
        ("w sierpniu", 8),
        ("we wrześniu", 9),
        ("w październiku", 10),
        ("w listopadzie", 11),
        ("w grudniu", 12),
    ];

    let month_names_en = [
        ("in january", 1),
        ("in february", 2),
        ("in march", 3),
        ("in april", 4),
        ("in may", 5),
        ("in june", 6),
        ("in july", 7),
        ("in august", 8),
        ("in september", 9),
        ("in october", 10),
        ("in november", 11),
        ("in december", 12),
    ];

    for (phrase, month) in month_names_pl.iter().chain(month_names_en.iter()) {
        if lower == *phrase {
            let year = today.year();
            return Ok(match bound {
                DateBound::From => NaiveDate::from_ymd_opt(year, *month, 1).unwrap(),
                DateBound::To => last_day_of_month(year, *month),
            });
        }
    }

    Err(DateParseError {
        input: input.to_string(),
    })
}

fn last_day_of_month(year: i32, month: u32) -> NaiveDate {
    let next_month = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap()
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap()
    };
    next_month - chrono::Duration::days(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iso_full_date() {
        let date = parse_date("2025-01-15", DateBound::From).unwrap();
        assert_eq!(date.to_string(), "2025-01-15");
    }

    #[test]
    fn test_iso_month_from() {
        let date = parse_date("2025-01", DateBound::From).unwrap();
        assert_eq!(date.to_string(), "2025-01-01");
    }

    #[test]
    fn test_iso_month_to() {
        let date = parse_date("2025-01", DateBound::To).unwrap();
        assert_eq!(date.to_string(), "2025-01-31");
    }

    #[test]
    fn test_iso_month_february_leap() {
        let date = parse_date("2024-02", DateBound::To).unwrap();
        assert_eq!(date.to_string(), "2024-02-29");
    }

    #[test]
    fn test_iso_month_february_non_leap() {
        let date = parse_date("2025-02", DateBound::To).unwrap();
        assert_eq!(date.to_string(), "2025-02-28");
    }

    #[test]
    fn test_polish_dzisiaj() {
        let today = chrono::Local::now().date_naive();
        let date = parse_date("dzisiaj", DateBound::From).unwrap();
        assert_eq!(date, today);
    }

    #[test]
    fn test_english_today() {
        let today = chrono::Local::now().date_naive();
        let date = parse_date("today", DateBound::From).unwrap();
        assert_eq!(date, today);
    }

    #[test]
    fn test_polish_wczoraj() {
        let yesterday = chrono::Local::now().date_naive() - chrono::Duration::days(1);
        let date = parse_date("wczoraj", DateBound::From).unwrap();
        assert_eq!(date, yesterday);
    }

    #[test]
    fn test_english_yesterday() {
        let yesterday = chrono::Local::now().date_naive() - chrono::Duration::days(1);
        let date = parse_date("yesterday", DateBound::From).unwrap();
        assert_eq!(date, yesterday);
    }

    #[test]
    fn test_polish_last_week_from() {
        let date = parse_date("zeszły tydzień", DateBound::From).unwrap();
        assert_eq!(date.weekday().num_days_from_monday(), 0); // Monday
    }

    #[test]
    fn test_polish_last_week_to() {
        let date = parse_date("zeszły tydzień", DateBound::To).unwrap();
        assert_eq!(date.weekday().num_days_from_monday(), 6); // Sunday
    }

    #[test]
    fn test_english_last_week_from() {
        let date = parse_date("last week", DateBound::From).unwrap();
        assert_eq!(date.weekday().num_days_from_monday(), 0);
    }

    #[test]
    fn test_english_last_week_to() {
        let date = parse_date("last week", DateBound::To).unwrap();
        assert_eq!(date.weekday().num_days_from_monday(), 6);
    }

    #[test]
    fn test_polish_last_month_from() {
        let date = parse_date("zeszły miesiąc", DateBound::From).unwrap();
        assert_eq!(date.day(), 1);
    }

    #[test]
    fn test_polish_last_month_to() {
        let date = parse_date("zeszły miesiąc", DateBound::To).unwrap();
        // Should be last day of previous month
        let today = chrono::Local::now().date_naive();
        let expected_month = if today.month() == 1 {
            12
        } else {
            today.month() - 1
        };
        assert_eq!(date.month(), expected_month);
        assert!(date.day() >= 28); // At least 28 days
    }

    #[test]
    fn test_english_last_month() {
        let date = parse_date("last month", DateBound::From).unwrap();
        assert_eq!(date.day(), 1);
    }

    #[test]
    fn test_polish_last_year_from() {
        let date = parse_date("zeszły rok", DateBound::From).unwrap();
        let today = chrono::Local::now().date_naive();
        assert_eq!(date.year(), today.year() - 1);
        assert_eq!(date.month(), 1);
        assert_eq!(date.day(), 1);
    }

    #[test]
    fn test_polish_last_year_to() {
        let date = parse_date("zeszły rok", DateBound::To).unwrap();
        let today = chrono::Local::now().date_naive();
        assert_eq!(date.year(), today.year() - 1);
        assert_eq!(date.month(), 12);
        assert_eq!(date.day(), 31);
    }

    #[test]
    fn test_english_last_year() {
        let date = parse_date("last year", DateBound::From).unwrap();
        let today = chrono::Local::now().date_naive();
        assert_eq!(date.year(), today.year() - 1);
    }

    #[test]
    fn test_polish_month_w_styczniu_from() {
        let date = parse_date("w styczniu", DateBound::From).unwrap();
        assert_eq!(date.month(), 1);
        assert_eq!(date.day(), 1);
    }

    #[test]
    fn test_polish_month_w_styczniu_to() {
        let date = parse_date("w styczniu", DateBound::To).unwrap();
        assert_eq!(date.month(), 1);
        assert_eq!(date.day(), 31);
    }

    #[test]
    fn test_polish_month_we_wrzesniu() {
        let date = parse_date("we wrześniu", DateBound::From).unwrap();
        assert_eq!(date.month(), 9);
        assert_eq!(date.day(), 1);
    }

    #[test]
    fn test_english_month_in_january() {
        let date = parse_date("in january", DateBound::From).unwrap();
        assert_eq!(date.month(), 1);
    }

    #[test]
    fn test_english_month_in_december_to() {
        let date = parse_date("in december", DateBound::To).unwrap();
        assert_eq!(date.month(), 12);
        assert_eq!(date.day(), 31);
    }

    #[test]
    fn test_invalid_input() {
        let result = parse_date("invalid date string", DateBound::From);
        assert!(result.is_err());
    }

    #[test]
    fn test_case_insensitive() {
        let date1 = parse_date("DZISIAJ", DateBound::From).unwrap();
        let date2 = parse_date("DzIsIaJ", DateBound::From).unwrap();
        assert_eq!(date1, date2);
    }

    #[test]
    fn test_whitespace_trim() {
        let date = parse_date("  2025-01-15  ", DateBound::From).unwrap();
        assert_eq!(date.to_string(), "2025-01-15");
    }
}
