use anyhow::{bail, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(default)]
pub struct LoggingConfig {
    pub level: String,
    pub file_level: String,
    pub dashboard_level: String,
    pub progress_percent_step: u8,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "debug".into(),
            file_level: "trace".into(),
            dashboard_level: "info".into(),
            progress_percent_step: 5,
        }
    }
}

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
    pub resolve_wikilinks: bool,
    pub logging: LoggingConfig,
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
            resolve_wikilinks: true,
            logging: LoggingConfig::default(),
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
    "resolve_wikilinks",
    "logging",
];

/// Fields allowed in vault-level config.
const VAULT_FIELDS: &[&str] = &["exclude_folders", "embedding_model"];

/// Load config with layered priority: defaults < global < per-vault < env < CLI.
///
/// `db_path` is the resolved database directory (may differ from `{vault}/.kajet`
/// for cloud-synced vaults). Per-vault config is read from `{db_path}/config.toml`.
pub fn load_config(
    db_path: &Path,
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
        .set_default("open_browser", false)?
        .set_default("resolve_wikilinks", true)?
        .set_default("logging.level", "debug")?
        .set_default("logging.file_level", "trace")?
        .set_default("logging.dashboard_level", "info")?
        .set_default("logging.progress_percent_step", 5_i64)?;

    // 2. Global config: ~/.config/kajet/config.toml
    if let Some(config_dir) = dirs::config_dir() {
        let global_path = config_dir.join("kajet").join("config.toml");
        builder = builder.add_source(File::from(global_path).required(false));
    }

    // 3. Per-vault config: {db_path}/config.toml
    let vault_config = db_path.join("config.toml");
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

/// Write updates to the vault-level config file ({db_path}/config.toml).
/// Only keys in `VAULT_FIELDS` are accepted.
pub fn write_vault_config(db_path: &Path, updates: &HashMap<String, toml::Value>) -> Result<()> {
    validate_fields(updates, VAULT_FIELDS, "vault")?;

    let config_path = db_path.join("config.toml");
    write_toml_config(&config_path, updates)
}

/// Reload config from all sources (re-runs the full load pipeline).
pub fn reload_config(
    db_path: &Path,
    cli_port: u16,
    cli_language: Option<String>,
) -> Result<KajetConfig> {
    load_config(db_path, cli_port, cli_language)
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

    // Deep merge: if both existing and new value are tables, merge keys instead of replacing
    for (key, value) in updates {
        if let (Some(toml::Value::Table(existing)), toml::Value::Table(incoming)) =
            (table.get_mut(key), value)
        {
            for (k, v) in incoming {
                existing.insert(k.clone(), v.clone());
            }
        } else {
            table.insert(key.clone(), value.clone());
        }
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
        let config =
            load_config(Path::new("/nonexistent/db_path"), 4000, Some("pl".into())).unwrap();
        assert_eq!(config.port, 4000);
        assert_eq!(config.language, "pl");
        assert_eq!(config.default_limit, 5);
    }

    #[test]
    fn load_config_with_cli_defaults() {
        // Note: global config file may exist on dev machine, so we test fields
        // that CLI explicitly overrides or that have no global override.
        let config =
            load_config(Path::new("/nonexistent/db_path"), 3579, Some("en".into())).unwrap();
        assert_eq!(config.port, 3579);
        assert_eq!(config.language, "en");
        assert_eq!(config.default_limit, 5);
        assert_eq!(
            config.embedding_model,
            "sentence-transformers/all-MiniLM-L6-v2"
        );
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

    #[test]
    fn logging_config_defaults() {
        let config = KajetConfig::default();
        assert_eq!(config.logging.level, "debug");
        assert_eq!(config.logging.file_level, "trace");
        assert_eq!(config.logging.dashboard_level, "info");
        assert_eq!(config.logging.progress_percent_step, 5);
    }

    #[test]
    fn logging_config_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut logging_table = toml::Table::new();
        logging_table.insert("level".into(), toml::Value::String("info".into()));
        logging_table.insert("file_level".into(), toml::Value::String("debug".into()));
        logging_table.insert("dashboard_level".into(), toml::Value::String("warn".into()));
        logging_table.insert("progress_percent_step".into(), toml::Value::Integer(10));

        let mut updates = HashMap::new();
        updates.insert("logging".into(), toml::Value::Table(logging_table));
        write_toml_config(&path, &updates).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        let table: toml::Table = content.parse().unwrap();
        let logging = table["logging"].as_table().unwrap();
        assert_eq!(logging["level"].as_str(), Some("info"));
        assert_eq!(logging["dashboard_level"].as_str(), Some("warn"));
        assert_eq!(logging["progress_percent_step"].as_integer(), Some(10));
    }

    #[test]
    fn deep_merge_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        // Write initial logging config
        let mut logging_table = toml::Table::new();
        logging_table.insert("level".into(), toml::Value::String("debug".into()));
        logging_table.insert("file_level".into(), toml::Value::String("trace".into()));
        let mut updates = HashMap::new();
        updates.insert("logging".into(), toml::Value::Table(logging_table));
        updates.insert("port".into(), toml::Value::Integer(3579));
        write_toml_config(&path, &updates).unwrap();

        // Update only dashboard_level — other logging keys should survive
        let mut logging_update = toml::Table::new();
        logging_update.insert(
            "dashboard_level".into(),
            toml::Value::String("error".into()),
        );
        let mut updates2 = HashMap::new();
        updates2.insert("logging".into(), toml::Value::Table(logging_update));
        write_toml_config(&path, &updates2).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        let table: toml::Table = content.parse().unwrap();
        assert_eq!(table["port"].as_integer(), Some(3579));
        let logging = table["logging"].as_table().unwrap();
        assert_eq!(logging["level"].as_str(), Some("debug"));
        assert_eq!(logging["file_level"].as_str(), Some("trace"));
        assert_eq!(logging["dashboard_level"].as_str(), Some("error"));
    }

    #[test]
    fn write_config_with_polish_values() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut updates = HashMap::new();
        updates.insert(
            "exclude_folders".into(),
            toml::Value::Array(vec![
                toml::Value::String("załączniki".into()),
                toml::Value::String("współpraca".into()),
                toml::Value::String(".kajet".into()),
            ]),
        );

        write_toml_config(&path, &updates).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        let table: toml::Table = content.parse().unwrap();
        let folders = table["exclude_folders"].as_array().unwrap();
        assert_eq!(folders[0].as_str(), Some("załączniki"));
        assert_eq!(folders[1].as_str(), Some("współpraca"));
    }
}
