use rmcp::schemars;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ReindexRequest {
    /// Optional relative path of a specific file to reindex. If omitted, full vault reindex.
    #[schemars(
        description = "Optional relative path of a specific file to reindex. Omit for full vault reindex."
    )]
    pub path: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ExamineRequest {
    /// Path to the document (full or partial, e.g. "myfile.md" or "subfolder/myfile")
    #[schemars(
        description = "Path to the document — full relative path or partial (filename). Fuzzy suffix matching is used if exact match fails."
    )]
    pub path: String,

    /// Content display mode: "summary" (first 500 chars, default), "full" (entire content), "slice" (use offset+length)
    #[schemars(
        description = "Content mode: 'summary' (first 500 chars, default), 'full' (entire content), 'slice' (use offset+length)"
    )]
    pub content: Option<String>,

    /// Character offset for 'slice' mode (default: 0)
    #[schemars(description = "Character offset for 'slice' mode (default: 0)")]
    pub offset: Option<usize>,

    /// Number of characters for 'slice' mode (default: 500)
    #[schemars(description = "Number of characters for 'slice' mode (default: 500)")]
    pub length: Option<usize>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SearchRequest {
    /// The search query
    #[schemars(description = "Search query for finding relevant documents in the Obsidian vault")]
    pub query: String,

    /// Max number of results
    #[schemars(description = "Maximum number of results to return (default: 5)")]
    pub limit: Option<usize>,

    /// Search mode: "hybrid" (default), "vector", "fts"
    #[schemars(
        description = "Search mode: 'hybrid' (vector + full-text, default), 'vector' (semantic only), 'fts' (keyword only)"
    )]
    pub mode: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ListTagsRequest {
    /// Filter by folder path prefix, e.g. 'journal/2025'
    #[schemars(description = "Filter by folder path prefix, e.g. 'journal/2025'")]
    pub folder: Option<String>,

    /// Include subfolders (default: true)
    #[schemars(description = "Include subfolders (default: true)")]
    pub recursive: Option<bool>,

    /// Detail level: 'names' (tag list), 'counts' (tag + doc count, default), 'full' (tag + count + file paths)
    #[schemars(
        description = "Detail level: 'names' (tag list), 'counts' (tag + doc count, default), 'full' (tag + count + file paths)"
    )]
    pub detail: Option<String>,
}
