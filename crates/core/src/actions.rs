use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::types::IndexStats;

pub type ActionId = String;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../frontend/src/lib/types/generated/")]
pub enum IndexMode {
    Full,
    Incremental,
    SingleFile(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../frontend/src/lib/types/generated/")]
pub enum ConfigSection {
    Global,
    Vault,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../frontend/src/lib/types/generated/")]
pub struct SearchResultSummary {
    pub note_path: String,
    pub breadcrumb: String,
    pub content_preview: String,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../frontend/src/lib/types/generated/")]
#[serde(tag = "type", content = "data")]
pub enum ActionEvent {
    IndexStarted {
        action_id: ActionId,
        mode: IndexMode,
    },
    IndexProgress {
        action_id: ActionId,
        processed: usize,
        total: usize,
    },
    IndexCompleted {
        action_id: ActionId,
        stats: IndexStats,
    },
    IndexFailed {
        action_id: ActionId,
        error: String,
    },
    ConfigChanged {
        section: ConfigSection,
    },
    EmbedderSwapped {
        new_backend: String,
        new_model: String,
    },
    StatsUpdated {
        note_count: usize,
        chunk_count: usize,
    },
    QueryExecuted {
        query: String,
        results: Vec<SearchResultSummary>,
        #[ts(type = "number")]
        duration_ms: u64,
        timestamp: String,
    },
    LogEntry {
        level: String,
        message: String,
        timestamp: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../frontend/src/lib/types/generated/")]
#[serde(tag = "action")]
pub enum ActionRequest {
    Reindex { path: Option<String> },
    RefreshStats,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../frontend/src/lib/types/generated/")]
pub struct ActionResponse {
    pub action_id: ActionId,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_event_serializes_with_tag() {
        let event = ActionEvent::IndexStarted {
            action_id: "test-123".into(),
            mode: IndexMode::Full,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""type":"IndexStarted""#));
        assert!(json.contains(r#""data""#));
    }

    #[test]
    fn action_request_deserializes_reindex() {
        let json = r#"{"action":"Reindex","path":null}"#;
        let req: ActionRequest = serde_json::from_str(json).unwrap();
        assert!(matches!(req, ActionRequest::Reindex { path: None }));
    }

    #[test]
    fn action_request_deserializes_reindex_with_path() {
        let json = r#"{"action":"Reindex","path":"notes/test.md"}"#;
        let req: ActionRequest = serde_json::from_str(json).unwrap();
        assert!(matches!(req, ActionRequest::Reindex { path: Some(_) }));
    }

    #[test]
    fn query_executed_includes_results() {
        let event = ActionEvent::QueryExecuted {
            query: "test query".into(),
            results: vec![SearchResultSummary {
                note_path: "a.md".into(),
                breadcrumb: "a.md > Title".into(),
                content_preview: "Some content...".into(),
                score: 0.85,
            }],
            duration_ms: 42,
            timestamp: "2024-01-01T12:00:00Z".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("a.md"));
        assert!(json.contains("0.85"));
    }
}
