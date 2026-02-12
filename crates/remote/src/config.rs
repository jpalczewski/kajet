use std::time::Duration;

#[derive(Debug, Clone)]
pub struct RemoteEmbedderConfig {
    pub base_url: String,
    pub model: String,
    pub api_key: Option<String>,
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
    pub max_retries: u32,
    pub initial_backoff: Duration,
    pub max_batch_size: usize,
}

impl Default for RemoteEmbedderConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:1234".to_string(),
            model: "nomic-embed-text-v1.5".to_string(),
            api_key: None,
            connect_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_secs(120),
            max_retries: 3,
            initial_backoff: Duration::from_millis(100),
            max_batch_size: 256,
        }
    }
}
