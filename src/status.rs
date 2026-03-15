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
