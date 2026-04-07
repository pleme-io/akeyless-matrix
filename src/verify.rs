use std::path::Path;

use anyhow::Result;
use chrono::Utc;

use crate::display;
use crate::hash;
use crate::matrix::{Builder, Language, Status};
use crate::nixexpr;
use crate::runner::CommandRunner;
use crate::storage::MatrixStore;

/// Run the `verify` subcommand: build ALL entries (not just pending) to validate
/// the full matrix.
pub async fn run(
    matrix_path: &Path,
    runner: &dyn CommandRunner,
    store: &dyn MatrixStore,
) -> Result<()> {
    let mut matrix = store.load(matrix_path)?;
    display::print_header("Verifying all matrix entries");

    let mut verified = 0u32;
    let mut failed = 0u32;
    let mut skipped = 0u32;

    let pkg_names: Vec<String> = matrix.packages.keys().cloned().collect();

    for pkg_name in &pkg_names {
        let pkg = matrix.packages.get(pkg_name).unwrap().clone();

        // Only verify packages with a source-built builder
        if matches!(
            pkg.builder,
            Builder::None
                | Builder::Fetchurl
                | Builder::MkJavaMavenPackage
                | Builder::MkDotnetPackage
                | Builder::MkTerraformModuleCheck
        ) {
            skipped += 1;
            continue;
        }

        let ver_keys: Vec<String> = pkg.versions.keys().cloned().collect();

        for ver_key in &ver_keys {
            let entry = pkg.versions.get(ver_key).unwrap().clone();
            display::print_build_start(pkg_name, ver_key);

            let result = verify_entry(&pkg, &entry, runner).await;

            let pkg_mut = matrix.packages.get_mut(pkg_name).unwrap();
            let entry_mut = pkg_mut.versions.get_mut(ver_key).unwrap();

            match result {
                Ok(()) => {
                    entry_mut.status = Status::Verified;
                    entry_mut.verified_at = Some(Utc::now());
                    display::print_build_success(pkg_name, ver_key);
                    verified += 1;
                }
                Err(e) => {
                    entry_mut.status = Status::Broken;
                    display::print_build_failure(pkg_name, ver_key, &e.to_string());
                    failed += 1;
                }
            }

            store.save(matrix_path, &matrix)?;
        }
    }

    println!();
    println!(
        "  done: {verified} verified, {failed} failed, {skipped} skipped"
    );

    Ok(())
}

/// Verify a single entry by running the actual nix build with the stored hashes.
async fn verify_entry(
    pkg: &crate::matrix::Package,
    entry: &crate::matrix::VersionEntry,
    runner: &dyn CommandRunner,
) -> Result<()> {
    let source_hash = entry
        .source_hash
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("missing source_hash"))?;

    let expr = match pkg.language {
        Language::Go => {
            let vendor_hash = entry
                .vendor_hash
                .as_deref()
                .map_or("null".to_string(), |h| format!("\"{h}\""));
            nixexpr::go_expr(pkg, &entry.rev, source_hash, &vendor_hash, true)
        }
        Language::Rust => {
            let cargo_hash = entry
                .cargo_hash
                .as_deref()
                .unwrap_or(hash::DUMMY_HASH);
            nixexpr::rust_expr(pkg, &entry.rev, source_hash, cargo_hash, true)
        }
        Language::TypeScript => {
            let npm_hash = entry
                .npm_deps_hash
                .as_deref()
                .unwrap_or(hash::DUMMY_HASH);
            nixexpr::typescript_expr(pkg, &entry.rev, source_hash, npm_hash, true)
        }
        Language::Python => nixexpr::python_expr(pkg, &entry.rev, source_hash, true),
        Language::Java | Language::Ruby | Language::Php | Language::Csharp | Language::Helm => {
            return Ok(()); // source-only or externally built, nothing to verify
        }
    };

    let (success, _stdout, stderr) = hash::nix_build_expr(runner, &expr).await?;

    if success {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "nix build failed: {}",
            first_error_line(&stderr)
        ))
    }
}

