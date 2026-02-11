use crate::filters::TagEditResult;
use anyhow::Result;

/// Apply tag operations to note content: remove tags first, then add, then optional timestamp update.
///
/// This function orchestrates the sequence of operations that must be applied in order:
/// 1. Remove tags (if any)
/// 2. Add tags (if any)
/// 3. Update timestamp field (if timestamp_update is Some)
///
/// # Parameters
///
/// - `content`: The markdown content with frontmatter
/// - `add`: Optional list of tags to add
/// - `remove`: Optional list of tags to remove
/// - `timestamp_update`: Optional (field_name, timestamp_value) to update in frontmatter
///
/// # Returns
///
/// `TagEditResult` with the modified content and counts of operations performed.
pub fn apply_tag_edits(
    content: &str,
    add: Option<&[String]>,
    remove: Option<&[String]>,
    timestamp_update: Option<(&str, &str)>,
) -> Result<TagEditResult> {
    let mut modified = content.to_string();
    let tags_removed = remove.map(|r| r.len()).unwrap_or(0);
    let tags_added = add.map(|a| a.len()).unwrap_or(0);

    if let Some(remove_tags) = remove {
        modified = kajet_parser::remove_tags(&modified, remove_tags)?;
    }

    if let Some(add_tags) = add {
        modified = kajet_parser::add_tags(&modified, add_tags)?;
    }

    let timestamp_updated = if let Some((field, value)) = timestamp_update {
        modified = kajet_parser::update_existing_frontmatter_field(&modified, field, value);
        true
    } else {
        false
    };

    Ok(TagEditResult {
        content: modified,
        tags_added,
        tags_removed,
        timestamp_updated,
    })
}
