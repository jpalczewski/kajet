#[derive(Debug, thiserror::Error)]
pub enum RemoteEmbedderError {
    #[error("transport error: {0}")]
    Transport(String),
    #[error("http status error: {status} body={body}")]
    HttpStatus { status: u16, body: String },
    #[error("invalid response: {reason}")]
    InvalidResponse { reason: String },
    #[error("dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch { expected: usize, got: usize },
    #[error("probe failed: {0}")]
    ProbeFailed(String),
}
