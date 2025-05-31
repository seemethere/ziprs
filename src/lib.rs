use pyo3::prelude::*;

pub mod unzip;
pub mod zip;

pub use unzip::unzip_files_pywrapper;
pub use zip::zip_files_pywrapper;

#[pymodule]
fn ziprs(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(zip_files_pywrapper, m)?)?;
    m.add_function(wrap_pyfunction!(unzip_files_pywrapper, m)?)?;
    Ok(())
}
