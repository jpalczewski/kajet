use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../frontend/src/lib/types/generated/")]
pub struct DocumentSummary {
    pub source_file: String,
    pub title: String,
    pub tags: Vec<String>,
    pub chunk_count: usize,
    pub last_modified: f64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../frontend/src/lib/types/generated/")]
pub struct ChunkDetail {
    pub chunk_index: u32,
    pub breadcrumb: String,
    pub content: String,
    pub raw_content: String,
    pub links: Vec<kajet_parser::Link>,
    pub char_count: usize,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../frontend/src/lib/types/generated/")]
pub struct DocumentListResponse {
    pub documents: Vec<DocumentSummary>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../frontend/src/lib/types/generated/")]
pub struct DocumentDetail {
    pub document: crate::types::Document,
    pub chunks: Vec<ChunkDetail>,
}
