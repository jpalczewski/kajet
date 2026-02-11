use crate::domain::analytics_input::RecentContextInput;
use crate::format::{RecentContextEntryView, RecentContextView, format_recent_context};
use crate::ports::AnalyticsPorts;
use kajet_core::text_utils::truncate_with_ellipsis;
use kajet_core::types::Document;
use std::collections::{HashMap, HashSet};

const ANALYTICS_SCAN_LIMIT: usize = 20_000;
const RECENT_SNIPPET_MAX_BYTES: usize = 200;
type EntryLengthStat = Option<(String, usize)>;

pub(crate) struct RecentContextOutput {
    pub days: u32,
    pub entry_count: usize,
    pub tag_count: usize,
    pub summary: String,
}

pub(crate) async fn execute_recent_context(
    ports: &impl AnalyticsPorts,
    input: RecentContextInput,
) -> anyhow::Result<RecentContextOutput> {
    let total_start = std::time::Instant::now();
    tracing::info!(
        days = input.days,
        limit = input.limit,
        from_ts = input.from_ts,
        to_ts = input.to_ts,
        prev_from_ts = input.prev_from_ts,
        prev_to_ts = input.prev_to_ts,
        "recent_context use case started"
    );

    let stage_start = std::time::Instant::now();
    let current_period_docs = ports
        .query_documents(
            Some(input.from_ts),
            Some(input.to_ts),
            None,
            ANALYTICS_SCAN_LIMIT,
        )
        .await?;
    tracing::debug!(
        stage = "fetch_current_period",
        docs = current_period_docs.len(),
        elapsed_ms = stage_start.elapsed().as_millis() as u64,
        "recent_context stage completed"
    );

    let stage_start = std::time::Instant::now();
    let previous_period_docs = ports
        .query_documents(
            Some(input.prev_from_ts),
            Some(input.prev_to_ts),
            None,
            ANALYTICS_SCAN_LIMIT,
        )
        .await?;
    tracing::debug!(
        stage = "fetch_previous_period",
        docs = previous_period_docs.len(),
        elapsed_ms = stage_start.elapsed().as_millis() as u64,
        "recent_context stage completed"
    );

    let total_current_entries = current_period_docs.len();
    tracing::trace!(
        current_paths = ?current_period_docs
            .iter()
            .map(|d| d.source_file.clone())
            .collect::<Vec<_>>(),
        "recent_context current period paths"
    );

    if total_current_entries == 0 {
        let summary = format_recent_context(&RecentContextView {
            days: input.days,
            total_current_entries: 0,
            previous_period_entries: previous_period_docs.len(),
            tag_count: 0,
            top_tags: vec![],
            new_tags: vec![],
            gap_dates: vec![],
            longest: None,
            shortest: None,
            entries: vec![],
        });
        tracing::info!(
            days = input.days,
            previous_entries = previous_period_docs.len(),
            elapsed_ms = total_start.elapsed().as_millis() as u64,
            "recent_context completed with empty period"
        );
        return Ok(RecentContextOutput {
            days: input.days,
            entry_count: 0,
            tag_count: 0,
            summary,
        });
    }

    let stage_start = std::time::Instant::now();
    let all_docs = ports.get_all_documents().await?;
    tracing::debug!(
        stage = "fetch_all_documents",
        docs = all_docs.len(),
        elapsed_ms = stage_start.elapsed().as_millis() as u64,
        "recent_context stage completed"
    );

    let stage_start = std::time::Instant::now();
    let top_tags = aggregate_period_tags(&current_period_docs);
    let tag_count = top_tags.len();
    tracing::debug!(
        stage = "aggregate_tags",
        unique_tags = tag_count,
        elapsed_ms = stage_start.elapsed().as_millis() as u64,
        "recent_context stage completed"
    );
    tracing::trace!(top_tags = ?top_tags, "recent_context top tags");

    let stage_start = std::time::Instant::now();
    let new_tags = find_new_tags(&current_period_docs, &all_docs, input.from_ts);
    tracing::debug!(
        stage = "detect_new_tags",
        new_tags_modified_window = new_tags.len(),
        elapsed_ms = stage_start.elapsed().as_millis() as u64,
        "recent_context stage completed"
    );
    tracing::trace!(
        new_tags_modified_window = ?new_tags,
        "recent_context new tags (based on modified window)"
    );

    let stage_start = std::time::Instant::now();
    let activity_gap_dates = compute_gap_dates(&current_period_docs, input.from_ts, input.to_ts);
    let journal_gap_dates =
        compute_journal_gap_dates(&current_period_docs, input.from_ts, input.to_ts);
    let (gap_mode, gap_dates) = match journal_gap_dates {
        Some(gaps) => ("journal", gaps),
        None => ("activity", activity_gap_dates.clone()),
    };
    tracing::debug!(
        stage = "compute_gaps",
        gaps = gap_dates.len(),
        gap_mode,
        elapsed_ms = stage_start.elapsed().as_millis() as u64,
        "recent_context stage completed"
    );
    tracing::trace!(
        activity_gap_dates = ?activity_gap_dates,
        selected_gap_dates = ?gap_dates,
        "recent_context gap dates"
    );

    let stage_start = std::time::Instant::now();
    let (longest, shortest) = compute_length_extremes(&current_period_docs);
    tracing::debug!(
        stage = "compute_length_extremes",
        has_longest = longest.is_some(),
        has_shortest = shortest.is_some(),
        elapsed_ms = stage_start.elapsed().as_millis() as u64,
        "recent_context stage completed"
    );

    let stage_start = std::time::Instant::now();
    let entries = to_entry_view(current_period_docs, input.limit);
    tracing::debug!(
        stage = "prepare_entry_view",
        entries = entries.len(),
        elapsed_ms = stage_start.elapsed().as_millis() as u64,
        "recent_context stage completed"
    );
    tracing::trace!(
        entry_paths = ?entries.iter().map(|e| e.path.clone()).collect::<Vec<_>>(),
        "recent_context output entries"
    );

    let new_tags_modified_window = new_tags.len();
    let gap_days_count = gap_dates.len();
    let stage_start = std::time::Instant::now();
    let summary = format_recent_context(&RecentContextView {
        days: input.days,
        total_current_entries,
        previous_period_entries: previous_period_docs.len(),
        tag_count,
        top_tags,
        new_tags,
        gap_dates,
        longest,
        shortest,
        entries,
    });
    tracing::debug!(
        stage = "format_output",
        summary_chars = summary.len(),
        elapsed_ms = stage_start.elapsed().as_millis() as u64,
        "recent_context stage completed"
    );

    tracing::info!(
        days = input.days,
        total_entries = total_current_entries,
        previous_entries = previous_period_docs.len(),
        unique_tags = tag_count,
        new_tags_modified_window,
        gap_days = gap_days_count,
        gap_mode,
        elapsed_ms = total_start.elapsed().as_millis() as u64,
        "recent_context use case completed"
    );

    Ok(RecentContextOutput {
        days: input.days,
        entry_count: total_current_entries,
        tag_count,
        summary,
    })
}

