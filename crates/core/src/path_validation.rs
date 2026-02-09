//! Path validation utilities for vault security.
//!
//! These functions prevent path traversal attacks and ensure that all file
//! operations stay within the designated vault directory.

use anyhow::{Result, bail};
use std::path::{Component, Path};

/// Validate that a path stays within the vault — rejects `..` traversal and absolute paths.
///
/// # Examples
///
/// ```
/// use kajet_core::path_validation::ensure_within_vault;
///
/// assert!(ensure_within_vault("foo/bar.md").is_ok());
/// assert!(ensure_within_vault("./foo/bar.md").is_ok());
/// assert!(ensure_within_vault("../etc/passwd").is_err());
/// assert!(ensure_within_vault("/etc/passwd").is_err());
/// ```
pub fn ensure_within_vault(rel_path: &str) -> Result<()> {
    let path = Path::new(rel_path);
    for component in path.components() {
        if matches!(component, Component::ParentDir) {
            bail!(
                "Path traversal rejected: '{}' contains '..' component",
                rel_path
            );
        }
        if matches!(component, Component::RootDir | Component::Prefix(_)) {
            bail!("Absolute paths are not allowed: '{}'", rel_path);
        }
    }
    Ok(())
}

/// Verify that a resolved absolute path is actually within the vault after symlink resolution.
///
/// Walks up the path to find the deepest existing ancestor, canonicalizes it,
/// then checks the prefix relationship. This works for paths where intermediate
/// directories don't exist yet (they'll be created later by `create_dir_all`).
///
/// # Examples
///
/// ```no_run
/// use kajet_core::path_validation::ensure_canonical_within_vault;
/// use std::path::Path;
///
/// # #[tokio::main]
/// # async fn main() -> anyhow::Result<()> {
/// let vault = Path::new("/Users/me/vault");
/// let valid = vault.join("note.md");
/// ensure_canonical_within_vault(&valid, vault).await?;
/// # Ok(())
/// # }
/// ```
pub async fn ensure_canonical_within_vault(abs_path: &Path, vault_path: &Path) -> Result<()> {
    let canonical_vault = tokio::fs::canonicalize(vault_path).await?;

    // Find the deepest existing ancestor and collect non-existent suffixes
    let mut existing = abs_path.to_path_buf();
    let mut suffix_parts: Vec<std::ffi::OsString> = Vec::new();

    while tokio::fs::metadata(&existing).await.is_err() {
        if let Some(name) = existing.file_name() {
            suffix_parts.push(name.to_owned());
        }
        match existing.parent() {
            Some(parent) => existing = parent.to_path_buf(),
            None => bail!("Cannot resolve path: '{}'", abs_path.display()),
        }
    }

    let mut canonical = tokio::fs::canonicalize(&existing).await?;
    for part in suffix_parts.iter().rev() {
        canonical.push(part);
    }

    if !canonical.starts_with(&canonical_vault) {
        bail!(
            "Path escapes vault via symlink: '{}' resolves outside '{}'",
            abs_path.display(),
            vault_path.display()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_within_vault_rejects_parent_dir() {
        assert!(ensure_within_vault("../etc/passwd").is_err());
        assert!(ensure_within_vault("foo/../../bar").is_err());
        assert!(ensure_within_vault("../../..").is_err());
    }

    #[test]
    fn ensure_within_vault_rejects_absolute() {
        assert!(ensure_within_vault("/etc/passwd").is_err());
        assert!(ensure_within_vault("/Users/foo/bar").is_err());
    }

    #[test]
    fn ensure_within_vault_accepts_valid() {
        assert!(ensure_within_vault("foo/bar.md").is_ok());
        assert!(ensure_within_vault("./foo/bar.md").is_ok());
        assert!(ensure_within_vault("journal/2024/note.md").is_ok());
        assert!(ensure_within_vault("").is_ok()); // Empty path is valid (refers to vault root)
    }

    #[tokio::test]
    async fn ensure_canonical_within_vault_works() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path();

        // Create a test file
        let valid = vault.join("note.md");
        tokio::fs::write(&valid, "# Test").await.unwrap();

        assert!(ensure_canonical_within_vault(&valid, vault).await.is_ok());
    }

    #[tokio::test]
    async fn ensure_canonical_within_vault_rejects_parent_escape() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path();

        // Try to escape to parent
        let escaped = vault.join("../etc/passwd");

        let result = ensure_canonical_within_vault(&escaped, vault).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn ensure_canonical_within_vault_handles_nonexistent() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path();

        // Path that doesn't exist yet (but would be valid when created)
        let future_path = vault.join("new_folder").join("note.md");

        assert!(
            ensure_canonical_within_vault(&future_path, vault)
                .await
                .is_ok()
        );
    }
}
