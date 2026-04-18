use std::path::Path;

use crate::agent::{DetectedConfig, all_agents};

/// Result of scanning a project for agent configurations
#[derive(Debug)]
pub struct DiscoveryResult {
    /// All detected agent config files, sorted by modification time (newest first)
    pub configs: Vec<DetectedConfig>,
    /// The "leader" — the most recently modified config
    pub leader: Option<DetectedConfig>,
}

/// Scan the project root for all known agent config files
pub fn discover(project_root: &Path) -> DiscoveryResult {
    let agents = all_agents();
    let mut configs: Vec<DetectedConfig> = Vec::new();

    for agent in &agents {
        let mut detected = agent.detect(project_root);
        configs.append(&mut detected);
    }

    // Sort by modification time, newest first
    configs.sort_by(|a, b| b.modified.cmp(&a.modified));

    let leader = configs.first().cloned();

    DiscoveryResult { configs, leader }
}

/// Get the display-friendly relative path
pub fn relative_path(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}