fn to_entry_view(mut docs: Vec<Document>, limit: usize) -> Vec<RecentContextEntryView> {
    docs.sort_by(|a, b| {
        b.last_modified
            .partial_cmp(&a.last_modified)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.source_file.cmp(&b.source_file))
    });
    docs.truncate(limit);

    docs.into_iter()
        .map(|doc| RecentContextEntryView {
            title: doc.title,
            path: doc.source_file,
            date: timestamp_to_date(doc.last_modified),
            tags: doc.tags,
            snippet: truncate_with_ellipsis(&doc.full_text, RECENT_SNIPPET_MAX_BYTES),
        })
        .collect()
}

/// Count words in text by splitting by whitespace.
fn word_count(text: &str) -> usize {
    text.split_whitespace().count()
}

fn compute_length_extremes(docs: &[Document]) -> (EntryLengthStat, EntryLengthStat) {
    let mut longest: EntryLengthStat = None;
    let mut shortest: EntryLengthStat = None;

    for doc in docs {
        let wc = word_count(&doc.full_text);

        match &longest {
            None => longest = Some((doc.title.clone(), wc)),
            Some((title, longest_wc)) => {
                if wc > *longest_wc || (wc == *longest_wc && doc.title < *title) {
                    longest = Some((doc.title.clone(), wc));
                }
            }
        }

        match &shortest {
            None => shortest = Some((doc.title.clone(), wc)),
            Some((title, shortest_wc)) => {
                if wc < *shortest_wc || (wc == *shortest_wc && doc.title < *title) {
                    shortest = Some((doc.title.clone(), wc));
                }
            }
        }
    }

    (longest, shortest)
}

