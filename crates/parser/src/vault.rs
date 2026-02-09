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

/// Entry from a vault filesystem walk.
pub struct WalkEntry {
    /// NFC-normalized path relative to vault root.
    pub rel_path: String,
    /// Whether this entry is a directory.
    pub is_dir: bool,
}

/// Display mode for vault tree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShowMode {
    Folders,
    Files,
}

impl std::fmt::Display for ShowMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShowMode::Folders => write!(f, "folders"),
            ShowMode::Files => write!(f, "files"),
        }
    }
}

/// A folder node in the vault tree.
#[derive(Debug)]
pub struct VaultFolder {
    /// Folder name (last path segment), or vault name for root.
    pub name: String,
    /// Full relative path from vault root. Empty string for root.
    pub rel_path: String,
    /// Number of .md files directly in this folder.
    pub note_count: usize,
    /// Child folders (sorted alphabetically).
    pub subfolders: Vec<VaultFolder>,
    /// File names (only populated when show=Files).
    pub files: Vec<String>,
    /// Total notes in entire vault (only set for root node, None for children).
    /// Used to show "showing X of Y" when tree is truncated.
    pub total_notes_in_vault: Option<usize>,
}

/// Options for building vault tree.
pub struct VaultTreeOptions {
    /// Subfolder to start from (None = vault root).
    pub path: Option<String>,
    /// Maximum depth to traverse.
    pub depth: usize,
    /// Maximum total entries in output.
    pub size: usize,
    /// What to show: folders only or folders + files.
    pub show: ShowMode,
}

/// Walk vault directory collecting dirs and .md files using parallel traversal.
/// Excludes specified folders. Paths are NFC-normalized.
///
/// # Security
///
/// **IMPORTANT**: `vault_path` is NOT validated by this function. Callers MUST
/// validate the path using `path_validation::ensure_within_vault()` before calling
/// this function to prevent path traversal attacks.
pub fn walk_vault(vault_path: &str, exclude_folders: &[String]) -> anyhow::Result<Vec<WalkEntry>> {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut builder = WalkBuilder::new(vault_path);
    builder.standard_filters(true);

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

            if path.is_dir() {
                let name = path.file_name().map(|n| n.to_string_lossy().to_string());
                if let Some(name) = name
                    && excludes.iter().any(|f| f == &name)
                {
                    return ignore::WalkState::Skip;
                }
                // Emit dir entry (skip vault root itself)
                if path != std::path::Path::new(&vault) {
                    let rel = path
                        .strip_prefix(&vault)
                        .unwrap_or(path)
                        .to_string_lossy()
                        .nfc()
                        .collect::<String>();
                    let _ = tx.send(WalkEntry {
                        rel_path: rel,
                        is_dir: true,
                    });
                }
                return ignore::WalkState::Continue;
            }

            if path.extension().is_none_or(|ext| ext != "md") {
                return ignore::WalkState::Continue;
            }

            let rel = path
                .strip_prefix(&vault)
                .unwrap_or(path)
                .to_string_lossy()
                .nfc()
                .collect::<String>();
            let _ = tx.send(WalkEntry {
                rel_path: rel,
                is_dir: false,
            });

            ignore::WalkState::Continue
        })
    });

    drop(tx);
    Ok(rx.into_iter().collect())
}

/// Scan vault directory for markdown files using parallel walking.
/// Returns `(relative_path, content)` pairs.
///
/// # Security
///
/// **IMPORTANT**: `vault_path` is NOT validated by this function. Callers MUST
/// validate the path before calling to prevent path traversal attacks.
pub fn scan_vault(
    vault_path: &str,
    exclude_folders: &[String],
) -> anyhow::Result<Vec<(String, String)>> {
    let entries = walk_vault(vault_path, exclude_folders)?;
    let mut results = Vec::new();
    let vault = std::path::Path::new(vault_path);
    for entry in entries {
        if entry.is_dir {
            continue;
        }
        let full_path = vault.join(&entry.rel_path);
        if let Ok(content) = std::fs::read_to_string(&full_path) {
            results.push((entry.rel_path, content));
        }
    }
    Ok(results)
}

