use crate::config::KajetConfig;
use crate::engine::Engine;
use tokio::sync::broadcast;

#[derive(Clone, Debug, serde::Serialize)]
pub struct QueryEvent {
    pub query: String,
    pub num_results: usize,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

pub struct AppState {
    pub engine: Engine,
    pub events: broadcast::Sender<QueryEvent>,
    pub config: KajetConfig,
}
