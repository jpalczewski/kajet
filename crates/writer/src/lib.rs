pub mod backup;
pub mod resolve;
pub mod timestamp;
pub mod types;

pub use types::{CreateNoteParams, CreateNoteResult, EditMode, EditNoteParams, EditNoteResult};

use anyhow::{Result, bail};
use kajet_core::config::WriterConfig;
use kajet_core::path_validation::{ensure_canonical_within_vault, ensure_within_vault};
use kajet_core::traits::DocumentStore;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::instrument;

pub struct NoteWriter {
    vault_path: PathBuf,
    db_path: PathBuf,
    config: WriterConfig,
    doc_store: Option<Arc<dyn DocumentStore>>,
}

impl NoteWriter {
    pub fn new(
        vault_path: PathBuf,
        db_path: PathBuf,
        config: WriterConfig,
        doc_store: Option<Arc<dyn DocumentStore>>,
    ) -> Self {
        Self {
            vault_path,
            db_path,
            config,
            doc_store,
        }
    }

    /// Create a new note in the vault.
    #[instrument(level = "info", skip(self, params), fields(target = %params.target))]
    pub async fn create(&self, params: CreateNoteParams) -> Result<CreateNoteResult> {
        if params.content.is_empty() {
            bail!("Content cannot be empty");
        }

        let rel_path = normalize_target(&params.target);
        ensure_within_vault(&rel_path)?;
        let abs_path = self.vault_path.join(&rel_path);

        if tokio::fs::metadata(&abs_path).await.is_ok() {
            bail!(
                "File already exists: '{}'. Use edit_note instead.",
                rel_path
            );
        }

        // Verify the path doesn't escape vault via symlinks BEFORE creating directories
        ensure_canonical_within_vault(&abs_path, &self.vault_path).await?;

        // Create parent directories if needed
        if let Some(parent) = abs_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Build the note content
        let title = title_from_path(&rel_path);
        let ts = if self.config.timestamps.enabled {
            Some(timestamp::format_now(&self.config.timestamps)?)
        } else {
            None
        };

        let frontmatter = kajet_parser::generate_frontmatter(
            &title,
            &params.tags,
            &self.config.frontmatter.default_tags,
            &params.aliases,
            ts.as_deref(),
            &self.config.timestamps.created_field,
            &self.config.timestamps.modified_field,
        );

        let full_content = format!("{frontmatter}{}", params.content);
        let bytes = full_content.len();

        tokio::fs::write(&abs_path, &full_content).await?;

        tracing::info!(path = %rel_path, bytes, "Note created");

        Ok(CreateNoteResult {
            path: rel_path,
            absolute_path: abs_path,
            bytes_written: bytes,
        })
    }

    /// Edit an existing note in the vault.
    #[instrument(level = "info", skip(self, params), fields(path = %params.path, mode = %params.mode.as_str()))]
    pub async fn edit(&self, params: EditNoteParams) -> Result<EditNoteResult> {
        if params.content.is_empty() && params.mode != EditMode::ReplaceText {
            bail!("Content cannot be empty");
        }

        self.validate_edit_params(&params)?;

        // Resolve path
        let resolved =
            resolve::resolve_note_path(&params.path, &self.vault_path, self.doc_store.as_ref())
                .await?;

        // Verify the resolved path doesn't escape vault via symlinks
        ensure_canonical_within_vault(&resolved.absolute, &self.vault_path).await?;

        // Read current content
        let current = tokio::fs::read_to_string(&resolved.absolute).await?;

        // Backup before destructive ops
        let backup_path = if params.mode.is_destructive() && self.config.backup_enabled {
            Some(
                backup::create_backup(
                    &resolved.absolute,
                    &self.db_path,
                    self.config.backup_max_per_file,
                )
                .await?,
            )
        } else {
            None
        };

        // Apply transform
        let transformed = self.apply_transform(&current, &params)?;

        // Update modified timestamp — only if the field already exists in frontmatter.
        // This avoids adding duplicate timestamp fields to notes created by other tools
        // (e.g. Obsidian with Polish field names like "Data aktualizacji").
        let final_content = if self.config.timestamps.enabled
            && !self.config.timestamps.modified_field.is_empty()
        {
            let ts = timestamp::format_now(&self.config.timestamps)?;
            kajet_parser::update_existing_frontmatter_field(
                &transformed,
                &self.config.timestamps.modified_field,
                &ts,
            )
        } else {
            transformed
        };

        let bytes = final_content.len();
        tokio::fs::write(&resolved.absolute, &final_content).await?;

        tracing::info!(
            path = %resolved.relative,
            mode = %params.mode.as_str(),
            bytes,
            backup = backup_path.is_some(),
            "Note edited"
        );

        Ok(EditNoteResult {
            path: resolved.relative,
            absolute_path: resolved.absolute,
            mode: params.mode.as_str().to_string(),
            bytes_written: bytes,
            backup_path,
        })
    }

