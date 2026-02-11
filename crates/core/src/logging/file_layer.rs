use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use tracing_subscriber::Layer;

use super::event_to_log_entry;

/// A tracing layer that writes JSON-lines to a log file.
pub struct FileLayer {
    writer: Mutex<File>,
    path: PathBuf,
}

impl FileLayer {
    /// Create a new `FileLayer` that writes to the given path.
    /// The file is truncated on creation (overwritten each start).
    pub fn new(path: PathBuf) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)?;
        Ok(Self {
            writer: Mutex::new(file),
            path,
        })
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl<S> Layer<S> for FileLayer
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let entry = event_to_log_entry(event);
        if let Ok(json) = serde_json::to_string(&entry)
            && let Ok(mut writer) = self.writer.lock()
        {
            let _ = writeln!(writer, "{json}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_subscriber::layer::SubscriberExt;

    #[test]
    fn file_layer_writes_json_lines() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("test.log");

        let layer = FileLayer::new(log_path.clone()).unwrap();
        let subscriber = tracing_subscriber::registry().with(layer);

        // Use a thread-local subscriber to avoid global init conflicts
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(path = "notes/test.md", "file:start");
            tracing::warn!("something happened");
        });

        let content = std::fs::read_to_string(&log_path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);

        // Each line should be valid JSON
        let entry: super::super::types::LogEntry = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(entry.level, "INFO");
        assert!(entry.message.contains("file:start"));

        let entry2: super::super::types::LogEntry = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(entry2.level, "WARN");
    }
}
