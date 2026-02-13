use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tracing_subscriber::Layer;

use super::event_to_log_entry;
use super::types::LogEntry;
use crate::actions::ActionEvent;

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

    #[cfg(any(test, feature = "test-utils"))]
    pub fn __test_new() -> Self {
        Self::new()
    }
}

/// A tracing layer that broadcasts log entries via a tokio broadcast channel
/// and keeps a ring buffer of recent entries for new WebSocket connections.
pub struct BroadcastLayer {
    tx: broadcast::Sender<ActionEvent>,
    buffer: Arc<LogBuffer>,
}

impl BroadcastLayer {
    pub fn new(tx: broadcast::Sender<ActionEvent>) -> (Self, Arc<LogBuffer>) {
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
        // Broadcast as ActionEvent::LogEntry
        let action_event = ActionEvent::LogEntry {
            level: entry.level.clone(),
            message: entry.message.clone(),
            timestamp: entry.timestamp.to_rfc3339(),
        };
        let _ = self.tx.send(action_event);
        // Keep LogEntry in buffer for history
        self.buffer.push(entry);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_subscriber::layer::SubscriberExt;

    #[test]
    fn broadcast_ring_buffer() {
        let (tx, _rx) = broadcast::channel::<ActionEvent>(128);
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
        let (tx, mut rx) = broadcast::channel::<ActionEvent>(128);
        let (layer, _buffer) = BroadcastLayer::new(tx);

        let subscriber = tracing_subscriber::registry().with(layer);

        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("hello broadcast");
        });

        let action_event = rx.try_recv().unwrap();
        match action_event {
            ActionEvent::LogEntry { level, message, .. } => {
                assert_eq!(level, "INFO");
                assert!(message.contains("hello broadcast"));
            }
            _ => panic!("Expected LogEntry event"),
        }
    }
}
