/// Strip YAML frontmatter (--- ... ---) from markdown content.
/// Returns (frontmatter_text, body_without_frontmatter).
pub fn strip_frontmatter(content: &str) -> (String, &str) {
    if !content.starts_with("---") {
        return (String::new(), content);
    }

    // Find closing ---
    if let Some(end) = content[3..].find("\n---") {
        let fm_end = end + 3; // offset from start of content[3..]
        let frontmatter = content[3..fm_end].trim().to_string();
        let body_start = fm_end + 4; // skip the closing \n---
        let body = &content[body_start..];
        (frontmatter, body)
    } else {
        (String::new(), content)
    }
}

pub fn extract_title(rel_path: &str, body: &str) -> String {
    // Look for first H1 in body
    for line in body.lines() {
        let trimmed = line.trim();
        if let Some(heading) = trimmed.strip_prefix("# ") {
            return heading.trim().to_string();
        }
        // Skip empty lines at start
        if !trimmed.is_empty() && !trimmed.starts_with('#') {
            break;
        }
    }

    // Fallback: filename without extension
    std::path::Path::new(rel_path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| rel_path.to_string())
}

pub fn extract_tags(frontmatter: &str) -> Vec<String> {
    if frontmatter.is_empty() {
        return Vec::new();
    }

    let mut tags = Vec::new();
    let mut in_tags = false;

    for line in frontmatter.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("tags:") {
            let inline = trimmed.strip_prefix("tags:").unwrap().trim();
            if !inline.is_empty() {
                // Inline format: tags: [tag1, tag2] or tags: tag1, tag2
                let clean = inline.trim_start_matches('[').trim_end_matches(']');
                for tag in clean.split(',') {
                    let t = tag.trim().trim_matches('"').trim_matches('\'').trim();
                    if !t.is_empty() {
                        tags.push(t.to_string());
                    }
                }
                return tags;
            }
            in_tags = true;
            continue;
        }

        if in_tags {
            if let Some(tag) = trimmed.strip_prefix("- ") {
                let t = tag.trim().trim_matches('"').trim_matches('\'');
                if !t.is_empty() {
                    tags.push(t.to_string());
                }
            } else {
                break; // End of tag list
            }
        }
    }

    tags
}

/// Extract date from frontmatter field and convert to Unix timestamp.
/// Supports ISO 8601, RFC 3339, and YYYY-MM-DD formats.
/// Returns None if field not found or parsing fails.
pub fn extract_date(frontmatter: &str, field_name: &str) -> Option<f64> {
    if frontmatter.is_empty() || field_name.is_empty() {
        return None;
    }

    let field_prefix = format!("{field_name}:");
    for line in frontmatter.lines() {
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_prefix(&field_prefix) {
            let date_str = value.trim().trim_matches('"').trim_matches('\'');
            return parse_date_string(date_str);
        }
    }

    None
}

fn parse_date_string(s: &str) -> Option<f64> {
    // Try ISO 8601 / RFC 3339 with timezone
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(dt.timestamp() as f64);
    }

    // Try ISO 8601 without timezone (assume UTC)
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
        return Some(dt.and_utc().timestamp() as f64);
    }

    // Try YYYY-MM-DD (date only, assume 00:00:00 UTC)
    if let Ok(date) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Some(date.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp() as f64);
    }

    // Try YYYY-MM-DD HH:MM:SS (space separator, no T)
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Some(dt.and_utc().timestamp() as f64);
    }

    None
}