/// Build a tree representation of the vault folder structure.
pub async fn vault_tree(
    vault_path: &str,
    exclude_folders: &[String],
    options: &VaultTreeOptions,
) -> anyhow::Result<VaultFolder> {
    let effective_path = match &options.path {
        Some(sub) => {
            // Validate path before joining
            crate::path_validation::ensure_within_vault(sub)?;

            let p = std::path::Path::new(vault_path).join(sub);

            // Verify canonical path is within vault (handles symlinks)
            crate::path_validation::ensure_canonical_within_vault(
                &p,
                std::path::Path::new(vault_path),
            )
            .await?;

            if !p.is_dir() {
                anyhow::bail!("Path not found: {}", sub);
            }
            p.to_string_lossy().to_string()
        }
        None => vault_path.to_string(),
    };

    let entries = walk_vault(&effective_path, exclude_folders)?;

    // Build tree from flat entries
    let root_name = options
        .path
        .as_deref()
        .and_then(|p| p.rsplit('/').next())
        .unwrap_or_else(|| {
            std::path::Path::new(vault_path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("vault")
        });

    let root_rel = options.path.clone().unwrap_or_default();

    let mut root = VaultFolder {
        name: root_name.to_string(),
        rel_path: root_rel,
        note_count: 0,
        subfolders: Vec::new(),
        files: Vec::new(),
        total_notes_in_vault: None, // Will be set before return
    };

    // Separate dirs and files
    let mut dirs: Vec<&str> = Vec::new();
    let mut files: Vec<&str> = Vec::new();
    for entry in &entries {
        if entry.is_dir {
            dirs.push(&entry.rel_path);
        } else {
            files.push(&entry.rel_path);
        }
    }

    // Count notes per directory and collect file names
    // Files in root (no '/' in rel_path)
    for file in &files {
        let parts: Vec<&str> = file.rsplitn(2, '/').collect();
        if parts.len() == 1 {
            // Root file
            root.note_count += 1;
            if options.show == ShowMode::Files {
                root.files.push(file.to_string());
            }
        }
    }

    // Sort dirs to build tree top-down
    dirs.sort();

    // Build folder map: rel_path -> VaultFolder
    let mut folder_map: std::collections::HashMap<String, VaultFolder> =
        std::collections::HashMap::new();

    for dir in &dirs {
        let depth = dir.matches('/').count() + 1;
        if depth > options.depth {
            continue;
        }

        let name = dir.rsplit('/').next().unwrap_or(dir).to_string();
        let full_rel = if root.rel_path.is_empty() {
            dir.to_string()
        } else {
            format!("{}/{}", root.rel_path, dir)
        };

        let mut folder = VaultFolder {
            name,
            rel_path: full_rel,
            note_count: 0,
            subfolders: Vec::new(),
            files: Vec::new(),
            total_notes_in_vault: None, // Only root has this
        };

        // Count files in this folder
        for file in &files {
            if let Some(parent) = file.rsplit_once('/').map(|(p, _)| p)
                && parent == *dir
            {
                folder.note_count += 1;
                if options.show == ShowMode::Files {
                    let fname = file.rsplit('/').next().unwrap_or(file);
                    folder.files.push(fname.to_string());
                }
            }
        }

        folder.files.sort();
        folder_map.insert(dir.to_string(), folder);
    }

    // Build tree by attaching children to parents (bottom-up from deepest)
    let mut sorted_dirs: Vec<String> = folder_map.keys().cloned().collect();
    sorted_dirs.sort_by(|a, b| {
        b.matches('/')
            .count()
            .cmp(&a.matches('/').count())
            .then(a.cmp(b))
    });

    for dir in sorted_dirs {
        let folder = folder_map.remove(&dir).unwrap();
        if let Some(parent) = dir.rsplit_once('/').map(|(p, _)| p.to_string()) {
            if let Some(parent_folder) = folder_map.get_mut(&parent) {
                parent_folder.subfolders.push(folder);
            }
            // else: parent beyond depth, skip
        } else {
            // Top-level folder
            root.subfolders.push(folder);
        }
    }

    // Sort subfolders alphabetically at all levels
    sort_subfolders(&mut root);

    // Apply size limit
    root.files.sort();

    // Store total notes before truncation (for "showing X of Y" header)
    let total = count_notes_recursive(&root);
    truncate_tree(&mut root, options.size);
    root.total_notes_in_vault = Some(total);

    Ok(root)
}

fn count_notes_recursive(folder: &VaultFolder) -> usize {
    folder.note_count
        + folder
            .subfolders
            .iter()
            .map(count_notes_recursive)
            .sum::<usize>()
}

fn sort_subfolders(folder: &mut VaultFolder) {
    folder.subfolders.sort_by(|a, b| a.name.cmp(&b.name));
    for sub in &mut folder.subfolders {
        sort_subfolders(sub);
    }
}

/// Truncate tree to at most `max_entries` total entries (folders + files).
///
/// Walks the tree depth-first, keeping subfolders until the budget is exhausted.
/// Returns the total number of entries kept (folders + files).
///
/// Counting strategy:
/// - Each file in `folder.files` counts as 1
/// - Each subfolder counts as 1 + all its children (recursive)
/// - Stops adding subfolders when count reaches `max_entries`
fn truncate_tree(folder: &mut VaultFolder, max_entries: usize) -> usize {
    let mut count = folder.files.len();
    let mut kept_subs = Vec::new();
    for mut sub in folder.subfolders.drain(..) {
        if count >= max_entries {
            break;
        }
        count += 1; // the folder itself
        let sub_used = truncate_tree(&mut sub, max_entries - count);
        count += sub_used;
        kept_subs.push(sub);
    }
    folder.subfolders = kept_subs;
    if folder.files.len() + count > max_entries {
        folder.files.truncate(max_entries.saturating_sub(count));
    }
    count
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

    #[test]
    fn walk_vault_returns_dirs_and_md_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("sub")).unwrap();
        std::fs::write(dir.path().join("root.md"), "# Root").unwrap();
        std::fs::write(dir.path().join("sub/child.md"), "# Child").unwrap();
        std::fs::write(dir.path().join("sub/ignore.txt"), "not md").unwrap();

        let entries = walk_vault(&dir.path().to_string_lossy(), &[]).unwrap();
        let dirs: Vec<_> = entries.iter().filter(|e| e.is_dir).collect();
        let files: Vec<_> = entries.iter().filter(|e| !e.is_dir).collect();

        assert_eq!(dirs.len(), 1); // "sub"
        assert_eq!(files.len(), 2); // root.md, sub/child.md
        assert!(files.iter().all(|e| e.rel_path.ends_with(".md")));
    }

    #[test]
    fn walk_vault_excludes_folders() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".obsidian")).unwrap();
        std::fs::create_dir_all(dir.path().join("keep")).unwrap();
        std::fs::write(dir.path().join(".obsidian/x.md"), "hidden").unwrap();
        std::fs::write(dir.path().join("keep/y.md"), "visible").unwrap();

        let entries = walk_vault(&dir.path().to_string_lossy(), &[".obsidian".into()]).unwrap();

        assert!(entries.iter().all(|e| !e.rel_path.contains(".obsidian")));
        assert!(entries.iter().any(|e| e.rel_path.contains("keep")));
    }

    #[test]
    fn walk_vault_nfc_normalizes_paths() {
        use unicode_normalization::UnicodeNormalization;
        let dir = tempfile::tempdir().unwrap();
        let nfd_name = "note\u{0328}.md";
        std::fs::write(dir.path().join(nfd_name), "# Test").unwrap();

        let entries = walk_vault(&dir.path().to_string_lossy(), &[]).unwrap();
        let nfc: String = nfd_name.nfc().collect();
        assert!(entries.iter().any(|e| e.rel_path == nfc));
    }

    #[tokio::test]
    async fn vault_tree_basic_structure() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("projects/kajet")).unwrap();
        std::fs::create_dir_all(dir.path().join("journal")).unwrap();
        std::fs::write(dir.path().join("root.md"), "# Root").unwrap();
        std::fs::write(dir.path().join("projects/a.md"), "# A").unwrap();
        std::fs::write(dir.path().join("projects/kajet/b.md"), "# B").unwrap();
        std::fs::write(dir.path().join("journal/c.md"), "# C").unwrap();

        let opts = VaultTreeOptions {
            path: None,
            depth: 10,
            size: 100,
            show: ShowMode::Folders,
        };
        let tree = vault_tree(&dir.path().to_string_lossy(), &[], &opts)
            .await
            .unwrap();

        assert_eq!(tree.note_count, 1); // root.md
        assert_eq!(tree.subfolders.len(), 2); // journal, projects
        assert!(tree.files.is_empty()); // show=Folders
        let projects = tree
            .subfolders
            .iter()
            .find(|f| f.name == "projects")
            .unwrap();
        assert_eq!(projects.note_count, 1); // a.md
        assert_eq!(projects.subfolders.len(), 1); // kajet
    }

    #[tokio::test]
    async fn vault_tree_show_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.md"), "# A").unwrap();
        std::fs::write(dir.path().join("b.md"), "# B").unwrap();

        let opts = VaultTreeOptions {
            path: None,
            depth: 10,
            size: 100,
            show: ShowMode::Files,
        };
        let tree = vault_tree(&dir.path().to_string_lossy(), &[], &opts)
            .await
            .unwrap();

        assert_eq!(tree.files.len(), 2);
        assert_eq!(tree.note_count, 2);
    }

    #[tokio::test]
    async fn vault_tree_depth_limit() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("a/b/c")).unwrap();
        std::fs::write(dir.path().join("a/b/c/deep.md"), "# Deep").unwrap();

        let opts = VaultTreeOptions {
            path: None,
            depth: 1,
            size: 100,
            show: ShowMode::Folders,
        };
        let tree = vault_tree(&dir.path().to_string_lossy(), &[], &opts)
            .await
            .unwrap();

        let a = tree.subfolders.iter().find(|f| f.name == "a").unwrap();
        assert!(a.subfolders.is_empty()); // depth=1, b not shown
    }

    #[tokio::test]
    async fn vault_tree_path_filter() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("journal/2025")).unwrap();
        std::fs::create_dir_all(dir.path().join("projects")).unwrap();
        std::fs::write(dir.path().join("journal/2025/jan.md"), "# Jan").unwrap();
        std::fs::write(dir.path().join("projects/x.md"), "# X").unwrap();

        let opts = VaultTreeOptions {
            path: Some("journal".into()),
            depth: 10,
            size: 100,
            show: ShowMode::Folders,
        };
        let tree = vault_tree(&dir.path().to_string_lossy(), &[], &opts)
            .await
            .unwrap();

        assert_eq!(tree.name, "journal");
        assert_eq!(tree.subfolders.len(), 1); // 2025
    }

    #[tokio::test]
    async fn vault_tree_size_limit() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..20 {
            let folder = format!("folder_{:02}", i);
            std::fs::create_dir_all(dir.path().join(&folder)).unwrap();
            std::fs::write(dir.path().join(format!("{}/note.md", folder)), "# Note").unwrap();
        }

        let opts = VaultTreeOptions {
            path: None,
            depth: 10,
            size: 5,
            show: ShowMode::Folders,
        };
        let tree = vault_tree(&dir.path().to_string_lossy(), &[], &opts)
            .await
            .unwrap();

        // Total entries (root folders) should be capped at size
        assert!(tree.subfolders.len() <= 5);
    }

    #[tokio::test]
    async fn vault_tree_rejects_path_traversal() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("note.md"), "# Note").unwrap();

        let opts = VaultTreeOptions {
            path: Some("../../../etc".into()),
            depth: 3,
            size: 100,
            show: ShowMode::Folders,
        };

        let result = vault_tree(&dir.path().to_string_lossy(), &[], &opts).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("traversal") || err_msg.contains(".."),
            "Expected path traversal error, got: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn vault_tree_rejects_absolute_path() {
        let dir = tempfile::tempdir().unwrap();

        let opts = VaultTreeOptions {
            path: Some("/etc".into()),
            depth: 3,
            size: 100,
            show: ShowMode::Folders,
        };

        let result = vault_tree(&dir.path().to_string_lossy(), &[], &opts).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Absolute") || err_msg.contains("absolute"),
            "Expected absolute path error, got: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn vault_tree_accepts_valid_relative_path() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("journal")).unwrap();
        std::fs::write(dir.path().join("journal/note.md"), "# Note").unwrap();

        let opts = VaultTreeOptions {
            path: Some("journal".into()),
            depth: 3,
            size: 100,
            show: ShowMode::Folders,
        };

        let result = vault_tree(&dir.path().to_string_lossy(), &[], &opts).await;
        assert!(result.is_ok());
    }
}
