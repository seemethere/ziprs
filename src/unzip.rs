use pyo3::exceptions::PyIOError;
use pyo3::prelude::*;
use rayon::prelude::*;
use std::fs::{self};
use std::io::{self, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use zip::ZipArchive;

// Core unzipping logic
pub fn do_unzip_internal(src_path: &Path, dst_path: &Path) -> io::Result<()> {
    if !dst_path.exists() {
        fs::create_dir_all(&dst_path).map_err(|e| {
            io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "Failed to create destination directory '{}': {}",
                    dst_path.display(),
                    e
                ),
            )
        })?;
    }

    let file = fs::File::open(&src_path).map_err(|e| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("Failed to open zip file '{}': {}", src_path.display(), e),
        )
    })?;

    let mut archive = ZipArchive::new(file).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Failed to read zip archive: {}", e),
        )
    })?;

    let mut dirs_to_create: Vec<PathBuf> = Vec::new();
    let mut files_to_extract: Vec<(PathBuf, Vec<u8>, Option<u32>)> = Vec::new();

    for i in 0..archive.len() {
        let mut file_in_zip = archive.by_index(i).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Failed to read file in zip by index {}: {}", i, e),
            )
        })?;

        let outpath = match file_in_zip.enclosed_name() {
            Some(path) => dst_path.join(path),
            None => continue,
        };

        if file_in_zip.name().ends_with('/') {
            dirs_to_create.push(outpath);
        } else {
            let mut content = Vec::new();
            file_in_zip.read_to_end(&mut content).map_err(|e| {
                io::Error::new(
                    io::ErrorKind::Other,
                    format!(
                        "Failed to read file content from zip entry '{}': {}",
                        file_in_zip.name(),
                        e
                    ),
                )
            })?;
            let mode = file_in_zip.unix_mode();
            files_to_extract.push((outpath, content, mode));
        }
    }

    for dir_path in dirs_to_create {
        fs::create_dir_all(&dir_path).map_err(|e| {
            io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "Failed to create directory structure at '{}': {}",
                    dir_path.display(),
                    e
                ),
            )
        })?;
    }

    files_to_extract.par_iter().with_max_len(8).try_for_each(
        |(path, content, mode_opt)| -> io::Result<()> {
            if let Some(p) = path.parent() {
                if !p.exists() {
                    fs::create_dir_all(&p).map_err(|e| {
                        io::Error::new(
                            io::ErrorKind::Other,
                            format!(
                                "Failed to create parent directory for file '{}': {}",
                                path.display(),
                                e
                            ),
                        )
                    })?;
                }
            }

            let mut outfile = fs::File::create(&path).map_err(|e| {
                io::Error::new(
                    io::ErrorKind::Other,
                    format!("Failed to create output file '{}': {}", path.display(), e),
                )
            })?;
            outfile.write_all(&content).map_err(|e| {
                io::Error::new(
                    io::ErrorKind::Other,
                    format!(
                        "Failed to write content to file '{}': {}",
                        path.display(),
                        e
                    ),
                )
            })?;

            #[cfg(unix)]
            if let Some(mode) = mode_opt {
                fs::set_permissions(&path, fs::Permissions::from_mode(*mode)).map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::Other,
                        format!("Failed to set permissions on '{}': {}", path.display(), e),
                    )
                })?;
            }
            Ok(())
        },
    )?;

    Ok(())
}

