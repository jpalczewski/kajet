/// Strip list markers from the start of a line.
/// Returns the remainder after the marker, or the original line if no marker found.
#[allow(dead_code)] // Used in later tasks
pub(crate) fn strip_list_marker(line: &str) -> &str {
    // First trim leading whitespace (to match original behavior in chunker.rs:204)
    let trimmed = line.trim_start();

    // Try unordered list markers: "- " or "* "
    if let Some(rest) = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
    {
        return rest;
    }

    // Try numbered list markers: "1. " or "1) "
    let after_digits = trimmed.trim_start_matches(|c: char| c.is_ascii_digit());
    if after_digits.len() < trimmed.len() {
        // We found at least one digit
        if let Some(rest) = after_digits
            .strip_prefix(". ")
            .or_else(|| after_digits.strip_prefix(") "))
        {
            return rest;
        }
    }

    // No marker found, return trimmed line
    trimmed
}

/// Count non-whitespace "prose" characters remaining after stripping wikilinks
/// and list markers. Used to detect link-only chunks that carry no semantic value.
pub(crate) fn prose_char_count(raw: &str) -> usize {
    let without_links = crate::wikilinks::WIKILINK_RE.replace_all(raw, "");
    without_links
        .lines()
        .map(|line| {
            let after_marker = strip_list_marker(line.trim_start());
            after_marker.chars().filter(|c| !c.is_whitespace()).count()
        })
        .sum()
}

use std::sync::LazyLock;

#[allow(dead_code)] // Used in later tasks
static CHANGELOG_DATE_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^(\[\[)?\d{4}-\d{2}-\d{2}(\]\])?").unwrap());

/// Detect changelog-like sections: >=80% of non-empty lines (after stripping
/// list markers) start with a date pattern (bare or wikilinked YYYY-MM-DD).
#[allow(dead_code)] // Used in later tasks
pub(crate) fn is_changelog_section(raw: &str) -> bool {
    let lines: Vec<&str> = raw
        .lines()
        .map(|l| strip_list_marker(l.trim_start()))
        .filter(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        return false;
    }
    let date_count = lines
        .iter()
        .filter(|l| CHANGELOG_DATE_RE.is_match(l))
        .count();
    date_count as f32 / lines.len() as f32 >= 0.8
}

use crate::types::ChunkConfig;

