/// Normalize a folder path to a prefix suitable for `starts_with` checks.
///
/// Example: `journal/2025` -> `journal/2025/`.
pub fn normalize_folder_prefix(folder: &str) -> String {
    if folder.ends_with('/') {
        folder.to_string()
    } else {
        format!("{folder}/")
    }
}

/// Check whether `path` belongs to `folder_prefix`.
///
/// - When `folder_prefix` is `None`, always returns `true`.
/// - When `recursive` is `false`, only direct children of the folder match.
pub fn path_matches_folder_prefix(
    path: &str,
    folder_prefix: Option<&str>,
    recursive: bool,
) -> bool {
    let Some(prefix) = folder_prefix else {
        return true;
    };

    if !path.starts_with(prefix) {
        return false;
    }

    if !recursive {
        let rest = &path[prefix.len()..];
        return !rest.contains('/');
    }

    true
}

/// Normalize note lookup input by converting Windows separators to `/`.
pub fn normalize_note_lookup_path(path: &str) -> String {
    path.replace('\\', "/")
}

/// Fuzzy-match a candidate note path against query path.
///
/// Matching rules:
/// - exact path (`journal/today.md`)
/// - exact with implicit `.md` (`journal/today`)
/// - suffix match (`today` -> `journal/today.md`)
pub fn note_path_fuzzy_matches(
    candidate_path: &str,
    query_path: &str,
    case_insensitive: bool,
) -> bool {
    let normalized_query = normalize_note_lookup_path(query_path);
    let query = if case_insensitive {
        normalized_query.to_lowercase()
    } else {
        normalized_query
    };
    let candidate = if case_insensitive {
        candidate_path.to_lowercase()
    } else {
        candidate_path.to_string()
    };

    let suffix = format!("/{query}");
    let suffix_md = format!("/{query}.md");
    let query_md = format!("{query}.md");

    candidate == query
        || candidate == query_md
        || candidate.ends_with(&suffix)
        || candidate.ends_with(&suffix_md)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_path_fuzzy_matches_exact_and_suffix() {
        assert!(note_path_fuzzy_matches(
            "journal/2026/today.md",
            "journal/2026/today.md",
            false
        ));
        assert!(note_path_fuzzy_matches(
            "journal/2026/today.md",
            "journal/2026/today",
            false
        ));
        assert!(note_path_fuzzy_matches(
            "journal/2026/today.md",
            "today",
            false
        ));
        assert!(note_path_fuzzy_matches(
            "journal/2026/today.md",
            "today.md",
            false
        ));
    }

    #[test]
    fn note_path_fuzzy_matches_normalizes_windows_separators() {
        assert!(note_path_fuzzy_matches(
            "journal/2026/today.md",
            "journal\\2026\\today",
            false
        ));
    }

    #[test]
    fn note_path_fuzzy_matches_case_insensitive_mode() {
        assert!(note_path_fuzzy_matches(
            "Journal/Today.MD",
            "journal/today",
            true
        ));
        assert!(!note_path_fuzzy_matches(
            "Journal/Today.MD",
            "journal/today",
            false
        ));
    }
}
