use crate::chunker::chunk_markdown;
use crate::frontmatter::strip_frontmatter;
use crate::types::{Chunk, ChunkConfig};
use ignore::WalkBuilder;
use unicode_normalization::UnicodeNormalization;

/// Read all markdown files from `vault_path` and chunk them.
/// Uses parallel walking via the `ignore` crate. Respects `.gitignore`.
/// Folders in `exclude_folders` (e.g. `.obsidian`, `.trash`) are skipped.
pub fn parse_vault(vault_path: &str, exclude_folders: &[String]) -> anyhow::Result<Vec<Chunk>> {
    let config = ChunkConfig::default();
    let entries = scan_vault(vault_path, exclude_folders)?;
    Ok(parse_vault_entries_with_config(&entries, &config))
}

/// Scan vault directory for markdown files using parallel walking.
/// Returns `(relative_path, content)` pairs.
pub fn scan_vault(
    vault_path: &str,
    exclude_folders: &[String],
) -> anyhow::Result<Vec<(String, String)>> {
    let (tx, rx) = std::sync::mpsc::channel();

    let mut builder = WalkBuilder::new(vault_path);
    builder.standard_filters(true); // respect .gitignore

    let vault = vault_path.to_string();
    let excludes: Vec<String> = exclude_folders.to_vec();

    builder.build_parallel().run(|| {
        let tx = tx.clone();
        let vault = vault.clone();
        let excludes = excludes.clone();
        Box::new(move |entry| {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => return ignore::WalkState::Continue,
            };

            let path = entry.path();

            // Skip excluded folders
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

            if let Ok(content) = std::fs::read_to_string(path) {
                let rel = path
                    .strip_prefix(&vault)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .nfc()
                    .collect::<String>();
                let _ = tx.send((rel, content));
            }

            ignore::WalkState::Continue
        })
    });

    drop(tx);
    Ok(rx.into_iter().collect())
}

/// Chunk a list of `(relative_path, markdown_content)` pairs.
/// Pure function — no filesystem access.
pub fn parse_vault_entries(entries: &[(String, String)]) -> Vec<Chunk> {
    parse_vault_entries_with_config(entries, &ChunkConfig::default())
}

pub fn parse_vault_entries_with_config(
    entries: &[(String, String)],
    config: &ChunkConfig,
) -> Vec<Chunk> {
    entries
        .iter()
        .flat_map(|(path, content)| {
            let (_, body) = strip_frontmatter(content);
            chunk_markdown(path, body, config)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_vault_entries_processes_multiple_files() {
        let entries = vec![
            (
                "a.md".to_string(),
                "# A\n\nLorem ipsum dolor sit amet, consectetur adipiscing elit sed do eiusmod."
                    .to_string(),
            ),
            (
                "b.md".to_string(),
                "# B\n\nUt enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi."
                    .to_string(),
            ),
        ];
        let chunks = parse_vault_entries(&entries);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].note_path, "a.md");
        assert_eq!(chunks[1].note_path, "b.md");
    }

    #[test]
    fn scan_vault_normalizes_nfd_paths_to_nfc() {
        use unicode_normalization::UnicodeNormalization;

        let dir = tempfile::tempdir().unwrap();
        // "ę" in NFD = e + combining ogonek
        let nfd_name = "note\u{0328}.md";
        let nfc_name: String = nfd_name.nfc().collect();
        std::fs::write(
            dir.path().join(nfd_name),
            "# Test\n\nContent with diacritics in filename",
        )
        .unwrap();

        let entries = scan_vault(&dir.path().to_string_lossy(), &[]).unwrap();
        assert_eq!(entries.len(), 1);

        let (path, _content) = &entries[0];
        assert_eq!(
            path,
            &nfc_name,
            "scan_vault should return NFC-normalized paths, got {:?}",
            path.chars()
                .map(|c| format!("U+{:04X}", c as u32))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn scan_vault_polish_subfolder_nfc() {
        use unicode_normalization::UnicodeNormalization;

        let dir = tempfile::tempdir().unwrap();
        // "Źródła" folder with NFD characters
        let nfd_folder = "Z\u{0301}ro\u{0301}dl\u{0142}a";
        let subdir = dir.path().join(nfd_folder);
        std::fs::create_dir_all(&subdir).unwrap();
        std::fs::write(subdir.join("test.md"), "# Test\n\nContent").unwrap();

        let entries = scan_vault(&dir.path().to_string_lossy(), &[]).unwrap();
        assert_eq!(entries.len(), 1);

        let (path, _) = &entries[0];
        let nfc_expected: String = format!("{nfd_folder}/test.md").nfc().collect();
        assert_eq!(
            path, &nfc_expected,
            "subfolder path should be NFC-normalized"
        );
    }

    #[test]
    fn parse_vault_entries_empty_input() {
        let chunks = parse_vault_entries(&[]);
        assert!(chunks.is_empty());
    }

    #[test]
    fn parse_vault_entries_filters_empty_files() {
        let entries = vec![
            ("empty.md".to_string(), "".to_string()),
            (
                "has_content.md".to_string(),
                "Lorem ipsum dolor sit amet, consectetur adipiscing elit sed do eiusmod."
                    .to_string(),
            ),
        ];
        let chunks = parse_vault_entries(&entries);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].note_path, "has_content.md");
    }
}
