use ignore::WalkBuilder;
use kajet_backend::hasher::hash_content;
use kajet_core::types::FileChange;
use std::collections::HashMap;
use std::path::Path;

/// Detect changes between the filesystem and stored document hashes.
/// Uses parallel walking via the `ignore` crate. Respects `.gitignore`.
pub fn detect_changes(
    vault_path: &Path,
    exclude_folders: &[String],
    stored_hashes: &HashMap<String, String>,
) -> anyhow::Result<Vec<FileChange>> {
    let mut changes = Vec::new();
    let mut seen_paths = std::collections::HashSet::new();

    // Collect files via parallel walker into channel
    let (tx, rx) = std::sync::mpsc::channel();
    let vault_str = vault_path.to_string_lossy().to_string();
    let excludes: Vec<String> = exclude_folders.to_vec();

    let mut builder = WalkBuilder::new(vault_path);
    builder.standard_filters(true);

    builder.build_parallel().run(|| {
        let tx = tx.clone();
        let vault_str = vault_str.clone();
        let excludes = excludes.clone();
        Box::new(move |entry| {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => return ignore::WalkState::Continue,
            };

            let path = entry.path();

            if path.is_dir() {
                let name = path.file_name().map(|n| n.to_string_lossy().to_string());
                if let Some(name) = name {
                    if excludes.iter().any(|f| f == &name) {
                        return ignore::WalkState::Skip;
                    }
                }
                return ignore::WalkState::Continue;
            }

            if path.extension().is_none_or(|ext| ext != "md") {
                return ignore::WalkState::Continue;
            }

            let rel_path = path
                .strip_prefix(&vault_str)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();

            let _ = tx.send((path.to_path_buf(), rel_path));
            ignore::WalkState::Continue
        })
    });

    drop(tx);

    for (abs_path, rel_path) in rx {
        seen_paths.insert(rel_path.clone());

        match stored_hashes.get(&rel_path) {
            None => {
                changes.push(FileChange::Added(abs_path));
            }
            Some(stored_hash) => {
                let content = std::fs::read_to_string(&abs_path)?;
                let current_hash = hash_content(&content);
                if &current_hash != stored_hash {
                    changes.push(FileChange::Modified(abs_path));
                } else {
                    tracing::debug!(path = %rel_path, reason = "unchanged hash", "file:skip");
                }
            }
        }
    }

    // Find deleted files
    for path in stored_hashes.keys() {
        if !seen_paths.contains(path) {
            changes.push(FileChange::Deleted(path.clone()));
        }
    }

    Ok(changes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn create_test_vault(dir: &Path) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join("note1.md"), "# Note 1\n\nContent 1").unwrap();
        fs::write(dir.join("note2.md"), "# Note 2\n\nContent 2").unwrap();
    }

    #[test]
    fn all_files_added_when_no_stored_hashes() {
        let dir = tempfile::tempdir().unwrap();
        create_test_vault(dir.path());

        let changes = detect_changes(dir.path(), &[], &HashMap::new()).unwrap();
        let added_count = changes
            .iter()
            .filter(|c| matches!(c, FileChange::Added(_)))
            .count();
        assert_eq!(added_count, 2);
    }

    #[test]
    fn unchanged_files_not_reported() {
        let dir = tempfile::tempdir().unwrap();
        create_test_vault(dir.path());

        let mut hashes = HashMap::new();
        hashes.insert(
            "note1.md".to_string(),
            hash_content("# Note 1\n\nContent 1"),
        );
        hashes.insert(
            "note2.md".to_string(),
            hash_content("# Note 2\n\nContent 2"),
        );

        let changes = detect_changes(dir.path(), &[], &hashes).unwrap();
        assert!(changes.is_empty());
    }

    #[test]
    fn modified_file_detected() {
        let dir = tempfile::tempdir().unwrap();
        create_test_vault(dir.path());

        let mut hashes = HashMap::new();
        hashes.insert("note1.md".to_string(), hash_content("old content"));
        hashes.insert(
            "note2.md".to_string(),
            hash_content("# Note 2\n\nContent 2"),
        );

        let changes = detect_changes(dir.path(), &[], &hashes).unwrap();
        let modified: Vec<_> = changes
            .iter()
            .filter(|c| matches!(c, FileChange::Modified(_)))
            .collect();
        assert_eq!(modified.len(), 1);
    }

    #[test]
    fn deleted_file_detected() {
        let dir = tempfile::tempdir().unwrap();
        create_test_vault(dir.path());

        let mut hashes = HashMap::new();
        hashes.insert(
            "note1.md".to_string(),
            hash_content("# Note 1\n\nContent 1"),
        );
        hashes.insert(
            "note2.md".to_string(),
            hash_content("# Note 2\n\nContent 2"),
        );
        hashes.insert("deleted.md".to_string(), hash_content("gone"));

        let changes = detect_changes(dir.path(), &[], &hashes).unwrap();
        let deleted: Vec<_> = changes
            .iter()
            .filter(|c| matches!(c, FileChange::Deleted(_)))
            .collect();
        assert_eq!(deleted.len(), 1);
    }

    #[test]
    fn exclude_folders_respected() {
        let dir = tempfile::tempdir().unwrap();
        create_test_vault(dir.path());
        let hidden = dir.path().join(".obsidian");
        fs::create_dir_all(&hidden).unwrap();
        fs::write(hidden.join("config.md"), "should be ignored").unwrap();

        let changes =
            detect_changes(dir.path(), &[".obsidian".to_string()], &HashMap::new()).unwrap();
        let added_count = changes
            .iter()
            .filter(|c| matches!(c, FileChange::Added(_)))
            .count();
        assert_eq!(added_count, 2); // Only note1.md and note2.md
    }
}
