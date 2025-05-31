// This module provides a Python extension for zipping files using Rust and the zip crate.
use pyo3::prelude::*;
// Unused imports PyIOError, File, Write, PermissionsExt, Path, PathBuf, fs were removed.

mod unzip;
mod zip; // Declare the new zip module

pub use unzip::unzip_files;
pub use zip::zip_files; // Publicly re-export zip_files

// The zip_files function, its helpers, and SimpleFileOptions alias have been moved to src/zip.rs

/// A Python module implemented in Rust.
#[pymodule]
fn ziprs(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Register the zip_files function as a Python-callable function
    m.add_function(wrap_pyfunction!(zip_files, m)?)?; // This will now refer to the re-exported zip_files
                                                      // Register the unzip_files function as a Python-callable function
    m.add_function(wrap_pyfunction!(unzip_files, m)?)?; // This will now refer to the re-exported unzip_files
    Ok(())
}

// Test module has been moved to src/zip.rs and src/unzip.rs respectively
