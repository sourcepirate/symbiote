use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::agent::{AgentConfig, DetectedConfig};
use crate::ir::{ScopedRule, UniversalRules};

pub struct Gemini;

impl AgentConfig for Gemini {
    fn name(&self) -> &'static str {
        "gemini"
    }

    fn detect(&self, project_root: &Path) -> Vec<DetectedConfig> {
        let mut configs = Vec::new();

        let path = project_root.join("GEMINI.md");
        if path.exists() {
            if let Ok(meta) = path.metadata() {
                configs.push(DetectedConfig {
                    path,
                    modified: meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH),
                    agent_name: "gemini".to_string(),
                });
            }
        }

        configs
    }

    fn parse(&self, content: &str, _path: &Path) -> Result<UniversalRules> {
        let mut rules = UniversalRules::new();
        parse_gemini_markdown(content, &mut rules);
        Ok(rules)
    }

    fn serialize(&self, rules: &UniversalRules) -> Vec<(PathBuf, String)> {
        let mut body = String::new();

        if !rules.project_context.is_empty() {
            body.push_str(&rules.project_context);
            body.push_str("\n\n");
        }

        if !rules.coding_standards.is_empty() {
            body.push_str("## Coding Standards\n\n");
            for standard in &rules.coding_standards {
                body.push_str("- ");
                body.push_str(standard);
                body.push('\n');
            }
            body.push('\n');
        }

        // Scoped rules → sections with glob pattern noted
        if !rules.scoped_rules.is_empty() {
            body.push_str("## File-Specific Rules\n\n");
            for rule in &rules.scoped_rules {
                body.push_str(&format!("### `{}`\n\n", rule.pattern));
                body.push_str(&rule.instruction);
                body.push_str("\n\n");
            }
        }

        if body.trim().is_empty() {
            return vec![];
        }

        vec![(PathBuf::from("GEMINI.md"), body.trim().to_string() + "\n")]
    }

    fn default_paths(&self) -> Vec<PathBuf> {
        vec![PathBuf::from("GEMINI.md")]
    }
}

fn parse_gemini_markdown(content: &str, rules: &mut UniversalRules) {
    let mut current_header: Option<String> = None;
    let mut current_content = String::new();
    let mut in_first_section = true;

    for line in content.lines() {
        if line.starts_with('#') {
            // Flush previous section
            flush_gemini_section(
                &current_header,
                &current_content,
                rules,
                &mut in_first_section,
            );
            current_header = Some(line.trim_start_matches('#').trim().to_string());
            current_content.clear();
        } else {
            current_content.push_str(line);
            current_content.push('\n');
        }
    }

    // Flush last section
    flush_gemini_section(
        &current_header,
        &current_content,
        rules,
        &mut in_first_section,
    );
}

fn flush_gemini_section(
    header: &Option<String>,
    content: &str,
    rules: &mut UniversalRules,
    in_first_section: &mut bool,
) {
    let trimmed = content.trim();
    if trimmed.is_empty() && header.is_none() {
        return;
    }

    // Check if this is a "File-Specific Rules" subsection with a backtick-quoted glob
    if let Some(h) = header {
        if h.starts_with('`') && h.ends_with('`') {
            let pattern = h.trim_matches('`').to_string();
            if !pattern.is_empty() && !trimmed.is_empty() {
                rules.scoped_rules.push(ScopedRule {
                    pattern,
                    instruction: trimmed.to_string(),
                });
                return;
            }
        }
    }

    if trimmed.is_empty() {
        return;
    }

    if *in_first_section {
        rules.project_context = trimmed.to_string();
        *in_first_section = false;
    } else {
        // Parse bullet points as standards
        let mut found_bullets = false;
        for line in trimmed.lines() {
            let line = line.trim();
            if let Some(stripped) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
                rules.coding_standards.push(stripped.to_string());
                found_bullets = true;
            }
        }
        if !found_bullets {
            rules.coding_standards.push(trimmed.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_gemini_md() {
        let gemini = Gemini;
        let content =
            "# My Project\n\nA Rust project.\n\n## Standards\n\n- Use clippy\n- Format code\n";
        let rules = gemini.parse(content, Path::new("GEMINI.md")).unwrap();
        assert_eq!(rules.project_context, "A Rust project.");
        assert!(rules.coding_standards.contains(&"Use clippy".to_string()));
    }

    #[test]
    fn test_parse_scoped_sections() {
        let gemini = Gemini;
        let content = "# Project\n\nContext here.\n\n## File-Specific Rules\n\n### `**/*.ts`\n\nUse strict TypeScript.\n";
        let rules = gemini.parse(content, Path::new("GEMINI.md")).unwrap();
        assert_eq!(rules.scoped_rules.len(), 1);
        assert_eq!(rules.scoped_rules[0].pattern, "**/*.ts");
    }

    #[test]
    fn test_serialize_roundtrip() {
        let gemini = Gemini;
        let rules = UniversalRules {
            project_context: "Test project".to_string(),
            coding_standards: vec!["Be nice".to_string()],
            scoped_rules: vec![ScopedRule {
                pattern: "**/*.rs".to_string(),
                instruction: "Use Rust idioms".to_string(),
            }],
        };
        let files = gemini.serialize(&rules);
        assert_eq!(files.len(), 1);
        assert!(files[0].1.contains("Test project"));
        assert!(files[0].1.contains("`**/*.rs`"));
    }
}
