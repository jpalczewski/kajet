use std::path::{Path, PathBuf};

const CLOUD_MARKERS: &[&str] = &[
    "Library/Mobile Documents", // iCloud
    "OneDrive",
    "Dropbox",
    "Google Drive",
    "MEGA",
    "pCloud",
];

/// Returns `true` if the given path is inside a cloud-synced directory.
pub fn is_cloud_synced(path: &Path) -> bool {
    let s = path.to_string_lossy();
    CLOUD_MARKERS.iter().any(|marker| s.contains(marker))
}

/// Resolve the database directory for a vault.
///
/// Cloud-synced vaults get an external path under the OS data directory to avoid
/// slow cloud reads on every LanceDB access. Local vaults keep the traditional
/// `.kajet/` inside the vault.
pub fn resolve_db_path(vault_path: &Path) -> PathBuf {
    let canonical = vault_path
        .canonicalize()
        .unwrap_or_else(|_| vault_path.to_path_buf());

    if is_cloud_synced(&canonical) {
        if let Some(data_dir) = dirs::data_dir() {
            let hash = xxhash_rust::xxh3::xxh3_64(canonical.to_string_lossy().as_bytes());
            let hex = format!("{hash:016x}");
            let db_path = data_dir.join("kajet").join("vaults").join(hex);
            tracing::info!(
                vault = %canonical.display(),
                db_path = %db_path.display(),
                "Cloud-synced vault detected, using external db path"
            );
            return db_path;
        }
        tracing::warn!(
            vault = %canonical.display(),
            "Cloud-synced vault detected but data_dir unavailable, falling back to .kajet/"
        );
    } else {
        tracing::info!(
            vault = %canonical.display(),
            "Local vault, using in-vault .kajet/ for database"
        );
    }

    canonical.join(".kajet")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_vault_returns_in_vault_path() {
        let vault = PathBuf::from("/tmp/my-vault");
        // For a non-existent path canonicalize fails, so it stays as-is
        let db = resolve_db_path(&vault);
        assert_eq!(db, vault.join(".kajet"));
    }

    #[test]
    fn icloud_vault_returns_external_path() {
        let vault = PathBuf::from("/Users/test/Library/Mobile Documents/iCloud~md~obsidian/vault");
        let db = resolve_db_path(&vault);
        assert!(db.starts_with(dirs::data_dir().unwrap().join("kajet/vaults")));
        assert!(!db.to_string_lossy().contains(".kajet"));
    }

    #[test]
    fn onedrive_vault_returns_external_path() {
        let vault = PathBuf::from("/Users/test/OneDrive/notes");
        let db = resolve_db_path(&vault);
        assert!(db.starts_with(dirs::data_dir().unwrap().join("kajet/vaults")));
    }

    #[test]
    fn dropbox_vault_returns_external_path() {
        let vault = PathBuf::from("/home/user/Dropbox/vault");
        let db = resolve_db_path(&vault);
        assert!(db.starts_with(dirs::data_dir().unwrap().join("kajet/vaults")));
    }

    #[test]
    fn is_cloud_synced_detects_all_providers() {
        assert!(is_cloud_synced(Path::new(
            "/Users/x/Library/Mobile Documents/foo"
        )));
        assert!(is_cloud_synced(Path::new("/Users/x/OneDrive/vault")));
        assert!(is_cloud_synced(Path::new("/Users/x/Dropbox/notes")));
        assert!(is_cloud_synced(Path::new("/Users/x/Google Drive/vault")));
        assert!(is_cloud_synced(Path::new("/Users/x/MEGA/vault")));
        assert!(is_cloud_synced(Path::new("/Users/x/pCloud/notes")));
        assert!(!is_cloud_synced(Path::new("/Users/x/Documents/vault")));
        assert!(!is_cloud_synced(Path::new("/tmp/vault")));
    }

    #[test]
    fn hash_is_deterministic() {
        let vault = PathBuf::from("/Users/test/Dropbox/vault");
        let db1 = resolve_db_path(&vault);
        let db2 = resolve_db_path(&vault);
        assert_eq!(db1, db2);
    }

    #[test]
    fn hash_differs_for_different_vaults() {
        let vault_a = PathBuf::from("/Users/test/Dropbox/vault-a");
        let vault_b = PathBuf::from("/Users/test/Dropbox/vault-b");
        let db_a = resolve_db_path(&vault_a);
        let db_b = resolve_db_path(&vault_b);
        assert_ne!(db_a, db_b);
    }
}
