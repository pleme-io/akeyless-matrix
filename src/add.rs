use std::path::Path;

use anyhow::{Result, bail};

use crate::display;
use crate::matrix::{Status, VersionEntry};
use crate::storage::MatrixStore;

/// Run the `add` subcommand: add a new pending version entry to a package.
pub fn run(
    matrix_path: &Path,
    package: &str,
    version: &str,
    rev: &str,
    store: &dyn MatrixStore,
) -> Result<()> {
    let mut matrix = store.load(matrix_path)?;

    let pkg = matrix
        .packages
        .get_mut(package)
        .ok_or_else(|| anyhow::anyhow!("package '{package}' not found in matrix"))?;

    if pkg.versions.contains_key(version) {
        bail!("version '{version}' already exists for package '{package}'");
    }

    let entry = VersionEntry {
        rev: rev.to_string(),
        source_hash: None,
        vendor_hash: None,
        cargo_hash: None,
        npm_deps_hash: None,
        status: Status::Pending,
        verified_at: None,
    };

    pkg.versions.insert(version.to_string(), entry);
    store.save(matrix_path, &matrix)?;

    display::print_add_success(package, version);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matrix::Matrix;
    use crate::storage::MatrixStore;
    use std::sync::Mutex;

    struct InMemoryStore {
        matrix: Mutex<Option<Matrix>>,
    }

    impl InMemoryStore {
        fn with_matrix(matrix: Matrix) -> Self {
            Self {
                matrix: Mutex::new(Some(matrix)),
            }
        }

        fn get(&self) -> Matrix {
            self.matrix.lock().unwrap().clone().unwrap()
        }
    }

    impl MatrixStore for InMemoryStore {
        fn load(&self, _path: &Path) -> Result<Matrix> {
            self.matrix
                .lock()
                .unwrap()
                .clone()
                .ok_or_else(|| anyhow::anyhow!("no matrix loaded"))
        }

        fn save(&self, _path: &Path, matrix: &Matrix) -> Result<()> {
            *self.matrix.lock().unwrap() = Some(matrix.clone());
            Ok(())
        }
    }

    fn minimal_matrix() -> Matrix {
        let toml = r#"
[packages.akeyless-test]
owner = "org"
repo = "test"
language = "go"
builder = "mkGoTool"
tier = 1
description = "test"
homepage = "https://example.com"
"#;
        Matrix::from_str(toml).unwrap()
    }

    #[test]
    fn test_add_creates_pending_entry() {
        let store = InMemoryStore::with_matrix(minimal_matrix());
        let path = Path::new("fake.toml");
        run(path, "akeyless-test", "1.0.0", "abc123", &store).unwrap();

        let matrix = store.get();
        let pkg = &matrix.packages["akeyless-test"];
        assert_eq!(pkg.versions.len(), 1);
        let entry = &pkg.versions["1.0.0"];
        assert_eq!(entry.rev, "abc123");
        assert_eq!(entry.status, Status::Pending);
        assert!(entry.source_hash.is_none());
    }

    #[test]
    fn test_add_rejects_duplicate_version() {
        let store = InMemoryStore::with_matrix(minimal_matrix());
        let path = Path::new("fake.toml");
        run(path, "akeyless-test", "1.0.0", "abc", &store).unwrap();
        let result = run(path, "akeyless-test", "1.0.0", "def", &store);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
    }

    #[test]
    fn test_add_rejects_unknown_package() {
        let store = InMemoryStore::with_matrix(minimal_matrix());
        let path = Path::new("fake.toml");
        let result = run(path, "nonexistent", "1.0.0", "abc", &store);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }
}
