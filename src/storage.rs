use std::path::Path;

use anyhow::{Context, Result};

use crate::matrix::Matrix;

/// Trait abstracting matrix.toml persistence for testability.
///
/// Production code uses `FsMatrixStore`, which reads/writes files.
/// Tests can substitute an in-memory store.
pub trait MatrixStore {
    fn load(&self, path: &Path) -> Result<Matrix>;
    fn save(&self, path: &Path, matrix: &Matrix) -> Result<()>;
}

/// Real implementation backed by the filesystem using `toml`/`toml_edit`.
pub struct FsMatrixStore;

impl MatrixStore for FsMatrixStore {
    fn load(&self, path: &Path) -> Result<Matrix> {
        Matrix::load_from_path(path)
    }

    fn save(&self, path: &Path, matrix: &Matrix) -> Result<()> {
        Matrix::save_to_path(path, matrix)
    }
}

/// Trait abstracting file writing for testability.
pub trait FileWriter {
    fn write_file(&self, path: &Path, content: &str) -> Result<()>;
    fn create_dir_all(&self, path: &Path) -> Result<()>;
}

/// Real implementation backed by the filesystem.
pub struct FsFileWriter;

impl FileWriter for FsFileWriter {
    fn write_file(&self, path: &Path, content: &str) -> Result<()> {
        std::fs::write(path, content)
            .with_context(|| format!("writing {}", path.display()))
    }

    fn create_dir_all(&self, path: &Path) -> Result<()> {
        std::fs::create_dir_all(path)
            .with_context(|| format!("creating {}", path.display()))
    }
}
