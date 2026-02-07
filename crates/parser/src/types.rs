// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ChunkConfig {
    pub max_chars: usize,
    pub overlap_chars: usize,
    pub resolve_wikilinks: bool,
}

impl Default for ChunkConfig {
    fn default() -> Self {
        Self {
            max_chars: 6000,    // ~1500 tokens
            overlap_chars: 600, // ~150 tokens
            resolve_wikilinks: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Link — a wikilink reference extracted from chunk content
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Link {
    pub target: String,
    pub alias: Option<String>,
    pub resolved_path: Option<String>,
}

// ---------------------------------------------------------------------------
// Chunk
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Chunk {
    pub note_path: String,
    pub breadcrumb: String,
    pub content: String,
    pub raw_content: String,
    pub chunk_index: u32,
    pub links: Vec<Link>,
}

impl Chunk {
    /// Text sent to the embedder: breadcrumb + content for context.
    pub fn embed_text(&self) -> String {
        format!("{}\n\n{}", self.breadcrumb, self.content)
    }
}

// ---------------------------------------------------------------------------
// Document — parsed metadata from a markdown file
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ParsedDocument {
    pub source_file: String,
    pub full_text: String,
    pub title: String,
    pub tags: Vec<String>,
}
