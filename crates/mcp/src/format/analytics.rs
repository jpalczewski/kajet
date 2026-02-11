#[derive(Debug, Clone)]
pub(crate) struct RecentContextEntryView {
    pub title: String,
    pub path: String,
    pub date: String,
    pub tags: Vec<String>,
    pub snippet: String,
}

#[derive(Debug, Clone)]
pub(crate) struct RecentContextView {
    pub days: u32,
    pub total_current_entries: usize,
    pub previous_period_entries: usize,
    pub tag_count: usize,
    pub top_tags: Vec<(String, usize)>,
    pub new_tags: Vec<String>,
    pub gap_dates: Vec<String>,
    pub longest: Option<(String, usize)>,
    pub shortest: Option<(String, usize)>,
    pub entries: Vec<RecentContextEntryView>,
}

pub(crate) fn format_recent_context(view: &RecentContextView) -> String {
    if view.total_current_entries == 0 {
        return t!("recent_context_empty", days = view.days).to_string();
    }

    let mut sections: Vec<String> = Vec::with_capacity(8);

    sections.push(
        t!(
            "recent_context_header",
            days = view.days,
            entry_count = view.total_current_entries,
            tag_count = view.tag_count
        )
        .to_string(),
    );

    sections.push(
        t!(
            "recent_context_frequency",
            current = view.total_current_entries,
            days = view.days,
            previous = view.previous_period_entries
        )
        .to_string(),
    );

    if let (Some((longest_title, longest_words)), Some((shortest_title, shortest_words))) =
        (&view.longest, &view.shortest)
    {
        sections.push(
            t!(
                "recent_context_length_stats",
                longest_title = longest_title,
                longest_words = longest_words,
                shortest_title = shortest_title,
                shortest_words = shortest_words
            )
            .to_string(),
        );
    }

    if view.gap_dates.is_empty() {
        sections.push(t!("recent_context_no_gaps").to_string());
    } else {
        sections.push(t!("recent_context_gaps", gaps = view.gap_dates.join(", ")).to_string());
    }

    sections.push(t!("recent_context_entries_header").to_string());
    for entry in &view.entries {
        if entry.tags.is_empty() {
            sections.push(
                t!(
                    "recent_context_entry_no_tags",
                    title = &entry.title,
                    path = &entry.path,
                    date = &entry.date,
                    snippet = &entry.snippet
                )
                .to_string(),
            );
        } else {
            let tags = entry
                .tags
                .iter()
                .map(|tag| format!("#{tag}"))
                .collect::<Vec<_>>()
                .join(" ");
            sections.push(
                t!(
                    "recent_context_entry",
                    title = &entry.title,
                    path = &entry.path,
                    date = &entry.date,
                    tags = tags,
                    snippet = &entry.snippet
                )
                .to_string(),
            );
        }
    }

    if !view.top_tags.is_empty() {
        let mut section = t!("recent_context_top_tags").to_string();
        for (tag, count) in &view.top_tags {
            section.push_str(&format!("\n- #{tag} ({count})"));
        }
        sections.push(section);
    }

    if view.new_tags.is_empty() {
        sections.push(t!("recent_context_no_new_tags").to_string());
    } else {
        let mut section = t!("recent_context_new_tags").to_string();
        for tag in &view.new_tags {
            section.push_str(&format!("\n- #{tag}"));
        }
        sections.push(section);
    }

    sections.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_locale() {
        rust_i18n::set_locale("en");
    }

    #[test]
    fn empty_period_shows_no_entries_message() {
        setup_locale();
        let out = format_recent_context(&RecentContextView {
            days: 7,
            total_current_entries: 0,
            previous_period_entries: 0,
            tag_count: 0,
            top_tags: vec![],
            new_tags: vec![],
            gap_dates: vec![],
            longest: None,
            shortest: None,
            entries: vec![],
        });

        assert_eq!(out, "No entries found in the last 7 days.");
    }

    #[test]
    fn includes_sections_for_populated_period() {
        setup_locale();
        let out = format_recent_context(&RecentContextView {
            days: 7,
            total_current_entries: 5,
            previous_period_entries: 3,
            tag_count: 2,
            top_tags: vec![("reflection".to_string(), 4), ("work".to_string(), 2)],
            new_tags: vec!["meditation".to_string()],
            gap_dates: vec!["2025-01-11".to_string()],
            longest: Some(("Morning Thoughts".to_string(), 10)),
            shortest: Some(("Quick Note".to_string(), 2)),
            entries: vec![RecentContextEntryView {
                title: "Morning Thoughts".to_string(),
                path: "journal/2025-01-15.md".to_string(),
                date: "2025-01-15".to_string(),
                tags: vec!["reflection".to_string()],
                snippet: "A deep reflection...".to_string(),
            }],
        });

        assert!(out.contains("Recent Activity (last 7 days)"));
        assert!(out.contains("5 entries | 2 unique tags"));
        assert!(out.contains("Days without entries: 2025-01-11"));
        assert!(out.contains("Morning Thoughts"));
        assert!(out.contains("#reflection"));
        assert!(out.contains("Top Tags"));
        assert!(out.contains("New Tags"));
        assert!(out.contains("#meditation"));
    }

    #[test]
    fn no_gaps_and_no_new_tags_have_fallback_messages() {
        setup_locale();
        let out = format_recent_context(&RecentContextView {
            days: 1,
            total_current_entries: 1,
            previous_period_entries: 0,
            tag_count: 0,
            top_tags: vec![],
            new_tags: vec![],
            gap_dates: vec![],
            longest: Some(("A".to_string(), 1)),
            shortest: Some(("A".to_string(), 1)),
            entries: vec![RecentContextEntryView {
                title: "A".to_string(),
                path: "a.md".to_string(),
                date: "2025-01-15".to_string(),
                tags: vec![],
                snippet: "text".to_string(),
            }],
        });

        assert!(out.contains("No gaps"));
        assert!(out.contains("No new tags"));
    }
}
