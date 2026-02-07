use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::types::QueryEvent;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub target: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub fields: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum WsMessage {
    #[serde(rename = "query")]
    Query(QueryEvent),
    #[serde(rename = "log")]
    Log(LogEntry),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ws_message_query_serialization() {
        let msg = WsMessage::Query(QueryEvent {
            query: "test".into(),
            num_results: 3,
            timestamp: Utc::now(),
        });
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"query""#));
        assert!(json.contains(r#""data""#));

        let parsed: WsMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, WsMessage::Query(_)));
    }

    #[test]
    fn ws_message_log_serialization() {
        let msg = WsMessage::Log(LogEntry {
            timestamp: Utc::now(),
            level: "INFO".into(),
            target: "kajet_indexer::pipeline".into(),
            message: "file:start".into(),
            fields: {
                let mut m = HashMap::new();
                m.insert("path".into(), serde_json::json!("notes/rust.md"));
                m
            },
        });
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"log""#));
        assert!(json.contains("file:start"));

        let parsed: WsMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, WsMessage::Log(_)));
    }

    #[test]
    fn log_entry_empty_fields_omitted() {
        let entry = LogEntry {
            timestamp: Utc::now(),
            level: "DEBUG".into(),
            target: "test".into(),
            message: "hello".into(),
            fields: HashMap::new(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(!json.contains("fields"));
    }
}
