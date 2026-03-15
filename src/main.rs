use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod add;
mod build;
mod certification;
mod display;
mod generate;
mod hash;
mod matrix;
mod nix;
mod nixexpr;
mod runner;
mod status;
mod storage;
mod verify;

use crate::runner::SystemRunner;
use crate::storage::{FsFileWriter, FsMatrixStore, MatrixStore};

#[derive(Parser)]
#[command(
    name = "akeyless-matrix",
    version,
    about = "Version matrix manager for Akeyless Nix packages"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Path to matrix.toml
    #[arg(long, global = true)]
    matrix: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    /// Print matrix status table
    Status,

    /// Add a pending version entry
    Add {
        /// Package key (e.g., akeyless-cli)
        #[arg(long)]
        package: String,
        /// Version string
        #[arg(long)]
        version: String,
        /// Git revision (commit SHA)
        #[arg(long)]
        rev: String,
    },

    /// Build pending entries (prefetch + hash extraction)
    Build {
        /// Only build a specific package
        #[arg(long)]
        package: Option<String>,
    },

    /// Generate Nix files from matrix
    Generate {
        /// Output directory (default: directory containing matrix.toml)
        #[arg(long)]
        dir: Option<PathBuf>,
    },

    /// Verify all entries by building
    Verify,

    /// Build pending entries + generate Nix files in one step
    Certify {
        /// Only certify a specific package
        #[arg(long)]
        package: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let matrix_path = cli.matrix.unwrap_or_else(default_matrix_path);
    let runner = SystemRunner;
    let store = FsMatrixStore;
    let writer = FsFileWriter;

    match cli.command {
        Commands::Status => status::run(&matrix_path, &store)?,
        Commands::Add {
            package,
            version,
            rev,
        } => add::run(&matrix_path, &package, &version, &rev, &store)?,
        Commands::Build { package } => {
            build::run(&matrix_path, package.as_deref(), &runner, &store).await?;
        }
        Commands::Generate { dir } => {
            generate::run(&matrix_path, dir.as_deref(), &store, &writer)?;
        }
        Commands::Verify => verify::run(&matrix_path, &runner, &store).await?,
        Commands::Certify { package } => {
            // Snapshot the matrix before building (for delta computation)
            let prev_matrix = store.load(&matrix_path)?;

            build::run(&matrix_path, package.as_deref(), &runner, &store).await?;
            generate::run(&matrix_path, None, &store, &writer)?;

            // Record certification with fingerprint and delta
            let current_matrix = store.load(&matrix_path)?;
            let matrix_dir = matrix_path.parent().unwrap_or(std::path::Path::new("."));
            let cert = certification::record(matrix_dir, &prev_matrix, &current_matrix)?;
            display::print_certification(&cert);
        }
    }

    Ok(())
}

fn default_matrix_path() -> PathBuf {
    PathBuf::from("matrix.toml")
}