/// Generate YAML frontmatter for a new note.
pub fn generate_frontmatter(
    title: &str,
    tags: &[String],
    default_tags: &[String],
    aliases: &[String],
    timestamp: Option<&str>,
    created_field: &str,
    modified_field: &str,
) -> String {
    let mut lines = vec!["---".to_string()];

    // Title — escape quotes
    let escaped_title = title.replace('"', "\\\"");
    lines.push(format!("title: \"{escaped_title}\""));

    // Aliases
    if !aliases.is_empty() {
        let formatted: Vec<String> = aliases.iter().map(|a| a.to_string()).collect();
        lines.push(format!("aliases: [{}]", formatted.join(", ")));
    }

    // Merge user tags + default_tags (deduped, preserving order)
    let mut all_tags: Vec<&str> = tags.iter().map(|s| s.as_str()).collect();
    for dt in default_tags {
        if !all_tags.contains(&dt.as_str()) {
            all_tags.push(dt.as_str());
        }
    }
    if !all_tags.is_empty() {
        let formatted: Vec<String> = all_tags.iter().map(|t| t.to_string()).collect();
        lines.push(format!("tags: [{}]", formatted.join(", ")));
    }

    // Timestamps
    if let Some(ts) = timestamp {
        if !created_field.is_empty() {
            lines.push(format!("{created_field}: {ts}"));
        }
        if !modified_field.is_empty() {
            lines.push(format!("{modified_field}: {ts}"));
        }
    }

    lines.push("---".to_string());
    lines.join("\n") + "\n"
}

/// Update or add a field in existing frontmatter.
///
/// - If the field exists: replaces the value.
/// - If the field is missing but frontmatter exists: adds before closing `---`.
/// - If no frontmatter: creates one with just this field.
pub fn update_frontmatter_field(content: &str, field: &str, value: &str) -> String {
    if !content.starts_with("---") {
        // No frontmatter — create minimal one
        return format!("---\n{field}: {value}\n---\n{content}");
    }

    // Find closing \n--- after the opening ---
    let search_start = 3; // skip opening "---"
    let closing_offset = match content[search_start..].find("\n---") {
        Some(pos) => search_start + pos, // absolute position of \n in "\n---"
        None => {
            return format!("---\n{field}: {value}\n---\n{content}");
        }
    };

    // Frontmatter body: between "---\n" and "\n---"
    let fm_start = if content[3..].starts_with('\n') { 4 } else { 3 };
    let after_closing = closing_offset + 4; // skip "\n---"
    let after_fm = &content[after_closing..];

    // Empty frontmatter (e.g. "---\n---")
    if fm_start > closing_offset {
        return format!("---\n{field}: {value}\n---{after_fm}");
    }

    let fm_body = &content[fm_start..closing_offset];

    // Check if field already exists
    let field_prefix = format!("{field}:");
    let mut new_fm_lines: Vec<String> = Vec::new();
    let mut found = false;
    for line in fm_body.lines() {
        if line.trim_start().starts_with(&field_prefix) {
            new_fm_lines.push(format!("{field}: {value}"));
            found = true;
        } else {
            new_fm_lines.push(line.to_string());
        }
    }
    if !found {
        new_fm_lines.push(format!("{field}: {value}"));
    }

    format!("---\n{}\n---{after_fm}", new_fm_lines.join("\n"))
}

/// Update a field in existing frontmatter — only if it already exists.
///
/// Returns content unchanged if the field is not found or no frontmatter exists.
/// Use this for edit operations to avoid adding duplicate timestamp fields
/// when the note was created by a different tool (e.g. Obsidian).
pub fn update_existing_frontmatter_field(content: &str, field: &str, value: &str) -> String {
    if !content.starts_with("---") {
        return content.to_string();
    }

    let search_start = 3;
    let closing_offset = match content[search_start..].find("\n---") {
        Some(pos) => search_start + pos,
        None => return content.to_string(),
    };

    let fm_start = if content[3..].starts_with('\n') { 4 } else { 3 };
    let after_closing = closing_offset + 4;
    let after_fm = &content[after_closing..];

    // Empty frontmatter — field can't exist, nothing to update
    if fm_start > closing_offset {
        return content.to_string();
    }

    let fm_body = &content[fm_start..closing_offset];

    let field_prefix = format!("{field}:");
    let mut new_fm_lines: Vec<String> = Vec::new();
    let mut found = false;
    for line in fm_body.lines() {
        if line.trim_start().starts_with(&field_prefix) {
            new_fm_lines.push(format!("{field}: {value}"));
            found = true;
        } else {
            new_fm_lines.push(line.to_string());
        }
    }

    if !found {
        return content.to_string();
    }

    format!("---\n{}\n---{after_fm}", new_fm_lines.join("\n"))
}

