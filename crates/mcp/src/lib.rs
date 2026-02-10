#[macro_use]
extern crate rust_i18n;

i18n!("../../locales", fallback = "en");

mod date_parser;
pub(crate) mod filters;
mod format;
mod schema;
mod tools;

pub use format::{
    format_create_result, format_edit_result, format_edit_tags_result, format_entries,
    format_examine_result, format_list_tags, format_results, format_tree_too_large,
    format_vault_tree,
};
pub use schema::{
    CreateNoteRequest, EditNoteRequest, EditTagsRequest, ExamineRequest, ListTagsRequest,
    ReindexRequest, SearchRequest, TreeRequest,
};

use anyhow::Result;
use kajet_core::types::AppState;
use rmcp::{
    ServerHandler, ServiceExt, handler::server::router::tool::ToolRouter, model::*, tool_handler,
    transport::stdio,
};
use std::sync::Arc;

#[derive(Clone)]
pub struct KajetMcp {
    state: Arc<AppState>,
    tool_router: ToolRouter<Self>,
}

#[tool_handler]
impl ServerHandler for KajetMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::LATEST,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "kajet".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                title: Some(t!("server_title").into()),
                icons: None,
                website_url: None,
            },
            instructions: Some(t!("server_instructions").into()),
        }
    }
}

impl KajetMcp {
    pub fn new(state: Arc<AppState>) -> Self {
        Self {
            state,
            tool_router: Self::tool_router(),
        }
    }
}

pub async fn serve(state: Arc<AppState>) -> Result<()> {
    let handler = KajetMcp::new(state);
    let service = handler.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
