use anyhow::{Result, bail};
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
pub struct TimestampConfig {
    pub enabled: bool,
    pub created_field: String,
    pub modified_field: String,
    pub format: String,
    pub timezone: String,
}

impl Default for TimestampConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            created_field: "created".into(),
            modified_field: "modified".into(),
            format: "iso8601".into(),
            timezone: "local".into(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, serde::Serialize)]
#[serde(default)]
pub struct FrontmatterConfig {
    pub default_tags: Vec<String>,
    pub created_date_field: Option<String>,
    pub modified_date_field: Option<String>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(default)]
pub struct WriterConfig {
    pub backup_enabled: bool,
    pub backup_max_per_file: usize,
    pub timestamps: TimestampConfig,
    pub frontmatter: FrontmatterConfig,
}

impl Default for WriterConfig {
    fn default() -> Self {
        Self {
            backup_enabled: true,
            backup_max_per_file: 10,
            timestamps: TimestampConfig::default(),
            frontmatter: FrontmatterConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(default)]
pub struct TreeConfig {
    pub depth: usize,
    pub size: usize,
    pub max_chars: usize,
}

pub const DEFAULT_TREE_DEPTH: usize = 3;
pub const DEFAULT_TREE_SIZE: usize = 50;
pub const DEFAULT_TREE_MAX_CHARS: usize = 5000;

impl Default for TreeConfig {
    fn default() -> Self {
        Self {
            depth: DEFAULT_TREE_DEPTH,
            size: DEFAULT_TREE_SIZE,
            max_chars: DEFAULT_TREE_MAX_CHARS,
        }
    }
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(default)]
pub struct ExploreConnectionsConfig {
    pub default_depth: usize,
    pub default_limit: usize,
    pub default_dedup: bool,
    pub default_include_context: bool,
    pub default_filter_mode: String,
}

impl Default for ExploreConnectionsConfig {
    fn default() -> Self {
        Self {
            default_depth: 3,
            default_limit: 100,
            default_dedup: true,
            default_include_context: false,
            default_filter_mode: "display".into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(default)]
pub struct FindSimilarConfig {
    pub default_limit: usize,
    pub default_threshold: f32,
    pub default_aggregation: String,
    pub default_exclude_linked: String,
}

impl Default for FindSimilarConfig {
    fn default() -> Self {
        Self {
            default_limit: 10,
            default_threshold: 0.6,
            default_aggregation: "max".into(),
            default_exclude_linked: "outgoing".into(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, serde::Serialize)]
#[serde(default)]
pub struct McpConfig {
    pub explore_connections: ExploreConnectionsConfig,
    pub find_similar: FindSimilarConfig,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EmbeddingBackend {
    Candle,
    Remote,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(default)]
pub struct EmbeddingConfig {
    pub backend: EmbeddingBackend,
    pub model: String,
    pub base_url: String,
    pub api_key: String,
    pub document_prefix: String,
    pub query_prefix: String,
    pub remote_max_batch_size: usize,
    pub remote_max_input_chars: usize,
    pub remote_max_concurrent_requests: usize,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            backend: EmbeddingBackend::Candle,
            model: "sentence-transformers/all-MiniLM-L6-v2".into(),
            base_url: "http://localhost:1234".into(),
            api_key: String::new(),
            document_prefix: String::new(),
            query_prefix: String::new(),
            remote_max_batch_size: 32,
            remote_max_input_chars: 1800,
            remote_max_concurrent_requests: 4,
        }
    }
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(default)]
pub struct SimilarityGraphConfig {
    pub enabled: bool,
    pub k: u32,
    pub boilerplate_patterns: Vec<String>,
}

impl Default for SimilarityGraphConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            k: 32,
            boilerplate_patterns: vec![
                "^Historia zmian$".into(),
                "^Powiązane dokumenty$".into(),
                "^Notatki z dnia$".into(),
                "^Notatki$".into(),
            ],
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
    pub embedding: EmbeddingConfig,
    pub open_browser: bool,
    pub resolve_wikilinks: bool,
    pub filter_overfetch_multiplier: usize,
    pub tags_only_fetch_limit: usize,
    pub logging: LoggingConfig,
    pub writer: WriterConfig,
    pub tree: TreeConfig,
    pub mcp: McpConfig,
    pub similarity_graph: SimilarityGraphConfig,
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
            embedding: EmbeddingConfig::default(),
            open_browser: false,
            resolve_wikilinks: true,
            filter_overfetch_multiplier: 3,
            tags_only_fetch_limit: 500,
            logging: LoggingConfig::default(),
            writer: WriterConfig::default(),
            tree: TreeConfig::default(),
            mcp: McpConfig::default(),
            similarity_graph: SimilarityGraphConfig::default(),
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
    "embedding",
    "open_browser",
    "resolve_wikilinks",
    "filter_overfetch_multiplier",
    "tags_only_fetch_limit",
    "logging",
    "writer",
    "tree",
    "mcp",
    "similarity_graph",
];

/// Fields allowed in vault-level config.
const VAULT_FIELDS: &[&str] = &[
    "exclude_folders",
    "embedding",
    "writer",
    "tree",
    "mcp",
    "similarity_graph",
];

/// Load config with layered priority: defaults < global < per-vault < env < CLI.
///
/// `db_path` is the resolved database directory (may differ from `{vault}/.kajet`
/// for cloud-synced vaults). Per-vault config is read from `{db_path}/config.toml`.
pub fn load_config(
    db_path: &Path,
    cli_port: u16,
    cli_language: Option<String>,
) -> Result<KajetConfig> {
    use config::{Config, Environment, File, FileFormat};

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
        .set_default("embedding.backend", "candle")?
        .set_default("embedding.model", "sentence-transformers/all-MiniLM-L6-v2")?
        .set_default("embedding.base_url", "http://localhost:1234")?
        .set_default("embedding.api_key", "")?
        .set_default("embedding.document_prefix", "")?
        .set_default("embedding.query_prefix", "")?
        .set_default("embedding.remote_max_batch_size", 32_i64)?
        .set_default("embedding.remote_max_input_chars", 1800_i64)?
        .set_default("embedding.remote_max_concurrent_requests", 4_i64)?
        .set_default("open_browser", false)?
        .set_default("resolve_wikilinks", true)?
        .set_default("filter_overfetch_multiplier", 3_i64)?
        .set_default("tags_only_fetch_limit", 500_i64)?
        .set_default("logging.level", "debug")?
        .set_default("logging.file_level", "trace")?
        .set_default("logging.dashboard_level", "info")?
        .set_default("logging.progress_percent_step", 5_i64)?
        .set_default("writer.backup_enabled", true)?
        .set_default("writer.backup_max_per_file", 10_i64)?
        .set_default("writer.timestamps.enabled", true)?
        .set_default("writer.timestamps.created_field", "created")?
        .set_default("writer.timestamps.modified_field", "modified")?
        .set_default("writer.timestamps.format", "iso8601")?
        .set_default("writer.timestamps.timezone", "local")?
        .set_default::<&str, Vec<String>>("writer.frontmatter.default_tags", vec![])?
        .set_default::<&str, Option<String>>("writer.frontmatter.date_field", None)?
        .set_default("tree.depth", 3_i64)?
        .set_default("tree.size", 50_i64)?
        .set_default("tree.max_chars", 5000_i64)?
        .set_default("mcp.explore_connections.default_depth", 3_i64)?
        .set_default("mcp.explore_connections.default_limit", 100_i64)?
        .set_default("mcp.explore_connections.default_dedup", true)?
        .set_default("mcp.explore_connections.default_include_context", false)?
        .set_default("mcp.explore_connections.default_filter_mode", "display")?
        .set_default("mcp.find_similar.default_limit", 10_i64)?
        .set_default("mcp.find_similar.default_threshold", 0.6_f64)?
        .set_default("mcp.find_similar.default_aggregation", "max")?
        .set_default("mcp.find_similar.default_exclude_linked", "outgoing")?;

    // 2. Global config: ~/.config/kajet/config.toml
    if let Some(config_dir) = dirs::config_dir() {
        let global_path = config_dir.join("kajet").join("config.toml");
        builder = builder.add_source(File::from(global_path.clone()).required(false));
        if let Some(legacy_model) = legacy_embedding_model_alias(&global_path)? {
            let mut embedding = toml::Table::new();
            embedding.insert("model".into(), toml::Value::String(legacy_model));
            let mut root = toml::Table::new();
            root.insert("embedding".into(), toml::Value::Table(embedding));
            let alias_toml = toml::to_string(&root)?;
            builder = builder.add_source(File::from_str(&alias_toml, FileFormat::Toml));
        }
    }

    // 3. Per-vault config: {db_path}/config.toml
    let vault_config = db_path.join("config.toml");
    builder = builder.add_source(File::from(vault_config).required(false));
    if let Some(legacy_model) = legacy_embedding_model_alias(&db_path.join("config.toml"))? {
        let mut embedding = toml::Table::new();
        embedding.insert("model".into(), toml::Value::String(legacy_model));
        let mut root = toml::Table::new();
        root.insert("embedding".into(), toml::Value::Table(embedding));
        let alias_toml = toml::to_string(&root)?;
        builder = builder.add_source(File::from_str(&alias_toml, FileFormat::Toml));
    }

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

    let built = builder.build()?;
    let config: KajetConfig = built.try_deserialize()?;
    Ok(config)
}

fn legacy_embedding_model_alias(path: &Path) -> Result<Option<String>> {
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(path)?;
    let parsed: toml::Value = content.parse()?;

    let has_new_embedding_model = parsed
        .get("embedding")
        .and_then(toml::Value::as_table)
        .and_then(|table| table.get("model"))
        .is_some();
    if has_new_embedding_model {
        return Ok(None);
    }

    Ok(parsed
        .get("embedding_model")
        .and_then(toml::Value::as_str)
        .map(str::to_string))
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

/// Read raw vault config without merging with global
pub fn read_vault_config(db_path: &Path) -> Result<toml::Table> {
    let vault_config_path = db_path.join("config.toml");

    if !vault_config_path.exists() {
        return Ok(toml::Table::new());
    }

    let content = std::fs::read_to_string(vault_config_path)?;
    let table: toml::Table = content.parse()?;
    Ok(table)
}

/// Read raw global config without merging with vault or defaults
pub fn read_global_config() -> Result<toml::Table> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine config directory"))?
        .join("kajet");
    let global_config_path = config_dir.join("config.toml");

    if !global_config_path.exists() {
        return Ok(toml::Table::new());
    }

    let content = std::fs::read_to_string(global_config_path)?;
    let table: toml::Table = content.parse()?;
    Ok(table)
}

/// Delete specified fields from vault config.
/// Accepts field paths like "exclude_folders" or "embedding.model".
pub fn delete_vault_config_fields(db_path: &Path, field_paths: &[String]) -> Result<()> {
    let config_path = db_path.join("config.toml");

    // If config doesn't exist, nothing to delete
    if !config_path.exists() {
        return Ok(());
    }

    let content = std::fs::read_to_string(&config_path)?;
    let mut table: toml::Table = content.parse()?;

    for path in field_paths {
        // Split path by '.' for nested fields (e.g., "embedding.model")
        let parts: Vec<&str> = path.split('.').collect();

        if parts.len() == 1 {
            // Top-level field
            table.remove(parts[0]);
        } else if parts.len() == 2 {
            // Nested field (e.g., "embedding.model")
            if let Some(toml::Value::Table(section)) = table.get_mut(parts[0]) {
                section.remove(parts[1]);
                // If section is now empty, remove it entirely
                if section.is_empty() {
                    table.remove(parts[0]);
                }
            }
        } else {
            // Deeper nesting not currently needed, but could be added
            bail!("Field path with more than 2 levels not supported: {}", path);
        }
    }

    // Write back the modified config
    let content = toml::to_string_pretty(&table)?;
    let tmp_path = config_path.with_extension("toml.tmp");
    std::fs::write(&tmp_path, &content)?;
    std::fs::rename(&tmp_path, &config_path)?;

    Ok(())
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
    fn embedding_config_defaults() {
        let cfg = EmbeddingConfig::default();
        assert_eq!(cfg.backend, EmbeddingBackend::Candle);
        assert_eq!(cfg.model, "sentence-transformers/all-MiniLM-L6-v2");
        assert_eq!(cfg.base_url, "http://localhost:1234");
        assert!(cfg.api_key.is_empty());
        assert!(cfg.document_prefix.is_empty());
        assert!(cfg.query_prefix.is_empty());
        assert_eq!(cfg.remote_max_batch_size, 32);
        assert_eq!(cfg.remote_max_input_chars, 1800);
    }

    #[test]
    fn embedding_backend_serde_roundtrip() {
        let candle: EmbeddingBackend = serde_json::from_str("\"candle\"").unwrap();
        assert_eq!(candle, EmbeddingBackend::Candle);
        let remote: EmbeddingBackend = serde_json::from_str("\"remote\"").unwrap();
        assert_eq!(remote, EmbeddingBackend::Remote);
        assert_eq!(
            serde_json::to_string(&EmbeddingBackend::Remote).unwrap(),
            "\"remote\""
        );
    }

    #[test]
    fn default_config_values() {
        let config = KajetConfig::default();
        assert_eq!(config.port, 3579);
        assert_eq!(config.language, "en");
        assert_eq!(config.default_limit, 5);
        assert_eq!(
            config.embedding.model,
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
        let mut embedding = toml::Table::new();
        embedding.insert("model".into(), toml::Value::String("my-model".into()));
        updates2.insert("embedding".into(), toml::Value::Table(embedding));
        write_toml_config(&path, &updates2).unwrap();

        let content2 = fs::read_to_string(&path).unwrap();
        let table2: toml::Table = content2.parse().unwrap();
        assert_eq!(table2["language"].as_str(), Some("pl"));
        assert_eq!(table2["embedding"]["model"].as_str(), Some("my-model"));
    }

    #[test]
    fn config_accepts_legacy_embedding_model_key() {
        let dir = tempfile::tempdir().unwrap();
        let cfg_path = dir.path().join("config.toml");
        fs::write(
            &cfg_path,
            r#"
embedding_model = "legacy-model"
"#,
        )
        .unwrap();

        let cfg = load_config(dir.path(), 3579, Some("en".into())).unwrap();
        assert_eq!(cfg.embedding.model, "legacy-model");
    }

    #[test]
    fn config_prefers_new_embedding_model_over_legacy() {
        let dir = tempfile::tempdir().unwrap();
        let cfg_path = dir.path().join("config.toml");
        fs::write(
            &cfg_path,
            r#"
embedding_model = "legacy-model"
[embedding]
model = "new-model"
"#,
        )
        .unwrap();

        let cfg = load_config(dir.path(), 3579, Some("en".into())).unwrap();
        assert_eq!(cfg.embedding.model, "new-model");
    }

    #[test]
    fn legacy_embedding_alias_detects_legacy_key() {
        let dir = tempfile::tempdir().unwrap();
        let cfg_path = dir.path().join("config.toml");
        fs::write(&cfg_path, "embedding_model = \"legacy-model\"\n").unwrap();

        let alias = legacy_embedding_model_alias(&cfg_path).unwrap();
        assert_eq!(alias, Some("legacy-model".to_string()));
    }

    #[test]
    fn legacy_embedding_alias_ignores_when_new_key_exists() {
        let dir = tempfile::tempdir().unwrap();
        let cfg_path = dir.path().join("config.toml");
        fs::write(
            &cfg_path,
            r#"
embedding_model = "legacy-model"
[embedding]
model = "new-model"
"#,
        )
        .unwrap();

        let alias = legacy_embedding_model_alias(&cfg_path).unwrap();
        assert_eq!(alias, None);
    }

    #[test]
    fn tree_config_defaults() {
        let config = KajetConfig::default();
        assert_eq!(config.tree.depth, 3);
        assert_eq!(config.tree.size, 50);
        assert_eq!(config.tree.max_chars, 5000);
    }

    #[test]
    fn tree_config_allowed_in_global_and_vault() {
        let mut updates = HashMap::new();
        let mut tree_table = toml::Table::new();
        tree_table.insert("depth".into(), toml::Value::Integer(5));
        updates.insert("tree".into(), toml::Value::Table(tree_table));
        assert!(validate_fields(&updates, GLOBAL_FIELDS, "global").is_ok());
        assert!(validate_fields(&updates, VAULT_FIELDS, "vault").is_ok());
    }

    #[test]
    fn tree_config_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut tree_table = toml::Table::new();
        tree_table.insert("depth".into(), toml::Value::Integer(5));
        tree_table.insert("size".into(), toml::Value::Integer(100));
        tree_table.insert("max_chars".into(), toml::Value::Integer(10000));

        let mut updates = HashMap::new();
        updates.insert("tree".into(), toml::Value::Table(tree_table));
        write_toml_config(&path, &updates).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        let table: toml::Table = content.parse().unwrap();
        let tree = table["tree"].as_table().unwrap();
        assert_eq!(tree["depth"].as_integer(), Some(5));
        assert_eq!(tree["size"].as_integer(), Some(100));
        assert_eq!(tree["max_chars"].as_integer(), Some(10000));
    }

    #[test]
    fn explore_connections_config_defaults() {
        let config = KajetConfig::default();
        assert_eq!(config.mcp.explore_connections.default_depth, 3);
        assert_eq!(config.mcp.explore_connections.default_limit, 100);
        assert!(config.mcp.explore_connections.default_dedup);
        assert!(!config.mcp.explore_connections.default_include_context);
        assert_eq!(
            config.mcp.explore_connections.default_filter_mode,
            "display"
        );
    }

    #[test]
    fn explore_connections_config_allowed_in_global_and_vault() {
        let mut updates = HashMap::new();
        let mut explore_table = toml::Table::new();
        explore_table.insert("default_depth".into(), toml::Value::Integer(5));

        let mut mcp_table = toml::Table::new();
        mcp_table.insert(
            "explore_connections".into(),
            toml::Value::Table(explore_table),
        );
        updates.insert("mcp".into(), toml::Value::Table(mcp_table));

        assert!(validate_fields(&updates, GLOBAL_FIELDS, "global").is_ok());
        assert!(validate_fields(&updates, VAULT_FIELDS, "vault").is_ok());
    }

    #[test]
    fn find_similar_config_defaults() {
        let config = KajetConfig::default();
        assert_eq!(config.mcp.find_similar.default_limit, 10);
        assert!((config.mcp.find_similar.default_threshold - 0.6).abs() < f32::EPSILON);
        assert_eq!(config.mcp.find_similar.default_aggregation, "max");
        assert_eq!(config.mcp.find_similar.default_exclude_linked, "outgoing");
    }

    #[test]
    fn find_similar_config_allowed_in_global_and_vault() {
        let mut updates = HashMap::new();
        let mut similar_table = toml::Table::new();
        similar_table.insert("default_limit".into(), toml::Value::Integer(20));

        let mut mcp_table = toml::Table::new();
        mcp_table.insert("find_similar".into(), toml::Value::Table(similar_table));
        updates.insert("mcp".into(), toml::Value::Table(mcp_table));

        assert!(validate_fields(&updates, GLOBAL_FIELDS, "global").is_ok());
        assert!(validate_fields(&updates, VAULT_FIELDS, "vault").is_ok());
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
    fn writer_config_defaults() {
        let config = KajetConfig::default();
        assert!(config.writer.backup_enabled);
        assert_eq!(config.writer.backup_max_per_file, 10);
        assert!(config.writer.timestamps.enabled);
        assert_eq!(config.writer.timestamps.created_field, "created");
        assert_eq!(config.writer.timestamps.modified_field, "modified");
        assert_eq!(config.writer.timestamps.format, "iso8601");
        assert_eq!(config.writer.timestamps.timezone, "local");
        assert!(config.writer.frontmatter.default_tags.is_empty());
    }

    #[test]
    fn writer_config_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut ts = toml::Table::new();
        ts.insert("enabled".into(), toml::Value::Boolean(false));
        ts.insert(
            "created_field".into(),
            toml::Value::String("date_created".into()),
        );
        ts.insert("format".into(), toml::Value::String("date_only".into()));
        ts.insert(
            "timezone".into(),
            toml::Value::String("Europe/Warsaw".into()),
        );

        let mut fm = toml::Table::new();
        fm.insert(
            "default_tags".into(),
            toml::Value::Array(vec![toml::Value::String("kajet".into())]),
        );

        let mut writer = toml::Table::new();
        writer.insert("backup_enabled".into(), toml::Value::Boolean(false));
        writer.insert("backup_max_per_file".into(), toml::Value::Integer(5));
        writer.insert("timestamps".into(), toml::Value::Table(ts));
        writer.insert("frontmatter".into(), toml::Value::Table(fm));

        let mut updates = HashMap::new();
        updates.insert("writer".into(), toml::Value::Table(writer));
        write_toml_config(&path, &updates).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        let table: toml::Table = content.parse().unwrap();
        let w = table["writer"].as_table().unwrap();
        assert_eq!(w["backup_enabled"].as_bool(), Some(false));
        assert_eq!(w["backup_max_per_file"].as_integer(), Some(5));
        let ts = w["timestamps"].as_table().unwrap();
        assert_eq!(ts["enabled"].as_bool(), Some(false));
        assert_eq!(ts["format"].as_str(), Some("date_only"));
        assert_eq!(ts["timezone"].as_str(), Some("Europe/Warsaw"));
        let fm = w["frontmatter"].as_table().unwrap();
        let tags = fm["default_tags"].as_array().unwrap();
        assert_eq!(tags[0].as_str(), Some("kajet"));
    }

    #[test]
    fn writer_config_deep_merge() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        // Write initial writer config
        let mut ts = toml::Table::new();
        ts.insert("enabled".into(), toml::Value::Boolean(true));
        ts.insert("format".into(), toml::Value::String("iso8601".into()));
        let mut writer = toml::Table::new();
        writer.insert("backup_enabled".into(), toml::Value::Boolean(true));
        writer.insert("timestamps".into(), toml::Value::Table(ts));
        let mut updates = HashMap::new();
        updates.insert("writer".into(), toml::Value::Table(writer));
        write_toml_config(&path, &updates).unwrap();

        // Update only backup_enabled — timestamps should survive
        let mut writer_update = toml::Table::new();
        writer_update.insert("backup_enabled".into(), toml::Value::Boolean(false));
        let mut updates2 = HashMap::new();
        updates2.insert("writer".into(), toml::Value::Table(writer_update));
        write_toml_config(&path, &updates2).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        let table: toml::Table = content.parse().unwrap();
        let w = table["writer"].as_table().unwrap();
        assert_eq!(w["backup_enabled"].as_bool(), Some(false));
        // timestamps sub-table should be preserved
        assert!(w.contains_key("timestamps"));
        let ts = w["timestamps"].as_table().unwrap();
        assert_eq!(ts["enabled"].as_bool(), Some(true));
        assert_eq!(ts["format"].as_str(), Some("iso8601"));
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

    #[test]
    fn search_tuning_config_defaults() {
        let config = KajetConfig::default();
        assert_eq!(config.filter_overfetch_multiplier, 3);
        assert_eq!(config.tags_only_fetch_limit, 500);
    }

    #[test]
    fn search_tuning_config_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut updates = HashMap::new();
        updates.insert(
            "filter_overfetch_multiplier".into(),
            toml::Value::Integer(5),
        );
        updates.insert("tags_only_fetch_limit".into(), toml::Value::Integer(1000));
        write_toml_config(&path, &updates).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        let table: toml::Table = content.parse().unwrap();
        assert_eq!(table["filter_overfetch_multiplier"].as_integer(), Some(5));
        assert_eq!(table["tags_only_fetch_limit"].as_integer(), Some(1000));
    }

    #[test]
    fn search_tuning_config_allowed_in_global() {
        let mut updates = HashMap::new();
        updates.insert(
            "filter_overfetch_multiplier".into(),
            toml::Value::Integer(4),
        );
        updates.insert("tags_only_fetch_limit".into(), toml::Value::Integer(800));
        assert!(validate_fields(&updates, GLOBAL_FIELDS, "global").is_ok());
    }

    #[test]
    fn vault_config_survives_reload_with_original_cli_args() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path();

        // Write vault override: embedding.backend = remote
        let mut embedding = toml::Table::new();
        embedding.insert("backend".into(), toml::Value::String("remote".into()));
        let mut updates = HashMap::new();
        updates.insert("embedding".into(), toml::Value::Table(embedding));
        write_vault_config(db_path, &updates).unwrap();

        // Load with CLI defaults (port=3579, no language override)
        let cfg = load_config(db_path, 3579, None).unwrap();
        assert_eq!(
            cfg.embedding.backend,
            EmbeddingBackend::Remote,
            "vault override should be active"
        );

        // Simulate reload with ORIGINAL CLI args (not current config's port)
        let reloaded = reload_config(db_path, 3579, None).unwrap();
        assert_eq!(
            reloaded.embedding.backend,
            EmbeddingBackend::Remote,
            "vault override must survive reload"
        );
    }

    #[test]
    fn vault_config_lost_when_reloading_with_mutated_port() {
        // This test demonstrates that reloading with the *current* config's port
        // (instead of the original CLI port) causes vault overrides to be masked.
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path();

        // Global: port=3579 (default)
        // Vault override: embedding.backend = remote
        let mut embedding = toml::Table::new();
        embedding.insert("backend".into(), toml::Value::String("remote".into()));
        let mut updates = HashMap::new();
        updates.insert("embedding".into(), toml::Value::Table(embedding));
        write_vault_config(db_path, &updates).unwrap();

        let cfg = load_config(db_path, 3579, None).unwrap();
        assert_eq!(cfg.embedding.backend, EmbeddingBackend::Remote);

        // BUG SCENARIO: reload with cfg.port (same value but demonstrates the pattern)
        // The real bug is that web handlers pass cfg.port which may differ from CLI port
        let reloaded = reload_config(db_path, cfg.port, None).unwrap();
        assert_eq!(reloaded.embedding.backend, EmbeddingBackend::Remote);
    }

    #[test]
    fn read_vault_config_returns_empty_when_nonexistent() {
        let dir = tempfile::tempdir().unwrap();
        let table = read_vault_config(dir.path()).unwrap();
        assert!(table.is_empty());
    }

    #[test]
    fn read_vault_config_returns_raw_toml() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path();

        // Write vault config with embedding override
        let mut embedding = toml::Table::new();
        embedding.insert("backend".into(), toml::Value::String("remote".into()));
        embedding.insert("model".into(), toml::Value::String("custom-model".into()));
        let mut updates = HashMap::new();
        updates.insert("embedding".into(), toml::Value::Table(embedding));
        write_vault_config(db_path, &updates).unwrap();

        // Read raw vault config
        let table = read_vault_config(db_path).unwrap();
        assert!(table.contains_key("embedding"));
        let emb = table["embedding"].as_table().unwrap();
        assert_eq!(emb["backend"].as_str(), Some("remote"));
        assert_eq!(emb["model"].as_str(), Some("custom-model"));
    }

    #[test]
    fn read_vault_config_does_not_merge_with_global() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path();

        // Write partial vault config (only backend, not model)
        let mut embedding = toml::Table::new();
        embedding.insert("backend".into(), toml::Value::String("remote".into()));
        let mut updates = HashMap::new();
        updates.insert("embedding".into(), toml::Value::Table(embedding));
        write_vault_config(db_path, &updates).unwrap();

        // Read raw vault config - should only contain what was written
        let table = read_vault_config(db_path).unwrap();
        let emb = table["embedding"].as_table().unwrap();
        assert_eq!(emb["backend"].as_str(), Some("remote"));
        // model should NOT be present (unlike load_config which merges with defaults)
        assert!(!emb.contains_key("model"));
    }
}
