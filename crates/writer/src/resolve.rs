use anyhow::{Result, bail};
use kajet_core::path_utils::{normalize_note_lookup_path, note_path_fuzzy_matches};
use kajet_core::traits::DocumentStore;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

/// A resolved note path (both relative and absolute).
#[derive(Debug, Clone)]
pub struct ResolvedPath {
    pub relative: String,
    pub absolute: PathBuf,
}

/// Resolve a note path within the vault using a 3-step strategy:
///
/// 1. Exact filesystem match (try as-is, then with `.md` suffix)
/// 2. Fuzzy suffix match via DocumentStore index
/// 3. Filesystem walk fallback for unindexed files
pub async fn resolve_note_path(
    target: &str,
    vault_path: &Path,
    doc_store: Option<&Arc<dyn DocumentStore>>,
) -> Result<ResolvedPath> {
    // Reject path traversal
    let normalized = normalize_note_lookup_path(target);
    let path = Path::new(&normalized);
    for component in path.components() {
        if matches!(component, Component::ParentDir) {
            bail!(
                "Path traversal rejected: '{}' contains '..' component",
                target
            );
        }
        if matches!(component, Component::RootDir | Component::Prefix(_)) {
            bail!("Absolute paths are not allowed: '{}'", target);
        }
    }

    // Step 1: exact filesystem match
    let candidates = if normalized.ends_with(".md") {
        vec![normalized.clone()]
    } else {
        vec![normalized.clone(), format!("{normalized}.md")]
    };

    for candidate in &candidates {
        let abs = vault_path.join(candidate);
        if tokio::fs::metadata(&abs).await.is_ok() {
            return Ok(ResolvedPath {
                relative: candidate.clone(),
                absolute: abs,
            });
        }
    }

    // Step 2: fuzzy via DocumentStore
    if let Some(store) = doc_store
        && let Ok(docs) = store.get_all_documents().await
    {
        let matches: Vec<_> = docs
            .iter()
            .filter(|d| note_path_fuzzy_matches(&d.source_file, &normalized, true))
            .collect();

        match matches.len() {
            1 => {
                let rel = matches[0].source_file.clone();
                let abs = vault_path.join(&rel);
                return Ok(ResolvedPath {
                    relative: rel,
                    absolute: abs,
                });
            }
            n if n > 1 => {
                let paths: Vec<_> = matches.iter().map(|d| d.source_file.as_str()).collect();
                bail!(
                    "Ambiguous path '{}': {} matches found: {}",
                    target,
                    n,
                    paths.join(", ")
                );
            }
            _ => {}
        }
    }

    // Step 3: filesystem walk
    if !normalized.contains('/')
        && let Some(found) = walk_for_filename(vault_path, &normalized).await?
    {
        return Ok(found);
    }

    bail!("Note not found: '{}'", target)
}

/// Walk the vault directory looking for a file matching the given name.
async fn walk_for_filename(vault_path: &Path, target: &str) -> Result<Option<ResolvedPath>> {
    let vault = vault_path.to_path_buf();
    let target = target.to_string();

    tokio::task::spawn_blocking(move || {
        use ignore::WalkBuilder;
        use unicode_normalization::UnicodeNormalization;

        let needle = if target.ends_with(".md") {
            target.clone()
        } else {
            format!("{target}.md")
        };
        let needle_nfc: String = needle.nfc().collect();
        let needle_lower = needle_nfc.to_lowercase();

        let walker = WalkBuilder::new(&vault)
            .hidden(true)
            .git_ignore(false)
            .build();

        let mut matches = Vec::new();
        for entry in walker.flatten() {
            if !entry.file_type().is_some_and(|ft| ft.is_file()) {
                continue;
            }
            let path = entry.path();
            let file_name: String = path
                .file_name()
                .map(|n| n.to_string_lossy().nfc().collect::<String>())
                .unwrap_or_default();
            if file_name.to_lowercase() == needle_lower
                && let Ok(rel) = path.strip_prefix(&vault)
            {
                matches.push(ResolvedPath {
                    relative: rel.to_string_lossy().to_string(),
                    absolute: path.to_path_buf(),
                });
            }
        }

        match matches.len() {
            0 => Ok(None),
            1 => Ok(Some(matches.into_iter().next().unwrap())),
            n => anyhow::bail!(
                "Ambiguous: {} files match '{}': {}",
                n,
                target,
                matches
                    .iter()
                    .map(|m| m.relative.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    })
    .await?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn resolve_exact_path() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path();
        tokio::fs::write(vault.join("note.md"), "# Note")
            .await
            .unwrap();

        let resolved = resolve_note_path("note.md", vault, None).await.unwrap();
        assert_eq!(resolved.relative, "note.md");
        assert!(resolved.absolute.exists());
    }

    #[tokio::test]
    async fn resolve_without_extension() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path();
        tokio::fs::write(vault.join("note.md"), "# Note")
            .await
            .unwrap();

        let resolved = resolve_note_path("note", vault, None).await.unwrap();
        assert_eq!(resolved.relative, "note.md");
    }

    #[tokio::test]
    async fn resolve_nested_path() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path();
        tokio::fs::create_dir_all(vault.join("sub/folder"))
            .await
            .unwrap();
        tokio::fs::write(vault.join("sub/folder/deep.md"), "# Deep")
            .await
            .unwrap();

        let resolved = resolve_note_path("sub/folder/deep", vault, None)
            .await
            .unwrap();
        assert_eq!(resolved.relative, "sub/folder/deep.md");
    }

    #[tokio::test]
    async fn resolve_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let result = resolve_note_path("nonexistent", dir.path(), None).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn resolve_via_docstore() {
        use kajet_core::traits::mocks::MockDocumentStore;
        use kajet_core::types::Document;

        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path();
        tokio::fs::create_dir_all(vault.join("projects"))
            .await
            .unwrap();
        tokio::fs::write(vault.join("projects/ideas.md"), "# Ideas")
            .await
            .unwrap();

        let store = MockDocumentStore::new();
        store.documents.lock().unwrap().push(Document {
            source_file: "projects/ideas.md".into(),
            full_text: "# Ideas".into(),
            title: "Ideas".into(),
            tags: vec![],
            content_hash: "abc".into(),
            last_modified: 0.0,
            outgoing_links: vec![],
            backlinks: vec![],
        });

        let store: Arc<dyn DocumentStore> = Arc::new(store);
        let resolved = resolve_note_path("ideas", vault, Some(&store))
            .await
            .unwrap();
        assert_eq!(resolved.relative, "projects/ideas.md");
    }

    #[tokio::test]
    async fn resolve_via_docstore_case_insensitive_with_windows_path() {
        use kajet_core::traits::mocks::MockDocumentStore;
        use kajet_core::types::Document;

        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path();
        tokio::fs::create_dir_all(vault.join("projects"))
            .await
            .unwrap();
        tokio::fs::write(vault.join("projects/ideas.md"), "# Ideas")
            .await
            .unwrap();

        let store = MockDocumentStore::new();
        store.documents.lock().unwrap().push(Document {
            source_file: "projects/ideas.md".into(),
            full_text: "# Ideas".into(),
            title: "Ideas".into(),
            tags: vec![],
            content_hash: "abc".into(),
            last_modified: 0.0,
            outgoing_links: vec![],
            backlinks: vec![],
        });

        let store: Arc<dyn DocumentStore> = Arc::new(store);
        let resolved = resolve_note_path("IDEAS", vault, Some(&store))
            .await
            .unwrap();
        assert_eq!(resolved.relative, "projects/ideas.md");
    }
}
