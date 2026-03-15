use std::collections::BTreeMap;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Per-repo cached state from the last watch cycle.
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct RepoState {
    pub head: String,
    pub latest_tag: Option<String>,
    pub language: Option<String>,
}

/// The full watch cache state, keyed by repo name.
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct WatchState {
    #[serde(default)]
    pub repos: BTreeMap<String, RepoState>,
}

/// Trait abstracting watch cache persistence for testability.
pub trait WatchCache: Send + Sync {
    fn load(&self) -> Result<WatchState>;
    fn save(&self, state: &WatchState) -> Result<()>;
}

/// Real implementation backed by a TOML file on disk.
pub struct FileWatchCache {
    pub path: std::path::PathBuf,
}

impl WatchCache for FileWatchCache {
    fn load(&self) -> Result<WatchState> {
        if !self.path.exists() {
            return Ok(WatchState::default());
        }
        let content = std::fs::read_to_string(&self.path)
            .with_context(|| format!("reading watch cache {}", self.path.display()))?;
        if content.trim().is_empty() {
            return Ok(WatchState::default());
        }
        let state: WatchState = toml::from_str(&content)
            .with_context(|| format!("parsing watch cache {}", self.path.display()))?;
        Ok(state)
    }

    fn save(&self, state: &WatchState) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating cache dir {}", parent.display()))?;
        }
        let content =
            toml::to_string_pretty(state).context("serializing watch cache")?;
        std::fs::write(&self.path, content)
            .with_context(|| format!("writing watch cache {}", self.path.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watch_state_roundtrip() {
        let mut state = WatchState::default();
        state.repos.insert(
            "test-repo".to_string(),
            RepoState {
                head: "abc123".to_string(),
                latest_tag: Some("v1.0.0".to_string()),
                language: Some("go".to_string()),
            },
        );

        let serialized = toml::to_string_pretty(&state).unwrap();
        let deserialized: WatchState = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.repos.len(), 1);
        assert_eq!(deserialized.repos["test-repo"].head, "abc123");
        assert_eq!(
            deserialized.repos["test-repo"].latest_tag.as_deref(),
            Some("v1.0.0")
        );
    }

    #[test]
    fn test_watch_state_default() {
        let state = WatchState::default();
        assert!(state.repos.is_empty());
    }

    #[test]
    fn test_file_watch_cache_missing_file() {
        let cache = FileWatchCache {
            path: std::path::PathBuf::from("/tmp/nonexistent-watch-cache-test.toml"),
        };
        let state = cache.load().unwrap();
        assert!(state.repos.is_empty());
    }

    #[test]
    fn test_watch_state_deserialize_empty() {
        // After fixing with #[serde(default)], empty string should work
        let state: WatchState = toml::from_str("").unwrap();
        assert!(state.repos.is_empty());
    }

    #[test]
    fn test_watch_state_deserialize_no_repos_key() {
        // A file with other content but no repos should work
        let state: WatchState = toml::from_str("# just a comment\n").unwrap();
        assert!(state.repos.is_empty());
    }
}
