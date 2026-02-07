use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tracing_subscriber::Layer;

use super::event_to_log_entry;
use super::types::LogEntry;

const RING_BUFFER_SIZE: usize = 500;

/// Shared ring buffer of recent log entries, accessible from WebSocket handlers.
pub struct LogBuffer {
    buffer: Mutex<Vec<LogEntry>>,
}

impl LogBuffer {
    fn new() -> Self {
        Self {
            buffer: Mutex::new(Vec::with_capacity(RING_BUFFER_SIZE)),
        }
    }

    /// Returns a snapshot of recent log entries (up to 500).
    pub fn recent_entries(&self) -> Vec<LogEntry> {
        self.buffer.lock().unwrap().clone()
    }

    fn push(&self, entry: LogEntry) {
        let mut buf = self.buffer.lock().unwrap();
        if buf.len() >= RING_BUFFER_SIZE {
            buf.remove(0);
        }
        buf.push(entry);
    }
}

/// A tracing layer that broadcasts log entries via a tokio broadcast channel
/// and keeps a ring buffer of recent entries for new WebSocket connections.
pub struct BroadcastLayer {
    tx: broadcast::Sender<LogEntry>,
    buffer: Arc<LogBuffer>,
}

impl BroadcastLayer {
    pub fn new(tx: broadcast::Sender<LogEntry>) -> (Self, Arc<LogBuffer>) {
        let buffer = Arc::new(LogBuffer::new());
        let layer = Self {
            tx,
            buffer: buffer.clone(),
        };
        (layer, buffer)
    }
}

impl<S> Layer<S> for BroadcastLayer
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let entry = event_to_log_entry(event);
        let _ = self.tx.send(entry.clone());
        self.buffer.push(entry);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_subscriber::layer::SubscriberExt;

    #[test]
    fn broadcast_ring_buffer() {
        let (tx, _rx) = broadcast::channel::<LogEntry>(128);
        let (_layer, buffer) = BroadcastLayer::new(tx);

        // Fill beyond capacity
        for i in 0..(RING_BUFFER_SIZE + 10) {
            buffer.push(LogEntry {
                timestamp: chrono::Utc::now(),
                level: "INFO".into(),
                target: "test".into(),
                message: format!("msg {i}"),
                fields: Default::default(),
            });
        }

        let entries = buffer.recent_entries();
        assert_eq!(entries.len(), RING_BUFFER_SIZE);
        assert_eq!(entries[0].message, "msg 10");
        assert_eq!(
            entries[RING_BUFFER_SIZE - 1].message,
            format!("msg {}", RING_BUFFER_SIZE + 9)
        );
    }

    #[test]
    fn broadcast_sends_to_channel() {
        let (tx, mut rx) = broadcast::channel::<LogEntry>(128);
        let (layer, _buffer) = BroadcastLayer::new(tx);

        let subscriber = tracing_subscriber::registry().with(layer);

        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("hello broadcast");
        });

        let entry = rx.try_recv().unwrap();
        assert_eq!(entry.level, "INFO");
        assert!(entry.message.contains("hello broadcast"));
    }
}