/// Compute dates with no entries in the period [from_ts, to_ts].
fn compute_gap_dates(docs: &[Document], from_ts: f64, to_ts: f64) -> Vec<String> {
    let entry_dates: HashSet<String> = docs
        .iter()
        .map(|d| timestamp_to_date(d.last_modified))
        .collect();

    let from = chrono::DateTime::from_timestamp(from_ts as i64, 0)
        .map(|dt| dt.date_naive())
        .unwrap_or_else(|| chrono::NaiveDate::from_ymd_opt(1970, 1, 1).expect("valid fallback"));
    let to = chrono::DateTime::from_timestamp(to_ts as i64, 0)
        .map(|dt| dt.date_naive())
        .unwrap_or(from);

    let mut gaps = Vec::new();
    let mut date = from;
    while date <= to {
        let date_str = date.format("%Y-%m-%d").to_string();
        if !entry_dates.contains(&date_str) {
            gaps.push(date_str);
        }
        date += chrono::Duration::days(1);
    }

    gaps
}

/// Compute dates with no journal entries in the period [from_ts, to_ts], based on YYYY-MM-DD
/// extracted from document paths. Returns None when no journal-like dates are found.
fn compute_journal_gap_dates(docs: &[Document], from_ts: f64, to_ts: f64) -> Option<Vec<String>> {
    let entry_dates: HashSet<chrono::NaiveDate> = docs
        .iter()
        .filter_map(|d| extract_date_from_path(&d.source_file))
        .collect();

    if entry_dates.is_empty() {
        return None;
    }

    let from = chrono::DateTime::from_timestamp(from_ts as i64, 0)
        .map(|dt| dt.date_naive())
        .unwrap_or_else(|| chrono::NaiveDate::from_ymd_opt(1970, 1, 1).expect("valid fallback"));
    let to = chrono::DateTime::from_timestamp(to_ts as i64, 0)
        .map(|dt| dt.date_naive())
        .unwrap_or(from);

    let mut gaps = Vec::new();
    let mut date = from;
    while date <= to {
        if !entry_dates.contains(&date) {
            gaps.push(date.format("%Y-%m-%d").to_string());
        }
        date += chrono::Duration::days(1);
    }

    Some(gaps)
}

fn extract_date_from_path(path: &str) -> Option<chrono::NaiveDate> {
    for segment in path.split(|c: char| !c.is_ascii_digit() && c != '-') {
        if segment.len() == 10
            && segment.as_bytes().get(4) == Some(&b'-')
            && segment.as_bytes().get(7) == Some(&b'-')
            && let Ok(date) = chrono::NaiveDate::parse_from_str(segment, "%Y-%m-%d")
        {
            return Some(date);
        }
    }
    None
}

/// Aggregate tags from documents, returning (tag, count) sorted by count desc then tag asc.
fn aggregate_period_tags(docs: &[Document]) -> Vec<(String, usize)> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for doc in docs {
        for tag in &doc.tags {
            *counts.entry(tag.clone()).or_insert(0) += 1;
        }
    }

    let mut sorted: Vec<(String, usize)> = counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    sorted
}

/// Find tags that appear in `period_docs` but not before `from_ts`.
fn find_new_tags(period_docs: &[Document], all_docs: &[Document], from_ts: f64) -> Vec<String> {
    let period_tags: HashSet<&str> = period_docs
        .iter()
        .flat_map(|d| d.tags.iter().map(|t| t.as_str()))
        .collect();
    let older_tags: HashSet<&str> = all_docs
        .iter()
        .filter(|d| d.last_modified < from_ts)
        .flat_map(|d| d.tags.iter().map(|t| t.as_str()))
        .collect();

    let mut new_tags: Vec<String> = period_tags
        .difference(&older_tags)
        .map(|tag| (*tag).to_string())
        .collect();
    new_tags.sort();
    new_tags
}

