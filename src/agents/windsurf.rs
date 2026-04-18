use anyhow::Result;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::agent::{AgentConfig, DetectedConfig};
use crate::frontmatter;
use crate::ir::{ScopedRule, UniversalRules};

pub struct Windsurf;

impl AgentConfig for Windsurf {
    fn name(&self) -> &'static str {
        "windsurf"
    }

    fn detect(&self, project_root: &Path) -> Vec<DetectedConfig> {
        let mut configs = Vec::new();

        let rules_dir = project_root.join(".windsurf/rules");
        if rules_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&rules_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().is_some_and(|ext| ext == "md") {
                        if let Ok(meta) = path.metadata() {
                            configs.push(DetectedConfig {
                                path,
                                modified: meta
                                    .modified()
                                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
                                agent_name: "windsurf".to_string(),
                            });
                        }
                    }
                }
            }
        }

        configs
    }

    fn parse(&self, content: &str, _path: &Path) -> Result<UniversalRules> {
        let mut rules = UniversalRules::new();
        let (fm, body) = frontmatter::parse_frontmatter(content);

        if let Some(ref fm_map) = fm {
            let trigger = fm_map
                .get("trigger")
                .and_then(|v| v.as_str())
                .unwrap_or("always_on");

            match trigger {
                "glob" => {
                    let globs = extract_globs(fm_map);
                    for glob in globs {
                        rules.scoped_rules.push(ScopedRule {
                            pattern: glob,
                            instruction: body.trim().to_string(),
                        });
                    }
                    return Ok(rules);
                }
                "always_on" | "model_decision" => {
                    parse_body_into_rules(&body, &mut rules);
                }
                "manual" => {
                    // Manual rules → coding standard (user-activated)
                    rules.coding_standards.push(body.trim().to_string());
                }
                _ => {
                    parse_body_into_rules(&body, &mut rules);
                }
            }
        } else {
            // No frontmatter → treat as always-on
            parse_body_into_rules(content, &mut rules);
        }

        Ok(rules)
    }

    fn serialize(&self, rules: &UniversalRules) -> Vec<(PathBuf, String)> {
        let mut files = Vec::new();

        // General rules → always_on rule file
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
                "trigger".to_string(),
                serde_yaml::Value::String("always_on".to_string()),
            );

            let content = frontmatter::serialize_frontmatter(&fm, body.trim());
            files.push((PathBuf::from(".windsurf/rules/general.md"), content));
        }

        // Scoped rules → glob-triggered rule files
        for rule in &rules.scoped_rules {
            let filename = slugify_pattern(&rule.pattern);
            let path = PathBuf::from(format!(".windsurf/rules/{}.md", filename));

            let mut fm = BTreeMap::new();
            fm.insert(
                "trigger".to_string(),
                serde_yaml::Value::String("glob".to_string()),
            );
            fm.insert(
                "globs".to_string(),
                serde_yaml::Value::String(rule.pattern.clone()),
            );

            let content = frontmatter::serialize_frontmatter(&fm, &rule.instruction);
            files.push((path, content));
        }

        files
    }

    fn default_paths(&self) -> Vec<PathBuf> {
        vec![PathBuf::from(".windsurf/rules")]
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

fn parse_body_into_rules(body: &str, rules: &mut UniversalRules) {
    let mut current_content = String::new();
    let mut in_first_section = true;

    for line in body.lines() {
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
    fn test_parse_glob_rule() {
        let windsurf = Windsurf;
        let content = "---\ntrigger: glob\nglobs: \"**/*.test.ts\"\n---\n\nUse describe/it blocks.";
        let rules = windsurf
            .parse(content, Path::new(".windsurf/rules/testing.md"))
            .unwrap();
        assert_eq!(rules.scoped_rules.len(), 1);
        assert_eq!(rules.scoped_rules[0].pattern, "**/*.test.ts");
    }

    #[test]
    fn test_parse_always_on() {
        let windsurf = Windsurf;
        let content = "---\ntrigger: always_on\n---\n\n- Use bun, not npm\n- Prefer TypeScript\n";
        let rules = windsurf
            .parse(content, Path::new(".windsurf/rules/general.md"))
            .unwrap();
        assert!(rules.coding_standards.contains(&"Use bun, not npm".to_string()));
    }

    #[test]
    fn test_serialize() {
        let windsurf = Windsurf;
        let rules = UniversalRules {
            project_context: "My project".to_string(),
            coding_standards: vec!["Use Rust".to_string()],
            scoped_rules: vec![ScopedRule {
                pattern: "**/*.ts".to_string(),
                instruction: "Strict mode".to_string(),
            }],
        };
        let files = windsurf.serialize(&rules);
        assert_eq!(files.len(), 2);
        assert!(files[0].1.contains("always_on"));
        assert!(files[1].1.contains("glob"));
    }
}
