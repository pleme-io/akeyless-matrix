use anyhow::{Context, Result};
use async_trait::async_trait;

/// Output from a command execution.
pub struct CommandOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

/// Trait abstracting external command execution for testability.
///
/// Production code uses `SystemRunner`, which shells out to real processes.
/// Tests can substitute a mock that returns predetermined output.
#[async_trait]
pub trait CommandRunner: Send + Sync {
    /// Execute a command and return its output.
    async fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput>;
}

/// Real implementation that executes system processes.
pub struct SystemRunner;

#[async_trait]
impl CommandRunner for SystemRunner {
    async fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput> {
        let output = tokio::process::Command::new(program)
            .args(args)
            .output()
            .await
            .with_context(|| format!("running {program}"))?;

        Ok(CommandOutput {
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
}
