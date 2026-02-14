use anyhow::Result;
use chrono::{DateTime, Utc};
use std::path::Path;

/// Bump this when storage format changes require a full reindex.
/// Version 2: Added chunk_index column to search results
pub const CURRENT_SCHEMA_VERSION: u32 = 2;

fn default_embedding_backend() -> String {
    "candle".to_string()
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VaultMetadata {
    #[serde(default = "default_embedding_backend")]
    pub embedding_backend: String,
    pub embedding_model: String,
    #[serde(default)]
    pub embedding_base_url: String,
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

    pub fn needs_reindex(&self, backend: &str, model: &str, base_url: &str) -> bool {
        let normalized_old = self.embedding_base_url.trim_end_matches('/');
        let normalized_new = base_url.trim_end_matches('/');
        let compare_base_url = self.embedding_backend == "remote" || backend == "remote";
        self.embedding_backend != backend
            || self.embedding_model != model
            || (compare_base_url && normalized_old != normalized_new)
    }

    pub fn save_embedding(
        db_path: &Path,
        backend: &str,
        model: &str,
        base_url: &str,
    ) -> Result<()> {
        std::fs::create_dir_all(db_path)?;
        let path = db_path.join("metadata.json");
        let persisted_base_url = if backend.eq_ignore_ascii_case("candle") {
            ""
        } else {
            base_url
        };
        let meta = Self {
            embedding_backend: backend.to_string(),
            embedding_model: model.to_string(),
            embedding_base_url: persisted_base_url.to_string(),
            last_indexed: Utc::now(),
            schema_version: Some(CURRENT_SCHEMA_VERSION),
        };
        let content = serde_json::to_string_pretty(&meta)?;
        let tmp_path = path.with_extension("json.tmp");
        std::fs::write(&tmp_path, &content)?;
        std::fs::rename(&tmp_path, &path)?;
        Ok(())
    }

    pub fn save(db_path: &Path, embedding_model: &str) -> Result<()> {
        Self::save_embedding(db_path, "candle", embedding_model, "")
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
        VaultMetadata::save_embedding(
            &db_path,
            "remote",
            "sentence-transformers/all-MiniLM-L6-v2",
            "http://localhost:1234/",
        )
        .unwrap();

        // Load
        let meta = VaultMetadata::load(&db_path).unwrap().unwrap();
        assert_eq!(meta.embedding_backend, "remote");
        assert_eq!(
            meta.embedding_model,
            "sentence-transformers/all-MiniLM-L6-v2"
        );
        assert_eq!(meta.embedding_base_url, "http://localhost:1234/");
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
            meta.embedding_backend, "candle",
            "legacy metadata without backend should default to candle"
        );
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

    #[test]
    fn reindex_on_backend_change() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("db");
        VaultMetadata::save_embedding(&db_path, "candle", "all-MiniLM", "").unwrap();
        let meta = VaultMetadata::load(&db_path).unwrap().unwrap();
        assert!(meta.needs_reindex("remote", "all-MiniLM", "http://localhost:1234"));
        assert!(!meta.needs_reindex("candle", "all-MiniLM", ""));
    }

    #[test]
    fn base_url_normalization_avoids_false_reindex() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("db");
        VaultMetadata::save_embedding(&db_path, "remote", "nomic-embed", "http://localhost:1234/")
            .unwrap();
        let meta = VaultMetadata::load(&db_path).unwrap().unwrap();
        assert!(!meta.needs_reindex("remote", "nomic-embed", "http://localhost:1234"));
    }

    #[test]
    fn candle_ignores_base_url_changes() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("db");
        VaultMetadata::save_embedding(&db_path, "candle", "all-MiniLM", "").unwrap();
        let meta = VaultMetadata::load(&db_path).unwrap().unwrap();
        assert!(!meta.needs_reindex("candle", "all-MiniLM", "http://localhost:1234"));
    }

    #[test]
    fn candle_persists_empty_base_url_even_if_value_passed() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("db");
        VaultMetadata::save_embedding(
            &db_path,
            "candle",
            "sentence-transformers/all-MiniLM-L6-v2",
            "http://localhost:8080",
        )
        .unwrap();

        let meta = VaultMetadata::load(&db_path).unwrap().unwrap();
        assert_eq!(meta.embedding_base_url, "");
    }
}
