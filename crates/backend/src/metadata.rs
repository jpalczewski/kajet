use anyhow::Result;
use chrono::{DateTime, Utc};
use std::path::Path;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VaultMetadata {
    pub embedding_model: String,
    pub last_indexed: DateTime<Utc>,
}

impl VaultMetadata {
    pub fn load(vault_path: &Path) -> Result<Option<Self>> {
        let path = vault_path.join(".kajet").join("metadata.json");
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&path)?;
        let meta: Self = serde_json::from_str(&content)?;
        Ok(Some(meta))
    }

    pub fn save(vault_path: &Path, embedding_model: &str) -> Result<()> {
        let dir = vault_path.join(".kajet");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("metadata.json");
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
        let vault = dir.path();

        // Initially no metadata
        assert!(VaultMetadata::load(vault).unwrap().is_none());

        // Save
        VaultMetadata::save(vault, "sentence-transformers/all-MiniLM-L6-v2").unwrap();

        // Load
        let meta = VaultMetadata::load(vault).unwrap().unwrap();
        assert_eq!(
            meta.embedding_model,
            "sentence-transformers/all-MiniLM-L6-v2"
        );
        assert!(meta.last_indexed <= Utc::now());
    }

    #[test]
    fn overwrite_existing() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path();

        VaultMetadata::save(vault, "model-a").unwrap();
        VaultMetadata::save(vault, "model-b").unwrap();

        let meta = VaultMetadata::load(vault).unwrap().unwrap();
        assert_eq!(meta.embedding_model, "model-b");
    }
}
