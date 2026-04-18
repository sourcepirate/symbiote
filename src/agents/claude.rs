use anyhow::Result;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::agent::{AgentConfig, DetectedConfig};
use crate::frontmatter;
use crate::ir::{ScopedRule, UniversalRules};

pub struct Claude;

impl AgentConfig for Claude {
    fn name(&self) -> &'static str {
        "claude"
    }

    fn detect(&self, project_root: &Path) -> Vec<DetectedConfig> {
        let mut configs = Vec::new();

        // CLAUDE.md at project root
        let main_path = project_root.join("CLAUDE.md");
        detect_file(&main_path, &mut configs);

        // .claude/CLAUDE.md
        let alt_path = project_root.join(".claude/CLAUDE.md");
        detect_file(&alt_path, &mut configs);

        // .claude/rules/*.md
        let rules_dir = project_root.join(".claude/rules");
        if rules_dir.is_dir()
            && let Ok(entries) = std::fs::read_dir(&rules_dir)
        {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "md") {
                    detect_file(&path, &mut configs);
                }
            }
        }

        configs
    }

    fn parse(&self, content: &str, path: &Path) -> Result<UniversalRules> {
        let mut rules = UniversalRules::new();

        // .claude/rules/*.md files may have `paths` frontmatter
        let is_rule_file = path.to_string_lossy().contains(".claude/rules/");

        if is_rule_file {
            let (fm, body) = frontmatter::parse_frontmatter(content);
            if let Some(ref fm_map) = fm
                && let Some(paths_val) = fm_map.get("paths")
            {
                let patterns = yaml_value_to_patterns(paths_val);
                for pattern in patterns {
                    rules.scoped_rules.push(ScopedRule {
                        pattern,
                        instruction: body.trim().to_string(),
                    });
                }
                return Ok(rules);
            }
            // Rule file without frontmatter → treat as coding standard
            rules.coding_standards.push(body.trim().to_string());
            return Ok(rules);
        }

        // Main CLAUDE.md — parse sections
        parse_claude_markdown(content, &mut rules);
        Ok(rules)
    }

    fn serialize(&self, rules: &UniversalRules) -> Vec<(PathBuf, String)> {
        let mut files = Vec::new();

        // Main CLAUDE.md
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

        if !body.trim().is_empty() {
            files.push((PathBuf::from("CLAUDE.md"), body.trim().to_string() + "\n"));
        }

        // Scoped rules → .claude/rules/*.md with `paths` frontmatter
        for rule in &rules.scoped_rules {
            let filename = slugify_pattern(&rule.pattern);
            let path = PathBuf::from(format!(".claude/rules/{}.md", filename));

            let mut fm = BTreeMap::new();
            fm.insert(
                "paths".to_string(),
                serde_yaml::Value::Sequence(vec![serde_yaml::Value::String(rule.pattern.clone())]),
            );
            let content = frontmatter::serialize_frontmatter(&fm, &rule.instruction);
            files.push((path, content));
        }

        files
    }

    fn default_paths(&self) -> Vec<PathBuf> {
        vec![PathBuf::from("CLAUDE.md")]
    }
}

fn detect_file(path: &Path, configs: &mut Vec<DetectedConfig>) {
    if path.exists()
        && let Ok(meta) = path.metadata()
    {
        configs.push(DetectedConfig {
            path: path.to_path_buf(),
            modified: meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH),
            agent_name: "claude".to_string(),
        });
    }
}

fn yaml_value_to_patterns(val: &serde_yaml::Value) -> Vec<String> {
    match val {
        serde_yaml::Value::String(s) => vec![s.clone()],
        serde_yaml::Value::Sequence(seq) => seq
            .iter()
            .filter_map(|v| {
                if let serde_yaml::Value::String(s) = v {
                    Some(s.clone())
                } else {
                    None
                }
            })
            .collect(),
        _ => vec![],
    }
}

fn parse_claude_markdown(content: &str, rules: &mut UniversalRules) {
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
                    extract_standards(trimmed, rules);
                }
            }
            current_content.clear();
        } else {
            current_content.push_str(line);
            current_content.push('\n');
        }
    }

    // Flush remaining
    let trimmed = current_content.trim();
    if !trimmed.is_empty() {
        if in_first_section {
            rules.project_context = trimmed.to_string();
        } else {
            extract_standards(trimmed, rules);
        }
    }
}

fn extract_standards(content: &str, rules: &mut UniversalRules) {
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

fn slugify_pattern(pattern: &str) -> String {
    pattern
        .replace("**", "all")
        .replace('*', "any")
        .replace(['/', '.'], "-")
        .replace(['{', '}'], "")
        .replace(',', "-")
        .trim_matches('-')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_claude_md() {
        let claude = Claude;
        let content =
            "# My Project\n\nA great Rust project.\n\n## Rules\n\n- Always test\n- Use clippy\n";
        let rules = claude.parse(content, Path::new("CLAUDE.md")).unwrap();
        assert_eq!(rules.project_context, "A great Rust project.");
        assert!(rules.coding_standards.contains(&"Always test".to_string()));
    }

    #[test]
    fn test_parse_scoped_rule() {
        let claude = Claude;
        let content = "---\npaths:\n  - \"src/api/**/*.ts\"\n---\n\nUse REST conventions.";
        let rules = claude
            .parse(content, Path::new(".claude/rules/api.md"))
            .unwrap();
        assert_eq!(rules.scoped_rules.len(), 1);
        assert_eq!(rules.scoped_rules[0].pattern, "src/api/**/*.ts");
    }

    #[test]
    fn test_serialize() {
        let claude = Claude;
        let rules = UniversalRules {
            project_context: "Test project".to_string(),
            coding_standards: vec!["Be nice".to_string()],
            scoped_rules: vec![ScopedRule {
                pattern: "**/*.rs".to_string(),
                instruction: "Use Rust idioms".to_string(),
            }],
        };
        let files = claude.serialize(&rules);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].0, PathBuf::from("CLAUDE.md"));
        assert!(files[0].1.contains("Test project"));
    }
}
