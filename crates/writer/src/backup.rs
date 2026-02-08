use anyhow::Result;
use std::path::{Path, PathBuf};

/// Create a backup of a file before destructive edits.
///
/// Saves to `{db_path}/backups/{timestamp}_{filename}`.
/// Returns the path to the backup file.
pub async fn create_backup(
    source_path: &Path,
    db_path: &Path,
    max_per_file: usize,
) -> Result<PathBuf> {
    let backup_dir = db_path.join("backups");
    tokio::fs::create_dir_all(&backup_dir).await?;

    let filename = source_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".into());

    let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%S");
    let backup_name = format!("{timestamp}_{filename}");
    let backup_path = backup_dir.join(&backup_name);

    tokio::fs::copy(source_path, &backup_path).await?;

    tracing::debug!(
        backup = %backup_path.display(),
        source = %source_path.display(),
        "Backup created"
    );

    cleanup_old_backups(&backup_dir, &filename, max_per_file).await?;

    Ok(backup_path)
}

/// Keep only the N most recent backups per filename.
async fn cleanup_old_backups(backup_dir: &Path, filename: &str, max_per_file: usize) -> Result<()> {
    let mut entries = Vec::new();
    let mut read_dir = tokio::fs::read_dir(backup_dir).await?;

    while let Some(entry) = read_dir.next_entry().await? {
        let name = entry.file_name().to_string_lossy().to_string();
        // Match pattern: {timestamp}_{filename}
        if name.ends_with(&format!("_{filename}")) {
            entries.push((name, entry.path()));
        }
    }

    if entries.len() <= max_per_file {
        return Ok(());
    }

    // Sort by name (timestamp prefix ensures chronological order)
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    // Remove oldest entries
    let to_remove = entries.len() - max_per_file;
    for (_, path) in entries.iter().take(to_remove) {
        tokio::fs::remove_file(path).await?;
        tracing::debug!(path = %path.display(), "Removed old backup");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn backup_create_and_cleanup() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("db");
        tokio::fs::create_dir_all(&db_path).await.unwrap();

        let source = dir.path().join("note.md");
        tokio::fs::write(&source, "# Content").await.unwrap();

        // Create 3 backups with max 2
        let b1 = create_backup(&source, &db_path, 2).await.unwrap();
        assert!(b1.exists());

        // Small delay so timestamps differ
        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
        let b2 = create_backup(&source, &db_path, 2).await.unwrap();
        assert!(b2.exists());

        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
        let b3 = create_backup(&source, &db_path, 2).await.unwrap();
        assert!(b3.exists());

        // Count remaining backups for "note.md"
        let backup_dir = db_path.join("backups");
        let mut count = 0;
        let mut read_dir = tokio::fs::read_dir(&backup_dir).await.unwrap();
        while let Some(entry) = read_dir.next_entry().await.unwrap() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with("_note.md") {
                count += 1;
            }
        }
        assert_eq!(count, 2, "Should keep only 2 most recent backups");
    }
}
