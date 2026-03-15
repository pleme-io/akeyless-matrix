use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::git::GitOps;
use crate::provider::GitHubProvider;
use crate::watch_cache::{RepoState, WatchCache};

// ---------------------------------------------------------------------------
// Configuration types
// ---------------------------------------------------------------------------

/// Clone method for workspace repos.
#[derive(Debug, Clone)]
pub enum CloneMethod {
    Ssh,
    Https,
}

/// Watch-specific configuration for a workspace.
#[derive(Debug, Clone)]
pub struct WatchConfig {
    pub enable: bool,
    pub matrix_file: Option<String>,
    pub auto_commit: bool,
}

/// A workspace definition (subset of tend's full Workspace).
#[derive(Debug, Clone)]
pub struct Workspace {
    pub name: String,
    pub provider: String,
    pub base_dir: String,
    pub clone_method: CloneMethod,
    pub discover: bool,
    pub org: Option<String>,
    pub exclude: Vec<String>,
    pub extra_repos: Vec<String>,
    pub flake_deps: std::collections::HashMap<String, String>,
    pub watch: Option<WatchConfig>,
}

// ---------------------------------------------------------------------------
// Matrix appender
// ---------------------------------------------------------------------------

/// Trait abstracting matrix file appending for testability.
pub trait MatrixAppender: Send + Sync {
    fn append_entry(
        &self,
        matrix_file: &Path,
        repo_name: &str,
        version: &str,
        rev: &str,
        language: Option<&str>,
    ) -> Result<bool>;
}

/// Real implementation that uses `append_matrix_entry`.
pub struct TomlMatrixAppender;

impl MatrixAppender for TomlMatrixAppender {
    fn append_entry(
        &self,
        matrix_file: &Path,
        repo_name: &str,
        version: &str,
        rev: &str,
        language: Option<&str>,
    ) -> Result<bool> {
        append_matrix_entry(matrix_file, repo_name, version, rev, language)
    }
}

/// Append a new pending version entry to a matrix TOML file.
///
/// Searches through all `[packages.*]` entries for one whose `repo` field
/// matches `repo_name`. If found and the version does not already exist,
/// creates a new `[packages.*.versions."<version>"]` table with `rev`,
/// `status = "pending"`, and optionally `language`.
///
/// Returns `true` if an entry was appended, `false` if the repo was not
/// found or the version already existed.
pub fn append_matrix_entry(
    matrix_file: &Path,
    repo_name: &str,
    version: &str,
    rev: &str,
    language: Option<&str>,
) -> Result<bool> {
    let content = std::fs::read_to_string(matrix_file)
        .with_context(|| format!("reading {}", matrix_file.display()))?;

    let mut doc: toml_edit::DocumentMut = content
        .parse()
        .with_context(|| format!("parsing {}", matrix_file.display()))?;

    let Some(packages) = doc.get_mut("packages").and_then(|p| p.as_table_mut()) else {
        return Ok(false);
    };

    // Find the package whose `repo` field matches
    let mut pkg_key = None;
    for (key, item) in packages.iter() {
        if let Some(table) = item.as_table() {
            if table.get("repo").and_then(|v| v.as_str()) == Some(repo_name) {
                pkg_key = Some(key.to_string());
                break;
            }
        }
    }

    let Some(key) = pkg_key else {
        return Ok(false);
    };

    let pkg_table = packages[&key]
        .as_table_mut()
        .context("package entry is not a table")?;

    // Ensure versions sub-table exists
    if pkg_table.get("versions").is_none() {
        pkg_table.insert("versions", toml_edit::Item::Table(toml_edit::Table::new()));
    }

    let versions = pkg_table["versions"]
        .as_table_mut()
        .context("versions is not a table")?;

    // Skip if version already exists
    if versions.contains_key(version) {
        return Ok(false);
    }

    // Create the version entry
    let mut version_table = toml_edit::Table::new();
    version_table.insert("rev", toml_edit::value(rev));
    version_table.insert("status", toml_edit::value("pending"));
    if let Some(lang) = language {
        version_table.insert("language", toml_edit::value(lang));
    }

    versions.insert(version, toml_edit::Item::Table(version_table));

    std::fs::write(matrix_file, doc.to_string())
        .with_context(|| format!("writing {}", matrix_file.display()))?;

    Ok(true)
}

// ---------------------------------------------------------------------------
// Watch cycle
// ---------------------------------------------------------------------------

