use anyhow::Result;
use chrono::{DateTime, Utc};
use std::path::Path;

/// Bump this when storage format changes require a full reindex.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VaultMetadata {
    pub embedding_model: String,
    pub last_indexed: DateTime<Utc>,
    /// Schema version for detecting format changes (e.g. NFD→NFC path normalization).
    /// `None` for databases created before versioning was added.
    #[serde(default)]
    pub schema_version: Option<u32>,
}

impl VaultMetadata {
    pub fn load(db_path: &Path) -> Result<Option<Self>> {
        let path = db_path.join("metadata.json");
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&path)?;
        let meta: Self = serde_json::from_str(&content)?;
        Ok(Some(meta))
    }

    pub fn save(db_path: &Path, embedding_model: &str) -> Result<()> {
        std::fs::create_dir_all(db_path)?;
        let path = db_path.join("metadata.json");
        let meta = Self {
            embedding_model: embedding_model.to_string(),
            last_indexed: Utc::now(),
            schema_version: Some(CURRENT_SCHEMA_VERSION),
        };
        let content = serde_json::to_string_pretty(&meta)?;
        let tmp_path = path.with_extension("json.tmp");
        std::fs::write(&tmp_path, &content)?;
        std::fs::rename(&tmp_path, &path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("db");

        // Initially no metadata
        assert!(VaultMetadata::load(&db_path).unwrap().is_none());

        // Save (creates db_path directory)
        VaultMetadata::save(&db_path, "sentence-transformers/all-MiniLM-L6-v2").unwrap();

        // Load
        let meta = VaultMetadata::load(&db_path).unwrap().unwrap();
        assert_eq!(
            meta.embedding_model,
            "sentence-transformers/all-MiniLM-L6-v2"
        );
        assert!(meta.last_indexed <= Utc::now());
    }

    #[test]
    fn load_legacy_metadata_without_schema_version() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("db");
        std::fs::create_dir_all(&db_path).unwrap();

        // Simulate old metadata.json without schema_version field
        let legacy_json = r#"{
            "embedding_model": "sentence-transformers/all-MiniLM-L6-v2",
            "last_indexed": "2025-01-01T00:00:00Z"
        }"#;
        std::fs::write(db_path.join("metadata.json"), legacy_json).unwrap();

        let meta = VaultMetadata::load(&db_path).unwrap().unwrap();
        assert_eq!(
            meta.schema_version, None,
            "legacy metadata should have schema_version = None"
        );
    }

    #[test]
    fn save_includes_current_schema_version() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("db");

        VaultMetadata::save(&db_path, "test-model").unwrap();

        let meta = VaultMetadata::load(&db_path).unwrap().unwrap();
        assert_eq!(
            meta.schema_version,
            Some(CURRENT_SCHEMA_VERSION),
            "new metadata should have current schema version"
        );
    }

    #[test]
    fn schema_version_mismatch_detected() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("db");
        std::fs::create_dir_all(&db_path).unwrap();

        // Simulate metadata with old schema version
        let old_json = r#"{
            "embedding_model": "test-model",
            "last_indexed": "2025-01-01T00:00:00Z",
            "schema_version": 0
        }"#;
        std::fs::write(db_path.join("metadata.json"), old_json).unwrap();

        let meta = VaultMetadata::load(&db_path).unwrap().unwrap();
        assert_ne!(
            meta.schema_version,
            Some(CURRENT_SCHEMA_VERSION),
            "old schema version should trigger reindex"
        );
    }

    #[test]
    fn overwrite_existing() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("db");

        VaultMetadata::save(&db_path, "model-a").unwrap();
        VaultMetadata::save(&db_path, "model-b").unwrap();

        let meta = VaultMetadata::load(&db_path).unwrap().unwrap();
        assert_eq!(meta.embedding_model, "model-b");
    }
}
