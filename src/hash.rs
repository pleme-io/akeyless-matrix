use anyhow::{Context, Result, bail};
use regex::Regex;

use crate::runner::CommandRunner;

/// Dummy hash used to trigger a hash mismatch so Nix prints the real hash.
pub const DUMMY_HASH: &str = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";

/// Run `nix-prefetch-github` to fetch the source hash for a GitHub repo.
pub async fn prefetch_github(
    runner: &dyn CommandRunner,
    owner: &str,
    repo: &str,
    rev: &str,
) -> Result<String> {
    let output = runner
        .run("nix-prefetch-github", &[owner, repo, "--rev", rev, "--nix"])
        .await
        .context("running nix-prefetch-github")?;

    if !output.success {
        bail!(
            "nix-prefetch-github failed for {owner}/{repo}@{rev}: {}",
            output.stderr
        );
    }

    extract_hash_from_prefetch(&output.stdout)
        .with_context(|| format!("extracting hash from nix-prefetch-github output for {owner}/{repo}@{rev}"))
}

/// Extract a `sha256-...` hash from `nix-prefetch-github` Nix expression output.
fn extract_hash_from_prefetch(output: &str) -> Option<String> {
    let re = Regex::new(r#"(?:hash|sha256)\s*=\s*"(sha256-[A-Za-z0-9+/=]+)""#).ok()?;
    re.captures(output)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
}

/// Extract the real hash from Nix build stderr when a hash mismatch occurs.
/// Nix prints something like: `got:    sha256-RealHashHere=`
pub fn extract_hash_from_stderr(stderr: &str) -> Option<String> {
    let re = Regex::new(r"got:\s+(sha256-[A-Za-z0-9+/=]+)").ok()?;
    re.captures(stderr)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
}

/// Prefetch a URL and return its SRI hash.
pub async fn prefetch_url(runner: &dyn CommandRunner, url: &str) -> Result<String> {
    let output = runner
        .run("nix-prefetch-url", &[url, "--type", "sha256"])
        .await
        .context("running nix-prefetch-url")?;

    if !output.success {
        bail!("nix-prefetch-url failed for {url}: {}", output.stderr);
    }

    // nix-prefetch-url outputs the hash on stdout, convert to SRI
    let nix32_hash = output.stdout.trim().to_string();
    let sri_output = runner
        .run("nix", &["hash", "to-sri", "--type", "sha256", &nix32_hash])
        .await
        .context("converting hash to SRI")?;

    if !sri_output.success {
        bail!("nix hash to-sri failed: {}", sri_output.stderr);
    }

    Ok(sri_output.stdout.trim().to_string())
}

/// Run a Nix build expression and return (success, stdout, stderr).
pub async fn nix_build_expr(
    runner: &dyn CommandRunner,
    expr: &str,
) -> Result<(bool, String, String)> {
    let output = runner
        .run("nix", &["build", "--no-link", "--impure", "--expr", expr])
        .await
        .context("running nix build")?;

    Ok((output.success, output.stdout, output.stderr))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_hash_from_stderr() {
        let stderr = r#"
error: hash mismatch in fixed-output derivation '/nix/store/xxx':
         specified: sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=
            got:    sha256-wE5GPDNe5p0WrgoO3TupIHDTH+HpyfD5vuzZsuwB80o=
"#;
        let hash = extract_hash_from_stderr(stderr);
        assert_eq!(
            hash.as_deref(),
            Some("sha256-wE5GPDNe5p0WrgoO3TupIHDTH+HpyfD5vuzZsuwB80o=")
        );
    }

    #[test]
    fn test_extract_hash_from_prefetch() {
        let output = r#"{ owner = "akeylesslabs";
  repo = "cli";
  rev = "731e5bd";
  hash = "sha256-e6JhyI7E1OYHHOx1Z6fSjxaMocsHmzSeBs+iOZ3dyCE=";
}"#;
        let hash = extract_hash_from_prefetch(output);
        assert_eq!(
            hash.as_deref(),
            Some("sha256-e6JhyI7E1OYHHOx1Z6fSjxaMocsHmzSeBs+iOZ3dyCE=")
        );
    }
}
