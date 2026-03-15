use std::path::Path;

use anyhow::Result;

use crate::display;
use crate::nix;
use crate::storage::{FileWriter, MatrixStore};

/// Run the `generate` subcommand: read the matrix and emit Nix files.
pub fn run(
    matrix_path: &Path,
    output_dir: Option<&Path>,
    store: &dyn MatrixStore,
    writer: &dyn FileWriter,
) -> Result<()> {
    let matrix = store.load(matrix_path)?;
    let base_dir = output_dir.unwrap_or_else(|| {
        matrix_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
    });

    display::print_header("Generating Nix files from matrix");

    // lib/sources.nix
    let sources_dir = base_dir.join("lib");
    writer.create_dir_all(&sources_dir)?;
    let sources_path = sources_dir.join("sources.nix");
    let sources_content = nix::generate_sources_nix(&matrix);
    writer.write_file(&sources_path, &sources_content)?;
    display::print_generate_file(&sources_path.display().to_string());

    // lib/matrix-metadata.nix
    let metadata_path = sources_dir.join("matrix-metadata.nix");
    let metadata_content = nix::generate_matrix_metadata(&matrix);
    writer.write_file(&metadata_path, &metadata_content)?;
    display::print_generate_file(&metadata_path.display().to_string());

    // builds/go/default.nix
    let go_dir = base_dir.join("builds").join("go");
    writer.create_dir_all(&go_dir)?;
    let go_path = go_dir.join("default.nix");
    let go_content = nix::generate_go_builds(&matrix);
    writer.write_file(&go_path, &go_content)?;
    display::print_generate_file(&go_path.display().to_string());

    // builds/rust/default.nix
    let rust_dir = base_dir.join("builds").join("rust");
    writer.create_dir_all(&rust_dir)?;
    let rust_path = rust_dir.join("default.nix");
    let rust_content = nix::generate_rust_builds(&matrix);
    writer.write_file(&rust_path, &rust_content)?;
    display::print_generate_file(&rust_path.display().to_string());

    // builds/python/default.nix
    let python_dir = base_dir.join("builds").join("python");
    writer.create_dir_all(&python_dir)?;
    let python_path = python_dir.join("default.nix");
    let python_content = nix::generate_python_builds(&matrix);
    writer.write_file(&python_path, &python_content)?;
    display::print_generate_file(&python_path.display().to_string());

    // builds/typescript/default.nix
    let ts_dir = base_dir.join("builds").join("typescript");
    writer.create_dir_all(&ts_dir)?;
    let ts_path = ts_dir.join("default.nix");
    let ts_content = nix::generate_typescript_builds(&matrix);
    writer.write_file(&ts_path, &ts_content)?;
    display::print_generate_file(&ts_path.display().to_string());

    // builds/binary/default.nix
    let bin_dir = base_dir.join("builds").join("binary");
    writer.create_dir_all(&bin_dir)?;
    let bin_path = bin_dir.join("default.nix");
    let bin_content = nix::generate_binary_builds(&matrix);
    writer.write_file(&bin_path, &bin_content)?;
    display::print_generate_file(&bin_path.display().to_string());

    println!();
    println!("  done: 7 files generated");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matrix::Matrix;
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use std::sync::Mutex;

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

    struct RecordingWriter {
        dirs: Mutex<Vec<PathBuf>>,
        files: Mutex<Vec<(PathBuf, String)>>,
    }

    impl RecordingWriter {
        fn new() -> Self {
            Self {
                dirs: Mutex::new(Vec::new()),
                files: Mutex::new(Vec::new()),
            }
        }
    }

    impl FileWriter for RecordingWriter {
        fn write_file(&self, path: &Path, content: &str) -> Result<()> {
            self.files
                .lock()
                .unwrap()
                .push((path.to_path_buf(), content.to_string()));
            Ok(())
        }
        fn create_dir_all(&self, path: &Path) -> Result<()> {
            self.dirs.lock().unwrap().push(path.to_path_buf());
            Ok(())
        }
    }

    #[test]
    fn test_generate_writes_seven_files() {
        let matrix = Matrix {
            packages: BTreeMap::new(),
        };
        let store = InMemoryStore { matrix };
        let writer = RecordingWriter::new();

        run(Path::new("/fake/matrix.toml"), None, &store, &writer).unwrap();

        let files = writer.files.lock().unwrap();
        assert_eq!(files.len(), 7);

        let paths: Vec<String> = files.iter().map(|(p, _)| p.display().to_string()).collect();
        assert!(paths.iter().any(|p| p.ends_with("lib/sources.nix")));
        assert!(paths.iter().any(|p| p.ends_with("lib/matrix-metadata.nix")));
        assert!(paths.iter().any(|p| p.ends_with("builds/go/default.nix")));
        assert!(paths.iter().any(|p| p.ends_with("builds/rust/default.nix")));
        assert!(paths
            .iter()
            .any(|p| p.ends_with("builds/python/default.nix")));
        assert!(paths
            .iter()
            .any(|p| p.ends_with("builds/typescript/default.nix")));
        assert!(paths
            .iter()
            .any(|p| p.ends_with("builds/binary/default.nix")));
    }

    #[test]
    fn test_generate_creates_six_dirs() {
        let matrix = Matrix {
            packages: BTreeMap::new(),
        };
        let store = InMemoryStore { matrix };
        let writer = RecordingWriter::new();

        run(Path::new("/fake/matrix.toml"), None, &store, &writer).unwrap();

        let dirs = writer.dirs.lock().unwrap();
        // lib/, builds/go/, builds/rust/, builds/python/, builds/typescript/, builds/binary/
        assert_eq!(dirs.len(), 6);
    }

    #[test]
    fn test_generate_respects_output_dir() {
        let matrix = Matrix {
            packages: BTreeMap::new(),
        };
        let store = InMemoryStore { matrix };
        let writer = RecordingWriter::new();

        run(
            Path::new("/fake/matrix.toml"),
            Some(Path::new("/custom/output")),
            &store,
            &writer,
        )
        .unwrap();

        let files = writer.files.lock().unwrap();
        // All paths should be under /custom/output
        for (path, _) in files.iter() {
            assert!(
                path.starts_with("/custom/output"),
                "expected path under /custom/output, got: {}",
                path.display()
            );
        }
    }
}
