use crate::config::KajetConfig;
use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../frontend/src/lib/types/generated/")]
pub struct ConfigSchema {
    pub sections: Vec<SchemaSection>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../frontend/src/lib/types/generated/")]
pub struct SchemaSection {
    pub key: String,
    pub i18n_key: String,
    pub fields: Vec<SchemaField>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../frontend/src/lib/types/generated/")]
pub struct SchemaField {
    pub key: String,
    pub i18n_key: String,
    pub field_type: FieldType,
    #[ts(type = "any")]
    pub default_value: serde_json::Value,
    pub constraints: Option<FieldConstraints>,
    pub widget: Option<String>,
    pub scope: FieldScope,
    pub restart_required: bool,
    pub hot_swap: bool,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../frontend/src/lib/types/generated/")]
#[serde(tag = "type", content = "data")]
pub enum FieldType {
    String,
    Number,
    Bool,
    Enum { options: Vec<String> },
    Array { item_type: Box<FieldType> },
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../frontend/src/lib/types/generated/")]
pub struct FieldConstraints {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../frontend/src/lib/types/generated/")]
pub enum FieldScope {
    Global,
    Vault,
    Both,
}

/// Build complete config schema from current config values.
pub fn build_config_schema(config: &KajetConfig) -> ConfigSchema {
    ConfigSchema {
        sections: vec![
            build_general_section(config),
            build_embedding_section(config),
            build_logging_section(config),
            build_writer_section(config),
            build_tree_section(config),
            build_search_tuning_section(config),
        ],
    }
}

fn build_general_section(config: &KajetConfig) -> SchemaSection {
    SchemaSection {
        key: "general".to_string(),
        i18n_key: "settings_section_general".to_string(),
        fields: vec![
            SchemaField {
                key: "port".to_string(),
                i18n_key: "settings_field_general_port".to_string(),
                field_type: FieldType::Number,
                default_value: serde_json::json!(config.port),
                constraints: Some(FieldConstraints {
                    min: Some(1024.0),
                    max: Some(65535.0),
                }),
                widget: Some("number".to_string()),
                scope: FieldScope::Global,
                restart_required: true,
                hot_swap: false,
            },
            SchemaField {
                key: "language".to_string(),
                i18n_key: "settings_field_general_language".to_string(),
                field_type: FieldType::String,
                default_value: serde_json::json!(config.language),
                constraints: None,
                widget: Some("text".to_string()),
                scope: FieldScope::Global,
                restart_required: false,
                hot_swap: false,
            },
            SchemaField {
                key: "exclude_folders".to_string(),
                i18n_key: "settings_field_general_exclude_folders".to_string(),
                field_type: FieldType::Array {
                    item_type: Box::new(FieldType::String),
                },
                default_value: serde_json::json!(config.exclude_folders),
                constraints: None,
                widget: Some("array".to_string()),
                scope: FieldScope::Both,
                restart_required: false,
                hot_swap: false,
            },
            SchemaField {
                key: "default_limit".to_string(),
                i18n_key: "settings_field_general_default_limit".to_string(),
                field_type: FieldType::Number,
                default_value: serde_json::json!(config.default_limit),
                constraints: None,
                widget: Some("number".to_string()),
                scope: FieldScope::Global,
                restart_required: false,
                hot_swap: false,
            },
            SchemaField {
                key: "max_concurrent_files".to_string(),
                i18n_key: "settings_field_general_max_concurrent_files".to_string(),
                field_type: FieldType::Number,
                default_value: serde_json::json!(config.max_concurrent_files),
                constraints: None,
                widget: Some("number".to_string()),
                scope: FieldScope::Global,
                restart_required: false,
                hot_swap: false,
            },
            SchemaField {
                key: "pipeline_buffer_size".to_string(),
                i18n_key: "settings_field_general_pipeline_buffer_size".to_string(),
                field_type: FieldType::Number,
                default_value: serde_json::json!(config.pipeline_buffer_size),
                constraints: None,
                widget: Some("number".to_string()),
                scope: FieldScope::Global,
                restart_required: false,
                hot_swap: false,
            },
            SchemaField {
                key: "open_browser".to_string(),
                i18n_key: "settings_field_general_open_browser".to_string(),
                field_type: FieldType::Bool,
                default_value: serde_json::json!(config.open_browser),
                constraints: None,
                widget: Some("toggle".to_string()),
                scope: FieldScope::Global,
                restart_required: false,
                hot_swap: false,
            },
            SchemaField {
                key: "resolve_wikilinks".to_string(),
                i18n_key: "settings_field_general_resolve_wikilinks".to_string(),
                field_type: FieldType::Bool,
                default_value: serde_json::json!(config.resolve_wikilinks),
                constraints: None,
                widget: Some("toggle".to_string()),
                scope: FieldScope::Global,
                restart_required: false,
                hot_swap: false,
            },
        ],
    }
}

fn build_embedding_section(config: &KajetConfig) -> SchemaSection {
    SchemaSection {
        key: "embedding".to_string(),
        i18n_key: "settings_section_embedding".to_string(),
        fields: vec![
            SchemaField {
                key: "backend".to_string(),
                i18n_key: "settings_field_embedding_backend".to_string(),
                field_type: FieldType::Enum {
                    options: vec!["candle".to_string(), "remote".to_string()],
                },
                default_value: serde_json::json!(config.embedding.backend),
                constraints: None,
                widget: Some("select".to_string()),
                scope: FieldScope::Both,
                restart_required: false,
                hot_swap: true,
            },
            SchemaField {
                key: "model".to_string(),
                i18n_key: "settings_field_embedding_model".to_string(),
                field_type: FieldType::String,
                default_value: serde_json::json!(config.embedding.model),
                constraints: None,
                widget: Some("text".to_string()),
                scope: FieldScope::Both,
                restart_required: false,
                hot_swap: true,
            },
            SchemaField {
                key: "base_url".to_string(),
                i18n_key: "settings_field_embedding_base_url".to_string(),
                field_type: FieldType::String,
                default_value: serde_json::json!(config.embedding.base_url),
                constraints: None,
                widget: Some("text".to_string()),
                scope: FieldScope::Both,
                restart_required: false,
                hot_swap: true,
            },
            SchemaField {
                key: "api_key".to_string(),
                i18n_key: "settings_field_embedding_api_key".to_string(),
                field_type: FieldType::String,
                default_value: serde_json::json!(config.embedding.api_key),
                constraints: None,
                widget: Some("text".to_string()),
                scope: FieldScope::Both,
                restart_required: false,
                hot_swap: true,
            },
            SchemaField {
                key: "document_prefix".to_string(),
                i18n_key: "settings_field_embedding_document_prefix".to_string(),
                field_type: FieldType::String,
                default_value: serde_json::json!(config.embedding.document_prefix),
                constraints: None,
                widget: Some("text".to_string()),
                scope: FieldScope::Both,
                restart_required: false,
                hot_swap: false,
            },
            SchemaField {
                key: "query_prefix".to_string(),
                i18n_key: "settings_field_embedding_query_prefix".to_string(),
                field_type: FieldType::String,
                default_value: serde_json::json!(config.embedding.query_prefix),
                constraints: None,
                widget: Some("text".to_string()),
                scope: FieldScope::Both,
                restart_required: false,
                hot_swap: false,
            },
            SchemaField {
                key: "remote_max_batch_size".to_string(),
                i18n_key: "settings_field_embedding_remote_max_batch_size".to_string(),
                field_type: FieldType::Number,
                default_value: serde_json::json!(config.embedding.remote_max_batch_size),
                constraints: None,
                widget: Some("number".to_string()),
                scope: FieldScope::Both,
                restart_required: false,
                hot_swap: false,
            },
            SchemaField {
                key: "remote_max_input_chars".to_string(),
                i18n_key: "settings_field_embedding_remote_max_input_chars".to_string(),
                field_type: FieldType::Number,
                default_value: serde_json::json!(config.embedding.remote_max_input_chars),
                constraints: None,
                widget: Some("number".to_string()),
                scope: FieldScope::Both,
                restart_required: false,
                hot_swap: false,
            },
        ],
    }
}

fn build_logging_section(config: &KajetConfig) -> SchemaSection {
    SchemaSection {
        key: "logging".to_string(),
        i18n_key: "settings_section_logging".to_string(),
        fields: vec![
            SchemaField {
                key: "level".to_string(),
                i18n_key: "settings_field_logging_level".to_string(),
                field_type: FieldType::Enum {
                    options: vec![
                        "trace".to_string(),
                        "debug".to_string(),
                        "info".to_string(),
                        "warn".to_string(),
                        "error".to_string(),
                    ],
                },
                default_value: serde_json::json!(config.logging.level),
                constraints: None,
                widget: Some("select".to_string()),
                scope: FieldScope::Global,
                restart_required: false,
                hot_swap: true,
            },
            SchemaField {
                key: "file_level".to_string(),
                i18n_key: "settings_field_logging_file_level".to_string(),
                field_type: FieldType::Enum {
                    options: vec![
                        "trace".to_string(),
                        "debug".to_string(),
                        "info".to_string(),
                        "warn".to_string(),
                        "error".to_string(),
                    ],
                },
                default_value: serde_json::json!(config.logging.file_level),
                constraints: None,
                widget: Some("select".to_string()),
                scope: FieldScope::Global,
                restart_required: false,
                hot_swap: true,
            },
            SchemaField {
                key: "dashboard_level".to_string(),
                i18n_key: "settings_field_logging_dashboard_level".to_string(),
                field_type: FieldType::Enum {
                    options: vec![
                        "trace".to_string(),
                        "debug".to_string(),
                        "info".to_string(),
                        "warn".to_string(),
                        "error".to_string(),
                    ],
                },
                default_value: serde_json::json!(config.logging.dashboard_level),
                constraints: None,
                widget: Some("select".to_string()),
                scope: FieldScope::Global,
                restart_required: false,
                hot_swap: true,
            },
            SchemaField {
                key: "progress_percent_step".to_string(),
                i18n_key: "settings_field_logging_progress_percent_step".to_string(),
                field_type: FieldType::Number,
                default_value: serde_json::json!(config.logging.progress_percent_step),
                constraints: Some(FieldConstraints {
                    min: Some(1.0),
                    max: Some(100.0),
                }),
                widget: Some("number".to_string()),
                scope: FieldScope::Global,
                restart_required: false,
                hot_swap: true,
            },
        ],
    }
}

fn build_writer_section(config: &KajetConfig) -> SchemaSection {
    SchemaSection {
        key: "writer".to_string(),
        i18n_key: "settings_section_writer".to_string(),
        fields: vec![
            SchemaField {
                key: "backup_enabled".to_string(),
                i18n_key: "settings_field_writer_backup_enabled".to_string(),
                field_type: FieldType::Bool,
                default_value: serde_json::json!(config.writer.backup_enabled),
                constraints: None,
                widget: Some("toggle".to_string()),
                scope: FieldScope::Both,
                restart_required: false,
                hot_swap: false,
            },
            SchemaField {
                key: "backup_max_per_file".to_string(),
                i18n_key: "settings_field_writer_backup_max_per_file".to_string(),
                field_type: FieldType::Number,
                default_value: serde_json::json!(config.writer.backup_max_per_file),
                constraints: Some(FieldConstraints {
                    min: Some(0.0),
                    max: Some(100.0),
                }),
                widget: Some("number".to_string()),
                scope: FieldScope::Both,
                restart_required: false,
                hot_swap: false,
            },
            SchemaField {
                key: "timestamps_enabled".to_string(),
                i18n_key: "settings_field_writer_timestamps_enabled".to_string(),
                field_type: FieldType::Bool,
                default_value: serde_json::json!(config.writer.timestamps.enabled),
                constraints: None,
                widget: Some("toggle".to_string()),
                scope: FieldScope::Both,
                restart_required: false,
                hot_swap: false,
            },
            SchemaField {
                key: "timestamps_created_field".to_string(),
                i18n_key: "settings_field_writer_timestamps_created_field".to_string(),
                field_type: FieldType::String,
                default_value: serde_json::json!(config.writer.timestamps.created_field),
                constraints: None,
                widget: Some("text".to_string()),
                scope: FieldScope::Both,
                restart_required: false,
                hot_swap: false,
            },
            SchemaField {
                key: "timestamps_modified_field".to_string(),
                i18n_key: "settings_field_writer_timestamps_modified_field".to_string(),
                field_type: FieldType::String,
                default_value: serde_json::json!(config.writer.timestamps.modified_field),
                constraints: None,
                widget: Some("text".to_string()),
                scope: FieldScope::Both,
                restart_required: false,
                hot_swap: false,
            },
            SchemaField {
                key: "timestamps_format".to_string(),
                i18n_key: "settings_field_writer_timestamps_format".to_string(),
                field_type: FieldType::Enum {
                    options: vec!["iso8601".to_string(), "date_only".to_string()],
                },
                default_value: serde_json::json!(config.writer.timestamps.format),
                constraints: None,
                widget: Some("select".to_string()),
                scope: FieldScope::Both,
                restart_required: false,
                hot_swap: false,
            },
            SchemaField {
                key: "timestamps_timezone".to_string(),
                i18n_key: "settings_field_writer_timestamps_timezone".to_string(),
                field_type: FieldType::String,
                default_value: serde_json::json!(config.writer.timestamps.timezone),
                constraints: None,
                widget: Some("text".to_string()),
                scope: FieldScope::Both,
                restart_required: false,
                hot_swap: false,
            },
            SchemaField {
                key: "frontmatter_default_tags".to_string(),
                i18n_key: "settings_field_writer_frontmatter_default_tags".to_string(),
                field_type: FieldType::Array {
                    item_type: Box::new(FieldType::String),
                },
                default_value: serde_json::json!(config.writer.frontmatter.default_tags),
                constraints: None,
                widget: Some("array".to_string()),
                scope: FieldScope::Both,
                restart_required: false,
                hot_swap: false,
            },
            SchemaField {
                key: "frontmatter_created_date_field".to_string(),
                i18n_key: "settings_field_writer_frontmatter_created_date_field".to_string(),
                field_type: FieldType::String,
                default_value: serde_json::to_value(&config.writer.frontmatter.created_date_field)
                    .unwrap(),
                constraints: None,
                widget: Some("text".to_string()),
                scope: FieldScope::Both,
                restart_required: false,
                hot_swap: false,
            },
            SchemaField {
                key: "frontmatter_modified_date_field".to_string(),
                i18n_key: "settings_field_writer_frontmatter_modified_date_field".to_string(),
                field_type: FieldType::String,
                default_value: serde_json::to_value(&config.writer.frontmatter.modified_date_field)
                    .unwrap(),
                constraints: None,
                widget: Some("text".to_string()),
                scope: FieldScope::Both,
                restart_required: false,
                hot_swap: false,
            },
        ],
    }
}

fn build_tree_section(config: &KajetConfig) -> SchemaSection {
    SchemaSection {
        key: "tree".to_string(),
        i18n_key: "settings_section_tree".to_string(),
        fields: vec![
            SchemaField {
                key: "depth".to_string(),
                i18n_key: "settings_field_tree_depth".to_string(),
                field_type: FieldType::Number,
                default_value: serde_json::json!(config.tree.depth),
                constraints: Some(FieldConstraints {
                    min: Some(1.0),
                    max: Some(10.0),
                }),
                widget: Some("number".to_string()),
                scope: FieldScope::Both,
                restart_required: false,
                hot_swap: false,
            },
            SchemaField {
                key: "size".to_string(),
                i18n_key: "settings_field_tree_size".to_string(),
                field_type: FieldType::Number,
                default_value: serde_json::json!(config.tree.size),
                constraints: Some(FieldConstraints {
                    min: Some(1.0),
                    max: Some(1000.0),
                }),
                widget: Some("number".to_string()),
                scope: FieldScope::Both,
                restart_required: false,
                hot_swap: false,
            },
            SchemaField {
                key: "max_chars".to_string(),
                i18n_key: "settings_field_tree_max_chars".to_string(),
                field_type: FieldType::Number,
                default_value: serde_json::json!(config.tree.max_chars),
                constraints: Some(FieldConstraints {
                    min: Some(100.0),
                    max: Some(100000.0),
                }),
                widget: Some("number".to_string()),
                scope: FieldScope::Both,
                restart_required: false,
                hot_swap: false,
            },
        ],
    }
}

fn build_search_tuning_section(config: &KajetConfig) -> SchemaSection {
    SchemaSection {
        key: "search_tuning".to_string(),
        i18n_key: "settings_section_search_tuning".to_string(),
        fields: vec![
            SchemaField {
                key: "filter_overfetch_multiplier".to_string(),
                i18n_key: "settings_field_search_tuning_filter_overfetch_multiplier".to_string(),
                field_type: FieldType::Number,
                default_value: serde_json::json!(config.filter_overfetch_multiplier),
                constraints: None,
                widget: Some("number".to_string()),
                scope: FieldScope::Global,
                restart_required: false,
                hot_swap: false,
            },
            SchemaField {
                key: "tags_only_fetch_limit".to_string(),
                i18n_key: "settings_field_search_tuning_tags_only_fetch_limit".to_string(),
                field_type: FieldType::Number,
                default_value: serde_json::json!(config.tags_only_fetch_limit),
                constraints: None,
                widget: Some("number".to_string()),
                scope: FieldScope::Global,
                restart_required: false,
                hot_swap: false,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_has_embedding_section() {
        let config = KajetConfig::default();
        let schema = build_config_schema(&config);
        let embedding = schema.sections.iter().find(|s| s.key == "embedding");
        assert!(embedding.is_some(), "schema must have embedding section");
        let fields = &embedding.unwrap().fields;
        assert!(fields.iter().any(|f| f.key == "backend"));
        assert!(fields.iter().any(|f| f.key == "model"));
    }

    #[test]
    fn schema_has_all_sections() {
        let config = KajetConfig::default();
        let schema = build_config_schema(&config);
        assert_eq!(schema.sections.len(), 6);

        let section_keys: Vec<&str> = schema.sections.iter().map(|s| s.key.as_str()).collect();
        assert_eq!(
            section_keys,
            vec![
                "general",
                "embedding",
                "logging",
                "writer",
                "tree",
                "search_tuning"
            ]
        );
    }

    #[test]
    fn general_section_has_port_field() {
        let config = KajetConfig::default();
        let schema = build_config_schema(&config);
        let general = schema.sections.iter().find(|s| s.key == "general").unwrap();
        let port = general.fields.iter().find(|f| f.key == "port").unwrap();

        assert_eq!(port.i18n_key, "settings_field_general_port");
        assert!(matches!(port.field_type, FieldType::Number));
        assert_eq!(port.default_value, serde_json::json!(3579));
        assert!(matches!(port.scope, FieldScope::Global));
        assert!(port.restart_required);
        assert!(!port.hot_swap);

        let constraints = port.constraints.as_ref().unwrap();
        assert_eq!(constraints.min, Some(1024.0));
        assert_eq!(constraints.max, Some(65535.0));
    }

    #[test]
    fn embedding_backend_is_enum_with_hot_swap() {
        let config = KajetConfig::default();
        let schema = build_config_schema(&config);
        let embedding = schema
            .sections
            .iter()
            .find(|s| s.key == "embedding")
            .unwrap();
        let backend = embedding
            .fields
            .iter()
            .find(|f| f.key == "backend")
            .unwrap();

        assert!(matches!(backend.field_type, FieldType::Enum { .. }));
        if let FieldType::Enum { options } = &backend.field_type {
            assert_eq!(options, &vec!["candle".to_string(), "remote".to_string()]);
        }
        assert!(matches!(backend.scope, FieldScope::Both));
        assert!(!backend.restart_required);
        assert!(backend.hot_swap);
    }

    #[test]
    fn logging_level_is_enum_with_five_options() {
        let config = KajetConfig::default();
        let schema = build_config_schema(&config);
        let logging = schema.sections.iter().find(|s| s.key == "logging").unwrap();
        let level = logging.fields.iter().find(|f| f.key == "level").unwrap();

        if let FieldType::Enum { options } = &level.field_type {
            assert_eq!(options.len(), 5);
            assert_eq!(options, &vec!["trace", "debug", "info", "warn", "error"]);
        } else {
            panic!("level should be Enum");
        }
        assert!(matches!(level.scope, FieldScope::Global));
        assert!(level.hot_swap);
    }

    #[test]
    fn exclude_folders_is_array_with_both_scope() {
        let config = KajetConfig::default();
        let schema = build_config_schema(&config);
        let general = schema.sections.iter().find(|s| s.key == "general").unwrap();
        let exclude = general
            .fields
            .iter()
            .find(|f| f.key == "exclude_folders")
            .unwrap();

        assert!(matches!(exclude.field_type, FieldType::Array { .. }));
        if let FieldType::Array { item_type } = &exclude.field_type {
            assert!(matches!(**item_type, FieldType::String));
        }
        assert!(matches!(exclude.scope, FieldScope::Both));
    }

    #[test]
    fn tree_depth_has_constraints() {
        let config = KajetConfig::default();
        let schema = build_config_schema(&config);
        let tree = schema.sections.iter().find(|s| s.key == "tree").unwrap();
        let depth = tree.fields.iter().find(|f| f.key == "depth").unwrap();

        let constraints = depth.constraints.as_ref().unwrap();
        assert_eq!(constraints.min, Some(1.0));
        assert_eq!(constraints.max, Some(10.0));
        assert!(matches!(depth.scope, FieldScope::Both));
    }

    #[test]
    fn writer_timestamps_fields_are_flattened() {
        let config = KajetConfig::default();
        let schema = build_config_schema(&config);
        let writer = schema.sections.iter().find(|s| s.key == "writer").unwrap();

        assert!(writer.fields.iter().any(|f| f.key == "timestamps_enabled"));
        assert!(
            writer
                .fields
                .iter()
                .any(|f| f.key == "timestamps_created_field")
        );
        assert!(writer.fields.iter().any(|f| f.key == "timestamps_format"));

        let format = writer
            .fields
            .iter()
            .find(|f| f.key == "timestamps_format")
            .unwrap();
        if let FieldType::Enum { options } = &format.field_type {
            assert_eq!(options, &vec!["iso8601", "date_only"]);
        } else {
            panic!("timestamps_format should be Enum");
        }
    }

    #[test]
    fn writer_frontmatter_default_tags_is_array() {
        let config = KajetConfig::default();
        let schema = build_config_schema(&config);
        let writer = schema.sections.iter().find(|s| s.key == "writer").unwrap();
        let tags = writer
            .fields
            .iter()
            .find(|f| f.key == "frontmatter_default_tags")
            .unwrap();

        assert!(matches!(tags.field_type, FieldType::Array { .. }));
        assert!(matches!(tags.scope, FieldScope::Both));
    }

    #[test]
    fn writer_frontmatter_date_fields_present() {
        let config = KajetConfig::default();
        let schema = build_config_schema(&config);
        let writer = schema.sections.iter().find(|s| s.key == "writer").unwrap();

        // Check created_date_field
        let created = writer
            .fields
            .iter()
            .find(|f| f.key == "frontmatter_created_date_field")
            .expect("frontmatter_created_date_field must be present");
        assert!(matches!(created.field_type, FieldType::String));
        assert_eq!(
            created.i18n_key,
            "settings_field_writer_frontmatter_created_date_field"
        );
        assert!(matches!(created.scope, FieldScope::Both));

        // Check modified_date_field
        let modified = writer
            .fields
            .iter()
            .find(|f| f.key == "frontmatter_modified_date_field")
            .expect("frontmatter_modified_date_field must be present");
        assert!(matches!(modified.field_type, FieldType::String));
        assert_eq!(
            modified.i18n_key,
            "settings_field_writer_frontmatter_modified_date_field"
        );
        assert!(matches!(modified.scope, FieldScope::Both));
    }

    #[test]
    fn search_tuning_section_has_two_fields() {
        let config = KajetConfig::default();
        let schema = build_config_schema(&config);
        let search = schema
            .sections
            .iter()
            .find(|s| s.key == "search_tuning")
            .unwrap();

        assert_eq!(search.fields.len(), 2);
        assert!(
            search
                .fields
                .iter()
                .any(|f| f.key == "filter_overfetch_multiplier")
        );
        assert!(
            search
                .fields
                .iter()
                .any(|f| f.key == "tags_only_fetch_limit")
        );
    }

    #[test]
    fn all_fields_have_i18n_keys() {
        let config = KajetConfig::default();
        let schema = build_config_schema(&config);

        for section in &schema.sections {
            assert!(section.i18n_key.starts_with("settings_section_"));
            for field in &section.fields {
                assert!(field.i18n_key.starts_with("settings_field_"));
                assert!(field.i18n_key.contains(&section.key));
            }
        }
    }

    #[test]
    fn default_values_match_config() {
        let config = KajetConfig::default();
        let schema = build_config_schema(&config);

        let general = schema.sections.iter().find(|s| s.key == "general").unwrap();
        let port = general.fields.iter().find(|f| f.key == "port").unwrap();
        assert_eq!(port.default_value, serde_json::json!(config.port));

        let embedding = schema
            .sections
            .iter()
            .find(|s| s.key == "embedding")
            .unwrap();
        let model = embedding.fields.iter().find(|f| f.key == "model").unwrap();
        assert_eq!(
            model.default_value,
            serde_json::json!(config.embedding.model)
        );

        let tree = schema.sections.iter().find(|s| s.key == "tree").unwrap();
        let depth = tree.fields.iter().find(|f| f.key == "depth").unwrap();
        assert_eq!(depth.default_value, serde_json::json!(config.tree.depth));
    }
}