/// Returns true if a section should be excluded from chunking.
/// Checks: (1) low prose content (link-only sections), (2) changelog patterns.
#[allow(dead_code)] // Used in later tasks
pub(crate) fn should_skip_section(raw: &str, config: &ChunkConfig) -> bool {
    if config.min_content_chars > 0 && prose_char_count(raw) < config.min_content_chars {
        return true;
    }
    is_changelog_section(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_dash_marker() {
        assert_eq!(strip_list_marker("- item text"), "item text");
    }

    #[test]
    fn strip_star_marker() {
        assert_eq!(strip_list_marker("* item text"), "item text");
    }

    #[test]
    fn strip_numbered_dot_marker() {
        assert_eq!(strip_list_marker("1. first item"), "first item");
        assert_eq!(strip_list_marker("12. twelfth item"), "twelfth item");
    }

    #[test]
    fn strip_numbered_paren_marker() {
        assert_eq!(strip_list_marker("1) first item"), "first item");
        assert_eq!(strip_list_marker("2) second item"), "second item");
    }

    #[test]
    fn no_marker_returns_original() {
        assert_eq!(strip_list_marker("plain text"), "plain text");
        assert_eq!(strip_list_marker(""), "");
    }

    #[test]
    fn dash_without_space_not_stripped() {
        assert_eq!(strip_list_marker("-no space"), "-no space");
    }

    #[test]
    fn strip_marker_with_leading_whitespace() {
        assert_eq!(strip_list_marker("  - item text"), "item text");
        assert_eq!(strip_list_marker("\t* item text"), "item text");
    }

    #[test]
    fn prose_char_count_link_only_list() {
        let raw = "- [[Rewolucja Doloriańska]]\n- [[Bled Przeklęty]]\n- [[Komuna Martinaise]]";
        assert!(
            prose_char_count(raw) < 10,
            "link-only list should have near-zero prose"
        );
    }

    #[test]
    fn prose_char_count_mixed_prose_and_links() {
        let raw = "Albowiem sprawa ta, przez ([[Duch Rewolucji|wieki]]) skryta w mrokach Bledy, wyłoniła się nagle z odmętów niczym okręt widmo.";
        assert!(
            prose_char_count(raw) > 50,
            "prose with links should count the surrounding text"
        );
    }

    #[test]
    fn prose_char_count_numbered_list_of_links() {
        let raw = "1. [[Towarzysze z portu]]\n2. [[Bled od północy]]\n3. [[Komitet Rewolucyjny]]";
        assert!(prose_char_count(raw) < 10);
    }

    #[test]
    fn prose_char_count_plain_text() {
        let raw = "Oto wszak Revachol stoi zasię na skraju przepaści, a Bled zbliża się od północy nieodparcie.";
        assert_eq!(
            prose_char_count(raw),
            raw.chars().filter(|c| !c.is_whitespace()).count()
        );
    }

    #[test]
    fn changelog_pure_date_lines() {
        let raw = "- 2026-02-06: Akta sprawy otwarte, vide protokół\n- 2026-02-14 00:15: Zaktualizowane po odprawie à Martinaise";
        assert!(is_changelog_section(raw));
    }

    #[test]
    fn changelog_wikilinked_dates() {
        let raw = "- [[2026-01-31]]: Pierwsza ekspedycja na le Pale\n- [[2026-02-06]]: Rewizja w porcie Revachol";
        assert!(is_changelog_section(raw));
    }

    #[test]
    fn changelog_single_date_line() {
        let raw = "- 2026-02-06: Notatki z przesłuchania, na rozkaz porucznika Kitsuragi";
        assert!(is_changelog_section(raw));
    }

    #[test]
    fn changelog_mixed_dates_and_wikilink_dates() {
        let raw = "- 2026-02-06: Otwarte\n- [[2026-01-31]]: Rekonesans, ничего nie znaleziono\n- 2026-02-14 00:15: Zaktualizowane";
        assert!(is_changelog_section(raw));
    }

    #[test]
    fn not_changelog_narrative_with_one_date() {
        let raw = "2026-02-06 roku pękło coś w onéj skowanej pamięci detektywa.\nStał w zaułku Martinaise jak człek bez miana i bez przeszłości.\nTovarisch Kitsuragi rzekł cicho: ne teryaj golovu, Harrier.\nQuelque chose zamknęło się wówczas we wnętrzu i nie otworzyło zasię.";
        assert!(!is_changelog_section(raw));
    }

    #[test]
    fn not_changelog_prose_section() {
        let raw = "Bled zstępuje na Revachol od północy, jeno skrywszy oblicze za mgłą niepamiętania.\nOficerowie RCM donoszą o anomaliach, gdyż granica Bledy przesunęła się zasię ku centrum.";
        assert!(!is_changelog_section(raw));
    }

    #[test]
    fn not_changelog_empty_section() {
        assert!(!is_changelog_section(""));
        assert!(!is_changelog_section("   \n  \n  "));
    }

    #[test]
    fn not_changelog_links_with_comments() {
        let raw = "- [[Measurehead]] - zawał ideologiczny, źródło przekonania że klasa est sans discipline\n- [[Joyce Messier]] - dzierży klucze do portu, levier politique nad związkiem";
        assert!(!is_changelog_section(raw));
    }

    use crate::types::ChunkConfig;

    #[test]
    fn should_skip_link_only_section() {
        let config = ChunkConfig::default(); // min_content_chars: 50
        let raw = "- [[Rewolucja Doloriańska]]\n- [[Bled Przeklęty]]\n- [[Komuna Martinaise]]";
        assert!(should_skip_section(raw, &config));
    }

    #[test]
    fn should_skip_changelog_section() {
        let config = ChunkConfig::default();
        let raw =
            "- 2026-02-06: Akta otwarte w Martinaise\n- 2026-02-14: Zaktualizowane po odprawie";
        assert!(should_skip_section(raw, &config));
    }

    #[test]
    fn should_keep_prose_section() {
        let config = ChunkConfig::default();
        let raw = "Bled zstępuje na Revachol od północy, jeno skrywszy oblicze za gęstą mgłą niepamiętania, albowiem kończy się czas Rewolucji.";
        assert!(!should_skip_section(raw, &config));
    }

    #[test]
    fn should_keep_links_with_prose_comments() {
        let config = ChunkConfig::default();
        let raw = "- [[Measurehead]] - zawał ideologiczny, źródło przekonania że klasa robotnicza est sans discipline i bez przyszłości\n- [[Joyce Messier]] - dzierży klucze do portu, levier politique над związkiem zawodowym marynarzy";
        assert!(!should_skip_section(raw, &config));
    }

    #[test]
    fn should_skip_disabled_when_min_chars_zero() {
        let config = ChunkConfig {
            min_content_chars: 0,
            ..ChunkConfig::default()
        };
        // Link-only section passes because prose filter is disabled
        // but changelog filter still applies
        let changelog = "- 2026-02-06: Akta otwarte w Martinaise, vide protokół";
        assert!(should_skip_section(changelog, &config));

        let links = "- [[Rewolucja]]\n- [[Bled]]";
        assert!(!should_skip_section(links, &config));
    }
}
