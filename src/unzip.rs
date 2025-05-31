use pyo3::exceptions::PyIOError;
use pyo3::prelude::*;
use rayon::prelude::*;
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use zip::ZipArchive;

#[pyfunction]
pub fn unzip_files(src: String, dst: String) -> PyResult<()> {
    let src_path = Path::new(&src);
    let dst_path = Path::new(&dst);

    // Ensure destination directory exists
    if !dst_path.exists() {
        fs::create_dir_all(&dst_path).map_err(|e| {
            PyIOError::new_err(format!("Failed to create destination directory: {}", e))
        })?;
    }

    let file = fs::File::open(&src_path)
        .map_err(|e| PyIOError::new_err(format!("Failed to open zip file: {}", e)))?;
    let mut archive = ZipArchive::new(file)
        .map_err(|e| PyIOError::new_err(format!("Failed to read zip archive: {}", e)))?;

    let mut dirs_to_create: Vec<PathBuf> = Vec::new();
    let mut files_to_extract: Vec<(PathBuf, Vec<u8>, Option<u32>)> = Vec::new();

    // Iterate over each file and directory in the zip archive.
    for i in 0..archive.len() {
        let mut file_in_zip = archive
            .by_index(i)
            .map_err(|e| PyIOError::new_err(format!("Failed to read file in zip: {}", e)))?;

        // Construct the full output path for the current item.
        // `enclosed_name` ensures that the path is safe and does not traverse outside the destination.
        let outpath = match file_in_zip.enclosed_name() {
            Some(path) => dst_path.join(path),
            None => continue, // Skip potentially malicious or invalid paths.
        };

        // Check if the entry is a directory.
        if file_in_zip.name().ends_with('/') {
            // If it's a directory, add it to a list for later creation.
            dirs_to_create.push(outpath);
        } else {
            // If it's a file, read its content.
            let mut content = Vec::new();
            file_in_zip.read_to_end(&mut content).map_err(|e| {
                PyIOError::new_err(format!("Failed to read file content from zip: {}", e))
            })?;

            // Get the Unix mode (permissions) of the file, if available.
            let mode = file_in_zip.unix_mode();
            // Add the file's path, content, and mode to a list for later extraction.
            files_to_extract.push((outpath, content, mode));
        }
    }

    // Create all directories first. `create_dir_all` is idempotent.
    for dir_path in dirs_to_create {
        fs::create_dir_all(&dir_path).map_err(|e| {
            PyIOError::new_err(format!("Failed to create directory structure: {}", e))
        })?;
    }

    // Extract files in parallel
    files_to_extract.par_iter().with_max_len(8).try_for_each(
        |(path, content, mode_opt)| -> PyResult<()> {
            // Ensure parent directory exists (for files whose parent dirs might not be explicit in zip)
            if let Some(p) = path.parent() {
                if !p.exists() {
                    // Check to avoid redundant calls if already created
                    fs::create_dir_all(&p).map_err(|e| {
                        PyIOError::new_err(format!(
                            "Failed to create parent directory for file {}: {}",
                            path.display(),
                            e
                        ))
                    })?;
                }
            }

            let mut outfile = fs::File::create(&path).map_err(|e| {
                PyIOError::new_err(format!(
                    "Failed to create output file {}: {}",
                    path.display(),
                    e
                ))
            })?;
            outfile.write_all(&content).map_err(|e| {
                PyIOError::new_err(format!(
                    "Failed to write content to file {}: {}",
                    path.display(),
                    e
                ))
            })?;

            #[cfg(unix)]
            if let Some(mode) = mode_opt {
                fs::set_permissions(&path, fs::Permissions::from_mode(*mode)).map_err(|e| {
                    PyIOError::new_err(format!(
                        "Failed to set permissions on {}: {}",
                        path.display(),
                        e
                    ))
                })?;
            }
            Ok(())
        },
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zip_files; // Import the zip_files function from lib.rs
    use std::fs;
    use std::io::Read;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    #[test]
    fn test_unzip_files_basic() {
        let original_dir = tempdir().unwrap();
        let zip_file_path = original_dir.path().join("archive.zip");
        let extracted_dir = tempdir().unwrap();

        // 1. Create source files and directories
        let file1_path = original_dir.path().join("file1.txt");
        let subdir_path = original_dir.path().join("subdir");
        let subfile_path = subdir_path.join("subfile.txt");

        fs::write(&file1_path, "hello from file1").unwrap();
        fs::create_dir(&subdir_path).unwrap();
        fs::write(&subfile_path, "hello from subfile").unwrap();

        // Set specific permissions for testing
        let mut perms_file1 = fs::metadata(&file1_path).unwrap().permissions();
        perms_file1.set_mode(0o644); // rw-r--r--
        fs::set_permissions(&file1_path, perms_file1).unwrap();

        let mut perms_subfile = fs::metadata(&subfile_path).unwrap().permissions();
        perms_subfile.set_mode(0o755); // rwxr-xr-x
        fs::set_permissions(&subfile_path, perms_subfile).unwrap();

        // 2. Zip these files using the zip_files function from lib.rs
        let srcs_to_zip = vec![
            file1_path.to_str().unwrap().to_string(),
            subdir_path.to_str().unwrap().to_string(),
        ];
        zip_files(zip_file_path.to_str().unwrap().to_string(), srcs_to_zip).unwrap();

        // 3. Unzip the archive using unzip_files
        unzip_files(
            zip_file_path.to_str().unwrap().to_string(),
            extracted_dir.path().to_str().unwrap().to_string(),
        )
        .unwrap();

        // 4. Verify extracted content and structure
        let extracted_file1 = extracted_dir.path().join("file1.txt");
        let extracted_subdir = extracted_dir.path().join("subdir");
        let extracted_subfile = extracted_subdir.join("subfile.txt");

        assert!(extracted_file1.exists(), "file1.txt should be extracted");
        assert!(
            extracted_subdir.is_dir(),
            "subdir should be extracted as a directory"
        );
        assert!(
            extracted_subfile.exists(),
            "subdir/subfile.txt should be extracted"
        );

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

        // 5. Verify permissions
        #[cfg(unix)]
        {
            let perms_ext_file1 = fs::metadata(&extracted_file1).unwrap().permissions().mode();
            assert_eq!(
                perms_ext_file1 & 0o777,
                0o644,
                "Permissions for file1.txt mismatch. Expected {:o}, got {:o}",
                0o644,
                perms_ext_file1 & 0o777
            );

            let perms_ext_subfile = fs::metadata(&extracted_subfile)
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(
                perms_ext_subfile & 0o777,
                0o755,
                "Permissions for subfile.txt mismatch. Expected {:o}, got {:o}",
                0o755,
                perms_ext_subfile & 0o777
            );
        }
    }

    #[test]
    fn test_unzip_to_non_existent_destination() {
        let original_dir = tempdir().unwrap();
        let zip_file_path = original_dir.path().join("archive.zip");
        let extracted_base_dir = tempdir().unwrap();
        let extracted_dir_path = extracted_base_dir.path().join("new_dest_dir"); // This directory does not exist yet

        // Create a dummy file to zip
        let file1_path = original_dir.path().join("dummy.txt");
        fs::write(&file1_path, "dummy content").unwrap();
        let srcs_to_zip = vec![file1_path.to_str().unwrap().to_string()];
        zip_files(zip_file_path.to_str().unwrap().to_string(), srcs_to_zip).unwrap();

        // Attempt to unzip to a non-existent directory
        let result = unzip_files(
            zip_file_path.to_str().unwrap().to_string(),
            extracted_dir_path.to_str().unwrap().to_string(),
        );
        assert!(
            result.is_ok(),
            "Unzipping to a non-existent path should succeed by creating the directory."
        );

        // Verify the directory and file were created
        assert!(
            extracted_dir_path.exists(),
            "Destination directory should have been created."
        );
        assert!(
            extracted_dir_path.is_dir(),
            "Destination path should be a directory."
        );
        let extracted_file = extracted_dir_path.join("dummy.txt");
        assert!(
            extracted_file.exists(),
            "dummy.txt should be extracted into the new directory."
        );
    }

    #[test]
    fn test_unzip_empty_directory() {
        let original_dir = tempdir().unwrap();
        let zip_file_path = original_dir.path().join("archive_with_empty_dir.zip");
        let extracted_dir = tempdir().unwrap();

        // Create an empty directory
        let empty_subdir_path = original_dir.path().join("empty_subdir");
        fs::create_dir(&empty_subdir_path).unwrap();

        // Zip this empty directory
        let srcs_to_zip = vec![empty_subdir_path.to_str().unwrap().to_string()];
        zip_files(zip_file_path.to_str().unwrap().to_string(), srcs_to_zip).unwrap();

        // Unzip
        unzip_files(
            zip_file_path.to_str().unwrap().to_string(),
            extracted_dir.path().to_str().unwrap().to_string(),
        )
        .unwrap();

        // Verify the empty directory was created
        let extracted_empty_subdir = extracted_dir.path().join("empty_subdir");
        assert!(
            extracted_empty_subdir.exists(),
            "Empty subdirectory should be extracted."
        );
        assert!(
            extracted_empty_subdir.is_dir(),
            "Extracted path for empty subdirectory should be a directory."
        );
        assert!(
            fs::read_dir(&extracted_empty_subdir)
                .unwrap()
                .next()
                .is_none(),
            "Extracted empty subdirectory should be empty."
        );
    }
}
