use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

/// Parameters for creating a new note.
#[derive(Debug, Clone)]
pub struct CreateNoteParams {
    /// Relative path within the vault, e.g. "Projects/ideas.md"
    pub target: String,
    /// Markdown body content
    pub content: String,
    /// Tags to include in frontmatter
    pub tags: Vec<String>,
    /// Aliases for the note (alternative names used by Obsidian for linking)
    pub aliases: Vec<String>,
}

/// Parameters for editing an existing note.
#[derive(Debug, Clone)]
pub struct EditNoteParams {
    /// Path to the note (exact, partial, or fuzzy)
    pub path: String,
    /// New content to insert/replace with
    pub content: String,
    /// Edit mode
    pub mode: EditMode,
    /// Target heading for section-level operations
    pub target_heading: Option<String>,
    /// Old text for replace_text mode
    pub old_text: Option<String>,
}

/// Edit mode for edit_note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditMode {
    Append,
    Prepend,
    Overwrite,
    ReplaceSection,
    ReplaceText,
    InsertAfter,
}

impl FromStr for EditMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "append" => Ok(Self::Append),
            "prepend" => Ok(Self::Prepend),
            "overwrite" => Ok(Self::Overwrite),
            "replace_section" => Ok(Self::ReplaceSection),
            "replace_text" => Ok(Self::ReplaceText),
            "insert_after" => Ok(Self::InsertAfter),
            other => Err(format!("unknown edit mode: '{other}'")),
        }
    }
}

impl fmt::Display for EditMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl EditMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Append => "append",
            Self::Prepend => "prepend",
            Self::Overwrite => "overwrite",
            Self::ReplaceSection => "replace_section",
            Self::ReplaceText => "replace_text",
            Self::InsertAfter => "insert_after",
        }
    }

    pub fn is_destructive(&self) -> bool {
        matches!(
            self,
            Self::Overwrite | Self::ReplaceSection | Self::ReplaceText
        )
    }

    /// Modes that require `old_text` parameter.
    pub fn requires_old_text(&self) -> bool {
        matches!(self, Self::ReplaceText | Self::InsertAfter)
    }
}

/// Result of creating a note.
#[derive(Debug, Clone)]
pub struct CreateNoteResult {
    pub path: String,
    pub absolute_path: PathBuf,
    pub bytes_written: usize,
}

/// Result of editing a note.
#[derive(Debug, Clone)]
pub struct EditNoteResult {
    pub path: String,
    pub absolute_path: PathBuf,
    pub mode: String,
    pub bytes_written: usize,
    pub backup_path: Option<PathBuf>,
}
