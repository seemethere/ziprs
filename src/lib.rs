// This module provides a Python extension for zipping files using Rust and the zip crate.
use pyo3::exceptions::PyIOError;
use pyo3::prelude::*;
use std::fs::File;
use std::path::{Path, PathBuf};
use zip::{write::FileOptions, ZipWriter};

// Type alias for simpler usage of FileOptions with default parameters
type SimpleFileOptions = FileOptions<'static, ()>;

// Zips a list of srcs (files or directories) into a single zip file
#[pyfunction]
fn zip_files(dst: String, srcs: Vec<String>) -> PyResult<()> {
    let mut zip = ZipWriter::new(File::create(&dst).map_err(PyIOError::new_err)?);

    for src in srcs {
        let src_path = PathBuf::from(&src);

        if src_path.is_file() {
            // Add single file
            add_file_to_zip(
                &mut zip,
                &src_path,
                src_path.file_name().unwrap().to_str().unwrap(),
            )?;
        } else if src_path.is_dir() {
            // Recursively add all files in the directory
            for entry in walkdir::WalkDir::new(&src_path)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let path = entry.path();
                if path.is_file() {
                    // Compute the path relative to the directory root
                    let rel_path = path.strip_prefix(&src_path).unwrap();
                    let rel_path_str = rel_path.to_str().unwrap();
                    add_file_to_zip(&mut zip, path, rel_path_str)?;
                }
            }
        }
    }
    Ok(())
}

// Helper function to add a file to the zip archive
fn add_file_to_zip<W: std::io::Write + std::io::Seek>(
    zip: &mut ZipWriter<W>,
    file_path: &Path,
    archive_path: &str,
) -> PyResult<()> {
    zip.start_file(archive_path, SimpleFileOptions::default())
        .map_err(|e| PyIOError::new_err(e.to_string()))?;
    std::io::copy(
        &mut File::open(file_path).map_err(|e| PyIOError::new_err(e.to_string()))?,
        zip,
    )
    .map_err(|e| PyIOError::new_err(e.to_string()))?;
    Ok(())
}

/// A Python module implemented in Rust.
#[pymodule]
fn ziprs(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Register the zip_files function as a Python-callable function
    m.add_function(wrap_pyfunction!(zip_files, m)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_zip_files_creates_zip() {
        // Create a temporary directory
        let dir = tempdir().unwrap();
        let file1_path = dir.path().join("file1.txt");
        let file2_path = dir.path().join("file2.txt");
        let zip_path = dir.path().join("archive.zip");

        // Write some content to the files
        fs::write(&file1_path, b"hello").unwrap();
        fs::write(&file2_path, b"world").unwrap();

        // Call the zip_files function
        let srcs = vec![
            file1_path.to_str().unwrap().to_string(),
            file2_path.to_str().unwrap().to_string(),
        ];
        let result = zip_files(zip_path.to_str().unwrap().to_string(), srcs);
        assert!(result.is_ok());

        // Check that the zip file exists and is not empty
        let metadata = fs::metadata(&zip_path).unwrap();
        assert!(metadata.is_file());
        assert!(metadata.len() > 0);
    }

    #[test]
    fn test_zip_files_and_directories() {
        // Create a temporary directory
        let dir = tempdir().unwrap();
        let file1_path = dir.path().join("file1.txt");
        let file2_path = dir.path().join("file2.txt");
        let subdir_path = dir.path().join("subdir");
        let subfile_path = subdir_path.join("subfile.txt");
        let zip_path = dir.path().join("archive.zip");

        // Write some content to the files
        fs::write(&file1_path, b"hello").unwrap();
        fs::write(&file2_path, b"world").unwrap();
        fs::create_dir(&subdir_path).unwrap();
        fs::write(&subfile_path, b"subdir file").unwrap();

        // Call the zip_files function with both files and the directory
        let srcs = vec![
            file1_path.to_str().unwrap().to_string(),
            subdir_path.to_str().unwrap().to_string(),
        ];
        let result = zip_files(zip_path.to_str().unwrap().to_string(), srcs);
        assert!(result.is_ok());

        // Check that the zip file exists and is not empty
        let metadata = fs::metadata(&zip_path).unwrap();
        assert!(metadata.is_file());
        assert!(metadata.len() > 0);

        // Open the zip and check the contents
        let zip_file = File::open(&zip_path).unwrap();
        let mut archive = zip::ZipArchive::new(zip_file).unwrap();
        let mut names = vec![];
        for i in 0..archive.len() {
            let file = archive.by_index(i).unwrap();
            names.push(file.name().to_string());
        }
        // file1.txt should be present
        assert!(names.contains(&"file1.txt".to_string()));
        // subfile.txt should be present as subfile.txt or subdir/subfile.txt depending on how it's zipped
        assert!(
            names.contains(&"subfile.txt".to_string())
                || names.contains(&"subdir/subfile.txt".to_string())
        );
    }
}
