use crate::errors::internal_error;
use crate::ports::DiscoverPorts;
use crate::schema::DiscoverBridgesRequest;
use kajet_core::discover::{BridgesMode, BridgesParams, find_bridges};
use rmcp::{handler::server::wrapper::Parameters, model::*, tool, tool_router};
use std::collections::HashMap;

#[tool_router(router = tool_router_discover, vis = "pub(crate)")]
impl crate::KajetMcp {
    #[tool(
        description = "Find hidden semantic bridges: pairs of notes that discuss similar topics \
            but are not linked to each other. Reveals blind spots in vault structure — \
            notes that don't know about each other despite semantic similarity."
    )]
    async fn discover_bridges(
        &self,
        params: Parameters<DiscoverBridgesRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let ctx = DiscoverPorts::get_discover_context(self)
            .await
            .ok_or_else(|| internal_error(t!("bridges_no_graph").to_string()))?;

        let req = params.0;
        let limit = req
            .limit
            .unwrap_or_else(|| DiscoverPorts::default_limit(self))
            .clamp(1, 50);

        let mode = match req.mode.as_deref() {
            Some("similarity") => BridgesMode::Similarity,
            _ => BridgesMode::Surprise,
        };

        // Load tags index only when tags filter is requested.
        let (required_tags, doc_tags) = match req.tags.filter(|t| !t.is_empty()) {
            Some(tags) => {
                let docs = DiscoverPorts::get_all_documents(self)
                    .await
                    .map_err(|e| internal_error(e.to_string()))?;
                let index: HashMap<String, Vec<String>> =
                    docs.into_iter().map(|d| (d.source_file, d.tags)).collect();
                (tags, index)
            }
            None => (vec![], HashMap::new()),
        };

        let bridge_params = BridgesParams {
            limit,
            mode,
            min_similarity: req.min_similarity.unwrap_or(0.7).clamp(0.0, 1.0),
            min_graph_distance: req.min_graph_distance.unwrap_or(3),
            cross_layer_bonus: req.cross_layer_bonus.unwrap_or(0.3),
            folder: req.folder,
            required_tags,
            doc_tags,
        };

        let start = std::time::Instant::now();
        let result = find_bridges(&ctx.similarity_graph, &ctx.link_graph, &bridge_params)
            .map_err(|e| internal_error(t!("bridges_failed", error = e.to_string()).to_string()))?;

        tracing::info!(
            elapsed_ms = start.elapsed().as_millis() as u64,
            bridges = result.bridges.len(),
            "MCP discover_bridges"
        );

        let json =
            serde_json::to_string_pretty(&result).map_err(|e| internal_error(e.to_string()))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }
}
