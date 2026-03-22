use serde::Deserialize;
use std::time::Duration;

/// Response from TEI's GET /info endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct TeiInfo {
    pub model_id: String,
    #[serde(default)]
    pub model_dtype: Option<String>,
    pub max_batch_tokens: Option<u32>,
    pub max_input_length: Option<u32>,
    pub max_client_batch_size: Option<u32>,
    pub max_batch_requests: Option<u32>,
}

/// Optimal embedding params derived from TEI /info.
#[derive(Debug, Clone, Copy)]
pub struct OptimalParams {
    pub batch_size: usize,
    pub concurrent_requests: usize,
}

/// Probe GET /info on a TEI server. Returns None if endpoint unavailable
/// (e.g. Ollama, LM Studio don't expose /info).
pub async fn fetch_tei_info(base_url: &str) -> Option<TeiInfo> {
    let url = format!("{}/info", base_url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .ok()?;
    let resp = client.get(&url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    resp.json::<TeiInfo>().await.ok()
}

/// Compute optimal batch_size and concurrency from TEI /info response.
///
/// batch_size = max_batch_tokens / max_input_length, capped by max_client_batch_size
/// concurrent_requests = min(config_concurrent, max_batch_requests) if max_batch_requests set
pub fn compute_optimal_params(info: &TeiInfo, config_concurrent: usize) -> OptimalParams {
    let max_input_length = info.max_input_length.unwrap_or(512) as usize;
    let max_batch_tokens = info.max_batch_tokens.unwrap_or(4096) as usize;

    let computed_batch = (max_batch_tokens / max_input_length).max(1);
    let batch_size = if let Some(cap) = info.max_client_batch_size {
        computed_batch.min(cap as usize)
    } else {
        computed_batch
    };

    let concurrent_requests = if let Some(max_req) = info.max_batch_requests {
        config_concurrent.min(max_req as usize).max(1)
    } else {
        config_concurrent
    };

    OptimalParams {
        batch_size,
        concurrent_requests,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roberta_info() -> TeiInfo {
        TeiInfo {
            model_id: "sdadas/mmlw-retrieval-roberta-large-v2".into(),
            model_dtype: Some("float16".into()),
            max_batch_tokens: Some(4096),
            max_input_length: Some(512),
            max_client_batch_size: Some(32),
            max_batch_requests: Some(4),
        }
    }

    #[test]
    fn parse_full_info_response() {
        let json = r#"{
            "model_id": "sdadas/mmlw-retrieval-roberta-large-v2",
            "model_dtype": "float16",
            "max_batch_tokens": 4096,
            "max_input_length": 512,
            "max_client_batch_size": 32,
            "max_batch_requests": 4
        }"#;
        let info: TeiInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.model_id, "sdadas/mmlw-retrieval-roberta-large-v2");
        assert_eq!(info.model_dtype.as_deref(), Some("float16"));
        assert_eq!(info.max_batch_tokens, Some(4096));
        assert_eq!(info.max_input_length, Some(512));
        assert_eq!(info.max_client_batch_size, Some(32));
        assert_eq!(info.max_batch_requests, Some(4));
    }

    #[test]
    fn parse_info_with_null_max_batch_requests() {
        let json = r#"{
            "model_id": "nomic-embed-text",
            "max_input_length": 512,
            "max_client_batch_size": 64
        }"#;
        let info: TeiInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.max_batch_requests, None);
        assert_eq!(info.max_client_batch_size, Some(64));
    }

    #[test]
    fn unknown_fields_do_not_cause_errors() {
        let json = r#"{
            "model_id": "test-model",
            "model_sha": "abc123",
            "tokenization_workers": 4,
            "max_input_length": 512
        }"#;
        // Should not panic or error
        let info: TeiInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.model_id, "test-model");
    }

    #[test]
    fn compute_optimal_params_roberta_large() {
        // 4096 / 512 = 8, capped by max_client_batch_size=32 → 8
        // concurrent: min(4, 4) = 4
        let info = roberta_info();
        let params = compute_optimal_params(&info, 4);
        assert_eq!(params.batch_size, 8);
        assert_eq!(params.concurrent_requests, 4);
    }

    #[test]
    fn compute_optimal_params_respects_max_batch_requests() {
        // config_concurrent=8 but max_batch_requests=4 → capped to 4
        let info = roberta_info();
        let params = compute_optimal_params(&info, 8);
        assert_eq!(params.concurrent_requests, 4);
    }

    #[test]
    fn compute_optimal_params_batch_capped_by_max_client_batch_size() {
        // 16384 / 256 = 64, but max_client_batch_size=32 → 32
        let info = TeiInfo {
            model_id: "test".into(),
            model_dtype: None,
            max_batch_tokens: Some(16384),
            max_input_length: Some(256),
            max_client_batch_size: Some(32),
            max_batch_requests: None,
        };
        let params = compute_optimal_params(&info, 4);
        assert_eq!(params.batch_size, 32);
    }

    #[test]
    fn compute_optimal_params_no_max_batch_requests_uses_config() {
        let info = TeiInfo {
            model_id: "test".into(),
            model_dtype: None,
            max_batch_tokens: Some(4096),
            max_input_length: Some(512),
            max_client_batch_size: None,
            max_batch_requests: None,
        };
        let params = compute_optimal_params(&info, 6);
        assert_eq!(params.concurrent_requests, 6);
    }
}
