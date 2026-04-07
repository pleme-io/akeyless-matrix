use std::path::Path;

use anyhow::Result;

use crate::display;
use crate::storage::MatrixStore;

/// Run the `status` subcommand: load the matrix and print a status table.
pub fn run(matrix_path: &Path, store: &dyn MatrixStore) -> Result<()> {
    let matrix = store.load(matrix_path)?;
    display::print_header("Akeyless Version Matrix");
    display::print_status_table(&matrix);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matrix::{
        Builder, Language, Matrix, Package, Status, TrackMode, VersionEntry,
    };
    use std::collections::BTreeMap;

    struct InMemoryStore {
        matrix: Matrix,
    }

    impl MatrixStore for InMemoryStore {
        fn load(&self, _path: &Path) -> Result<Matrix> {
            Ok(self.matrix.clone())
        }
        fn save(&self, _path: &Path, _matrix: &Matrix) -> Result<()> {
            Ok(())
        }
    }

    struct FailingStore;

    impl MatrixStore for FailingStore {
        fn load(&self, _path: &Path) -> Result<Matrix> {
            anyhow::bail!("store load failed")
        }
        fn save(&self, _path: &Path, _matrix: &Matrix) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn test_status_run_empty_matrix() {
        let store = InMemoryStore {
            matrix: Matrix {
                packages: BTreeMap::new(),
            },
        };
        let result = run(Path::new("fake.toml"), &store);
        assert!(result.is_ok());
    }

    #[test]
    fn test_status_run_with_packages() {
        let mut versions = BTreeMap::new();
        versions.insert(
            "1.0.0".to_string(),
            VersionEntry {
                rev: "abc".into(),
                source_hash: Some("sha256-src".into()),
                vendor_hash: None,
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
                track: TrackMode::default(),
                unstable_base: None,
                versions,
            },
        );
        let store = InMemoryStore {
            matrix: Matrix { packages },
        };
        let result = run(Path::new("fake.toml"), &store);
        assert!(result.is_ok());
    }

    #[test]
    fn test_status_run_propagates_load_error() {
        let store = FailingStore;
        let result = run(Path::new("fake.toml"), &store);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("store load failed"));
    }
}
