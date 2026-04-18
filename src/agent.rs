use anyhow::Result;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::ir::UniversalRules;

/// Metadata about a detected agent configuration file
#[derive(Debug, Clone)]
pub struct DetectedConfig {
    pub path: PathBuf,
    pub modified: SystemTime,
    pub agent_name: String,
}

/// Every agent vendor must implement this trait
pub trait AgentConfig {
    /// Human-readable name of this agent (e.g. "copilot", "claude", "cursor")
    fn name(&self) -> &'static str;

    /// Detect all config files for this agent under the given project root
    fn detect(&self, project_root: &Path) -> Vec<DetectedConfig>;

    /// Parse the content of a config file into the universal IR
    fn parse(&self, content: &str, path: &Path) -> Result<UniversalRules>;

    /// Serialize universal rules into one or more files for this agent
    /// Returns Vec of (relative_path, file_content) pairs
    fn serialize(&self, rules: &UniversalRules) -> Vec<(PathBuf, String)>;

    /// The default/primary marker file paths (relative to project root)
    fn default_paths(&self) -> Vec<PathBuf>;
}

/// Registry of all supported agents
pub fn all_agents() -> Vec<Box<dyn AgentConfig>> {
    vec![
        Box::new(crate::agents::copilot::Copilot),
        Box::new(crate::agents::claude::Claude),
        Box::new(crate::agents::cursor::Cursor),
        Box::new(crate::agents::windsurf::Windsurf),
        Box::new(crate::agents::gemini::Gemini),
        Box::new(crate::agents::opencode::OpenCode),
    ]
}

/// Get an agent by name (case-insensitive)
pub fn get_agent(name: &str) -> Option<Box<dyn AgentConfig>> {
    let lower = name.to_lowercase();
    all_agents().into_iter().find(|a| a.name() == lower)
}
