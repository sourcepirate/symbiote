use anyhow::Result;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::agent::{AgentConfig, DetectedConfig};
use crate::frontmatter;
use crate::ir::{ScopedRule, UniversalRules};

pub struct Copilot;

impl AgentConfig for Copilot {
    fn name(&self) -> &'static str {
        "copilot"
    }

    fn detect(&self, project_root: &Path) -> Vec<DetectedConfig> {
        let mut configs = Vec::new();

        // Main instructions file
        let main_path = project_root.join(".github/copilot-instructions.md");
        if main_path.exists()
            && let Ok(meta) = main_path.metadata()
        {
            configs.push(DetectedConfig {
                path: main_path,
                modified: meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH),
                agent_name: "copilot".to_string(),
            });
        }

        // Scoped instruction files
        let instructions_dir = project_root.join(".github/instructions");
        if instructions_dir.is_dir()
            && let Ok(entries) = std::fs::read_dir(&instructions_dir)
        {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "md")
                    && path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .ends_with(".instructions.md")
                    && let Ok(meta) = path.metadata()
                {
                    configs.push(DetectedConfig {
                        path,
                        modified: meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH),
                        agent_name: "copilot".to_string(),
                    });
                }
            }
        }

        configs
    }

    fn parse(&self, content: &str, path: &Path) -> Result<UniversalRules> {
        let (fm, body) = frontmatter::parse_frontmatter(content);
        let mut rules = UniversalRules::new();

        // Check if this is a scoped instruction file (has applyTo frontmatter)
        if let Some(ref fm_map) = fm
            && let Some(apply_to) = fm_map.get("applyTo")
        {
            let pattern = yaml_value_to_string(apply_to);
            if !pattern.is_empty() {
                rules.scoped_rules.push(ScopedRule {
                    pattern,
                    instruction: body.trim().to_string(),
                });
                return Ok(rules);
            }
        }

        // Main instructions file — parse markdown into project context + coding standards
        parse_markdown_body(&body, &mut rules);

        // If path is a scoped file but no frontmatter, use filename as hint
        if path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .ends_with(".instructions.md")
            && path.file_name().unwrap_or_default().to_string_lossy() != "copilot-instructions.md"
        {
            // Treat entire content as a general standard if no glob found
            if rules.project_context.is_empty() && rules.coding_standards.is_empty() {
                rules.coding_standards.push(body.trim().to_string());
            }
        }

        Ok(rules)
    }

    fn serialize(&self, rules: &UniversalRules) -> Vec<(PathBuf, String)> {
        let mut files = Vec::new();

        // Main instructions file with project context + coding standards
        let mut main_body = String::new();
        if !rules.project_context.is_empty() {
            main_body.push_str(&rules.project_context);
            main_body.push_str("\n\n");
        }
        if !rules.coding_standards.is_empty() {
            main_body.push_str("## Coding Standards\n\n");
            for standard in &rules.coding_standards {
                main_body.push_str("- ");
                main_body.push_str(standard);
                main_body.push('\n');
            }
        }

        if !main_body.trim().is_empty() {
            files.push((
                PathBuf::from(".github/copilot-instructions.md"),
                main_body.trim().to_string() + "\n",
            ));
        }

        // Scoped rules → separate .instructions.md files
        for rule in &rules.scoped_rules {
            let filename = slugify_pattern(&rule.pattern);
            let path = PathBuf::from(format!(".github/instructions/{}.instructions.md", filename));

            let mut fm = BTreeMap::new();
            fm.insert(
                "applyTo".to_string(),
                serde_yaml::Value::String(rule.pattern.clone()),
            );
            let content = frontmatter::serialize_frontmatter(&fm, &rule.instruction);
            files.push((path, content));
        }

        files
    }

    fn default_paths(&self) -> Vec<PathBuf> {
        vec![PathBuf::from(".github/copilot-instructions.md")]
    }
}

fn yaml_value_to_string(val: &serde_yaml::Value) -> String {
    match val {
        serde_yaml::Value::String(s) => s.clone(),
        other => serde_yaml::to_string(other)
            .unwrap_or_default()
            .trim()
            .to_string(),
    }
}

fn parse_markdown_body(body: &str, rules: &mut UniversalRules) {
    let mut current_section: Option<String> = None;
    let mut current_content = String::new();
    let mut first_section = true;

    for line in body.lines() {
        if line.starts_with('#') {
            // Flush previous section
            if let Some(ref _section) = current_section {
                flush_section(rules, &current_content, first_section);
                first_section = false;
            } else if !current_content.trim().is_empty() {
                // Content before any header → project context
                rules.project_context = current_content.trim().to_string();
                first_section = false;
            }
            current_section = Some(line.trim_start_matches('#').trim().to_string());
            current_content.clear();
        } else {
            current_content.push_str(line);
            current_content.push('\n');
        }
    }

    // Flush last section
    if current_section.is_some() {
        flush_section(rules, &current_content, first_section);
    } else if !current_content.trim().is_empty() {
        rules.project_context = current_content.trim().to_string();
    }
}

fn flush_section(rules: &mut UniversalRules, content: &str, is_first: bool) {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return;
    }

    if is_first && rules.project_context.is_empty() {
        rules.project_context = trimmed.to_string();
    } else {
        // Parse bullet points as individual coding standards
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
    fn test_parse_main_instructions() {
        let copilot = Copilot;
        let content = "# Project\n\nThis is a Rust project.\n\n## Standards\n\n- Use snake_case\n- Write tests\n";
        let rules = copilot
            .parse(content, Path::new(".github/copilot-instructions.md"))
            .unwrap();
        assert_eq!(rules.project_context, "This is a Rust project.");
        assert!(
            rules
                .coding_standards
                .contains(&"Use snake_case".to_string())
        );
        assert!(rules.coding_standards.contains(&"Write tests".to_string()));
    }

    #[test]
    fn test_parse_scoped_instruction() {
        let copilot = Copilot;
        let content = "---\napplyTo: \"**/*.ts\"\n---\n\nUse strict TypeScript with no any types.";
        let rules = copilot
            .parse(
                content,
                Path::new(".github/instructions/ts.instructions.md"),
            )
            .unwrap();
        assert_eq!(rules.scoped_rules.len(), 1);
        assert_eq!(rules.scoped_rules[0].pattern, "**/*.ts");
    }

    #[test]
    fn test_serialize_roundtrip() {
        let copilot = Copilot;
        let rules = UniversalRules {
            project_context: "A test project".to_string(),
            coding_standards: vec!["Use Rust".to_string(), "Write docs".to_string()],
            scoped_rules: vec![ScopedRule {
                pattern: "**/*.ts".to_string(),
                instruction: "Use strict mode".to_string(),
            }],
        };
        let files = copilot.serialize(&rules);
        assert_eq!(files.len(), 2); // main + 1 scoped
        assert!(files[0].1.contains("A test project"));
        assert!(files[1].1.contains("applyTo"));
    }
}
