use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "symbiote",
    about = "Single Source of Truth for AI coding agent instructions",
    long_about = "Symbiote discovers, syncs, and translates project rules across GitHub Copilot, Claude Code, Cursor, Windsurf, Gemini CLI, and OpenCode.",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// List all detected agent configs and identify the leader (most recently updated)
    Detect,

    /// Sync the leader config to all follower agents
    Sync {
        /// Source agent to sync from (e.g. "claude", "cursor")
        #[arg(long)]
        from: Option<String>,

        /// Target agent to sync to (e.g. "copilot", "gemini")
        #[arg(long)]
        to: Option<String>,
    },

    /// Show a diff of how instructions vary between agents
    Diff {
        /// First agent to compare
        agent_a: Option<String>,

        /// Second agent to compare
        agent_b: Option<String>,
    },

    /// Initialize the .symbiote directory for checksum tracking
    Init,
}
