mod config;
mod embedder;
mod error;
mod info;
mod transport;
mod validator;

pub use config::RemoteEmbedderConfig;
pub use embedder::RemoteEmbedder;
pub use error::RemoteEmbedderError;
pub use info::{TeiInfo, compute_optimal_params, fetch_tei_info};
