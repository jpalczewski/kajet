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
}
