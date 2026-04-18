use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::agent::{AgentConfig, DetectedConfig};
use crate::ir::UniversalRules;

pub struct OpenCode;

impl AgentConfig for OpenCode {
    fn name(&self) -> &'static str {
        "opencode"
    }

    fn detect(&self, project_root: &Path) -> Vec<DetectedConfig> {
        let mut configs = Vec::new();

        // .opencode.json (config file — indicates OpenCode is used)
        let config_path = project_root.join(".opencode.json");
        if config_path.exists()
            && let Ok(meta) = config_path.metadata()
        {
            configs.push(DetectedConfig {
                path: config_path,
                modified: meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH),
                agent_name: "opencode".to_string(),
            });
        }

        // OpenCode.md (memory/instructions file)
        let md_path = project_root.join("OpenCode.md");
        if md_path.exists()
            && let Ok(meta) = md_path.metadata()
        {
            configs.push(DetectedConfig {
                path: md_path,
                modified: meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH),
                agent_name: "opencode".to_string(),
            });
        }

        configs
    }

    fn parse(&self, content: &str, path: &Path) -> Result<UniversalRules> {
        let mut rules = UniversalRules::new();

        // .opencode.json is a config file, not instructions — skip parsing instructions from it
        if path.extension().is_some_and(|ext| ext == "json") {
            return Ok(rules);
        }

        // OpenCode.md → plain markdown parsing
        parse_opencode_markdown(content, &mut rules);
        Ok(rules)
    }

    fn serialize(&self, rules: &UniversalRules) -> Vec<(PathBuf, String)> {
        let mut body = String::new();

        if !rules.project_context.is_empty() {
            body.push_str("# Project Context\n\n");
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

        vec![(PathBuf::from("OpenCode.md"), body.trim().to_string() + "\n")]
    }

    fn default_paths(&self) -> Vec<PathBuf> {
        vec![PathBuf::from("OpenCode.md")]
    }
}

fn parse_opencode_markdown(content: &str, rules: &mut UniversalRules) {
    let mut current_content = String::new();
    let mut in_first_section = true;

    for line in content.lines() {
        if line.starts_with('#') {
            let trimmed = current_content.trim();
            if !trimmed.is_empty() {
                if in_first_section {
                    rules.project_context = trimmed.to_string();
                    in_first_section = false;
                } else {
                    add_standards(trimmed, rules);
                }
            }
            current_content.clear();
        } else {
            current_content.push_str(line);
            current_content.push('\n');
        }
    }

    let trimmed = current_content.trim();
    if !trimmed.is_empty() {
        if in_first_section {
            rules.project_context = trimmed.to_string();
        } else {
            add_standards(trimmed, rules);
        }
    }
}

fn add_standards(content: &str, rules: &mut UniversalRules) {
    let mut found_bullets = false;
    for line in content.lines() {
        let line = line.trim();
        if let Some(stripped) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
            rules.coding_standards.push(stripped.to_string());
            found_bullets = true;
        }
    }
    if !found_bullets && !content.is_empty() {
        rules.coding_standards.push(content.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_opencode_md() {
        let opencode = OpenCode;
        let content = "# Project\n\nA Rust project.\n\n## Rules\n\n- Test everything\n";
        let rules = opencode.parse(content, Path::new("OpenCode.md")).unwrap();
        assert_eq!(rules.project_context, "A Rust project.");
        assert!(
            rules
                .coding_standards
                .contains(&"Test everything".to_string())
        );
    }

    #[test]
    fn test_parse_json_skipped() {
        let opencode = OpenCode;
        let content = r#"{"providers": {}, "debug": false}"#;
        let rules = opencode
            .parse(content, Path::new(".opencode.json"))
            .unwrap();
        assert!(rules.is_empty());
    }

    #[test]
    fn test_serialize() {
        let opencode = OpenCode;
        let rules = UniversalRules {
            project_context: "Test".to_string(),
            coding_standards: vec!["Rule 1".to_string()],
            scoped_rules: vec![],
        };
        let files = opencode.serialize(&rules);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, PathBuf::from("OpenCode.md"));
    }
}