/// Summary of a single watch cycle.
#[derive(Debug, Default)]
pub struct WatchCycleSummary {
    pub checked: u32,
    pub new_versions: u32,
    pub errors: u32,
}

/// Run a single watch cycle: check each repo for new tags, append to matrix.
pub async fn run_watch_cycle(
    workspace: &Workspace,
    _dry_run: bool,
    github: &dyn GitHubProvider,
    cache: &dyn WatchCache,
    matrix_appender: &dyn MatrixAppender,
    git_ops: &dyn GitOps,
) -> Result<WatchCycleSummary> {
    let org = workspace
        .org
        .as_deref()
        .unwrap_or("unknown");

    let watch_config = workspace
        .watch
        .as_ref()
        .context("workspace has no watch config")?;

    let matrix_file_str = watch_config
        .matrix_file
        .as_deref()
        .context("no matrix_file configured")?;
    let matrix_file = PathBuf::from(matrix_file_str);

    let mut state = cache.load().unwrap_or_default();
    let mut summary = WatchCycleSummary::default();

    let repos = &workspace.extra_repos;

    for repo_name in repos {
        summary.checked += 1;

        // 1. Get HEAD commit SHA
        let head = match github.get_head(org, repo_name).await {
            Ok(h) => h,
            Err(e) => {
                eprintln!("  [warn] failed to get HEAD for {repo_name}: {e}");
                summary.errors += 1;
                continue;
            }
        };

        // 2. Get latest tag
        let latest_tag = match github.get_latest_tag(org, repo_name).await {
            Ok(t) => t,
            Err(e) => {
                eprintln!("  [warn] failed to get tags for {repo_name}: {e}");
                summary.errors += 1;
                continue;
            }
        };

        // 3. Detect language (reuse cached if HEAD unchanged)
        let cached = state.repos.get(repo_name);
        let language = if cached.map(|c| c.head.as_str()) == Some(head.as_str()) {
            // HEAD unchanged, reuse cached language
            cached.and_then(|c| c.language.clone())
        } else {
            // HEAD changed or not cached, detect language
            match github.get_language(org, repo_name).await {
                Ok(Some(lang)) => {
                    Some(crate::provider::normalize_language(&lang).to_string())
                }
                Ok(None) => None,
                Err(_) => cached.and_then(|c| c.language.clone()),
            }
        };

        // 4. Check if we have a new version
        let cached_tag = cached.and_then(|c| c.latest_tag.as_deref());
        let new_tag = latest_tag.as_deref();

        if let Some(version) = new_tag {
            let is_new = cached_tag != Some(version);
            if is_new {
                // Strip leading 'v' for version string
                let clean_version = version.strip_prefix('v').unwrap_or(version);
                match matrix_appender.append_entry(
                    &matrix_file,
                    repo_name,
                    clean_version,
                    &head,
                    language.as_deref(),
                ) {
                    Ok(true) => {
                        summary.new_versions += 1;
                        println!("  [new] {repo_name} {version}");
                    }
                    Ok(false) => {
                        // Already existed or repo not in matrix
                    }
                    Err(e) => {
                        eprintln!("  [warn] failed to append {repo_name} {version}: {e}");
                        summary.errors += 1;
                    }
                }
            }
        }

        // 5. Update cache
        state.repos.insert(
            repo_name.clone(),
            RepoState {
                head,
                latest_tag,
                language,
            },
        );
    }

    // 6. Save cache
    if let Err(e) = cache.save(&state) {
        eprintln!("  [warn] failed to save watch cache: {e}");
    }

    // 7. Auto-commit if configured and we found new versions
    if watch_config.auto_commit && summary.new_versions > 0 {
        let repo_dir = PathBuf::from(&workspace.base_dir);

        if let Err(e) = git_ops.add(&repo_dir, &matrix_file) {
            eprintln!("  [warn] git add failed: {e}");
        } else {
            match git_ops.has_staged_changes(&repo_dir) {
                Ok(true) => {
                    let msg = format!(
                        "chore: update matrix with {} new version(s)",
                        summary.new_versions
                    );
                    if let Err(e) = git_ops.commit(&repo_dir, &msg) {
                        eprintln!("  [warn] git commit failed: {e}");
                    } else if let Err(e) = git_ops.push(&repo_dir) {
                        eprintln!("  [warn] git push failed: {e}");
                    }
                }
                Ok(false) => {
                    // No staged changes, nothing to commit
                }
                Err(e) => {
                    eprintln!("  [warn] git has_staged_changes failed: {e}");
                }
            }
        }
    }

    Ok(summary)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::watch_cache::WatchState;
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    // -- Mock GitHub Provider --

    struct MockGitHub {
        heads: BTreeMap<String, String>,
        tags: BTreeMap<String, Option<String>>,
        languages: BTreeMap<String, String>,
    }

    #[async_trait::async_trait]
    impl GitHubProvider for MockGitHub {
        async fn get_head(&self, _org: &str, repo: &str) -> Result<String> {
            self.heads
                .get(repo)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("no head for {repo}"))
        }

        async fn get_latest_tag(&self, _org: &str, repo: &str) -> Result<Option<String>> {
            self.tags
                .get(repo)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("no tags for {repo}"))
        }

        async fn get_language(&self, _org: &str, repo: &str) -> Result<Option<String>> {
            Ok(self.languages.get(repo).cloned())
        }
    }

    // -- Mock Watch Cache --

    struct MockCache {
        state: Mutex<WatchState>,
    }

    impl WatchCache for MockCache {
        fn load(&self) -> Result<WatchState> {
            Ok(self.state.lock().unwrap().clone())
        }

        fn save(&self, state: &WatchState) -> Result<()> {
            *self.state.lock().unwrap() = state.clone();
            Ok(())
        }
    }

    // -- Mock Matrix Appender --

    struct MockAppender {
        appended: Mutex<Vec<(String, String, String)>>, // (repo, version, rev)
    }

    impl MatrixAppender for MockAppender {
        fn append_entry(
            &self,
            _matrix_file: &Path,
            repo_name: &str,
            version: &str,
            rev: &str,
            _language: Option<&str>,
        ) -> Result<bool> {
            self.appended.lock().unwrap().push((
                repo_name.to_string(),
                version.to_string(),
                rev.to_string(),
            ));
            Ok(true)
        }
    }

    // -- Mock GitOps (no-op) --

    struct MockGitOps;

    impl GitOps for MockGitOps {
        fn add(&self, _repo_dir: &Path, _file_path: &Path) -> Result<()> {
            Ok(())
        }
        fn has_staged_changes(&self, _repo_dir: &Path) -> Result<bool> {
            Ok(false)
        }
        fn commit(&self, _repo_dir: &Path, _message: &str) -> Result<()> {
            Ok(())
        }
        fn push(&self, _repo_dir: &Path) -> Result<()> {
            Ok(())
        }
    }

    // -- Recording GitOps (tracks calls) --

    struct RecordingGitOps {
        calls: Mutex<Vec<String>>,
    }

    impl RecordingGitOps {
        fn new() -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl GitOps for RecordingGitOps {
        fn add(&self, _repo_dir: &Path, _file_path: &Path) -> Result<()> {
            self.calls.lock().unwrap().push("add".into());
            Ok(())
        }
        fn has_staged_changes(&self, _repo_dir: &Path) -> Result<bool> {
            self.calls
                .lock()
                .unwrap()
                .push("has_staged_changes".into());
            Ok(true) // pretend there are changes
        }
        fn commit(&self, _repo_dir: &Path, _message: &str) -> Result<()> {
            self.calls.lock().unwrap().push("commit".into());
            Ok(())
        }
        fn push(&self, _repo_dir: &Path) -> Result<()> {
            self.calls.lock().unwrap().push("push".into());
            Ok(())
        }
    }

    // -- Helper to build a test workspace --

    fn test_workspace(repos: Vec<String>, auto_commit: bool) -> Workspace {
        Workspace {
            name: "test-ws".to_string(),
            provider: "github".to_string(),
            base_dir: "/tmp/test".to_string(),
            clone_method: CloneMethod::Ssh,
            discover: false,
            org: Some("testorg".to_string()),
            exclude: vec![],
            extra_repos: repos,
            flake_deps: std::collections::HashMap::new(),
            watch: Some(WatchConfig {
                enable: true,
                matrix_file: Some("/tmp/test-matrix.toml".to_string()),
                auto_commit,
            }),
        }
    }

    // -----------------------------------------------------------------------
    // Tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_watch_cycle_detects_new_version() {
        let ws = test_workspace(vec!["test-repo".to_string()], false);

        let mut heads = BTreeMap::new();
        heads.insert("test-repo".to_string(), "abc123".to_string());
        let mut tags = BTreeMap::new();
        tags.insert("test-repo".to_string(), Some("v1.0.0".to_string()));
        let github = MockGitHub {
            heads,
            tags,
            languages: BTreeMap::new(),
        };

        let cache = MockCache {
            state: Mutex::new(WatchState::default()),
        };
        let appender = MockAppender {
            appended: Mutex::new(Vec::new()),
        };
        let git_ops = MockGitOps;

        let summary = run_watch_cycle(&ws, true, &github, &cache, &appender, &git_ops)
            .await
            .unwrap();

        assert_eq!(summary.checked, 1);
        assert_eq!(summary.new_versions, 1);
        assert_eq!(summary.errors, 0);

        let appended = appender.appended.lock().unwrap();
        assert_eq!(appended.len(), 1);
        assert_eq!(appended[0].0, "test-repo");
        assert_eq!(appended[0].1, "1.0.0"); // v stripped
        assert_eq!(appended[0].2, "abc123"); // rev passed through
    }

    #[tokio::test]
    async fn test_watch_cycle_skips_unchanged() {
        let ws = test_workspace(vec!["test-repo".to_string()], false);

        let mut heads = BTreeMap::new();
        heads.insert("test-repo".to_string(), "abc123".to_string());
        let mut tags = BTreeMap::new();
        tags.insert("test-repo".to_string(), Some("v1.0.0".to_string()));
        let github = MockGitHub {
            heads,
            tags,
            languages: BTreeMap::new(),
        };

        // Cache already has this tag
        let mut cache_state = WatchState::default();
        cache_state.repos.insert(
            "test-repo".to_string(),
            RepoState {
                head: "abc123".to_string(),
                latest_tag: Some("v1.0.0".to_string()),
                language: None,
            },
        );
        let cache = MockCache {
            state: Mutex::new(cache_state),
        };
        let appender = MockAppender {
            appended: Mutex::new(Vec::new()),
        };
        let git_ops = MockGitOps;

        let summary = run_watch_cycle(&ws, true, &github, &cache, &appender, &git_ops)
            .await
            .unwrap();

        assert_eq!(summary.checked, 1);
        assert_eq!(summary.new_versions, 0);

        let appended = appender.appended.lock().unwrap();
        assert_eq!(appended.len(), 0);
    }

    #[tokio::test]
    async fn test_watch_cycle_auto_commit() {
        let ws = test_workspace(vec!["test-repo".to_string()], true);

        // GitHub returns a new tag vs cached
        let mut heads = BTreeMap::new();
        heads.insert("test-repo".to_string(), "newhead123".to_string());
        let mut tags = BTreeMap::new();
        tags.insert("test-repo".to_string(), Some("v2.0.0".to_string()));
        let github = MockGitHub {
            heads,
            tags,
            languages: BTreeMap::new(),
        };

        // Cache has an OLD tag
        let mut cache_state = WatchState::default();
        cache_state.repos.insert(
            "test-repo".to_string(),
            RepoState {
                head: "oldhead".to_string(),
                latest_tag: Some("v1.0.0".to_string()),
                language: Some("go".to_string()),
            },
        );
        let cache = MockCache {
            state: Mutex::new(cache_state),
        };

        let appender = MockAppender {
            appended: Mutex::new(Vec::new()),
        };

        let git_ops = RecordingGitOps::new();

        let summary = run_watch_cycle(&ws, true, &github, &cache, &appender, &git_ops)
            .await
            .unwrap();

        assert_eq!(summary.new_versions, 1);

        // Verify git operations were called (add, has_staged_changes, commit, push)
        let calls = git_ops.calls();
        assert!(calls.contains(&"add".to_string()));
        assert!(calls.contains(&"has_staged_changes".to_string()));
        assert!(calls.contains(&"commit".to_string()));
        assert!(calls.contains(&"push".to_string()));
    }

    #[tokio::test]
    async fn test_watch_cycle_handles_api_errors() {
        let ws = test_workspace(vec!["failing-repo".to_string()], false);

        // GitHub returns an error for this repo (heads map is empty)
        let github = MockGitHub {
            heads: BTreeMap::new(),
            tags: BTreeMap::new(),
            languages: BTreeMap::new(),
        };

        let cache = MockCache {
            state: Mutex::new(WatchState::default()),
        };

        let appender = MockAppender {
            appended: Mutex::new(Vec::new()),
        };

        let git_ops = MockGitOps;

        let summary = run_watch_cycle(&ws, true, &github, &cache, &appender, &git_ops)
            .await
            .unwrap();

        // Should count 1 checked, 0 new versions, 1 error
        assert_eq!(summary.checked, 1);
        assert_eq!(summary.new_versions, 0);
        assert_eq!(summary.errors, 1);
    }

    #[tokio::test]
    async fn test_watch_cycle_reuses_cached_language() {
        let ws = test_workspace(vec!["cached-repo".to_string()], false);

        // HEAD is SAME as cached -- language should be reused
        let mut heads = BTreeMap::new();
        heads.insert("cached-repo".to_string(), "sameHEAD".to_string());
        let mut tags = BTreeMap::new();
        tags.insert("cached-repo".to_string(), Some("v2.0.0".to_string()));
        // languages map is EMPTY -- if it tries to detect, it would get None
        let github = MockGitHub {
            heads,
            tags,
            languages: BTreeMap::new(),
        };

        let mut cache_state = WatchState::default();
        cache_state.repos.insert(
            "cached-repo".to_string(),
            RepoState {
                head: "sameHEAD".to_string(), // same HEAD
                latest_tag: Some("v1.0.0".to_string()), // OLD tag
                language: Some("rust".to_string()), // cached language
            },
        );
        let cache = MockCache {
            state: Mutex::new(cache_state),
        };

        let appender = MockAppender {
            appended: Mutex::new(Vec::new()),
        };

        let git_ops = MockGitOps;

        let summary = run_watch_cycle(&ws, true, &github, &cache, &appender, &git_ops)
            .await
            .unwrap();

        assert_eq!(summary.new_versions, 1);

        // Verify the appended entry exists
        let appended = appender.appended.lock().unwrap();
        assert_eq!(appended.len(), 1);

        // Verify cached language was preserved in the cache state
        let saved_state = cache.state.lock().unwrap();
        let repo_state = &saved_state.repos["cached-repo"];
        assert_eq!(repo_state.language.as_deref(), Some("rust"));
    }

    #[test]
    fn test_append_matrix_entry_creates_entry() {
        let dir = std::env::temp_dir().join("tend-test-append");
        std::fs::create_dir_all(&dir).unwrap();
        let matrix_file = dir.join("matrix.toml");

        // Create a minimal matrix.toml
        std::fs::write(
            &matrix_file,
            r#"
[packages.akeyless-test]
owner = "testorg"
repo = "test-repo"
language = "go"
builder = "mkGoTool"
tier = 1
description = "test"
homepage = "https://example.com"
"#,
        )
        .unwrap();

        let result =
            append_matrix_entry(&matrix_file, "test-repo", "1.0.0", "abc123def", Some("go"))
                .unwrap();
        assert!(result);

        // Read back and verify
        let content = std::fs::read_to_string(&matrix_file).unwrap();
        assert!(content.contains("[packages.akeyless-test.versions.\"1.0.0\"]"));
        assert!(content.contains("status = \"pending\""));
        assert!(content.contains("rev = \"abc123def\""));

        // Clean up
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_append_matrix_entry_skips_unknown_repo() {
        let dir = std::env::temp_dir().join("tend-test-append-unknown");
        std::fs::create_dir_all(&dir).unwrap();
        let matrix_file = dir.join("matrix.toml");

        std::fs::write(
            &matrix_file,
            r#"
[packages.akeyless-test]
owner = "testorg"
repo = "test-repo"
language = "go"
builder = "mkGoTool"
tier = 1
description = "test"
homepage = "https://example.com"
"#,
        )
        .unwrap();

        let result =
            append_matrix_entry(&matrix_file, "unknown-repo", "1.0.0", "abc", None).unwrap();
        assert!(!result); // false = not found

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_append_matrix_entry_skips_duplicate_version() {
        let dir = std::env::temp_dir().join("tend-test-append-dup");
        std::fs::create_dir_all(&dir).unwrap();
        let matrix_file = dir.join("matrix.toml");

        std::fs::write(
            &matrix_file,
            r#"
[packages.akeyless-test]
owner = "testorg"
repo = "test-repo"
language = "go"
builder = "mkGoTool"
tier = 1
description = "test"
homepage = "https://example.com"

[packages.akeyless-test.versions."1.0.0"]
rev = "existing"
status = "verified"
"#,
        )
        .unwrap();

        let result =
            append_matrix_entry(&matrix_file, "test-repo", "1.0.0", "new-rev", None).unwrap();
        assert!(!result); // false = already exists

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