    fn validate_edit_params(&self, params: &EditNoteParams) -> Result<()> {
        match params.mode {
            EditMode::Overwrite if params.target_heading.is_some() => {
                bail!("Cannot use target_heading with overwrite mode");
            }
            EditMode::ReplaceSection if params.target_heading.is_none() => {
                bail!("replace_section requires target_heading");
            }
            EditMode::ReplaceText | EditMode::InsertAfter if params.old_text.is_none() => {
                bail!("{} requires old_text", params.mode.as_str());
            }
            _ => Ok(()),
        }
    }

    fn apply_transform(&self, current: &str, params: &EditNoteParams) -> Result<String> {
        match params.mode {
            EditMode::Append => kajet_parser::append_content(
                current,
                &params.content,
                params.target_heading.as_deref(),
            )
            .map_err(|e| anyhow::anyhow!("{e}")),

            EditMode::Prepend => kajet_parser::prepend_content(
                current,
                &params.content,
                params.target_heading.as_deref(),
            )
            .map_err(|e| anyhow::anyhow!("{e}")),

            EditMode::Overwrite => Ok(kajet_parser::overwrite_body(current, &params.content)),

            EditMode::ReplaceSection => {
                let heading = params.target_heading.as_deref().unwrap();
                kajet_parser::replace_section(current, heading, &params.content)
                    .map_err(|e| anyhow::anyhow!("{e}"))
            }

            EditMode::ReplaceText => {
                let old = params.old_text.as_deref().unwrap();
                kajet_parser::replace_text(current, old, &params.content)
                    .map_err(|e| anyhow::anyhow!("{e}"))
            }

            EditMode::InsertAfter => {
                let anchor = params.old_text.as_deref().unwrap();
                kajet_parser::insert_after(current, anchor, &params.content)
                    .map_err(|e| anyhow::anyhow!("{e}"))
            }
        }
    }
}

/// Normalize a target path: ensure `.md` extension, normalize separators.
fn normalize_target(target: &str) -> String {
    let normalized = target.replace('\\', "/");
    if normalized.ends_with(".md") {
        normalized
    } else {
        format!("{normalized}.md")
    }
}

