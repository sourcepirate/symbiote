use anyhow::{Context, Result, bail};
use colored::Colorize;
use std::fs;
use std::path::Path;

use crate::agent::{AgentConfig, DetectedConfig, all_agents, get_agent};
use crate::checksums::ChecksumRegistry;
use crate::discovery;
use crate::ir::UniversalRules;

/// Sync result statistics
pub struct SyncResult {
    pub files_written: usize,
    pub files_skipped: usize,
    pub errors: Vec<String>,
}

/// Sync the leader config to all follower agents
pub fn sync_all(project_root: &Path) -> Result<SyncResult> {
    let discovery_result = discovery::discover(project_root);

    let leader = discovery_result
        .leader
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No agent configuration files found. Nothing to sync."))?;

    println!(
        "{} {} ({})",
        "Leader:".bold().green(),
        discovery::relative_path(project_root, &leader.path),
        leader.agent_name.cyan()
    );

    // Parse leader into IR
    let rules = parse_detected_config(project_root, leader)?;

    if rules.is_empty() {
        bail!("Leader config is empty. Nothing to sync.");
    }

    // Sync to all other agents
    let mut registry = ChecksumRegistry::load(project_root)?;
    let agents = all_agents();
    let mut result = SyncResult {
        files_written: 0,
        files_skipped: 0,
        errors: Vec::new(),
    };

    for agent in &agents {
        if agent.name() == leader.agent_name {
            continue; // Skip the leader itself
        }
        sync_agent(
            project_root,
            agent.as_ref(),
            &rules,
            &mut registry,
            &mut result,
        );
    }

    registry.save()?;
    Ok(result)
}

/// Sync from a specific source agent to a specific target agent
pub fn sync_to(project_root: &Path, from: &str, to: &str) -> Result<SyncResult> {
    let source_agent =
        get_agent(from).ok_or_else(|| anyhow::anyhow!("Unknown source agent: {}", from))?;
    let target_agent =
        get_agent(to).ok_or_else(|| anyhow::anyhow!("Unknown target agent: {}", to))?;

    // Find source config files
    let source_configs = source_agent.detect(project_root);
    if source_configs.is_empty() {
        bail!(
            "No {} configuration found in this project.",
            source_agent.name()
        );
    }

    // Parse all source configs and merge into a single IR
    let mut rules = UniversalRules::new();
    for config in &source_configs {
        let content = fs::read_to_string(&config.path)
            .with_context(|| format!("Failed to read {}", config.path.display()))?;
        let parsed = source_agent.parse(&content, &config.path)?;
        rules.merge(&parsed);
    }

    if rules.is_empty() {
        bail!("Source config is empty. Nothing to sync.");
    }

    println!(
        "{} {} {} {}",
        "Syncing:".bold().green(),
        source_agent.name().cyan(),
        "→".bold(),
        target_agent.name().cyan()
    );

    // Sync to target
    let mut registry = ChecksumRegistry::load(project_root)?;
    let mut result = SyncResult {
        files_written: 0,
        files_skipped: 0,
        errors: Vec::new(),
    };

    sync_agent(
        project_root,
        target_agent.as_ref(),
        &rules,
        &mut registry,
        &mut result,
    );

    registry.save()?;
    Ok(result)
}

/// Write an agent's serialized files, respecting checksums
fn sync_agent(
    project_root: &Path,
    agent: &dyn AgentConfig,
    rules: &UniversalRules,
    registry: &mut ChecksumRegistry,
    result: &mut SyncResult,
) {
    let files = agent.serialize(rules);

    for (relative_path, content) in files {
        let rel_str = relative_path.to_string_lossy().to_string();
        let full_path = project_root.join(&relative_path);

        // Check if content has actually changed
        if !registry.has_changed(&rel_str, &content) {
            result.files_skipped += 1;
            println!("  {} {} (unchanged)", "skip".dimmed(), rel_str.dimmed());
            continue;
        }

        // Create parent directories
        if let Some(parent) = full_path.parent()
            && let Err(e) = fs::create_dir_all(parent)
        {
            result
                .errors
                .push(format!("Failed to create directory for {}: {}", rel_str, e));
            continue;
        }

        // Write the file
        match fs::write(&full_path, &content) {
            Ok(()) => {
                registry.update(&rel_str, &content);
                result.files_written += 1;
                println!(
                    "  {} {} ({})",
                    "write".green(),
                    rel_str,
                    agent.name().cyan()
                );
            }
            Err(e) => {
                result
                    .errors
                    .push(format!("Failed to write {}: {}", rel_str, e));
            }
        }
    }
}

/// Parse a detected config file into UniversalRules using the appropriate agent parser
fn parse_detected_config(project_root: &Path, config: &DetectedConfig) -> Result<UniversalRules> {
    let agent = get_agent(&config.agent_name)
        .ok_or_else(|| anyhow::anyhow!("Unknown agent: {}", config.agent_name))?;

    // If this agent has multiple config files, parse and merge all of them
    let all_configs = agent.detect(project_root);
    let mut rules = UniversalRules::new();

    for cfg in &all_configs {
        let content = fs::read_to_string(&cfg.path)
            .with_context(|| format!("Failed to read {}", cfg.path.display()))?;
        let parsed = agent.parse(&content, &cfg.path)?;
        rules.merge(&parsed);
    }

    Ok(rules)
}
