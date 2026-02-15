use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub target: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub fields: HashMap<String, serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

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
