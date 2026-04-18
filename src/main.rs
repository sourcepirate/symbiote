mod agent;
mod agents;
mod checksums;
mod cli;
mod diff;
mod discovery;
mod frontmatter;
mod ir;
mod sync;

use anyhow::Result;
use clap::Parser;
use colored::Colorize;
use std::env;
use std::time::UNIX_EPOCH;

use cli::{Cli, Commands};

fn main() -> Result<()> {
    let cli = Cli::parse();
    let project_root = env::current_dir()?;

    match cli.command {
        Commands::Detect => cmd_detect(&project_root),
        Commands::Sync { from, to } => cmd_sync(&project_root, from, to),
        Commands::Diff { agent_a, agent_b } => cmd_diff(&project_root, agent_a, agent_b),
        Commands::Init => cmd_init(&project_root),
    }
}

fn cmd_detect(project_root: &std::path::Path) -> Result<()> {
    let result = discovery::discover(project_root);

    if result.configs.is_empty() {
        println!("{}", "No agent configuration files detected.".yellow());
        println!("\nSupported agents: copilot, claude, cursor, windsurf, gemini, opencode");
        return Ok(());
    }

    println!("{}", "Detected agent configurations:".bold());
    println!();

    for (i, config) in result.configs.iter().enumerate() {
        let rel_path = discovery::relative_path(project_root, &config.path);
        let age = format_age(config.modified);

        let is_leader = i == 0;
        if is_leader {
            println!(
                "  {} {} ({}) — {} {}",
                "★".yellow(),
                rel_path.bold(),
                config.agent_name.cyan(),
                age.dimmed(),
                "[LEADER]".yellow().bold()
            );
        } else {
            println!(
                "  {} {} ({}) — {}",
                "●".dimmed(),
                rel_path,
                config.agent_name.cyan(),
                age.dimmed()
            );
        }
    }

    println!();
    println!(
        "The {} is the most recently modified config and will be used as the source of truth.",
        "leader".yellow().bold()
    );

    Ok(())
}

fn cmd_sync(
    project_root: &std::path::Path,
    from: Option<String>,
    to: Option<String>,
) -> Result<()> {
    let result = match (from, to) {
        (Some(from), Some(to)) => sync::sync_to(project_root, &from, &to)?,
        (None, None) => sync::sync_all(project_root)?,
        (Some(_), None) => {
            anyhow::bail!("--from requires --to. Use: symbiote sync --from <agent> --to <agent>");
        }
        (None, Some(_)) => {
            anyhow::bail!("--to requires --from. Use: symbiote sync --from <agent> --to <agent>");
        }
    };

    println!();
    println!(
        "{} {} written, {} skipped, {} errors",
        "Done:".bold().green(),
        result.files_written.to_string().green(),
        result.files_skipped.to_string().dimmed(),
        if result.errors.is_empty() {
            "0".green().to_string()
        } else {
            result.errors.len().to_string().red().to_string()
        }
    );

    for err in &result.errors {
        eprintln!("  {} {}", "error:".red(), err);
    }

    Ok(())
}

fn cmd_diff(
    project_root: &std::path::Path,
    agent_a: Option<String>,
    agent_b: Option<String>,
) -> Result<()> {
    match (agent_a, agent_b) {
        (Some(a), Some(b)) => diff::diff_pair(project_root, &a, &b),
        (None, None) => diff::diff_all(project_root),
        _ => {
            anyhow::bail!("Provide either zero arguments (diff all) or two agent names.");
        }
    }
}

fn cmd_init(project_root: &std::path::Path) -> Result<()> {
    let dir = checksums::ChecksumRegistry::init(project_root)?;
    println!(
        "{} Initialized {} directory.",
        "✓".green(),
        dir.strip_prefix(project_root)
            .unwrap_or(&dir)
            .to_string_lossy()
    );
    println!(
        "  Add {} to your .gitignore if desired.",
        ".symbiote/".dimmed()
    );
    Ok(())
}

fn format_age(modified: std::time::SystemTime) -> String {
    let elapsed = modified.duration_since(UNIX_EPOCH).unwrap_or_default();
    let now = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let diff_secs = now.as_secs().saturating_sub(elapsed.as_secs());

    if diff_secs < 60 {
        "just now".to_string()
    } else if diff_secs < 3600 {
        format!("{}m ago", diff_secs / 60)
    } else if diff_secs < 86400 {
        format!("{}h ago", diff_secs / 3600)
    } else {
        format!("{}d ago", diff_secs / 86400)
    }
}
