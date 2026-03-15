use std::collections::BTreeMap;

use anyhow::Result;
use async_trait::async_trait;

/// Trait abstracting GitHub API interactions for testability.
#[async_trait]
pub trait GitHubProvider: Send + Sync {
    /// Get the HEAD commit SHA for a repo.
    async fn get_head(&self, org: &str, repo: &str) -> Result<String>;

    /// Get the latest tag for a repo (if any).
    async fn get_latest_tag(&self, org: &str, repo: &str) -> Result<Option<String>>;

    /// Detect the primary language of a repo.
    async fn get_language(&self, org: &str, repo: &str) -> Result<Option<String>>;
}

/// Normalize a GitHub language string to our convention.
#[must_use]
pub fn normalize_language(lang: &str) -> &str {
    match lang {
        "Go" => "go",
        "Rust" => "rust",
        "Python" => "python",
        "TypeScript" | "JavaScript" => "typescript",
        "Java" => "java",
        "HCL" => "go", // Terraform providers are Go
        "C#" | "C Sharp" => "csharp",
        _ => {
            // Return the input lowercased -- but since we return &str we
            // can only do this for known static cases.  For truly unknown
            // languages, leak a lowercased copy (rare path).
            // In practice this is fine because the set of GitHub languages
            // is bounded and we only call this once per repo per cycle.
            Box::leak(lang.to_lowercase().into_boxed_str())
        }
    }
}

/// Real implementation using the GitHub REST API.
pub struct GitHubApi {
    pub token: Option<String>,
}

#[async_trait]
impl GitHubProvider for GitHubApi {
    async fn get_head(&self, org: &str, repo: &str) -> Result<String> {
        let url = format!("https://api.github.com/repos/{org}/{repo}/commits?per_page=1");
        let client = reqwest::Client::new();
        let mut req = client.get(&url).header("User-Agent", "akeyless-matrix");
        if let Some(ref token) = self.token {
            req = req.header("Authorization", format!("token {token}"));
        }
        let resp = req.send().await?;
        let body: serde_json::Value = resp.json().await?;
        let sha = body
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|c| c["sha"].as_str())
            .ok_or_else(|| anyhow::anyhow!("no commits found for {org}/{repo}"))?;
        Ok(sha.to_string())
    }

    async fn get_latest_tag(&self, org: &str, repo: &str) -> Result<Option<String>> {
        let url = format!("https://api.github.com/repos/{org}/{repo}/tags?per_page=1");
        let client = reqwest::Client::new();
        let mut req = client.get(&url).header("User-Agent", "akeyless-matrix");
        if let Some(ref token) = self.token {
            req = req.header("Authorization", format!("token {token}"));
        }
        let resp = req.send().await?;
        let body: serde_json::Value = resp.json().await?;
        let tag = body
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|t| t["name"].as_str())
            .map(String::from);
        Ok(tag)
    }

    async fn get_language(&self, org: &str, repo: &str) -> Result<Option<String>> {
        let url = format!("https://api.github.com/repos/{org}/{repo}/languages");
        let client = reqwest::Client::new();
        let mut req = client.get(&url).header("User-Agent", "akeyless-matrix");
        if let Some(ref token) = self.token {
            req = req.header("Authorization", format!("token {token}"));
        }
        let resp = req.send().await?;
        let body: BTreeMap<String, u64> = resp.json().await?;
        // Return the language with the most bytes
        let lang = body.into_iter().max_by_key(|(_, v)| *v).map(|(k, _)| k);
        Ok(lang)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_language() {
        assert_eq!(normalize_language("Go"), "go");
        assert_eq!(normalize_language("Rust"), "rust");
        assert_eq!(normalize_language("Python"), "python");
        assert_eq!(normalize_language("TypeScript"), "typescript");
        assert_eq!(normalize_language("JavaScript"), "typescript");
        assert_eq!(normalize_language("Java"), "java");
        assert_eq!(normalize_language("HCL"), "go");
        assert_eq!(normalize_language("C#"), "csharp");
    }

    #[test]
    fn test_normalize_language_edge_cases() {
        // Default case - unknown languages get lowercased
        assert_eq!(normalize_language("Fortran"), "fortran");
        assert_eq!(normalize_language("COBOL"), "cobol");
        assert_eq!(normalize_language(""), "");
    }
}
