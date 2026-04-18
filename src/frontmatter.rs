use std::collections::BTreeMap;

/// Parse YAML frontmatter from a markdown document.
/// Returns (frontmatter_map, body) where frontmatter_map is None if no frontmatter found.
pub fn parse_frontmatter(content: &str) -> (Option<BTreeMap<String, serde_yaml::Value>>, String) {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return (None, content.to_string());
    }

    // Find the closing ---
    let after_open = &trimmed[3..];
    if let Some(close_pos) = after_open.find("\n---") {
        let yaml_str = &after_open[..close_pos].trim();
        let body_start = close_pos + 4; // skip \n---
        let body = after_open[body_start..]
            .trim_start_matches('\n')
            .to_string();

        match serde_yaml::from_str::<BTreeMap<String, serde_yaml::Value>>(yaml_str) {
            Ok(map) => (Some(map), body),
            Err(_) => (None, content.to_string()),
        }
    } else {
        (None, content.to_string())
    }
}

/// Serialize a frontmatter map and body back into a markdown document
pub fn serialize_frontmatter(
    frontmatter: &BTreeMap<String, serde_yaml::Value>,
    body: &str,
) -> String {
    if frontmatter.is_empty() {
        return body.to_string();
    }
    let yaml = serde_yaml::to_string(frontmatter).unwrap_or_default();
    format!("---\n{}---\n\n{}", yaml, body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_no_frontmatter() {
        let content = "# Hello\n\nSome content";
        let (fm, body) = parse_frontmatter(content);
        assert!(fm.is_none());
        assert_eq!(body, content);
    }

    #[test]
    fn test_parse_with_frontmatter() {
        let content = "---\napplyTo: \"**/*.ts\"\n---\n\n# Rules\n\nSome rules here";
        let (fm, body) = parse_frontmatter(content);
        assert!(fm.is_some());
        let fm = fm.unwrap();
        assert_eq!(
            fm.get("applyTo").unwrap(),
            &serde_yaml::Value::String("**/*.ts".to_string())
        );
        assert!(body.starts_with("# Rules"));
    }

    #[test]
    fn test_roundtrip() {
        let mut fm = BTreeMap::new();
        fm.insert(
            "globs".to_string(),
            serde_yaml::Value::String("**/*.rs".to_string()),
        );
        let body = "# My Rules\n\nDo stuff";
        let serialized = serialize_frontmatter(&fm, body);
        let (parsed_fm, parsed_body) = parse_frontmatter(&serialized);
        assert!(parsed_fm.is_some());
        assert_eq!(parsed_body.trim(), body);
    }
}