/// Add tags to frontmatter. Creates frontmatter block if missing.
/// Deduplicates - adding existing tag is a no-op.
pub fn add_tags(content: &str, tags: &[String]) -> Result<String, anyhow::Error> {
    let (fm_text, body) = strip_frontmatter(content);

    if fm_text.is_empty() {
        // No frontmatter → create new with tags
        let tags_str = tags.join(", ");
        let new_fm = format!("---\ntags: [{tags_str}]\n---\n");
        return Ok(format!("{new_fm}{body}"));
    }

    // Deserialize YAML
    let mut fm: serde_yaml::Value = serde_yaml::from_str(&fm_text)?;

    // Get existing tags (or empty list)
    let existing = if let Some(tags_val) = fm.get("tags") {
        if let Some(seq) = tags_val.as_sequence() {
            // Handle array format: tags: [rust, python]
            seq.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        } else if let Some(s) = tags_val.as_str() {
            // Handle string format: tags: rust OR tags: rust, python
            s.split(',').map(|t| t.trim().to_string()).collect()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    // Merge + deduplicate
    let mut merged = existing;
    for tag in tags {
        if !merged.contains(tag) {
            merged.push(tag.clone());
        }
    }

    // Update YAML structure
    fm["tags"] =
        serde_yaml::Value::Sequence(merged.into_iter().map(serde_yaml::Value::String).collect());

    // Serialize back
    let new_fm = serde_yaml::to_string(&fm)?;
    Ok(format!("---\n{new_fm}---\n{body}"))
}

/// Remove tags from frontmatter. Removing non-existent tag is a no-op.
pub fn remove_tags(content: &str, tags: &[String]) -> Result<String, anyhow::Error> {
    let (fm_text, body) = strip_frontmatter(content);

    if fm_text.is_empty() {
        // No frontmatter → nothing to remove
        return Ok(content.to_string());
    }

    // Deserialize YAML
    let mut fm: serde_yaml::Value = serde_yaml::from_str(&fm_text)?;

    // Get existing tags (or empty list)
    let existing = if let Some(tags_val) = fm.get("tags") {
        if let Some(seq) = tags_val.as_sequence() {
            // Handle array format: tags: [rust, python]
            seq.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        } else if let Some(s) = tags_val.as_str() {
            // Handle string format: tags: rust OR tags: rust, python
            s.split(',').map(|t| t.trim().to_string()).collect()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    // Filter out tags to remove
    let filtered: Vec<String> = existing.into_iter().filter(|t| !tags.contains(t)).collect();

    // Update YAML structure
    fm["tags"] = serde_yaml::Value::Sequence(
        filtered
            .into_iter()
            .map(serde_yaml::Value::String)
            .collect(),
    );

    // Serialize back
    let new_fm = serde_yaml::to_string(&fm)?;
    Ok(format!("---\n{new_fm}---\n{body}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_frontmatter_basic() {
        let md = "---\ntags:\n  - rust\n---\n# Title\n\nBody";
        let (fm, body) = strip_frontmatter(md);
        assert_eq!(fm, "tags:\n  - rust");
        assert!(body.contains("# Title"));
    }

    #[test]
    fn strip_frontmatter_none() {
        let md = "# Title\n\nBody";
        let (fm, body) = strip_frontmatter(md);
        assert!(fm.is_empty());
        assert_eq!(body, md);
    }

    #[test]
    fn extract_title_from_h1() {
        assert_eq!(extract_title("note.md", "# My Title\n\nText"), "My Title");
    }

    #[test]
    fn extract_title_fallback_to_filename() {
        assert_eq!(
            extract_title("my-note.md", "Just some text without a heading"),
            "my-note"
        );
    }

    #[test]
    fn extract_tags_list_format() {
        let fm = "tags:\n  - rust\n  - programming";
        assert_eq!(extract_tags(fm), vec!["rust", "programming"]);
    }

    #[test]
    fn extract_tags_inline_format() {
        assert_eq!(
            extract_tags("tags: [rust, programming]"),
            vec!["rust", "programming"]
        );
    }

    #[test]
    fn extract_tags_empty() {
        assert!(extract_tags("").is_empty());
    }

    // -- generate_frontmatter tests --

    #[test]
    fn generate_frontmatter_basic() {
        let result = generate_frontmatter(
            "My Note",
            &["rust".into(), "test".into()],
            &[],
            &[],
            Some("2026-02-08T12:00:00"),
            "created",
            "modified",
        );
        assert!(result.starts_with("---\n"));
        assert!(result.ends_with("---\n"));
        assert!(result.contains("title: \"My Note\""));
        assert!(result.contains("tags: [rust, test]"));
        assert!(result.contains("created: 2026-02-08T12:00:00"));
        assert!(result.contains("modified: 2026-02-08T12:00:00"));
    }

    #[test]
    fn generate_frontmatter_merge_default_tags() {
        let result = generate_frontmatter(
            "Note",
            &["rust".into()],
            &["kajet".into(), "rust".into()], // rust already in user tags
            &[],
            None,
            "created",
            "modified",
        );
        assert!(result.contains("tags: [rust, kajet]"));
        // No duplicates: "rust" appears only once
        let tag_count = result.matches("rust").count();
        assert_eq!(tag_count, 1);
    }

    #[test]
    fn generate_frontmatter_no_tags_no_timestamp() {
        let result = generate_frontmatter("Note", &[], &[], &[], None, "created", "modified");
        assert!(result.contains("title: \"Note\""));
        assert!(!result.contains("tags:"));
        assert!(!result.contains("created:"));
        assert!(!result.contains("modified:"));
    }

    #[test]
    fn generate_frontmatter_escapes_quotes() {
        let result = generate_frontmatter(
            "Note \"with\" quotes",
            &[],
            &[],
            &[],
            None,
            "created",
            "modified",
        );
        assert!(result.contains("title: \"Note \\\"with\\\" quotes\""));
    }

    #[test]
    fn generate_frontmatter_with_aliases() {
        let result = generate_frontmatter(
            "My Note",
            &["rust".into()],
            &[],
            &["alias1".into(), "alias2".into()],
            None,
            "created",
            "modified",
        );
        assert!(result.contains("aliases: [alias1, alias2]"));
    }

    #[test]
    fn generate_frontmatter_without_aliases() {
        let result = generate_frontmatter("My Note", &[], &[], &[], None, "created", "modified");
        assert!(!result.contains("aliases:"));
    }

    #[test]
    fn generate_frontmatter_empty_created_field() {
        let result =
            generate_frontmatter("Note", &[], &[], &[], Some("2026-02-08"), "", "modified");
        assert!(!result.contains("created:"));
        // Note: we don't search for a bare empty field name
        assert!(result.contains("modified: 2026-02-08"));
    }

    #[test]
    fn generate_frontmatter_both_fields_empty() {
        let result = generate_frontmatter("Note", &[], &[], &[], Some("2026-02-08"), "", "");
        assert!(!result.contains("2026-02-08"));
    }

    // -- update_frontmatter_field tests --

    #[test]
    fn update_existing_field() {
        let content = "---\ntitle: \"Old\"\nmodified: 2025-01-01\n---\n# Body\n";
        let result = update_frontmatter_field(content, "modified", "2026-02-08");
        assert!(result.contains("modified: 2026-02-08"));
        assert!(!result.contains("2025-01-01"));
        assert!(result.contains("# Body"));
    }

    #[test]
    fn update_add_new_field() {
        let content = "---\ntitle: \"Note\"\n---\n# Body\n";
        let result = update_frontmatter_field(content, "modified", "2026-02-08");
        assert!(result.contains("modified: 2026-02-08"));
        assert!(result.contains("title: \"Note\""));
        assert!(result.contains("# Body"));
    }

    #[test]
    fn update_no_frontmatter() {
        let content = "# Body\n\nText here.\n";
        let result = update_frontmatter_field(content, "modified", "2026-02-08");
        assert!(result.starts_with("---\n"));
        assert!(result.contains("modified: 2026-02-08"));
        assert!(result.contains("# Body"));
    }

    #[test]
    fn update_preserves_body_with_wikilinks() {
        let content = "---\ntags: [test]\n---\n# Współpraca\n\nSee [[Link]] and [[Other|alias]].\n";
        let result = update_frontmatter_field(content, "modified", "2026-02-08");
        assert!(result.contains("[[Link]]"));
        assert!(result.contains("[[Other|alias]]"));
        assert!(result.contains("Współpraca"));
    }

    // -- update_existing_frontmatter_field tests --

    #[test]
    fn update_existing_only_updates_present_field() {
        let content = "---\ntitle: \"Note\"\nmodified: 2025-01-01\n---\n# Body\n";
        let result = update_existing_frontmatter_field(content, "modified", "2026-02-08");
        assert!(result.contains("modified: 2026-02-08"));
        assert!(!result.contains("2025-01-01"));
    }

    #[test]
    fn update_existing_skips_missing_field() {
        // Note created by Obsidian with Polish field names — kajet should NOT add "modified:"
        let content = "---\ntitle: \"Note\"\nData aktualizacji: 2025-01-01\n---\n# Body\n";
        let result = update_existing_frontmatter_field(content, "modified", "2026-02-08");
        assert!(
            !result.contains("modified:"),
            "Should not add missing field"
        );
        assert!(
            result.contains("Data aktualizacji: 2025-01-01"),
            "Original field preserved"
        );
        assert_eq!(result, content);
    }

    #[test]
    fn update_existing_no_frontmatter_unchanged() {
        let content = "# Body\n\nText here.\n";
        let result = update_existing_frontmatter_field(content, "modified", "2026-02-08");
        assert_eq!(result, content);
    }

    #[test]
    fn update_empty_frontmatter() {
        let content = "---\n---\n# Body\n";
        let result = update_frontmatter_field(content, "modified", "2026-02-08");
        assert!(result.contains("modified: 2026-02-08"));
        assert!(result.contains("# Body"));
    }

    #[test]
    fn update_existing_empty_frontmatter_unchanged() {
        let content = "---\n---\n# Body\n";
        let result = update_existing_frontmatter_field(content, "modified", "2026-02-08");
        assert_eq!(result, content);
    }

    // -- add_tags / remove_tags tests --

    #[test]
    fn add_tags_to_empty_frontmatter() {
        let content = "# Note\n\nBody text.";
        let result = add_tags(content, &["rust".into(), "test".into()]).unwrap();
        assert!(result.starts_with("---\n"));
        assert!(result.contains("tags:"));
        assert!(result.contains("rust"));
        assert!(result.contains("test"));
        assert!(result.contains("# Note"));
    }

    #[test]
    fn add_tags_deduplicate() {
        let content = "---\ntags: [rust, programming]\n---\n# Note";
        let result = add_tags(content, &["rust".into(), "new".into()]).unwrap();
        // rust already exists → only new is added
        assert!(result.contains("rust"));
        assert!(result.contains("new"));
        // Count occurrences - "rust" should appear only once in tags section
        let rust_count = result.matches("rust").count();
        assert_eq!(rust_count, 1);
    }

    #[test]
    fn remove_tags_nonexistent() {
        let content = "---\ntags: [rust]\n---\n# Note";
        let result = remove_tags(content, &["python".into()]).unwrap();
        // Removing non-existent tag → no error
        assert!(result.contains("rust"));
        assert!(!result.contains("python"));
    }

    #[test]
    fn add_and_remove_same_tag() {
        let content = "---\ntags: [foo]\n---\n# Note";
        // Remove first, then add (operation order)
        let removed = remove_tags(content, &["foo".into()]).unwrap();
        let added = add_tags(&removed, &["foo".into()]).unwrap();
        // Result: tag exists (add wins)
        assert!(added.contains("foo"));
    }

    #[test]
    fn remove_all_tags() {
        let content = "---\ntags: [rust, python]\n---\n# Note";
        let result = remove_tags(content, &["rust".into(), "python".into()]).unwrap();
        // Tags field remains as empty list
        assert!(result.contains("tags:"));
        assert!(!result.contains("rust"));
        assert!(!result.contains("python"));
    }

    #[test]
    fn add_tags_string_format_single() {
        let content = "---\ntags: rust\n---\n# Note";
        let result = add_tags(content, &["python".into()]).unwrap();
        // Should preserve existing "rust" tag and add "python"
        assert!(result.contains("rust"));
        assert!(result.contains("python"));
    }

    #[test]
    fn add_tags_string_format_comma_separated() {
        let content = "---\ntags: rust, programming\n---\n# Note";
        let result = add_tags(content, &["test".into()]).unwrap();
        // Should preserve both existing tags
        assert!(result.contains("rust"));
        assert!(result.contains("programming"));
        assert!(result.contains("test"));
    }

    #[test]
    fn remove_tags_string_format_single() {
        let content = "---\ntags: rust\n---\n# Note";
        let result = remove_tags(content, &["rust".into()]).unwrap();
        // Should remove the tag successfully
        assert!(!result.contains("rust"));
    }

    #[test]
    fn remove_tags_string_format_comma_separated() {
        let content = "---\ntags: rust, python, test\n---\n# Note";
        let result = remove_tags(content, &["python".into()]).unwrap();
        // Should preserve "rust" and "test", remove "python"
        assert!(result.contains("rust"));
        assert!(result.contains("test"));
        assert!(!result.contains("python"));
    }

    #[test]
    fn remove_tags_string_format_nonexistent() {
        let content = "---\ntags: rust\n---\n# Note";
        let result = remove_tags(content, &["python".into()]).unwrap();
        // Should preserve "rust" when removing non-existent tag
        assert!(result.contains("rust"));
        assert!(!result.contains("python"));
    }

    // -- extract_date tests --

    #[test]
    fn extract_date_iso8601_with_timezone() {
        let fm = "created: 2026-02-08T12:30:00+01:00";
        let ts = extract_date(fm, "created");
        assert!(ts.is_some());
        // Verify it's a reasonable timestamp (around Feb 2026)
        let ts_val = ts.unwrap();
        assert!(ts_val > 1700000000.0 && ts_val < 2000000000.0);
    }

    #[test]
    fn extract_date_iso8601_without_timezone() {
        let fm = "modified: 2026-02-08T12:30:00";
        let ts = extract_date(fm, "modified");
        assert!(ts.is_some());
    }

    #[test]
    fn extract_date_date_only() {
        let fm = "created: 2026-02-08";
        let ts = extract_date(fm, "created");
        assert!(ts.is_some());
        // Should be midnight UTC (just verify timestamp is reasonable)
        let ts_val = ts.unwrap();
        assert!(ts_val > 0.0);
    }

    #[test]
    fn extract_date_space_separator() {
        let fm = "Data utworzenia: 2026-02-08 14:45:30";
        let ts = extract_date(fm, "Data utworzenia");
        assert!(ts.is_some());
    }

    #[test]
    fn extract_date_quoted_value() {
        let fm = "created: \"2026-02-08\"";
        let ts = extract_date(fm, "created");
        assert!(ts.is_some());
    }

    #[test]
    fn extract_date_field_not_found() {
        let fm = "title: My Note\ntags: [test]";
        let ts = extract_date(fm, "created");
        assert!(ts.is_none());
    }

    #[test]
    fn extract_date_invalid_format() {
        let fm = "created: not a date";
        let ts = extract_date(fm, "created");
        assert!(ts.is_none());
    }

    #[test]
    fn extract_date_empty_field_name() {
        let fm = "created: 2026-02-08";
        let ts = extract_date(fm, "");
        assert!(ts.is_none());
    }

    #[test]
    fn extract_date_polish_field_name() {
        let fm = "Data utworzenia: 2026-02-08\nTagi: [test]";
        let ts = extract_date(fm, "Data utworzenia");
        assert!(ts.is_some());
    }
}
