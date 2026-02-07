use anyhow::Result;
use chrono::{DateTime, Utc};
use std::path::Path;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VaultMetadata {
    pub embedding_model: String,
    pub last_indexed: DateTime<Utc>,
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
    fn overwrite_existing() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("db");

        VaultMetadata::save(&db_path, "model-a").unwrap();
        VaultMetadata::save(&db_path, "model-b").unwrap();

        let meta = VaultMetadata::load(&db_path).unwrap().unwrap();
        assert_eq!(meta.embedding_model, "model-b");
    }
}
