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
    /// The search query (optional if filters provided)
    #[schemars(
        description = "Search query for finding relevant documents. Optional if using filters (from/to/tags/folder) for browse mode."
    )]
    pub query: Option<String>,

    /// Max number of results
    #[schemars(description = "Maximum number of results to return (default: 5)")]
    pub limit: Option<usize>,

    /// Search mode: "hybrid" (default), "vector", "fts"
    #[schemars(
        description = "Search mode: 'hybrid' (vector + full-text, default), 'vector' (semantic only), 'fts' (keyword only). Ignored in browse mode."
    )]
    pub mode: Option<String>,

    /// Filter by date range start
    #[schemars(
        description = "Date filter start. Formats: ISO (2025-01-15, 2025-01), Polish (dzisiaj, wczoraj, zeszły tydzień/miesiąc/rok, w styczniu), English (today, yesterday, last week/month/year, in january)."
    )]
    pub from: Option<String>,

    /// Filter by date range end
    #[schemars(
        description = "Date filter end. Same formats as 'from'. Defaults to today if 'from' is set but 'to' is not."
    )]
    pub to: Option<String>,

    /// Filter by tags (all required)
    #[schemars(description = "Filter by tags. Documents must have ALL specified tags.")]
    pub tags: Option<Vec<String>>,

    /// Filter by folder path prefix
    #[schemars(
        description = "Filter by folder path prefix, e.g. 'journal/2025'. Matches documents in this folder and subfolders."
    )]
    pub folder: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CreateNoteRequest {
    /// Relative path for the new note, e.g. "Projects/ideas.md". Parent directories are created automatically.
    #[schemars(
        description = "Relative path for the new note, e.g. 'Projects/ideas.md'. The .md extension is added if missing."
    )]
    pub target: String,

    /// Markdown body content for the note
    #[schemars(description = "Markdown body content for the note")]
    pub content: String,

    /// Tags to include in the note's frontmatter
    #[schemars(description = "Tags to include in the note's frontmatter")]
    pub tags: Option<Vec<String>>,

    /// Aliases for the note (alternative names used by Obsidian for linking)
    #[schemars(
        description = "Aliases for the note (alternative names used by Obsidian for linking)"
    )]
    pub aliases: Option<Vec<String>>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct EditNoteRequest {
    /// Path to the note (full, partial, or filename — fuzzy suffix matching is used)
    #[schemars(
        description = "Path to the note — full relative path or partial (filename). Fuzzy suffix matching resolves partial paths."
    )]
    pub path: String,

    /// New content to insert or replace with
    #[schemars(description = "New content to insert or replace with")]
    pub content: String,

    /// Edit mode: 'append', 'prepend', 'overwrite', 'replace_section', 'replace_text', 'insert_after'
    #[schemars(
        description = "Edit mode: 'append' (add to end), 'prepend' (add after frontmatter), 'overwrite' (replace body), 'replace_section' (replace heading section), 'replace_text' (exact string replacement), 'insert_after' (insert content after exact text anchor — use old_text as anchor)"
    )]
    pub mode: String,

    /// Target heading for section-level operations. Required for replace_section, optional for append/prepend.
    #[schemars(
        description = "Target heading for section-level operations (e.g. '## Tasks'). Required for replace_section, optional for append/prepend."
    )]
    pub target_heading: Option<String>,

    /// Old text to replace or anchor (required for replace_text and insert_after modes)
    #[schemars(
        description = "The exact text to find and replace (required for replace_text mode) or anchor to insert after (required for insert_after mode)"
    )]
    pub old_text: Option<String>,
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

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct EditTagsRequest {
    /// Path to the note (full, partial, or filename — fuzzy suffix matching is used)
    #[schemars(
        description = "Path to the note — full relative path or partial (filename). Fuzzy suffix matching resolves partial paths."
    )]
    pub path: String,

    /// Tags to add to the note's frontmatter
    #[schemars(description = "Tags to add to the note's frontmatter")]
    pub add: Option<Vec<String>>,

    /// Tags to remove from the note's frontmatter
    #[schemars(description = "Tags to remove from the note's frontmatter")]
    pub remove: Option<Vec<String>>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct TreeRequest {
    /// Starting folder path, e.g. 'journal' or 'Projects/archived'
    #[schemars(description = "Starting folder path relative to vault root. Omit for full vault.")]
    pub path: Option<String>,

    /// Maximum depth to traverse
    #[schemars(description = "Maximum folder depth to show (default: from config, typically 3)")]
    pub depth: Option<usize>,

    /// Maximum entries in output
    #[schemars(
        description = "Maximum total entries (folders + files) in output (default: from config, typically 50)"
    )]
    pub size: Option<usize>,

    /// Display mode: 'folders' (default) or 'files'
    #[schemars(
        description = "Display mode: 'folders' (folder names + note counts, default) or 'files' (include .md filenames)"
    )]
    pub show: Option<String>,
}
