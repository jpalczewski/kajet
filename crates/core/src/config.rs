use anyhow::Result;
use config::{Config, Environment, File};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct KajetConfig {
    pub port: u16,
    pub language: String,
    pub exclude_folders: Vec<String>,
    pub default_limit: usize,
}

impl Default for KajetConfig {
    fn default() -> Self {
        Self {
            port: 3579,
            language: "en".into(),
            exclude_folders: vec![".obsidian".into(), ".trash".into(), ".kajet".into()],
            default_limit: 5,
        }
    }
}

/// Load config with layered priority: defaults < global < per-vault < env < CLI.
pub fn load_config(
    vault_path: &str,
    cli_port: u16,
    cli_language: Option<String>,
) -> Result<KajetConfig> {
    let mut builder = Config::builder()
        // 1. Hardcoded defaults
        .set_default("port", 3579_i64)?
        .set_default("language", "en")?
        .set_default::<&str, Vec<String>>(
            "exclude_folders",
            vec![".obsidian".into(), ".trash".into(), ".kajet".into()],
        )?
        .set_default("default_limit", 5_i64)?;

    // 2. Global config: ~/.config/kajet/config.toml
    if let Some(config_dir) = dirs::config_dir() {
        let global_path = config_dir.join("kajet").join("config.toml");
        builder = builder.add_source(File::from(global_path).required(false));
    }

    // 3. Per-vault config: {vault}/.kajet/config.toml
    let vault_config = std::path::PathBuf::from(vault_path)
        .join(".kajet")
        .join("config.toml");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_values() {
        let config = KajetConfig::default();
        assert_eq!(config.port, 3579);
        assert_eq!(config.language, "en");
        assert_eq!(config.default_limit, 5);
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
    }
}
