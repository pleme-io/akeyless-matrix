use std::path::Path;

use anyhow::{Result, bail};
use chrono::Utc;

use crate::display;
use crate::hash;
use crate::matrix::{Builder, Package, Status, VersionEntry};
use crate::nixexpr;
use crate::runner::CommandRunner;
use crate::storage::MatrixStore;

/// Run the `build` subcommand: prefetch source hashes and extract vendor/cargo/npm
/// hashes for pending entries.
pub async fn run(
    matrix_path: &Path,
    filter_package: Option<&str>,
    runner: &dyn CommandRunner,
    store: &dyn MatrixStore,
) -> Result<()> {
    let mut matrix = store.load(matrix_path)?;
    display::print_header("Building pending matrix entries");

    let pkg_names: Vec<String> = match filter_package {
        Some(name) => {
            if !matrix.packages.contains_key(name) {
                bail!("package '{name}' not found in matrix");
            }
            vec![name.to_string()]
        }
        None => matrix.packages.keys().cloned().collect(),
    };

    let mut built = 0u32;
    let mut failed = 0u32;

    for pkg_name in &pkg_names {
        let pkg = matrix.packages.get(pkg_name).unwrap().clone();
        let pending_versions: Vec<(String, VersionEntry)> = pkg
            .versions
            .iter()
            .filter(|(_, v)| v.status == Status::Pending || v.status == Status::Building)
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        for (ver_key, mut entry) in pending_versions {
            display::print_build_start(pkg_name, &ver_key);

            match build_entry(&pkg, &mut entry, runner).await {
                Ok(()) => {
                    entry.status = Status::Verified;
                    entry.verified_at = Some(Utc::now());
                    display::print_build_success(pkg_name, &ver_key);
                    built += 1;
                }
                Err(e) => {
                    entry.status = Status::Broken;
                    display::print_build_failure(pkg_name, &ver_key, &e.to_string());
                    failed += 1;
                }
            }

            // Update the entry in-place
            let pkg_mut = matrix.packages.get_mut(pkg_name).unwrap();
            pkg_mut.versions.insert(ver_key, entry);

            // Save after each entry so progress is not lost
            store.save(matrix_path, &matrix)?;
        }
    }

    println!();
    if failed == 0 {
        println!("  done: {built} built, 0 failed");
    } else {
        println!("  done: {built} built, {failed} failed");
    }

    Ok(())
}

/// Build a single version entry: prefetch source, then extract vendor/cargo/npm hash.
async fn build_entry(
    pkg: &Package,
    entry: &mut VersionEntry,
    runner: &dyn CommandRunner,
) -> Result<()> {
    // Step 1: Prefetch source hash if missing (skip for fetchurl packages)
    if entry.source_hash.is_none() && pkg.builder != Builder::Fetchurl {
        display::print_prefetch_start(&pkg.owner, &pkg.repo, &entry.rev);
        let source_hash =
            hash::prefetch_github(runner, &pkg.owner, &pkg.repo, &entry.rev).await?;
        entry.source_hash = Some(source_hash);
    }

    // Step 2: Extract the build-specific hash (vendor/cargo/npm) via nix build
    match pkg.builder {
        Builder::MkGoTool | Builder::MkGoLibraryCheck => {
            // Go packages need a vendor hash (unless deps are vendored, e.g. go-sdk)
            if needs_vendor_hash(pkg, entry) {
                display::print_hash_extraction("vendor");
                let vendor_hash = extract_go_vendor_hash(pkg, entry, runner).await?;
                entry.vendor_hash = Some(vendor_hash);
            }
        }
        Builder::BuildRustPackage => {
            if entry.cargo_hash.is_none() {
                display::print_hash_extraction("cargo");
                let cargo_hash = extract_rust_cargo_hash(pkg, entry, runner).await?;
                entry.cargo_hash = Some(cargo_hash);
            }
        }
        Builder::BuildNpmPackage => {
            if entry.npm_deps_hash.is_none() {
                display::print_hash_extraction("npm deps");
                let npm_hash = extract_npm_deps_hash(pkg, entry, runner).await?;
                entry.npm_deps_hash = Some(npm_hash);
            }
        }
        Builder::Fetchurl => {
            // For binary packages, prefetch all platform URLs
            if let Some(ref urls) = pkg.platform_urls {
                display::print_hash_extraction("binary (all platforms)");
                for (platform, url) in urls {
                    let hash_result = hash::prefetch_url(runner, url).await?;
                    match platform.as_str() {
                        "aarch64-darwin" => entry.hash_aarch64_darwin = Some(hash_result),
                        "x86_64-darwin" => entry.hash_x86_64_darwin = Some(hash_result),
                        "x86_64-linux" => entry.hash_x86_64_linux = Some(hash_result),
                        "aarch64-linux" => entry.hash_aarch64_linux = Some(hash_result),
                        _ => {}
                    }
                }
            }
        }
        Builder::MkPythonPackage | Builder::None => {
            // Python packages and source-only packages don't need extra hashes
        }
    }

    Ok(())
}

