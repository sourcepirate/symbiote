use serde::{Deserialize, Serialize};

/// The universal intermediate representation for agent instructions.
/// All agent formats are parsed into this, and serialized from this.
/// Translation: Agent A → UniversalRules → Agent B
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UniversalRules {
    /// High-level project context / description
    pub project_context: String,
    /// General coding standards (not tied to specific file patterns)
    pub coding_standards: Vec<String>,
    /// Rules scoped to specific file patterns (globs)
    pub scoped_rules: Vec<ScopedRule>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScopedRule {
    /// Glob pattern (e.g. "**/*.ts", "src/api/**")
    pub pattern: String,
    /// The instruction text for files matching this pattern
    pub instruction: String,
}

impl UniversalRules {
    pub fn new() -> Self {
        Self {
            project_context: String::new(),
            coding_standards: Vec::new(),
            scoped_rules: Vec::new(),
        }
    }

    /// Merge another UniversalRules into this one
    pub fn merge(&mut self, other: &UniversalRules) {
        if !other.project_context.is_empty() {
            if !self.project_context.is_empty() {
                self.project_context.push_str("\n\n");
            }
            self.project_context.push_str(&other.project_context);
        }
        self.coding_standards
            .extend(other.coding_standards.iter().cloned());
        self.scoped_rules.extend(other.scoped_rules.iter().cloned());
    }

    pub fn is_empty(&self) -> bool {
        self.project_context.is_empty()
            && self.coding_standards.is_empty()
            && self.scoped_rules.is_empty()
    }
}

impl Default for UniversalRules {
    fn default() -> Self {
        Self::new()
    }
}
