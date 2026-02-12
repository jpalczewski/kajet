use crate::error::RemoteEmbedderError;

pub fn validate_and_reorder(
    input_count: usize,
    mut data: Vec<(usize, Vec<f32>)>,
    expected_dim: Option<usize>,
) -> Result<Vec<Vec<f32>>, RemoteEmbedderError> {
    if input_count == 0 {
        return Ok(Vec::new());
    }
    if data.len() != input_count {
        return Err(RemoteEmbedderError::InvalidResponse {
            reason: format!(
                "embedding count mismatch: expected {}, got {}",
                input_count,
                data.len()
            ),
        });
    }

    data.sort_by_key(|(idx, _)| *idx);
    for (expected_idx, (idx, emb)) in data.iter().enumerate() {
        if *idx != expected_idx {
            return Err(RemoteEmbedderError::InvalidResponse {
                reason: format!("invalid index sequence at {}: got {}", expected_idx, idx),
            });
        }
        if let Some(dim) = expected_dim
            && emb.len() != dim
        {
            return Err(RemoteEmbedderError::DimensionMismatch {
                expected: dim,
                got: emb.len(),
            });
        }
    }

    let dim = data[0].1.len();
    if dim == 0 {
        return Err(RemoteEmbedderError::InvalidResponse {
            reason: "empty embedding vectors".to_string(),
        });
    }
    if let Some((_, wrong)) = data.iter().find(|(_, emb)| emb.len() != dim) {
        return Err(RemoteEmbedderError::InvalidResponse {
            reason: format!(
                "inconsistent embedding dimensions: expected {}, got {}",
                dim,
                wrong.len()
            ),
        });
    }

    Ok(data.into_iter().map(|(_, emb)| emb).collect())
}
