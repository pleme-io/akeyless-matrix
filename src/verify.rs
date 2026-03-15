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
        if pkg.builder == Builder::None || pkg.builder == Builder::Fetchurl {
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
        Language::Java => return Ok(()), // source-only, nothing to build
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
}