/// Check if a Go package needs a vendor hash extraction.
/// Some Go packages (like akeyless-go-sdk) have vendored deps and use null vendor hash.
fn needs_vendor_hash(pkg: &Package, entry: &VersionEntry) -> bool {
    if entry.vendor_hash.is_some() {
        return false;
    }
    // mkGoLibraryCheck packages without an existing vendor hash are likely
    // using vendored deps (vendor_hash = null in Nix)
    if pkg.builder == Builder::MkGoLibraryCheck && pkg.proxy_vendor.is_none() {
        return false;
    }
    true
}

/// Generate a Nix expression to build a Go package with a dummy vendor hash,
/// then extract the real hash from the error output.
async fn extract_go_vendor_hash(
    pkg: &Package,
    entry: &VersionEntry,
    runner: &dyn CommandRunner,
) -> Result<String> {
    let source_hash = entry.source_hash.as_deref().unwrap_or(hash::DUMMY_HASH);
    let vendor_hash = format!("\"{}\"", hash::DUMMY_HASH);

    let expr = nixexpr::go_expr(pkg, &entry.rev, source_hash, &vendor_hash, false);

    let (success, _stdout, stderr) = hash::nix_build_expr(runner, &expr).await?;

    if success {
        // Build succeeded with dummy hash -- this means vendor hash is not needed
        bail!("build succeeded with dummy vendor hash (unexpected)");
    }

    hash::extract_hash_from_stderr(&stderr)
        .ok_or_else(|| anyhow::anyhow!("could not extract vendor hash from nix build stderr"))
}

/// Generate a Nix expression to build a Rust package with a dummy cargo hash,
/// then extract the real hash from the error output.
async fn extract_rust_cargo_hash(
    pkg: &Package,
    entry: &VersionEntry,
    runner: &dyn CommandRunner,
) -> Result<String> {
    let source_hash = entry.source_hash.as_deref().unwrap_or(hash::DUMMY_HASH);

    let expr = nixexpr::rust_expr(pkg, &entry.rev, source_hash, hash::DUMMY_HASH, false);

    let (success, _stdout, stderr) = hash::nix_build_expr(runner, &expr).await?;

    if success {
        bail!("build succeeded with dummy cargo hash (unexpected)");
    }

    hash::extract_hash_from_stderr(&stderr)
        .ok_or_else(|| anyhow::anyhow!("could not extract cargo hash from nix build stderr"))
}

