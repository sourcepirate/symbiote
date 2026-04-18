use anyhow::Result;
use colored::Colorize;
use similar::{ChangeTag, TextDiff};
use std::fs;
use std::path::Path;

use crate::agent::{all_agents, get_agent};
use crate::discovery;
use crate::ir::UniversalRules;

/// Show diffs between all detected agent configurations
pub fn diff_all(project_root: &Path) -> Result<()> {
    let discovery_result = discovery::discover(project_root);

    if discovery_result.configs.is_empty() {
        println!("{}", "No agent configurations found.".yellow());
        return Ok(());
    }

    // Collect agent name → parsed rules
    let mut agent_rules: Vec<(String, UniversalRules)> = Vec::new();
    let agents = all_agents();

    for agent in &agents {
        let configs = agent.detect(project_root);
        if configs.is_empty() {
            continue;
        }

        let mut rules = UniversalRules::new();
        for config in &configs {
            let content = fs::read_to_string(&config.path)?;
            let parsed = agent.parse(&content, &config.path)?;
            rules.merge(&parsed);
        }
        agent_rules.push((agent.name().to_string(), rules));
    }

    if agent_rules.len() < 2 {
        println!(
            "{} Only {} agent config(s) found. Need at least 2 to diff.",
            "Note:".yellow(),
            agent_rules.len()
        );
        if let Some((name, rules)) = agent_rules.first() {
            print_summary(name, rules);
        }
        return Ok(());
    }

    // Show structural summary
    println!("{}", "=== Agent Config Summary ===".bold());
    for (name, rules) in &agent_rules {
        print_summary(name, rules);
    }
    println!();

    // Show pairwise diffs
    for i in 0..agent_rules.len() {
        for j in (i + 1)..agent_rules.len() {
            let (name_a, rules_a) = &agent_rules[i];
            let (name_b, rules_b) = &agent_rules[j];

            println!(
                "{} {} vs {}",
                "--- Diff:".bold(),
                name_a.cyan(),
                name_b.cyan()
            );

            let text_a = rules_to_canonical_text(rules_a);
            let text_b = rules_to_canonical_text(rules_b);

            if text_a == text_b {
                println!("  {}", "Identical.".green());
            } else {
                show_unified_diff(name_a, &text_a, name_b, &text_b);
            }
            println!();
        }
    }

    Ok(())
}

/// Diff a specific pair of agents
pub fn diff_pair(project_root: &Path, agent_a: &str, agent_b: &str) -> Result<()> {
    let a = get_agent(agent_a).ok_or_else(|| anyhow::anyhow!("Unknown agent: {}", agent_a))?;
    let b = get_agent(agent_b).ok_or_else(|| anyhow::anyhow!("Unknown agent: {}", agent_b))?;

    let rules_a = parse_agent_rules(project_root, a.as_ref())?;
    let rules_b = parse_agent_rules(project_root, b.as_ref())?;

    println!(
        "{} {} vs {}",
        "--- Diff:".bold(),
        agent_a.cyan(),
        agent_b.cyan()
    );

    let text_a = rules_to_canonical_text(&rules_a);
    let text_b = rules_to_canonical_text(&rules_b);

    if text_a == text_b {
        println!("  {}", "Identical.".green());
    } else {
        show_unified_diff(agent_a, &text_a, agent_b, &text_b);
    }

    Ok(())
}

fn parse_agent_rules(
    project_root: &Path,
    agent: &dyn crate::agent::AgentConfig,
) -> Result<UniversalRules> {
    let configs = agent.detect(project_root);
    let mut rules = UniversalRules::new();
    for config in &configs {
        let content = fs::read_to_string(&config.path)?;
        let parsed = agent.parse(&content, &config.path)?;
        rules.merge(&parsed);
    }
    Ok(rules)
}

/// Convert rules to a canonical text representation for diffing
fn rules_to_canonical_text(rules: &UniversalRules) -> String {
    let mut text = String::new();

    if !rules.project_context.is_empty() {
        text.push_str("# Project Context\n");
        text.push_str(&rules.project_context);
        text.push_str("\n\n");
    }

    if !rules.coding_standards.is_empty() {
        text.push_str("# Coding Standards\n");
        for standard in &rules.coding_standards {
            text.push_str("- ");
            text.push_str(standard);
            text.push('\n');
        }
        text.push('\n');
    }

    if !rules.scoped_rules.is_empty() {
        text.push_str("# Scoped Rules\n");
        for rule in &rules.scoped_rules {
            text.push_str(&format!("## {}\n", rule.pattern));
            text.push_str(&rule.instruction);
            text.push_str("\n\n");
        }
    }

    text
}

fn show_unified_diff(name_a: &str, text_a: &str, name_b: &str, text_b: &str) {
    let diff = TextDiff::from_lines(text_a, text_b);

    for change in diff.iter_all_changes() {
        let sign = match change.tag() {
            ChangeTag::Delete => "-",
            ChangeTag::Insert => "+",
            ChangeTag::Equal => " ",
        };
        let line = format!("{} {}", sign, change.value().trim_end_matches('\n'));
        let styled = match change.tag() {
            ChangeTag::Delete => line.red().to_string(),
            ChangeTag::Insert => line.green().to_string(),
            ChangeTag::Equal => line.dimmed().to_string(),
        };
        println!("  {}", styled);
    }

    let _ = (name_a, name_b); // Used in header above
}

fn print_summary(name: &str, rules: &UniversalRules) {
    println!(
        "  {} {} context={} standards={} scoped={}",
        "●".cyan(),
        name.bold(),
        if rules.project_context.is_empty() {
            "no".dimmed().to_string()
        } else {
            "yes".green().to_string()
        },
        rules.coding_standards.len().to_string().yellow(),
        rules.scoped_rules.len().to_string().yellow(),
    );
}
