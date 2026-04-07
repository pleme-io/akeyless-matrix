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

    // builds/java/default.nix
    let java_dir = base_dir.join("builds").join("java");
    writer.create_dir_all(&java_dir)?;
    let java_path = java_dir.join("default.nix");
    let java_content = nix::generate_java_builds(&matrix);
    writer.write_file(&java_path, &java_content)?;
    display::print_generate_file(&java_path.display().to_string());

    // builds/csharp/default.nix
    let csharp_dir = base_dir.join("builds").join("csharp");
    writer.create_dir_all(&csharp_dir)?;
    let csharp_path = csharp_dir.join("default.nix");
    let csharp_content = nix::generate_csharp_builds(&matrix);
    writer.write_file(&csharp_path, &csharp_content)?;
    display::print_generate_file(&csharp_path.display().to_string());

    // builds/ruby/default.nix
    let ruby_dir = base_dir.join("builds").join("ruby");
    writer.create_dir_all(&ruby_dir)?;
    let ruby_path = ruby_dir.join("default.nix");
    let ruby_content = nix::generate_ruby_builds(&matrix);
    writer.write_file(&ruby_path, &ruby_content)?;
    display::print_generate_file(&ruby_path.display().to_string());

    // builds/php/default.nix
    let php_dir = base_dir.join("builds").join("php");
    writer.create_dir_all(&php_dir)?;
    let php_path = php_dir.join("default.nix");
    let php_content = nix::generate_php_builds(&matrix);
    writer.write_file(&php_path, &php_content)?;
    display::print_generate_file(&php_path.display().to_string());

    // builds/helm/default.nix
    let helm_dir = base_dir.join("builds").join("helm");
    writer.create_dir_all(&helm_dir)?;
    let helm_path = helm_dir.join("default.nix");
    let helm_content = nix::generate_helm_builds(&matrix);
    writer.write_file(&helm_path, &helm_content)?;
    display::print_generate_file(&helm_path.display().to_string());

    println!();
    println!("  done: 12 files generated");

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
    fn test_generate_writes_twelve_files() {
        let matrix = Matrix {
            packages: BTreeMap::new(),
        };
        let store = InMemoryStore { matrix };
        let writer = RecordingWriter::new();

        run(Path::new("/fake/matrix.toml"), None, &store, &writer).unwrap();

        let files = writer.files.lock().unwrap();
        assert_eq!(files.len(), 12);

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
        assert!(paths
            .iter()
            .any(|p| p.ends_with("builds/java/default.nix")));
        assert!(paths
            .iter()
            .any(|p| p.ends_with("builds/csharp/default.nix")));
        assert!(paths
            .iter()
            .any(|p| p.ends_with("builds/ruby/default.nix")));
        assert!(paths
            .iter()
            .any(|p| p.ends_with("builds/php/default.nix")));
        assert!(paths
            .iter()
            .any(|p| p.ends_with("builds/helm/default.nix")));
    }

    #[test]
    fn test_generate_creates_eleven_dirs() {
        let matrix = Matrix {
            packages: BTreeMap::new(),
        };
        let store = InMemoryStore { matrix };
        let writer = RecordingWriter::new();

        run(Path::new("/fake/matrix.toml"), None, &store, &writer).unwrap();

        let dirs = writer.dirs.lock().unwrap();
        // lib/, builds/go/, builds/rust/, builds/python/, builds/typescript/,
        // builds/binary/, builds/java/, builds/csharp/, builds/ruby/, builds/php/, builds/helm/
        assert_eq!(dirs.len(), 11);
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
        for (path, _) in files.iter() {
            assert!(
                path.starts_with("/custom/output"),
                "expected path under /custom/output, got: {}",
                path.display()
            );
        }
    }

    /// Build a multi-language matrix for content verification tests.
    fn multi_lang_matrix() -> Matrix {
        use crate::matrix::{
            Builder, Language, Package, Status, TrackMode, VersionEntry,
        };

        let mut packages = BTreeMap::new();

        let go_ver = VersionEntry {
            rev: "gorev".into(),
            source_hash: Some("sha256-gosrc".into()),
            vendor_hash: Some("sha256-govendor".into()),
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
        };
        let mut go_versions = BTreeMap::new();
        go_versions.insert("1.0.0".into(), go_ver);
        packages.insert(
            "akeyless-cli-tool".into(),
            Package {
                owner: "testorg".into(),
                repo: "cli-tool".into(),
                language: Language::Go,
                builder: Builder::MkGoTool,
                tier: 1,
                sub_packages: Some(vec![".".into()]),
                proxy_vendor: None,
                license: None,
                description: "CLI tool".into(),
                homepage: "https://example.com".into(),
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
                versions: go_versions,
            },
        );

        let rust_ver = VersionEntry {
            rev: "rustrev".into(),
            source_hash: Some("sha256-rustsrc".into()),
            vendor_hash: None,
            cargo_hash: Some("sha256-rustcargo".into()),
            npm_deps_hash: None,
            maven_hash: None,
            nuget_deps_hash: None,
            status: Status::Verified,
            verified_at: None,
            hash_aarch64_darwin: None,
            hash_x86_64_darwin: None,
            hash_x86_64_linux: None,
            hash_aarch64_linux: None,
        };
        let mut rust_versions = BTreeMap::new();
        rust_versions.insert("0.5.0".into(), rust_ver);
        packages.insert(
            "akeyless-rust-agent".into(),
            Package {
                owner: "testorg".into(),
                repo: "rust-agent".into(),
                language: Language::Rust,
                builder: Builder::BuildRustPackage,
                tier: 2,
                sub_packages: None,
                proxy_vendor: None,
                license: None,
                description: "Rust agent".into(),
                homepage: "https://example.com/rust".into(),
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
                versions: rust_versions,
            },
        );

        Matrix { packages }
    }

    #[test]
    fn test_generate_sources_content_for_multi_lang() {
        let matrix = multi_lang_matrix();
        let store = InMemoryStore { matrix };
        let writer = RecordingWriter::new();

        run(Path::new("/fake/matrix.toml"), None, &store, &writer).unwrap();

        let files = writer.files.lock().unwrap();
        let sources = files
            .iter()
            .find(|(p, _)| p.display().to_string().ends_with("lib/sources.nix"))
            .map(|(_, c)| c.clone())
            .unwrap();

        assert!(sources.contains("cli-tool = {"));
        assert!(sources.contains(r#"hash = "sha256-gosrc";"#));
        assert!(sources.contains("rust-agent = {"));
        assert!(sources.contains(r#"hash = "sha256-rustsrc";"#));
    }

    #[test]
    fn test_generate_go_content_for_multi_lang() {
        let matrix = multi_lang_matrix();
        let store = InMemoryStore { matrix };
        let writer = RecordingWriter::new();

        run(Path::new("/fake/matrix.toml"), None, &store, &writer).unwrap();

        let files = writer.files.lock().unwrap();
        let go = files
            .iter()
            .find(|(p, _)| p.display().to_string().ends_with("builds/go/default.nix"))
            .map(|(_, c)| c.clone())
            .unwrap();

        assert!(go.contains("akeyless-cli-tool"));
        assert!(go.contains(r#"vendorHash = "sha256-govendor";"#));
        assert!(!go.contains("akeyless-rust-agent"));
    }

    #[test]
    fn test_generate_rust_content_for_multi_lang() {
        let matrix = multi_lang_matrix();
        let store = InMemoryStore { matrix };
        let writer = RecordingWriter::new();

        run(Path::new("/fake/matrix.toml"), None, &store, &writer).unwrap();

        let files = writer.files.lock().unwrap();
        let rust = files
            .iter()
            .find(|(p, _)| p.display().to_string().ends_with("builds/rust/default.nix"))
            .map(|(_, c)| c.clone())
            .unwrap();

        assert!(rust.contains("akeyless-rust-agent"));
        assert!(rust.contains(r#"cargoHash = "sha256-rustcargo";"#));
        assert!(!rust.contains("akeyless-cli-tool"));
    }

    #[test]
    fn test_generate_metadata_content_for_multi_lang() {
        let matrix = multi_lang_matrix();
        let store = InMemoryStore { matrix };
        let writer = RecordingWriter::new();

        run(Path::new("/fake/matrix.toml"), None, &store, &writer).unwrap();

        let files = writer.files.lock().unwrap();
        let meta = files
            .iter()
            .find(|(p, _)| p.display().to_string().ends_with("lib/matrix-metadata.nix"))
            .map(|(_, c)| c.clone())
            .unwrap();

        assert!(meta.contains(r#"akeyless-cli-tool = "cli-tool";"#));
        assert!(meta.contains(r#"akeyless-rust-agent = "rust-agent";"#));
        assert!(meta.contains("tier1Packages"));
        assert!(meta.contains("tier2Packages"));
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
    fn test_generate_propagates_store_error() {
        let store = FailingStore;
        let writer = RecordingWriter::new();

        let result = run(Path::new("/fake/matrix.toml"), None, &store, &writer);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("store load failed"));
    }

    struct FailingWriter;

    impl FileWriter for FailingWriter {
        fn write_file(&self, _path: &Path, _content: &str) -> Result<()> {
            anyhow::bail!("disk full")
        }
        fn create_dir_all(&self, _path: &Path) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn test_generate_propagates_write_error() {
        let matrix = Matrix {
            packages: BTreeMap::new(),
        };
        let store = InMemoryStore { matrix };
        let writer = FailingWriter;

        let result = run(Path::new("/fake/matrix.toml"), None, &store, &writer);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("disk full"));
    }

    /// Simulates the certify flow: build marks entries verified, then generate
    /// writes Nix files with the verified hashes. Uses only mock I/O.
    #[test]
    fn test_certify_then_generate_flow() {
        use crate::matrix::{
            Builder, Language, Package, Status, TrackMode, VersionEntry,
        };

        let mut go_ver = VersionEntry {
            rev: "abc123".into(),
            source_hash: Some("sha256-gosrc".into()),
            vendor_hash: Some("sha256-govendor".into()),
            cargo_hash: None,
            npm_deps_hash: None,
            maven_hash: None,
            nuget_deps_hash: None,
            status: Status::Pending,
            verified_at: None,
            hash_aarch64_darwin: None,
            hash_x86_64_darwin: None,
            hash_x86_64_linux: None,
            hash_aarch64_linux: None,
        };
        go_ver.status = Status::Verified;

        let mut go_versions = BTreeMap::new();
        go_versions.insert("1.0.0".into(), go_ver);

        let mut packages = BTreeMap::new();
        packages.insert(
            "akeyless-cli-tool".into(),
            Package {
                owner: "testorg".into(),
                repo: "cli-tool".into(),
                language: Language::Go,
                builder: Builder::MkGoTool,
                tier: 1,
                sub_packages: Some(vec![".".into()]),
                proxy_vendor: None,
                license: None,
                description: "CLI tool".into(),
                homepage: "https://example.com".into(),
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
                versions: go_versions,
            },
        );
        let matrix = Matrix { packages };

        let prev_matrix = Matrix {
            packages: BTreeMap::new(),
        };
        let (added, updated) =
            crate::certification::compute_delta(&prev_matrix, &matrix);
        assert_eq!(added, vec!["akeyless-cli-tool@1.0.0"]);
        assert!(updated.is_empty());

        let fp = crate::certification::compute_fingerprint(&matrix);
        assert_eq!(fp.len(), 64);

        let store = InMemoryStore {
            matrix: matrix.clone(),
        };
        let writer = RecordingWriter::new();
        run(Path::new("/fake/matrix.toml"), None, &store, &writer).unwrap();

        let files = writer.files.lock().unwrap();
        assert_eq!(files.len(), 12);

        let sources = files
            .iter()
            .find(|(p, _)| p.display().to_string().ends_with("lib/sources.nix"))
            .map(|(_, c)| c.clone())
            .unwrap();
        assert!(sources.contains(r#"hash = "sha256-gosrc";"#));

        let go_build = files
            .iter()
            .find(|(p, _)| p.display().to_string().ends_with("builds/go/default.nix"))
            .map(|(_, c)| c.clone())
            .unwrap();
        assert!(go_build.contains(r#"vendorHash = "sha256-govendor";"#));
    }

    struct FailingDirWriter;

    impl FileWriter for FailingDirWriter {
        fn write_file(&self, _path: &Path, _content: &str) -> Result<()> {
            Ok(())
        }
        fn create_dir_all(&self, _path: &Path) -> Result<()> {
            anyhow::bail!("permission denied")
        }
    }

    #[test]
    fn test_generate_propagates_dir_creation_error() {
        let matrix = Matrix {
            packages: BTreeMap::new(),
        };
        let store = InMemoryStore { matrix };
        let writer = FailingDirWriter;

        let result = run(Path::new("/fake/matrix.toml"), None, &store, &writer);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("permission denied"));
    }
}