/// Generate a Nix expression to build an npm package with a dummy deps hash,
/// then extract the real hash from the error output.
async fn extract_npm_deps_hash(
    pkg: &Package,
    entry: &VersionEntry,
    runner: &dyn CommandRunner,
) -> Result<String> {
    let source_hash = entry.source_hash.as_deref().unwrap_or(hash::DUMMY_HASH);

    let expr = nixexpr::typescript_expr(pkg, &entry.rev, source_hash, hash::DUMMY_HASH, false);

    let (success, _stdout, stderr) = hash::nix_build_expr(runner, &expr).await?;

    if success {
        bail!("build succeeded with dummy npm deps hash (unexpected)");
    }

    hash::extract_hash_from_stderr(&stderr)
        .ok_or_else(|| anyhow::anyhow!("could not extract npm deps hash from nix build stderr"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matrix::{Builder, Language, Package, Status, VersionEntry};
    use std::collections::BTreeMap;

    #[test]
    fn test_needs_vendor_hash() {
        let pkg = Package {
            owner: "test".into(),
            repo: "test".into(),
            language: Language::Go,
            builder: Builder::MkGoTool,
            tier: 1,
            sub_packages: None,
            proxy_vendor: None,
            license: None,
            description: "test".into(),
            homepage: "test".into(),
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

        let entry_with_hash = VersionEntry {
            rev: "abc".into(),
            source_hash: None,
            vendor_hash: Some("sha256-test".into()),
            cargo_hash: None,
            npm_deps_hash: None,
            status: Status::Pending,
            verified_at: None,
            hash_aarch64_darwin: None,
            hash_x86_64_darwin: None,
            hash_x86_64_linux: None,
            hash_aarch64_linux: None,
        };
        assert!(!needs_vendor_hash(&pkg, &entry_with_hash));

        let entry_no_hash = VersionEntry {
            rev: "abc".into(),
            source_hash: None,
            vendor_hash: None,
            cargo_hash: None,
            npm_deps_hash: None,
            status: Status::Pending,
            verified_at: None,
            hash_aarch64_darwin: None,
            hash_x86_64_darwin: None,
            hash_x86_64_linux: None,
            hash_aarch64_linux: None,
        };
        assert!(needs_vendor_hash(&pkg, &entry_no_hash));
    }

    #[test]
    fn test_needs_vendor_hash_lib_check_without_proxy() {
        // MkGoLibraryCheck without proxy_vendor → vendored deps → no hash needed
        let pkg = Package {
            owner: "t".into(),
            repo: "t".into(),
            language: Language::Go,
            builder: Builder::MkGoLibraryCheck,
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
        let entry = VersionEntry {
            rev: "abc".into(),
            source_hash: None,
            vendor_hash: None,
            cargo_hash: None,
            npm_deps_hash: None,
            status: Status::Pending,
            verified_at: None,
            hash_aarch64_darwin: None,
            hash_x86_64_darwin: None,
            hash_x86_64_linux: None,
            hash_aarch64_linux: None,
        };
        assert!(!needs_vendor_hash(&pkg, &entry));
    }

    #[test]
    fn test_needs_vendor_hash_lib_check_with_proxy() {
        // MkGoLibraryCheck WITH proxy_vendor → needs hash
        let pkg = Package {
            owner: "t".into(),
            repo: "t".into(),
            language: Language::Go,
            builder: Builder::MkGoLibraryCheck,
            tier: 3,
            sub_packages: None,
            proxy_vendor: Some(true),
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
        let entry = VersionEntry {
            rev: "abc".into(),
            source_hash: None,
            vendor_hash: None,
            cargo_hash: None,
            npm_deps_hash: None,
            status: Status::Pending,
            verified_at: None,
            hash_aarch64_darwin: None,
            hash_x86_64_darwin: None,
            hash_x86_64_linux: None,
            hash_aarch64_linux: None,
        };
        assert!(needs_vendor_hash(&pkg, &entry));
    }

    use crate::matrix::Matrix;
    use crate::runner::CommandOutput;
    use crate::storage::MatrixStore;
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

    #[tokio::test]
    async fn test_build_flow_go_package() {
        // Set up a matrix with one pending Go package
        let mut versions = BTreeMap::new();
        versions.insert(
            "1.0.0".to_string(),
            VersionEntry {
                rev: "abc123".into(),
                source_hash: None,
                vendor_hash: None,
                cargo_hash: None,
                npm_deps_hash: None,
                status: Status::Pending,
                verified_at: None,
                hash_aarch64_darwin: None,
                hash_x86_64_darwin: None,
                hash_x86_64_linux: None,
                hash_aarch64_linux: None,
            },
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
                sub_packages: Some(vec![".".into()]),
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

        // Mock responses:
        // 1. nix-prefetch-github → returns source hash
        // 2. nix build (vendor hash extraction) → fails with hash mismatch
        let runner = MockRunner {
            responses: Mutex::new(vec![
                CommandOutput {
                    success: true,
                    stdout: r#"{ hash = "sha256-SourceHash123"; }"#.into(),
                    stderr: String::new(),
                },
                CommandOutput {
                    success: false,
                    stdout: String::new(),
                    stderr: "got:    sha256-RealVendorHash456=\n".into(),
                },
            ]),
        };

        run(
            std::path::Path::new("fake.toml"),
            Some("akeyless-test"),
            &runner,
            &store,
        )
        .await
        .unwrap();

        // Verify the matrix was updated
        let result = store.matrix.lock().unwrap();
        let pkg = &result.packages["akeyless-test"];
        let entry = &pkg.versions["1.0.0"];
        assert_eq!(entry.status, Status::Verified);
        assert_eq!(
            entry.source_hash.as_deref(),
            Some("sha256-SourceHash123")
        );
        assert_eq!(
            entry.vendor_hash.as_deref(),
            Some("sha256-RealVendorHash456=")
        );
        assert!(entry.verified_at.is_some());
    }

    #[tokio::test]
    async fn test_build_flow_marks_broken_on_failure() {
        let mut versions = BTreeMap::new();
        versions.insert(
            "1.0.0".to_string(),
            VersionEntry {
                rev: "abc".into(),
                source_hash: None,
                vendor_hash: None,
                cargo_hash: None,
                npm_deps_hash: None,
                status: Status::Pending,
                verified_at: None,
                hash_aarch64_darwin: None,
                hash_x86_64_darwin: None,
                hash_x86_64_linux: None,
                hash_aarch64_linux: None,
            },
        );
        let mut packages = BTreeMap::new();
        packages.insert(
            "akeyless-test".to_string(),
            Package {
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
                versions,
            },
        );
        let matrix = Matrix { packages };
        let store = InMemoryStore {
            matrix: Mutex::new(matrix),
        };

        // Prefetch fails
        let runner = MockRunner {
            responses: Mutex::new(vec![CommandOutput {
                success: false,
                stdout: String::new(),
                stderr: "404 not found".into(),
            }]),
        };

        run(
            std::path::Path::new("fake.toml"),
            Some("akeyless-test"),
            &runner,
            &store,
        )
        .await
        .unwrap();

        let result = store.matrix.lock().unwrap();
        let entry = &result.packages["akeyless-test"].versions["1.0.0"];
        assert_eq!(entry.status, Status::Broken);
    }

    #[tokio::test]
    async fn test_build_python_skips_hash_extraction() {
        use crate::matrix::test_helpers::{pending_version, pkg};

        let mut p = pkg(Language::Python, Builder::MkPythonPackage);
        p.versions.insert("5.0.22".into(), pending_version("abc"));
        let mut packages = BTreeMap::new();
        packages.insert("akeyless-python-sdk".into(), p);
        let matrix = Matrix { packages };
        let store = InMemoryStore {
            matrix: Mutex::new(matrix),
        };

        // Only 1 response needed: prefetch (no hash extraction for Python)
        let runner = MockRunner {
            responses: Mutex::new(vec![CommandOutput {
                success: true,
                stdout: r#"{ hash = "sha256-PySource"; }"#.into(),
                stderr: String::new(),
            }]),
        };

        run(
            std::path::Path::new("fake.toml"),
            Some("akeyless-python-sdk"),
            &runner,
            &store,
        )
        .await
        .unwrap();

        let result = store.matrix.lock().unwrap();
        let entry = &result.packages["akeyless-python-sdk"].versions["5.0.22"];
        assert_eq!(entry.status, Status::Verified);
        assert_eq!(entry.source_hash.as_deref(), Some("sha256-PySource"));
        // No vendor/cargo/npm hash for Python
        assert!(entry.vendor_hash.is_none());
        assert!(entry.cargo_hash.is_none());
        assert!(entry.npm_deps_hash.is_none());
    }

    #[tokio::test]
    async fn test_build_none_builder_is_noop() {
        use crate::matrix::test_helpers::{pending_version, pkg};

        let mut p = pkg(Language::Java, Builder::None);
        p.versions.insert("5.0.22".into(), pending_version("abc"));
        let mut packages = BTreeMap::new();
        packages.insert("akeyless-java-sdk".into(), p);
        let matrix = Matrix { packages };
        let store = InMemoryStore {
            matrix: Mutex::new(matrix),
        };

        // No runner calls needed — None builder prefetches but skips hash extraction
        let runner = MockRunner {
            responses: Mutex::new(vec![CommandOutput {
                success: true,
                stdout: r#"{ hash = "sha256-JavaSrc"; }"#.into(),
                stderr: String::new(),
            }]),
        };

        run(
            std::path::Path::new("fake.toml"),
            Some("akeyless-java-sdk"),
            &runner,
            &store,
        )
        .await
        .unwrap();

        let result = store.matrix.lock().unwrap();
        let entry = &result.packages["akeyless-java-sdk"].versions["5.0.22"];
        assert_eq!(entry.status, Status::Verified);
        assert_eq!(entry.source_hash.as_deref(), Some("sha256-JavaSrc"));
    }
}
