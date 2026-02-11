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