/// Extract the first meaningful error line from stderr.
fn first_error_line(stderr: &str) -> String {
    stderr
        .lines()
        .find(|l| l.contains("error") || l.contains("Error"))
        .unwrap_or("unknown error")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_first_error_line_finds_error() {
        let stderr = "some output\nerror: hash mismatch\nmore output";
        assert_eq!(first_error_line(stderr), "error: hash mismatch");
    }

    #[test]
    fn test_first_error_line_returns_unknown() {
        let stderr = "no problems here\njust warnings";
        assert_eq!(first_error_line(stderr), "unknown error");
    }

    #[test]
    fn test_first_error_line_finds_capitalized_error() {
        let stderr = "building...\nError: compilation failed\nmore output";
        assert_eq!(first_error_line(stderr), "Error: compilation failed");
    }

    #[test]
    fn test_first_error_line_trims_whitespace() {
        let stderr = "  error: indented error message  \n";
        assert_eq!(first_error_line(stderr), "error: indented error message");
    }

    #[test]
    fn test_first_error_line_empty_input() {
        assert_eq!(first_error_line(""), "unknown error");
    }

    use crate::matrix::{Builder, Language, Matrix, Package, VersionEntry};
    use crate::runner::CommandOutput;
    use crate::storage::MatrixStore;
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    struct MockRunner {
        responses: Mutex<Vec<CommandOutput>>,
    }

    #[async_trait::async_trait]
    impl crate::runner::CommandRunner for MockRunner {
        async fn run(&self, _program: &str, _args: &[&str]) -> anyhow::Result<CommandOutput> {
            let mut responses = self.responses.lock().unwrap();
            if responses.is_empty() {
                anyhow::bail!("no more mock responses");
            }
            Ok(responses.remove(0))
        }
    }

    struct InMemoryStore {
        matrix: Mutex<Matrix>,
    }

    impl MatrixStore for InMemoryStore {
        fn load(&self, _path: &std::path::Path) -> anyhow::Result<Matrix> {
            Ok(self.matrix.lock().unwrap().clone())
        }
        fn save(&self, _path: &std::path::Path, matrix: &Matrix) -> anyhow::Result<()> {
            *self.matrix.lock().unwrap() = matrix.clone();
            Ok(())
        }
    }

    fn make_verified_entry(
        source_hash: &str,
        vendor_hash: Option<&str>,
    ) -> VersionEntry {
        VersionEntry {
            rev: "abc123".into(),
            source_hash: Some(source_hash.into()),
            vendor_hash: vendor_hash.map(|s| s.into()),
            cargo_hash: None,
            npm_deps_hash: None,
            maven_hash: None,
            nuget_deps_hash: None,
            status: Status::Verified,
            verified_at: None,
            hash_aarch64_darwin: None,
            hash_x86_64_darwin: None,
            hash_x86_64_linux: None,
            hash_aarch64_linux: None,
        }
    }

    #[tokio::test]
    async fn test_verify_go_package_success() {
        let mut versions = BTreeMap::new();
        versions.insert(
            "1.0.0".to_string(),
            make_verified_entry("sha256-src", Some("sha256-vendor")),
        );
        let mut packages = BTreeMap::new();
        packages.insert(
            "akeyless-test".to_string(),
            Package {
                owner: "testorg".into(),
                repo: "test".into(),
                language: Language::Go,
                builder: Builder::MkGoTool,
                tier: 1,
                sub_packages: None,
                proxy_vendor: None,
                license: None,
                description: "t".into(),
                homepage: "t".into(),
                fork_of: None,
                fork_reason: None,
                native_build_inputs: None,
                python_deps: None,
                pname_override: None,
                dont_npm_build: None,
                extra_post_install: None,
                binary_name: None,
                platform_urls: None,
                track: crate::matrix::TrackMode::default(),
                unstable_base: None,
                versions,
            },
        );
        let matrix = Matrix { packages };
        let store = InMemoryStore {
            matrix: Mutex::new(matrix),
        };

        let runner = MockRunner {
            responses: Mutex::new(vec![CommandOutput {
                success: true,
                stdout: String::new(),
                stderr: String::new(),
            }]),
        };

        run(std::path::Path::new("fake.toml"), &runner, &store)
            .await
            .unwrap();

        let result = store.matrix.lock().unwrap();
        let entry = &result.packages["akeyless-test"].versions["1.0.0"];
        assert_eq!(entry.status, Status::Verified);
        assert!(entry.verified_at.is_some());
    }

    #[tokio::test]
    async fn test_verify_marks_broken_on_failure() {
        let mut versions = BTreeMap::new();
        versions.insert(
            "1.0.0".to_string(),
            make_verified_entry("sha256-src", Some("sha256-vendor")),
        );
        let mut packages = BTreeMap::new();
        packages.insert(
            "akeyless-test".to_string(),
            Package {
                owner: "testorg".into(),
                repo: "test".into(),
                language: Language::Go,
                builder: Builder::MkGoTool,
                tier: 1,
                sub_packages: None,
                proxy_vendor: None,
                license: None,
                description: "t".into(),
                homepage: "t".into(),
                fork_of: None,
                fork_reason: None,
                native_build_inputs: None,
                python_deps: None,
                pname_override: None,
                dont_npm_build: None,
                extra_post_install: None,
                binary_name: None,
                platform_urls: None,
                track: crate::matrix::TrackMode::default(),
                unstable_base: None,
                versions,
            },
        );
        let matrix = Matrix { packages };
        let store = InMemoryStore {
            matrix: Mutex::new(matrix),
        };

        let runner = MockRunner {
            responses: Mutex::new(vec![CommandOutput {
                success: false,
                stdout: String::new(),
                stderr: "error: hash mismatch in derivation\n".into(),
            }]),
        };

        run(std::path::Path::new("fake.toml"), &runner, &store)
            .await
            .unwrap();

        let result = store.matrix.lock().unwrap();
        let entry = &result.packages["akeyless-test"].versions["1.0.0"];
        assert_eq!(entry.status, Status::Broken);
    }

    #[tokio::test]
    async fn test_verify_skips_none_builder() {
        let mut versions = BTreeMap::new();
        versions.insert(
            "1.0.0".to_string(),
            make_verified_entry("sha256-src", None),
        );
        let mut packages = BTreeMap::new();
        packages.insert(
            "akeyless-ruby-sdk".to_string(),
            Package {
                owner: "testorg".into(),
                repo: "test".into(),
                language: Language::Ruby,
                builder: Builder::None,
                tier: 3,
                sub_packages: None,
                proxy_vendor: None,
                license: None,
                description: "t".into(),
                homepage: "t".into(),
                fork_of: None,
                fork_reason: None,
                native_build_inputs: None,
                python_deps: None,
                pname_override: None,
                dont_npm_build: None,
                extra_post_install: None,
                binary_name: None,
                platform_urls: None,
                track: crate::matrix::TrackMode::default(),
                unstable_base: None,
                versions,
            },
        );
        let matrix = Matrix { packages };
        let store = InMemoryStore {
            matrix: Mutex::new(matrix),
        };

        let runner = MockRunner {
            responses: Mutex::new(vec![]),
        };

        run(std::path::Path::new("fake.toml"), &runner, &store)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_verify_entry_missing_source_hash() {
        let mut entry = make_verified_entry("sha256-src", None);
        entry.source_hash = None;

        let pkg = Package {
            owner: "t".into(),
            repo: "t".into(),
            language: Language::Go,
            builder: Builder::MkGoTool,
            tier: 1,
            sub_packages: None,
            proxy_vendor: None,
            license: None,
            description: "t".into(),
            homepage: "t".into(),
            fork_of: None,
            fork_reason: None,
            native_build_inputs: None,
            python_deps: None,
            pname_override: None,
            dont_npm_build: None,
            extra_post_install: None,
            binary_name: None,
            platform_urls: None,
            track: crate::matrix::TrackMode::default(),
            unstable_base: None,
            versions: BTreeMap::new(),
        };

        let runner = MockRunner {
            responses: Mutex::new(vec![]),
        };

        let result = verify_entry(&pkg, &entry, &runner).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("missing source_hash"));
    }

    #[tokio::test]
    async fn test_verify_skips_java_language() {
        let entry = make_verified_entry("sha256-src", None);

        let pkg = Package {
            owner: "t".into(),
            repo: "t".into(),
            language: Language::Java,
            builder: Builder::MkJavaMavenPackage,
            tier: 3,
            sub_packages: None,
            proxy_vendor: None,
            license: None,
            description: "t".into(),
            homepage: "t".into(),
            fork_of: None,
            fork_reason: None,
            native_build_inputs: None,
            python_deps: None,
            pname_override: None,
            dont_npm_build: None,
            extra_post_install: None,
            binary_name: None,
            platform_urls: None,
            track: crate::matrix::TrackMode::default(),
            unstable_base: None,
            versions: BTreeMap::new(),
        };

        let runner = MockRunner {
            responses: Mutex::new(vec![]),
        };

        let result = verify_entry(&pkg, &entry, &runner).await;
        assert!(result.is_ok());
    }
}
