use crate::errors::{internal_error, translate_path_error};
use crate::schema::TreeRequest;
use rmcp::{handler::server::wrapper::Parameters, model::*, tool, tool_router};
use tracing::instrument;

#[tool_router(router = tool_router_tree, vis = "pub(crate)")]
impl crate::KajetMcp {
    #[tool(
        description = "Show vault folder structure as a tree. Returns folder names with note counts. Use 'path' to focus on a subfolder, 'show: files' to include filenames."
    )]
    #[instrument(level = "debug", skip(self, params), fields(path, depth, size, show))]
    async fn tree(&self, params: Parameters<TreeRequest>) -> Result<CallToolResult, ErrorData> {
        let req = params.0;

        let (depth, size, max_chars, exclude) = {
            let config = self.state.config.read().unwrap();
            let depth = req.depth.unwrap_or(config.tree.depth);
            let size = req.size.unwrap_or(config.tree.size);
            let max_chars = config.tree.max_chars;
            let exclude = config.exclude_folders.clone();
            (depth, size, max_chars, exclude)
        };

        // Validate parameters
        if depth == 0 {
            return Err(internal_error(t!("tree_depth_zero").to_string()));
        }
        if size == 0 {
            return Err(internal_error(t!("tree_size_zero").to_string()));
        }

        let show = match req.show.as_deref() {
            None | Some("folders") => kajet_parser::ShowMode::Folders,
            Some("files") => kajet_parser::ShowMode::Files,
            Some(invalid) => {
                return Err(internal_error(
                    t!("tree_invalid_show", mode = invalid).to_string(),
                ));
            }
        };

        let span = tracing::Span::current();
        span.record("path", req.path.as_deref().unwrap_or("*"));
        span.record("depth", depth);
        span.record("size", size);
        span.record("show", show.to_string().as_str());

        let vault_path = self.state.vault_path.clone();
        let path = req.path.clone();
        let options = kajet_parser::VaultTreeOptions {
            path,
            depth,
            size,
            show,
        };

        let tree = kajet_parser::vault_tree(&vault_path, &exclude, &options)
            .await
            .map_err(translate_path_error)?;

        let output = crate::format::format_vault_tree(&tree);

        if output.len() > max_chars {
            let has_files = show == kajet_parser::ShowMode::Files;
            return Ok(CallToolResult::success(vec![Content::text(
                crate::format::format_tree_too_large(output.len(), max_chars, depth, has_files),
            )]));
        }

        tracing::info!(
            path = req.path.as_deref().unwrap_or("*"),
            depth,
            size,
            entries = tree.subfolders.len(),
            output_chars = output.len(),
            "MCP tree"
        );

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }
}