/// Extract a title from a relative file path (filename without extension).
fn title_from_path(rel_path: &str) -> String {
    Path::new(rel_path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| rel_path.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use kajet_core::config::WriterConfig;

    fn test_config() -> WriterConfig {
        WriterConfig {
            timestamps: kajet_core::config::TimestampConfig {
                enabled: false,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn create_basic() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().to_path_buf();
        let db = dir.path().join(".kajet");

        let writer = NoteWriter::new(vault.clone(), db, test_config(), None);
        let result = writer
            .create(CreateNoteParams {
                target: "hello.md".into(),
                content: "# Hello\n\nWorld".into(),
                tags: vec!["test".into()],
                aliases: vec![],
            })
            .await
            .unwrap();

        assert_eq!(result.path, "hello.md");
        assert!(result.absolute_path.exists());
        let content = tokio::fs::read_to_string(&result.absolute_path)
            .await
            .unwrap();
        assert!(content.contains("# Hello"));
        assert!(content.contains("tags: [test]"));
    }

    #[tokio::test]
    async fn create_error_if_exists() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().to_path_buf();
        let db = dir.path().join(".kajet");

        tokio::fs::write(vault.join("existing.md"), "content")
            .await
            .unwrap();

        let writer = NoteWriter::new(vault, db, test_config(), None);
        let result = writer
            .create(CreateNoteParams {
                target: "existing.md".into(),
                content: "new content".into(),
                tags: vec![],
                aliases: vec![],
            })
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
    }

    #[tokio::test]
    async fn create_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().to_path_buf();
        let db = dir.path().join(".kajet");

        let writer = NoteWriter::new(vault.clone(), db, test_config(), None);
        let result = writer
            .create(CreateNoteParams {
                target: "sub/folder/note".into(),
                content: "Deep note".into(),
                tags: vec![],
                aliases: vec![],
            })
            .await
            .unwrap();

        assert_eq!(result.path, "sub/folder/note.md");
        assert!(vault.join("sub/folder/note.md").exists());
    }

    #[tokio::test]
    async fn create_with_timestamps() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().to_path_buf();
        let db = dir.path().join(".kajet");

        let config = WriterConfig {
            timestamps: kajet_core::config::TimestampConfig {
                enabled: true,
                format: "date_only".into(),
                timezone: "UTC".into(),
                ..Default::default()
            },
            ..Default::default()
        };

        let writer = NoteWriter::new(vault, db, config, None);
        let result = writer
            .create(CreateNoteParams {
                target: "timestamped".into(),
                content: "Body".into(),
                tags: vec![],
                aliases: vec![],
            })
            .await
            .unwrap();

        let content = tokio::fs::read_to_string(&result.absolute_path)
            .await
            .unwrap();
        assert!(content.contains("created:"));
        assert!(content.contains("modified:"));
    }

    #[tokio::test]
    async fn edit_append() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().to_path_buf();
        let db = dir.path().join(".kajet");

        let note_path = vault.join("note.md");
        tokio::fs::write(&note_path, "# Title\n\nOriginal body.\n")
            .await
            .unwrap();

        let writer = NoteWriter::new(vault, db, test_config(), None);
        let result = writer
            .edit(EditNoteParams {
                path: "note.md".into(),
                content: "Appended line.".into(),
                mode: EditMode::Append,
                target_heading: None,
                old_text: None,
            })
            .await
            .unwrap();

        assert_eq!(result.mode, "append");
        let content = tokio::fs::read_to_string(&note_path).await.unwrap();
        assert!(content.contains("Original body."));
        assert!(content.contains("Appended line."));
    }

    #[tokio::test]
    async fn edit_overwrite_with_backup() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().to_path_buf();
        let db = dir.path().join(".kajet");

        let note_path = vault.join("note.md");
        tokio::fs::write(&note_path, "# Old\n\nOld body.\n")
            .await
            .unwrap();

        let config = WriterConfig {
            backup_enabled: true,
            timestamps: kajet_core::config::TimestampConfig {
                enabled: false,
                ..Default::default()
            },
            ..Default::default()
        };

        let writer = NoteWriter::new(vault, db.clone(), config, None);
        let result = writer
            .edit(EditNoteParams {
                path: "note.md".into(),
                content: "# New\n\nNew body.".into(),
                mode: EditMode::Overwrite,
                target_heading: None,
                old_text: None,
            })
            .await
            .unwrap();

        assert!(result.backup_path.is_some());
        let backup = result.backup_path.unwrap();
        assert!(backup.exists());
        // Backup should contain old content
        let backup_content = tokio::fs::read_to_string(&backup).await.unwrap();
        assert!(backup_content.contains("Old body."));
    }

    #[tokio::test]
    async fn edit_updates_existing_modified_timestamp() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().to_path_buf();
        let db = dir.path().join(".kajet");

        let note_path = vault.join("note.md");
        // Note already has modified: field (e.g. created by kajet)
        tokio::fs::write(
            &note_path,
            "---\ntitle: \"Note\"\nmodified: 2025-01-01\n---\n# Body\n",
        )
        .await
        .unwrap();

        let config = WriterConfig {
            timestamps: kajet_core::config::TimestampConfig {
                enabled: true,
                format: "date_only".into(),
                timezone: "UTC".into(),
                ..Default::default()
            },
            ..Default::default()
        };

        let writer = NoteWriter::new(vault, db, config, None);
        writer
            .edit(EditNoteParams {
                path: "note.md".into(),
                content: "Appended.".into(),
                mode: EditMode::Append,
                target_heading: None,
                old_text: None,
            })
            .await
            .unwrap();

        let content = tokio::fs::read_to_string(&note_path).await.unwrap();
        assert!(content.contains("modified:"));
        assert!(
            !content.contains("2025-01-01"),
            "Old timestamp should be replaced"
        );
    }

    #[tokio::test]
    async fn edit_does_not_add_missing_modified() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().to_path_buf();
        let db = dir.path().join(".kajet");

        let note_path = vault.join("note.md");
        // Note without modified: field (e.g. created by Obsidian)
        tokio::fs::write(&note_path, "---\ntitle: \"Note\"\n---\n# Body\n")
            .await
            .unwrap();

        let config = WriterConfig {
            timestamps: kajet_core::config::TimestampConfig {
                enabled: true,
                format: "date_only".into(),
                timezone: "UTC".into(),
                ..Default::default()
            },
            ..Default::default()
        };

        let writer = NoteWriter::new(vault, db, config, None);
        writer
            .edit(EditNoteParams {
                path: "note.md".into(),
                content: "Appended.".into(),
                mode: EditMode::Append,
                target_heading: None,
                old_text: None,
            })
            .await
            .unwrap();

        let content = tokio::fs::read_to_string(&note_path).await.unwrap();
        assert!(
            !content.contains("modified:"),
            "Should not add modified: to note that didn't have it"
        );
    }

    #[tokio::test]
    async fn edit_skips_empty_modified_field() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().to_path_buf();
        let db = dir.path().join(".kajet");

        let note_path = vault.join("note.md");
        tokio::fs::write(&note_path, "---\ntitle: \"Note\"\n---\n# Body\n")
            .await
            .unwrap();

        let config = WriterConfig {
            timestamps: kajet_core::config::TimestampConfig {
                enabled: true,
                modified_field: "".into(), // disabled
                format: "date_only".into(),
                timezone: "UTC".into(),
                ..Default::default()
            },
            ..Default::default()
        };

        let writer = NoteWriter::new(vault, db, config, None);
        writer
            .edit(EditNoteParams {
                path: "note.md".into(),
                content: "Appended.".into(),
                mode: EditMode::Append,
                target_heading: None,
                old_text: None,
            })
            .await
            .unwrap();

        let content = tokio::fs::read_to_string(&note_path).await.unwrap();
        assert!(!content.contains("modified:"));
    }

    #[tokio::test]
    async fn create_rejects_path_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().to_path_buf();
        let db = dir.path().join(".kajet");

        let writer = NoteWriter::new(vault, db, test_config(), None);

        // Direct parent traversal
        let result = writer
            .create(CreateNoteParams {
                target: "../../etc/evil".into(),
                content: "pwned".into(),
                tags: vec![],
                aliases: vec![],
            })
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("traversal"));

        // Nested traversal
        let result = writer
            .create(CreateNoteParams {
                target: "sub/../../../etc/evil".into(),
                content: "pwned".into(),
                tags: vec![],
                aliases: vec![],
            })
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("traversal"));
    }

    #[tokio::test]
    async fn create_rejects_absolute_path() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().to_path_buf();
        let db = dir.path().join(".kajet");

        let writer = NoteWriter::new(vault, db, test_config(), None);
        let result = writer
            .create(CreateNoteParams {
                target: "/etc/passwd".into(),
                content: "pwned".into(),
                tags: vec![],
                aliases: vec![],
            })
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Absolute"));
    }

    #[tokio::test]
    async fn edit_rejects_path_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().to_path_buf();
        let db = dir.path().join(".kajet");

        let writer = NoteWriter::new(vault, db, test_config(), None);
        let result = writer
            .edit(EditNoteParams {
                path: "../../etc/passwd".into(),
                content: "pwned".into(),
                mode: EditMode::Append,
                target_heading: None,
                old_text: None,
            })
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("traversal"));
    }

    #[tokio::test]
    async fn create_rejects_symlink_escape() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().join("vault");
        let outside = dir.path().join("outside");
        let db = dir.path().join(".kajet");

        tokio::fs::create_dir_all(&vault).await.unwrap();
        tokio::fs::create_dir_all(&outside).await.unwrap();

        // Create a symlink inside the vault pointing outside
        tokio::fs::symlink(&outside, vault.join("escape"))
            .await
            .unwrap();

        let writer = NoteWriter::new(vault, db, test_config(), None);
        let result = writer
            .create(CreateNoteParams {
                target: "escape/evil".into(),
                content: "pwned".into(),
                tags: vec![],
                aliases: vec![],
            })
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("symlink"),
            "Expected symlink error, got: {err}"
        );
        // Verify file was NOT created outside vault
        assert!(!outside.join("evil.md").exists());
    }

    #[tokio::test]
    async fn edit_rejects_symlink_escape() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().join("vault");
        let outside = dir.path().join("outside");
        let db = dir.path().join(".kajet");

        tokio::fs::create_dir_all(&vault).await.unwrap();
        tokio::fs::create_dir_all(&outside).await.unwrap();

        // Create a real file outside the vault
        let outside_note = outside.join("secret.md");
        tokio::fs::write(&outside_note, "# Secret\n\nDo not edit.")
            .await
            .unwrap();

        // Symlink it into the vault
        tokio::fs::symlink(&outside_note, vault.join("secret.md"))
            .await
            .unwrap();

        let writer = NoteWriter::new(vault, db, test_config(), None);
        let result = writer
            .edit(EditNoteParams {
                path: "secret.md".into(),
                content: "pwned".into(),
                mode: EditMode::Append,
                target_heading: None,
                old_text: None,
            })
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("symlink"),
            "Expected symlink error, got: {err}"
        );
        // Verify original file was NOT modified
        let content = tokio::fs::read_to_string(&outside_note).await.unwrap();
        assert_eq!(content, "# Secret\n\nDo not edit.");
    }

    #[tokio::test]
    async fn edit_empty_content_error() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().to_path_buf();
        let db = dir.path().join(".kajet");

        tokio::fs::write(vault.join("note.md"), "content")
            .await
            .unwrap();

        let writer = NoteWriter::new(vault, db, test_config(), None);
        let result = writer
            .edit(EditNoteParams {
                path: "note.md".into(),
                content: "".into(),
                mode: EditMode::Append,
                target_heading: None,
                old_text: None,
            })
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty"));
    }
}
