use anyhow::{bail, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(default)]
pub struct KajetConfig {
    pub port: u16,
    pub language: String,
    pub exclude_folders: Vec<String>,
    pub default_limit: usize,
    pub max_concurrent_files: usize,
    pub pipeline_buffer_size: usize,
    pub embedding_model: String,
    pub open_browser: bool,
}

impl Default for KajetConfig {
    fn default() -> Self {
        Self {
            port: 3579,
            language: "en".into(),
            exclude_folders: vec![".obsidian".into(), ".trash".into(), ".kajet".into()],
            default_limit: 5,
            max_concurrent_files: 16,
            pipeline_buffer_size: 256,
            embedding_model: "sentence-transformers/all-MiniLM-L6-v2".into(),
            open_browser: false,
        }
    }
}

/// Fields allowed in global config.
const GLOBAL_FIELDS: &[&str] = &[
    "language",
    "port",
    "max_concurrent_files",
    "pipeline_buffer_size",
    "default_limit",
    "exclude_folders",
    "embedding_model",
    "open_browser",
];

/// Fields allowed in vault-level config.
const VAULT_FIELDS: &[&str] = &["exclude_folders", "embedding_model"];

/// Load config with layered priority: defaults < global < per-vault < env < CLI.
pub fn load_config(
    vault_path: &str,
    cli_port: u16,
    cli_language: Option<String>,
) -> Result<KajetConfig> {
    use config::{Config, Environment, File};

    let mut builder = Config::builder()
        // 1. Hardcoded defaults
        .set_default("port", 3579_i64)?
        .set_default("language", "en")?
        .set_default::<&str, Vec<String>>(
            "exclude_folders",
            vec![".obsidian".into(), ".trash".into(), ".kajet".into()],
        )?
        .set_default("default_limit", 5_i64)?
        .set_default("max_concurrent_files", 16_i64)?
        .set_default("pipeline_buffer_size", 256_i64)?
        .set_default("embedding_model", "sentence-transformers/all-MiniLM-L6-v2")?
        .set_default("open_browser", false)?;

    // 2. Global config: ~/.config/kajet/config.toml
    if let Some(config_dir) = dirs::config_dir() {
        let global_path = config_dir.join("kajet").join("config.toml");
        builder = builder.add_source(File::from(global_path).required(false));
    }

    // 3. Per-vault config: {vault}/.kajet/config.toml
    let vault_config = PathBuf::from(vault_path).join(".kajet").join("config.toml");
    builder = builder.add_source(File::from(vault_config).required(false));

    // 4. Environment variables: KAJET_PORT, KAJET_LANGUAGE, etc.
    builder = builder.add_source(
        Environment::with_prefix("KAJET")
            .separator("_")
            .try_parsing(true),
    );

    // 5. CLI overrides (highest priority)
    builder = builder.set_override("port", cli_port as i64)?;
    if let Some(lang) = cli_language {
        builder = builder.set_override("language", lang)?;
    }

    let config: KajetConfig = builder.build()?.try_deserialize()?;
    Ok(config)
}

/// Write updates to the global config file (~/.config/kajet/config.toml).
/// Only keys in `GLOBAL_FIELDS` are accepted.
pub fn write_global_config(updates: &HashMap<String, toml::Value>) -> Result<()> {
    validate_fields(updates, GLOBAL_FIELDS, "global")?;

    let config_dir = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine config directory"))?
        .join("kajet");

    let config_path = config_dir.join("config.toml");
    write_toml_config(&config_path, updates)
}

/// Write updates to the vault-level config file ({vault}/.kajet/config.toml).
/// Only keys in `VAULT_FIELDS` are accepted.
pub fn write_vault_config(vault_path: &Path, updates: &HashMap<String, toml::Value>) -> Result<()> {
    validate_fields(updates, VAULT_FIELDS, "vault")?;

    let config_path = vault_path.join(".kajet").join("config.toml");
    write_toml_config(&config_path, updates)
}

/// Reload config from all sources (re-runs the full load pipeline).
pub fn reload_config(
    vault_path: &str,
    cli_port: u16,
    cli_language: Option<String>,
) -> Result<KajetConfig> {
    load_config(vault_path, cli_port, cli_language)
}

fn validate_fields(
    updates: &HashMap<String, toml::Value>,
    allowed: &[&str],
    scope: &str,
) -> Result<()> {
    for key in updates.keys() {
        if !allowed.contains(&key.as_str()) {
            bail!(
                "Field '{}' is not allowed in {} config. Allowed: {:?}",
                key,
                scope,
                allowed
            );
        }
    }
    Ok(())
}

