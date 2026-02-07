pub mod broadcast_layer;
pub mod file_layer;
pub mod types;

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::broadcast;
use tracing::Level;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

use crate::config::LoggingConfig;

use self::broadcast_layer::{BroadcastLayer, LogBuffer};
use self::file_layer::FileLayer;
use self::types::LogEntry;

/// Initialize the dual-sink logging system.
///
/// Returns an `Arc<LogBuffer>` so callers can access the ring buffer of recent entries.
pub fn init_logging(
    config: &LoggingConfig,
    vault_path: &Path,
    log_tx: broadcast::Sender<LogEntry>,
) -> Result<Arc<LogBuffer>> {
    let global_level = parse_level(&config.level);

    // File layer: .kajet/kajet.log
    let log_path = vault_path.join(".kajet").join("kajet.log");
    let file_level = effective_level(global_level, parse_level(&config.file_level));
    let file_layer = FileLayer::new(log_path)?;
    let file_filter = EnvFilter::new(format!("{file_level}"));
    let file_layer = file_layer.with_filter(file_filter);

    // Broadcast layer: sends to dashboard via WebSocket
    let dashboard_level = effective_level(global_level, parse_level(&config.dashboard_level));
    let (broadcast_layer, log_buffer) = BroadcastLayer::new(log_tx);
    let broadcast_filter = EnvFilter::new(format!("{dashboard_level}"));
    let broadcast_layer = broadcast_layer.with_filter(broadcast_filter);

    tracing_subscriber::registry()
        .with(file_layer)
        .with(broadcast_layer)
        .init();

    Ok(log_buffer)
}

/// Parse a level string (case-insensitive) into a tracing `Level`.
/// Falls back to `DEBUG` for unrecognized values.
pub fn parse_level(s: &str) -> Level {
    match s.to_lowercase().as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" | "warning" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::DEBUG,
    }
}

/// Effective level = max(global, sink), i.e. the less verbose of the two.
/// tracing `Level` ordering: ERROR < WARN < INFO < DEBUG < TRACE.
pub fn effective_level(global: Level, sink: Level) -> Level {
    if global < sink {
        global
    } else {
        sink
    }
}

/// Convert a tracing event into a `LogEntry`.
pub(crate) fn event_to_log_entry(event: &tracing::Event<'_>) -> LogEntry {
    let meta = event.metadata();

    let mut visitor = FieldVisitor::default();
    event.record(&mut visitor);

    LogEntry {
        timestamp: chrono::Utc::now(),
        level: meta.level().to_string(),
        target: meta.target().to_string(),
        message: visitor.message,
        fields: visitor.fields,
    }
}

#[derive(Default)]
struct FieldVisitor {
    message: String,
    fields: HashMap<String, serde_json::Value>,
}

impl tracing::field::Visit for FieldVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{value:?}");
        } else {
            self.fields.insert(
                field.name().to_string(),
                serde_json::json!(format!("{value:?}")),
            );
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        } else {
            self.fields
                .insert(field.name().to_string(), serde_json::json!(value));
        }
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.fields
            .insert(field.name().to_string(), serde_json::json!(value));
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.fields
            .insert(field.name().to_string(), serde_json::json!(value));
    }

    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        self.fields
            .insert(field.name().to_string(), serde_json::json!(value));
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.fields
            .insert(field.name().to_string(), serde_json::json!(value));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_level() {
        assert_eq!(parse_level("trace"), Level::TRACE);
        assert_eq!(parse_level("DEBUG"), Level::DEBUG);
        assert_eq!(parse_level("Info"), Level::INFO);
        assert_eq!(parse_level("WARN"), Level::WARN);
        assert_eq!(parse_level("warning"), Level::WARN);
        assert_eq!(parse_level("error"), Level::ERROR);
        assert_eq!(parse_level("nonsense"), Level::DEBUG);
    }

    #[test]
    fn test_effective_level() {
        // global=INFO, sink=TRACE → effective should be INFO (more restrictive)
        assert_eq!(effective_level(Level::INFO, Level::TRACE), Level::INFO);
        // global=TRACE, sink=WARN → effective should be WARN
        assert_eq!(effective_level(Level::TRACE, Level::WARN), Level::WARN);
        // same level
        assert_eq!(effective_level(Level::DEBUG, Level::DEBUG), Level::DEBUG);
    }
}
