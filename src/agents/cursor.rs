use anyhow::Result;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::agent::{AgentConfig, DetectedConfig};
use crate::frontmatter;
use crate::ir::{ScopedRule, UniversalRules};

pub struct Cursor;

impl AgentConfig for Cursor {
    fn name(&self) -> &'static str {
        "cursor"
    }

    fn detect(&self, project_root: &Path) -> Vec<DetectedConfig> {
        let mut configs = Vec::new();

        // Legacy .cursorrules
        let legacy = project_root.join(".cursorrules");
        if legacy.exists() {
            if let Ok(meta) = legacy.metadata() {
                configs.push(DetectedConfig {
                    path: legacy,
                    modified: meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH),
                    agent_name: "cursor".to_string(),
                });
            }
        }

        // New .cursor/rules/*.mdc and .cursor/rules/*.md
        let rules_dir = project_root.join(".cursor/rules");
        if rules_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&rules_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().is_some_and(|ext| ext == "mdc" || ext == "md") {
                        if let Ok(meta) = path.metadata() {
                            configs.push(DetectedConfig {
                                path,
                                modified: meta
                                    .modified()
                                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
                                agent_name: "cursor".to_string(),
                            });
                        }
                    }
                }
            }
        }

        configs
    }

    fn parse(&self, content: &str, path: &Path) -> Result<UniversalRules> {
        let mut rules = UniversalRules::new();
        let is_mdc = path.extension().is_some_and(|ext| ext == "mdc");

        if is_mdc {
            let (fm, body) = frontmatter::parse_frontmatter(content);
            if let Some(ref fm_map) = fm {
                let globs = extract_globs(fm_map);
                let always_apply = fm_map
                    .get("alwaysApply")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                if !globs.is_empty() {
                    for glob in globs {
                        rules.scoped_rules.push(ScopedRule {
                            pattern: glob,
                            instruction: body.trim().to_string(),
                        });
                    }
                    return Ok(rules);
                }

                if always_apply {
                    // alwaysApply = true → general coding standards
                    parse_body_as_standards(&body, &mut rules);
                    return Ok(rules);
                }
            }
            // MDC with no special frontmatter → treat as general
            parse_body_as_standards(&body, &mut rules);
        } else {
            // Legacy .cursorrules or plain .md → parse as markdown
            parse_body_as_standards(content, &mut rules);
        }

        Ok(rules)
    }

    fn serialize(&self, rules: &UniversalRules) -> Vec<(PathBuf, String)> {
        let mut files = Vec::new();

        // General rules → single .mdc with alwaysApply: true
        if !rules.project_context.is_empty() || !rules.coding_standards.is_empty() {
            let mut body = String::new();
            if !rules.project_context.is_empty() {
                body.push_str(&rules.project_context);
                body.push_str("\n\n");
            }
            if !rules.coding_standards.is_empty() {
                for standard in &rules.coding_standards {
                    body.push_str("- ");
                    body.push_str(standard);
                    body.push('\n');
                }
            }

            let mut fm = BTreeMap::new();
            fm.insert(
                "description".to_string(),
                serde_yaml::Value::String("General project rules".to_string()),
            );
            fm.insert(
                "alwaysApply".to_string(),
                serde_yaml::Value::Bool(true),
            );

            let content = frontmatter::serialize_frontmatter(&fm, body.trim());
            files.push((PathBuf::from(".cursor/rules/general.mdc"), content));
        }

        // Scoped rules → separate .mdc files
        for rule in &rules.scoped_rules {
            let filename = slugify_pattern(&rule.pattern);
            let path = PathBuf::from(format!(".cursor/rules/{}.mdc", filename));

            let mut fm = BTreeMap::new();
            fm.insert(
                "description".to_string(),
                serde_yaml::Value::String(format!("Rules for {}", rule.pattern)),
            );
            fm.insert(
                "globs".to_string(),
                serde_yaml::Value::String(rule.pattern.clone()),
            );
            fm.insert(
                "alwaysApply".to_string(),
                serde_yaml::Value::Bool(false),
            );

            let content = frontmatter::serialize_frontmatter(&fm, &rule.instruction);
            files.push((path, content));
        }

        files
    }

    fn default_paths(&self) -> Vec<PathBuf> {
        vec![PathBuf::from(".cursor/rules")]
    }
}

fn extract_globs(fm: &BTreeMap<String, serde_yaml::Value>) -> Vec<String> {
    match fm.get("globs") {
        Some(serde_yaml::Value::String(s)) => vec![s.clone()],
        Some(serde_yaml::Value::Sequence(seq)) => seq
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

fn parse_body_as_standards(content: &str, rules: &mut UniversalRules) {
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
        if in_first_section && rules.project_context.is_empty() {
            // If content has bullet points, treat as standards directly
            if trimmed.lines().any(|l| l.trim().starts_with("- ") || l.trim().starts_with("* ")) {
                add_standards(trimmed, rules);
            } else {
                rules.project_context = trimmed.to_string();
            }
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

fn slugify_pattern(pattern: &str) -> String {
    pattern
        .replace("**", "all")
        .replace('*', "any")
        .replace('/', "-")
        .replace('.', "-")
        .replace('{', "")
        .replace('}', "")
        .replace(',', "-")
        .trim_matches('-')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_mdc_with_globs() {
        let cursor = Cursor;
        let content =
            "---\ndescription: \"Test rules\"\nglobs: \"**/*.test.ts\"\nalwaysApply: false\n---\n\nUse describe/it blocks.";
        let rules = cursor
            .parse(content, Path::new(".cursor/rules/testing.mdc"))
            .unwrap();
        assert_eq!(rules.scoped_rules.len(), 1);
        assert_eq!(rules.scoped_rules[0].pattern, "**/*.test.ts");
    }

    #[test]
    fn test_parse_mdc_always_apply() {
        let cursor = Cursor;
        let content = "---\nalwaysApply: true\n---\n\n- Use snake_case\n- Write tests\n";
        let rules = cursor
            .parse(content, Path::new(".cursor/rules/general.mdc"))
            .unwrap();
        assert!(rules.coding_standards.contains(&"Use snake_case".to_string()));
    }

    #[test]
    fn test_parse_legacy_cursorrules() {
        let cursor = Cursor;
        let content = "- Always use TypeScript\n- Prefer functional style\n";
        let rules = cursor
            .parse(content, Path::new(".cursorrules"))
            .unwrap();
        assert_eq!(rules.coding_standards.len(), 2);
    }

    #[test]
    fn test_serialize() {
        let cursor = Cursor;
        let rules = UniversalRules {
            project_context: "My project".to_string(),
            coding_standards: vec!["Use Rust".to_string()],
            scoped_rules: vec![ScopedRule {
                pattern: "**/*.ts".to_string(),
                instruction: "Strict mode".to_string(),
            }],
        };
        let files = cursor.serialize(&rules);
        assert_eq!(files.len(), 2);
        assert!(files[0].0.to_string_lossy().contains("general.mdc"));
        assert!(files[0].1.contains("alwaysApply"));
    }
}
