use crate::types::Link;
use std::sync::LazyLock;

pub(crate) static WIKILINK_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"\[\[([^\]\|#]+)(?:#[^\]\|]*)?\|?([^\]]*)\]\]").unwrap());

/// Extract wikilinks from text, returning a `Link` per match.
pub fn extract_wikilinks(text: &str) -> Vec<Link> {
    WIKILINK_RE
        .captures_iter(text)
        .map(|cap| {
            let target = cap[1].trim().to_string();
            let alias_str = cap.get(2).map(|m| m.as_str().trim()).unwrap_or("");
            let alias = if alias_str.is_empty() {
                None
            } else {
                Some(alias_str.to_string())
            };
            Link {
                target,
                alias,
                resolved_path: None,
            }
        })
        .collect()
}

/// Replace `[[Target|alias]]` → `alias`, `[[Target]]` → `Target`.
pub fn resolve_wikilinks_in_text(text: &str) -> String {
    WIKILINK_RE
        .replace_all(text, |caps: &regex::Captures| {
            let alias = caps.get(2).map(|m| m.as_str().trim()).unwrap_or("");
            if alias.is_empty() {
                caps[1].trim().to_string()
            } else {
                alias.to_string()
            }
        })
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_wikilinks_basic() {
        let links = extract_wikilinks("See [[Note]] here");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target, "Note");
        assert_eq!(links[0].alias, None);
    }

    #[test]
    fn extract_wikilinks_with_alias() {
        let links = extract_wikilinks("[[Target|Display]]");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target, "Target");
        assert_eq!(links[0].alias, Some("Display".to_string()));
    }

    #[test]
    fn extract_wikilinks_with_heading() {
        let links = extract_wikilinks("[[Note#Section]]");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target, "Note");
        assert_eq!(links[0].alias, None);
    }

    #[test]
    fn extract_wikilinks_with_heading_and_alias() {
        let links = extract_wikilinks("[[Note#Sec|Alias]]");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target, "Note");
        assert_eq!(links[0].alias, Some("Alias".to_string()));
    }

    #[test]
    fn extract_wikilinks_multiple() {
        let links = extract_wikilinks("See [[A]] and [[B|display]] plus [[C#heading]]");
        assert_eq!(links.len(), 3);
        assert_eq!(links[0].target, "A");
        assert_eq!(links[1].target, "B");
        assert_eq!(links[1].alias, Some("display".to_string()));
        assert_eq!(links[2].target, "C");
    }

    #[test]
    fn resolve_wikilinks_cleans_content() {
        assert_eq!(resolve_wikilinks_in_text("See [[Note]]"), "See Note");
    }

    #[test]
    fn resolve_wikilinks_uses_alias() {
        assert_eq!(
            resolve_wikilinks_in_text("See [[Target|shown]]"),
            "See shown"
        );
    }

    #[test]
    fn resolve_wikilinks_with_heading() {
        assert_eq!(
            resolve_wikilinks_in_text("See [[Note#Section]]"),
            "See Note"
        );
    }

    #[test]
    fn resolve_wikilinks_with_heading_and_alias() {
        assert_eq!(
            resolve_wikilinks_in_text("See [[Note#Sec|Alias]]"),
            "See Alias"
        );
    }

    #[test]
    fn polish_wikilinks() {
        let links = extract_wikilinks("Zobacz [[Notatka|tekst]]");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target, "Notatka");
        assert_eq!(links[0].alias, Some("tekst".to_string()));

        assert_eq!(
            resolve_wikilinks_in_text("Zobacz [[Notatka|tekst]]"),
            "Zobacz tekst"
        );
    }

    #[test]
    fn no_wikilinks() {
        assert!(extract_wikilinks("Plain text without links").is_empty());
        assert_eq!(
            resolve_wikilinks_in_text("Plain text without links"),
            "Plain text without links"
        );
    }
}