fn timestamp_to_date(ts: f64) -> String {
    chrono::DateTime::from_timestamp(ts as i64, 0)
        .map(|dt| dt.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn make_doc(title: &str, path: &str, tags: Vec<&str>, text: &str, ts: f64) -> Document {
        Document {
            source_file: path.to_string(),
            full_text: text.to_string(),
            title: title.to_string(),
            tags: tags.into_iter().map(String::from).collect(),
            content_hash: "h".to_string(),
            last_modified: ts,
            outgoing_links: vec![],
            backlinks: vec![],
        }
    }

    #[test]
    fn word_count_basic() {
        assert_eq!(word_count("hello world"), 2);
        assert_eq!(word_count(""), 0);
        assert_eq!(word_count("  multiple   spaces  "), 2);
    }

    #[test]
    fn compute_gap_dates_finds_missing_days() {
        let docs = vec![
            make_doc("A", "a.md", vec![], "", 1736899200.0), // 2025-01-15
            make_doc("B", "b.md", vec![], "", 1736726400.0), // 2025-01-13
        ];

        let gaps = compute_gap_dates(&docs, 1736640000.0, 1736985599.0);
        assert_eq!(gaps, vec!["2025-01-12", "2025-01-14"]);
    }

    #[test]
    fn compute_journal_gap_dates_uses_dates_from_paths() {
        let docs = vec![
            make_doc("A", "journal/2025-01-15.md", vec![], "", 1736899200.0),
            make_doc("B", "journal/2025-01-13.md", vec![], "", 1736726400.0),
        ];

        let gaps = compute_journal_gap_dates(&docs, 1736640000.0, 1736985599.0)
            .expect("should infer journal dates");
        assert_eq!(gaps, vec!["2025-01-12", "2025-01-14"]);
    }

    #[test]
    fn compute_journal_gap_dates_returns_none_without_parsable_dates() {
        let docs = vec![
            make_doc("A", "notes/today.md", vec![], "", 1736899200.0),
            make_doc("B", "notes/yesterday.md", vec![], "", 1736726400.0),
        ];

        let gaps = compute_journal_gap_dates(&docs, 1736640000.0, 1736985599.0);
        assert!(gaps.is_none());
    }

    #[test]
    fn aggregate_period_tags_sorted_by_count() {
        let docs = vec![
            make_doc("A", "a.md", vec!["rust", "code"], "", 0.0),
            make_doc("B", "b.md", vec!["rust"], "", 0.0),
            make_doc("C", "c.md", vec!["python", "code"], "", 0.0),
        ];

        let tags = aggregate_period_tags(&docs);
        assert_eq!(tags[0], ("code".to_string(), 2));
        assert_eq!(tags[1], ("rust".to_string(), 2));
        assert_eq!(tags[2], ("python".to_string(), 1));
    }

    #[test]
    fn find_new_tags_detects_first_time_tags() {
        let period_docs = vec![make_doc(
            "A",
            "a.md",
            vec!["meditation", "reflection"],
            "",
            100.0,
        )];
        let all_docs = vec![
            make_doc("Old", "old.md", vec!["reflection", "work"], "", 50.0),
            make_doc("A", "a.md", vec!["meditation", "reflection"], "", 100.0),
        ];

        let new = find_new_tags(&period_docs, &all_docs, 100.0);
        assert_eq!(new, vec!["meditation"]);
    }

    #[derive(Debug, Clone, PartialEq)]
    struct QueryCall {
        from_ts: Option<f64>,
        to_ts: Option<f64>,
        folder: Option<String>,
        limit: usize,
    }

    #[derive(Clone)]
    struct FakeAnalyticsPorts {
        current_docs: Vec<Document>,
        previous_docs: Vec<Document>,
        all_docs: Vec<Document>,
        calls: Arc<Mutex<Vec<QueryCall>>>,
    }

    impl AnalyticsPorts for FakeAnalyticsPorts {
        async fn query_documents(
            &self,
            from_ts: Option<f64>,
            to_ts: Option<f64>,
            folder: Option<&str>,
            limit: usize,
        ) -> anyhow::Result<Vec<Document>> {
            self.calls.lock().expect("lock").push(QueryCall {
                from_ts,
                to_ts,
                folder: folder.map(ToString::to_string),
                limit,
            });

            if from_ts == Some(1736380800.0) && to_ts == Some(1736985599.0) {
                Ok(self.current_docs.clone())
            } else if from_ts == Some(1735776000.0) && to_ts == Some(1736380799.0) {
                Ok(self.previous_docs.clone())
            } else {
                Ok(vec![])
            }
        }

        async fn get_all_documents(&self) -> anyhow::Result<Vec<Document>> {
            Ok(self.all_docs.clone())
        }
    }

    #[tokio::test]
    async fn execute_recent_context_with_entries() {
        rust_i18n::set_locale("en");
        let calls = Arc::new(Mutex::new(Vec::new()));
        let ports = FakeAnalyticsPorts {
            current_docs: vec![
                make_doc(
                    "Today Note",
                    "journal/today.md",
                    vec!["reflection"],
                    "Some deep thoughts about life",
                    1736985500.0,
                ),
                make_doc(
                    "Yesterday Note",
                    "journal/yesterday.md",
                    vec!["work"],
                    "Work meeting notes",
                    1736899100.0,
                ),
            ],
            previous_docs: vec![make_doc(
                "Old",
                "old.md",
                vec!["work"],
                "Old stuff",
                1736200000.0,
            )],
            all_docs: vec![
                make_doc("Old", "old.md", vec!["work"], "Old stuff", 1736200000.0),
                make_doc(
                    "Today Note",
                    "journal/today.md",
                    vec!["reflection"],
                    "Some deep thoughts about life",
                    1736985500.0,
                ),
                make_doc(
                    "Yesterday Note",
                    "journal/yesterday.md",
                    vec!["work"],
                    "Work meeting notes",
                    1736899100.0,
                ),
            ],
            calls: calls.clone(),
        };

        let input = RecentContextInput {
            days: 7,
            limit: 10,
            from_ts: 1736380800.0,
            to_ts: 1736985599.0,
            prev_from_ts: 1735776000.0,
            prev_to_ts: 1736380799.0,
        };

        let result = execute_recent_context(&ports, input)
            .await
            .expect("should succeed");
        assert_eq!(result.entry_count, 2);
        assert_eq!(result.tag_count, 2);
        assert!(result.summary.contains("Today Note"));
        assert!(result.summary.contains("Yesterday Note"));
        assert!(result.summary.contains("#reflection"));

        let calls = calls.lock().expect("lock");
        assert_eq!(
            calls.as_slice(),
            &[
                QueryCall {
                    from_ts: Some(1736380800.0),
                    to_ts: Some(1736985599.0),
                    folder: None,
                    limit: ANALYTICS_SCAN_LIMIT
                },
                QueryCall {
                    from_ts: Some(1735776000.0),
                    to_ts: Some(1736380799.0),
                    folder: None,
                    limit: ANALYTICS_SCAN_LIMIT
                }
            ]
        );
    }

    #[tokio::test]
    async fn execute_recent_context_empty_period() {
        rust_i18n::set_locale("en");
        let calls = Arc::new(Mutex::new(Vec::new()));
        let ports = FakeAnalyticsPorts {
            current_docs: vec![],
            previous_docs: vec![],
            all_docs: vec![],
            calls,
        };

        let input = RecentContextInput {
            days: 7,
            limit: 10,
            from_ts: 1736380800.0,
            to_ts: 1736985599.0,
            prev_from_ts: 1735776000.0,
            prev_to_ts: 1736380799.0,
        };

        let result = execute_recent_context(&ports, input)
            .await
            .expect("should succeed");
        assert_eq!(result.entry_count, 0);
        assert_eq!(result.tag_count, 0);
        assert_eq!(result.summary, "No entries found in the last 7 days.");
    }
}
