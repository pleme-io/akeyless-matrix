use std::path::Path;

use anyhow::Result;

/// Trait abstracting git operations for testability.
pub trait GitOps: Send + Sync {
    /// Stage a file for commit.
    fn add(&self, repo_dir: &Path, file_path: &Path) -> Result<()>;

    /// Check if there are staged changes ready to commit.
    fn has_staged_changes(&self, repo_dir: &Path) -> Result<bool>;

    /// Create a commit with the given message.
    fn commit(&self, repo_dir: &Path, message: &str) -> Result<()>;

    /// Push the current branch to its remote.
    fn push(&self, repo_dir: &Path) -> Result<()>;
}

/// Real implementation backed by `git` CLI commands.
pub struct SystemGitOps;

impl GitOps for SystemGitOps {
    fn add(&self, repo_dir: &Path, file_path: &Path) -> Result<()> {
        let output = std::process::Command::new("git")
            .args(["add", &file_path.display().to_string()])
            .current_dir(repo_dir)
            .output()?;
        if !output.status.success() {
            anyhow::bail!(
                "git add failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Ok(())
    }

    fn has_staged_changes(&self, repo_dir: &Path) -> Result<bool> {
        let output = std::process::Command::new("git")
            .args(["diff", "--cached", "--quiet"])
            .current_dir(repo_dir)
            .output()?;
        // Exit code 1 means there are staged changes
        Ok(!output.status.success())
    }

    fn commit(&self, repo_dir: &Path, message: &str) -> Result<()> {
        let output = std::process::Command::new("git")
            .args(["commit", "-m", message])
            .current_dir(repo_dir)
            .output()?;
        if !output.status.success() {
            anyhow::bail!(
                "git commit failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Ok(())
    }

    fn push(&self, repo_dir: &Path) -> Result<()> {
        let output = std::process::Command::new("git")
            .args(["push"])
            .current_dir(repo_dir)
            .output()?;
        if !output.status.success() {
            anyhow::bail!(
                "git push failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Ok(())
    }
}
