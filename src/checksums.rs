use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const CHECKSUMS_DIR: &str = ".symbiote";
const CHECKSUMS_FILE: &str = "checksums.json";

/// Registry of file content checksums to prevent circular syncing
pub struct ChecksumRegistry {
    checksums_path: PathBuf,
    checksums: BTreeMap<String, String>,
}

impl ChecksumRegistry {
    /// Load the checksum registry from disk (or create empty if not found)
    pub fn load(project_root: &Path) -> Result<Self> {
        let checksums_path = project_root.join(CHECKSUMS_DIR).join(CHECKSUMS_FILE);
        let checksums = if checksums_path.exists() {
            let content =
                fs::read_to_string(&checksums_path).context("Failed to read checksums file")?;
            serde_json::from_str(&content).context("Failed to parse checksums file")?
        } else {
            BTreeMap::new()
        };

        Ok(Self {
            checksums_path,
            checksums,
        })
    }

    /// Check if a file's content has changed since the last sync
    pub fn has_changed(&self, relative_path: &str, content: &str) -> bool {
        let new_hash = compute_hash(content);
        match self.checksums.get(relative_path) {
            Some(stored_hash) => *stored_hash != new_hash,
            None => true, // New file, always "changed"
        }
    }

    /// Update the stored checksum for a file
    pub fn update(&mut self, relative_path: &str, content: &str) {
        let hash = compute_hash(content);
        self.checksums.insert(relative_path.to_string(), hash);
    }

    /// Save the registry to disk
    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.checksums_path.parent() {
            fs::create_dir_all(parent).context("Failed to create .symbiote directory")?;
        }
        let content = serde_json::to_string_pretty(&self.checksums)
            .context("Failed to serialize checksums")?;
        fs::write(&self.checksums_path, content).context("Failed to write checksums file")?;
        Ok(())
    }

    /// Initialize the .symbiote directory
    pub fn init(project_root: &Path) -> Result<PathBuf> {
        let dir = project_root.join(CHECKSUMS_DIR);
        fs::create_dir_all(&dir).context("Failed to create .symbiote directory")?;
        let registry = Self::load(project_root)?;
        registry.save()?;
        Ok(dir)
    }
}

fn compute_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_hash_deterministic() {
        let h1 = compute_hash("hello world");
        let h2 = compute_hash("hello world");
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_has_changed_new_file() {
        let registry = ChecksumRegistry {
            checksums_path: PathBuf::from("/tmp/test-checksums.json"),
            checksums: BTreeMap::new(),
        };
        assert!(registry.has_changed("CLAUDE.md", "some content"));
    }

    #[test]
    fn test_has_changed_same_content() {
        let mut registry = ChecksumRegistry {
            checksums_path: PathBuf::from("/tmp/test-checksums.json"),
            checksums: BTreeMap::new(),
        };
        registry.update("CLAUDE.md", "some content");
        assert!(!registry.has_changed("CLAUDE.md", "some content"));
    }

    #[test]
    fn test_has_changed_different_content() {
        let mut registry = ChecksumRegistry {
            checksums_path: PathBuf::from("/tmp/test-checksums.json"),
            checksums: BTreeMap::new(),
        };
        registry.update("CLAUDE.md", "old content");
        assert!(registry.has_changed("CLAUDE.md", "new content"));
    }
}