fn write_toml_config(path: &Path, updates: &HashMap<String, toml::Value>) -> Result<()> {
    // Read existing config or start fresh
    let mut table: toml::Table = if path.exists() {
        let content = std::fs::read_to_string(path)?;
        content.parse()?
    } else {
        toml::Table::new()
    };

    // Merge updates
    for (key, value) in updates {
        table.insert(key.clone(), value.clone());
    }

    let content = toml::to_string_pretty(&table)?;

    // Atomic write: write to tmp file then rename
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp_path = path.with_extension("toml.tmp");
    std::fs::write(&tmp_path, &content)?;
    std::fs::rename(&tmp_path, path)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn default_config_values() {
        let config = KajetConfig::default();
        assert_eq!(config.port, 3579);
        assert_eq!(config.language, "en");
        assert_eq!(config.default_limit, 5);
        assert_eq!(
            config.embedding_model,
            "sentence-transformers/all-MiniLM-L6-v2"
        );
        assert!(!config.open_browser);
        assert!(config.exclude_folders.contains(&".obsidian".to_string()));
        assert!(config.exclude_folders.contains(&".trash".to_string()));
        assert!(config.exclude_folders.contains(&".kajet".to_string()));
    }

    #[test]
    fn load_config_uses_cli_overrides() {
        let config = load_config("/nonexistent/vault", 4000, Some("pl".into())).unwrap();
        assert_eq!(config.port, 4000);
        assert_eq!(config.language, "pl");
        assert_eq!(config.default_limit, 5);
    }

    #[test]
    fn load_config_defaults_without_files() {
        let config = load_config("/nonexistent/vault", 3579, None).unwrap();
        assert_eq!(config.port, 3579);
        assert_eq!(config.language, "en");
        assert_eq!(config.default_limit, 5);
        assert_eq!(
            config.embedding_model,
            "sentence-transformers/all-MiniLM-L6-v2"
        );
        assert!(!config.open_browser);
    }

    #[test]
    fn validate_global_fields_accepts_valid() {
        let mut updates = HashMap::new();
        updates.insert("language".into(), toml::Value::String("pl".into()));
        updates.insert("port".into(), toml::Value::Integer(4000));
        assert!(validate_fields(&updates, GLOBAL_FIELDS, "global").is_ok());
    }

    #[test]
    fn validate_global_fields_rejects_invalid() {
        let mut updates = HashMap::new();
        updates.insert("nonexistent_field".into(), toml::Value::String("x".into()));
        assert!(validate_fields(&updates, GLOBAL_FIELDS, "global").is_err());
    }

    #[test]
    fn validate_vault_fields_rejects_global_only() {
        let mut updates = HashMap::new();
        updates.insert("port".into(), toml::Value::Integer(4000));
        assert!(validate_fields(&updates, VAULT_FIELDS, "vault").is_err());
    }

    #[test]
    fn write_and_read_toml_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut updates = HashMap::new();
        updates.insert("language".into(), toml::Value::String("pl".into()));
        updates.insert("port".into(), toml::Value::Integer(4000));

        write_toml_config(&path, &updates).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        let table: toml::Table = content.parse().unwrap();
        assert_eq!(table["language"].as_str(), Some("pl"));
        assert_eq!(table["port"].as_integer(), Some(4000));

        // Merge: add new key, existing preserved
        let mut updates2 = HashMap::new();
        updates2.insert(
            "embedding_model".into(),
            toml::Value::String("my-model".into()),
        );
        write_toml_config(&path, &updates2).unwrap();

        let content2 = fs::read_to_string(&path).unwrap();
        let table2: toml::Table = content2.parse().unwrap();
        assert_eq!(table2["language"].as_str(), Some("pl"));
        assert_eq!(table2["embedding_model"].as_str(), Some("my-model"));
    }

    #[test]
    fn write_global_config_validates_scope() {
        // This just tests validation logic, not actual file write
        let mut updates = HashMap::new();
        updates.insert("nonexistent".into(), toml::Value::String("x".into()));
        assert!(write_global_config(&updates).is_err());
    }

    #[test]
    fn write_vault_config_validates_scope() {
        let dir = tempfile::tempdir().unwrap();
        let mut updates = HashMap::new();
        updates.insert("port".into(), toml::Value::Integer(4000));
        assert!(write_vault_config(dir.path(), &updates).is_err());
    }
}