#[pyfunction]
pub fn unzip_files(src_py: String, dst_py: String) -> PyResult<()> {
    let src_path = PathBuf::from(src_py);
    let dst_path = PathBuf::from(dst_py);

    do_unzip_internal(&src_path, &dst_path).map_err(|e| PyIOError::new_err(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*; // For unzip_files (PyO3 wrapper) and do_unzip_internal
    use crate::zip::zip_files as zip_files_py_wrapper;
    use std::fs::{self};
    use std::io::Read as StdRead;
    use std::os::unix::fs::PermissionsExt as OsUnixPermissionsExt;
    use tempfile::tempdir;

    // Helper to call the internal unzip function for tests that want io::Result
    fn unzip_files_internal_wrapper(src: &Path, dst: &Path) -> io::Result<()> {
        super::do_unzip_internal(src, dst)
    }

    // Helper to call the PyO3 wrapped unzip function
    fn unzip_files_py_wrapper_local(src: String, dst: String) -> PyResult<()> {
        super::unzip_files(src, dst)
    }

    #[test]
    fn test_unzip_files_basic() {
        let original_dir = tempdir().unwrap();
        let zip_file_path = original_dir.path().join("archive.zip");
        let extracted_dir = tempdir().unwrap();

        let file1_path = original_dir.path().join("file1.txt");
        let subdir_path = original_dir.path().join("subdir");
        let subfile_path = subdir_path.join("subfile.txt");

        fs::write(&file1_path, "hello from file1").unwrap();
        fs::create_dir(&subdir_path).unwrap();
        fs::write(&subfile_path, "hello from subfile").unwrap();

        let mut perms_file1 = fs::metadata(&file1_path).unwrap().permissions();
        // Use the explicitly imported trait for set_mode
        OsUnixPermissionsExt::set_mode(&mut perms_file1, 0o644);
        fs::set_permissions(&file1_path, perms_file1).unwrap();

        let mut perms_subfile = fs::metadata(&subfile_path).unwrap().permissions();
        // Use the explicitly imported trait for set_mode
        OsUnixPermissionsExt::set_mode(&mut perms_subfile, 0o755);
        fs::set_permissions(&subfile_path, perms_subfile).unwrap();

        let srcs_to_zip = vec![
            file1_path.to_str().unwrap().to_string(),
            subdir_path.to_str().unwrap().to_string(),
        ];
        // Call the PyO3 wrapper for zipping from the zip module (via lib.rs)
        zip_files_py_wrapper(zip_file_path.to_str().unwrap().to_string(), srcs_to_zip).unwrap();

        // Test the PyO3 wrapper for unzipping
        unzip_files_py_wrapper_local(
            zip_file_path.to_str().unwrap().to_string(),
            extracted_dir.path().to_str().unwrap().to_string(),
        )
        .unwrap();

        let extracted_file1 = extracted_dir.path().join("file1.txt");
        let extracted_subdir = extracted_dir.path().join("subdir");
        let extracted_subfile = extracted_subdir.join("subfile.txt");

        assert!(extracted_file1.exists());
        assert!(extracted_subdir.is_dir());
        assert!(extracted_subfile.exists());

        let mut content_file1 = String::new();
        fs::File::open(&extracted_file1)
            .unwrap()
            .read_to_string(&mut content_file1)
            .unwrap();
        assert_eq!(content_file1, "hello from file1");

        let mut content_subfile = String::new();
        fs::File::open(&extracted_subfile)
            .unwrap()
            .read_to_string(&mut content_subfile)
            .unwrap();
        assert_eq!(content_subfile, "hello from subfile");

        #[cfg(unix)]
        {
            // Use the explicitly imported trait for mode()
            let perms_ext_file1 =
                OsUnixPermissionsExt::mode(&fs::metadata(&extracted_file1).unwrap().permissions());
            assert_eq!(perms_ext_file1 & 0o777, 0o644);

            let perms_ext_subfile = OsUnixPermissionsExt::mode(
                &fs::metadata(&extracted_subfile).unwrap().permissions(),
            );
            assert_eq!(perms_ext_subfile & 0o777, 0o755);
        }

        // Optionally, test internal unzip function
        let extracted_dir_internal = tempdir().unwrap();
        unzip_files_internal_wrapper(&zip_file_path, extracted_dir_internal.path()).unwrap();
        // Add assertions for internal version similar to above
        assert!(extracted_dir_internal.path().join("file1.txt").exists());
    }

    #[test]
    fn test_unzip_to_non_existent_destination() {
        let original_dir = tempdir().unwrap();
        let zip_file_path = original_dir.path().join("archive.zip");
        let extracted_base_dir = tempdir().unwrap();
        let extracted_dir_path = extracted_base_dir.path().join("new_dest_dir");

        let file1_path = original_dir.path().join("dummy.txt");
        fs::write(&file1_path, "dummy content").unwrap();
        let srcs_to_zip = vec![file1_path.to_str().unwrap().to_string()];
        zip_files_py_wrapper(zip_file_path.to_str().unwrap().to_string(), srcs_to_zip).unwrap();

        let result = unzip_files_py_wrapper_local(
            zip_file_path.to_str().unwrap().to_string(),
            extracted_dir_path.to_str().unwrap().to_string(),
        );
        assert!(result.is_ok());
        assert!(extracted_dir_path.exists() && extracted_dir_path.is_dir());
        assert!(extracted_dir_path.join("dummy.txt").exists());
    }

    #[test]
    fn test_unzip_empty_directory() {
        let original_dir = tempdir().unwrap();
        let zip_file_path = original_dir.path().join("archive.zip");
        let extracted_dir = tempdir().unwrap();

        let empty_dir_src = original_dir.path().join("empty_folder");
        fs::create_dir(&empty_dir_src).unwrap();

        let srcs_to_zip = vec![empty_dir_src.to_str().unwrap().to_string()];
        zip_files_py_wrapper(zip_file_path.to_str().unwrap().to_string(), srcs_to_zip).unwrap();

        unzip_files_py_wrapper_local(
            zip_file_path.to_str().unwrap().to_string(),
            extracted_dir.path().to_str().unwrap().to_string(),
        )
        .unwrap();

        let extracted_empty_dir = extracted_dir.path().join("empty_folder");
        assert!(extracted_empty_dir.exists() && extracted_empty_dir.is_dir());
        // Check if it's actually empty
        assert_eq!(fs::read_dir(&extracted_empty_dir).unwrap().count(), 0);
    }
}
