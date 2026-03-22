use ignore::WalkBuilder;
use kajet_backend::hasher::hash_content;
use kajet_core::traits::StoredFileInfo;
use kajet_core::types::FileChange;
use std::collections::HashMap;
use std::path::Path;
use unicode_normalization::UnicodeNormalization;

/// Detect changes between the filesystem and stored document hashes.
/// Uses parallel walking via the `ignore` crate. Respects `.gitignore`.
/// Uses mtime as a pre-filter to avoid reading file contents when unchanged
/// (important for iCloud-synced vaults where file reads are slow).
pub fn detect_changes(
    vault_path: &Path,
    exclude_folders: &[String],
    stored_hashes: &HashMap<String, StoredFileInfo>,
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
                if let Some(name) = name
                    && excludes.iter().any(|f| f == &name)
                {
                    return ignore::WalkState::Skip;
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
                .nfc()
                .collect::<String>();

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
            Some(stored_info) => {
                // Fast path: check mtime via metadata (no file read needed)
                if let Ok(metadata) = std::fs::metadata(&abs_path)
                    && let Ok(modified) = metadata.modified()
                {
                    let mtime = modified
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs_f64();
                    if (mtime - stored_info.last_modified).abs() < 1.0 {
                        tracing::debug!(path = %rel_path, "file:skip");
                        continue;
                    }
                }
                // Slow path: mtime changed, read + hash to confirm
                let content = std::fs::read_to_string(&abs_path)?;
                let current_hash = hash_content(&content);
                if current_hash != stored_info.content_hash {
                    changes.push(FileChange::Modified(abs_path));
                } else {
                    tracing::debug!(path = %rel_path, reason = "unchanged hash, mtime drift", "file:skip");
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

    fn file_mtime(path: &Path) -> f64 {
        fs::metadata(path)
            .unwrap()
            .modified()
            .unwrap()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64()
    }

    fn stored(content: &str, mtime: f64) -> StoredFileInfo {
        StoredFileInfo {
            content_hash: hash_content(content),
            last_modified: mtime,
        }
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
            stored(
                "# Note 1\n\nContent 1",
                file_mtime(&dir.path().join("note1.md")),
            ),
        );
        hashes.insert(
            "note2.md".to_string(),
            stored(
                "# Note 2\n\nContent 2",
                file_mtime(&dir.path().join("note2.md")),
            ),
        );

        let changes = detect_changes(dir.path(), &[], &hashes).unwrap();
        assert!(changes.is_empty());
    }

    #[test]
    fn modified_file_detected() {
        let dir = tempfile::tempdir().unwrap();
        create_test_vault(dir.path());

        let mut hashes = HashMap::new();
        // note1 has wrong hash and old mtime → should be detected as modified
        hashes.insert("note1.md".to_string(), stored("old content", 0.0));
        hashes.insert(
            "note2.md".to_string(),
            stored(
                "# Note 2\n\nContent 2",
                file_mtime(&dir.path().join("note2.md")),
            ),
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
            stored(
                "# Note 1\n\nContent 1",
                file_mtime(&dir.path().join("note1.md")),
            ),
        );
        hashes.insert(
            "note2.md".to_string(),
            stored(
                "# Note 2\n\nContent 2",
                file_mtime(&dir.path().join("note2.md")),
            ),
        );
        hashes.insert("deleted.md".to_string(), stored("gone", 0.0));

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
        assert_eq!(added_count, 2);
    }

    #[test]
    fn mtime_unchanged_skips_content_read() {
        let dir = tempfile::tempdir().unwrap();
        create_test_vault(dir.path());

        // Store with correct hash AND matching mtime → should skip (fast path)
        let mut hashes = HashMap::new();
        hashes.insert(
            "note1.md".to_string(),
            stored(
                "# Note 1\n\nContent 1",
                file_mtime(&dir.path().join("note1.md")),
            ),
        );
        hashes.insert(
            "note2.md".to_string(),
            stored(
                "# Note 2\n\nContent 2",
                file_mtime(&dir.path().join("note2.md")),
            ),
        );

        let changes = detect_changes(dir.path(), &[], &hashes).unwrap();
        assert!(changes.is_empty());
    }

    #[test]
    fn nfd_filename_detected_as_nfc() {
        use unicode_normalization::UnicodeNormalization;

        let dir = tempfile::tempdir().unwrap();
        // Create file with NFD name: "ę" = e + combining ogonek
        let nfd_name = "note\u{0328}.md";
        let nfc_name: String = nfd_name.nfc().collect();

        fs::write(
            dir.path().join(nfd_name),
            "# NFD test\n\nContent with diacritics",
        )
        .unwrap();

        let changes = detect_changes(dir.path(), &[], &HashMap::new()).unwrap();
        assert_eq!(changes.len(), 1);

        // The returned rel_path (inside Added) won't be directly accessible,
        // but we can verify via stored_hashes lookup: NFC key should match
        let mut hashes = HashMap::new();
        hashes.insert(
            nfc_name.clone(),
            stored(
                "# NFD test\n\nContent with diacritics",
                file_mtime(&dir.path().join(nfd_name)),
            ),
        );
        let changes = detect_changes(dir.path(), &[], &hashes).unwrap();
        assert!(
            changes.is_empty(),
            "NFC stored hash key should match NFC-normalized filesystem path, got {:?}",
            changes
        );
    }

    #[test]
    fn nfd_polish_path_matches_nfc_stored_hash() {
        use unicode_normalization::UnicodeNormalization;

        let dir = tempfile::tempdir().unwrap();
        let subdir = dir.path().join("Dzienniki");
        fs::create_dir_all(&subdir).unwrap();

        // "ść" in NFD: s + combining acute, c + combining acute
        let nfd_name = "nos\u{0301}c\u{0301}.md";
        let content = "# Polish test\n\nTreść po polsku z ogonkami";

        fs::write(subdir.join(nfd_name), content).unwrap();

        // Store hash with NFC key — should match after normalization
        let nfc_rel: String = format!("Dzienniki/{nfd_name}").nfc().collect();
        let mut hashes = HashMap::new();
        hashes.insert(nfc_rel, stored(content, file_mtime(&subdir.join(nfd_name))));

        let changes = detect_changes(dir.path(), &[], &hashes).unwrap();
        assert!(
            changes.is_empty(),
            "Polish NFD path should match NFC stored key"
        );
    }

    #[test]
    fn nfd_stored_hash_does_not_match_nfc_path() {
        // Opposite direction: if someone had NFD keys in stored_hashes (old bug),
        // they should NOT match — verifying our normalization breaks that pattern,
        // forcing a reindex
        use unicode_normalization::UnicodeNormalization;

        let dir = tempfile::tempdir().unwrap();
        let nfd_name = "note\u{0328}.md";
        let nfc_name: String = nfd_name.nfc().collect();
        let content = "# Test\n\nContent";

        fs::write(dir.path().join(nfd_name), content).unwrap();

        // Store with explicit NFD key (simulating old bug)
        let nfd_key = "note\u{0328}.md".to_string();
        if nfd_key == nfc_name {
            // On some systems NFD == NFC for this char, skip test
            return;
        }
        let mut hashes = HashMap::new();
        hashes.insert(
            nfd_key,
            stored(content, file_mtime(&dir.path().join(nfd_name))),
        );

        let changes = detect_changes(dir.path(), &[], &hashes).unwrap();
        // Should see Added (NFC path not found in NFD-keyed hashes) + Deleted (NFD key not on disk)
        assert!(
            !changes.is_empty(),
            "NFD stored key should NOT match NFC-normalized path"
        );
    }

    #[test]
    fn mtime_changed_but_content_same_skips() {
        let dir = tempfile::tempdir().unwrap();
        create_test_vault(dir.path());

        // Store with correct hash but OLD mtime → mtime check fails, reads file,
        // hash matches → no change reported
        let mut hashes = HashMap::new();
        hashes.insert("note1.md".to_string(), stored("# Note 1\n\nContent 1", 0.0));
        hashes.insert("note2.md".to_string(), stored("# Note 2\n\nContent 2", 0.0));

        let changes = detect_changes(dir.path(), &[], &hashes).unwrap();
        assert!(changes.is_empty());
    }
}
