// This module provides a Python extension for zipping files using Rust and the zip crate.
use pyo3::exceptions::PyIOError;
use pyo3::prelude::*;
use rayon::prelude::*;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use zip::{write::FileOptions, ZipWriter};

mod unzip; // Add this line to declare the unzip module

// Type alias for simpler usage of FileOptions with default parameters
type SimpleFileOptions = FileOptions<'static, ()>;

// Zips a list of srcs (files or directories) into a single zip file
#[pyfunction]
fn zip_files(dst: String, srcs: Vec<String>) -> PyResult<()> {
    let mut zip = ZipWriter::new(File::create(&dst).map_err(PyIOError::new_err)?);

    for src in srcs {
        let src_path = PathBuf::from(&src);

        if src_path.is_file() {
            // Add single file with preserved permissions
            let metadata =
                std::fs::metadata(&src_path).map_err(|e| PyIOError::new_err(e.to_string()))?;
            let permissions = metadata.permissions().mode(); // Keep full mode including file type

            add_file_from_path_to_zip_with_permissions(
                &mut zip,
                &src_path,
                src_path.file_name().unwrap().to_str().unwrap(),
                permissions,
            )?;
        } else if src_path.is_dir() {
            let dir_metadata =
                std::fs::metadata(&src_path).map_err(|e| PyIOError::new_err(e.to_string()))?;
            let dir_permissions = dir_metadata.permissions().mode();

            // This is the name for the directory itself in the archive, e.g., "subdir"
            // If src_path is ".", file_name is ".". If src_path is "/", file_name is effectively empty.
            let top_level_dir_name_in_zip = src_path
                .file_name()
                .unwrap_or_default()
                .to_str()
                .unwrap_or("");

            // Add the directory entry itself, e.g., "subdir/"
            // If top_level_dir_name_in_zip is "" (e.g. zipping root /) or "." (zipping current dir),
            // we might not add an explicit entry for "" or "./" itself,
            // but items inside will be correctly pathed relative to zip root.
            if !top_level_dir_name_in_zip.is_empty() && top_level_dir_name_in_zip != "." {
                let proper_dir_name = format!("{}/", top_level_dir_name_in_zip);
                zip.add_directory(
                    proper_dir_name,
                    FileOptions::<()>::default().unix_permissions(dir_permissions),
                )
                .map_err(|e| PyIOError::new_err(e.to_string()))?;
            }
            // Note: If top_level_dir_name_in_zip is ".", an entry for "./" is not explicitly added here,
            // but files like "./file.txt" will be correctly named later.

            let file_entries: Vec<_> = walkdir::WalkDir::new(&src_path)
                .into_iter()
                .filter_map(|e| e.ok())
                .collect();

            if file_entries.is_empty() {
                // Empty directory or only contained the root dir entry
                // If it was an empty named directory (e.g. "empty_dir"), it should have been added above.
                // If it was "." and empty, nothing more to do.
                continue;
            }

            let (sender, receiver) = mpsc::channel::<(String, Vec<u8>, u32)>();
            let src_path_clone = src_path.clone();
            // Capture top_level_dir_name_in_zip for use in the closure
            let top_level_dir_name_in_zip_clone = top_level_dir_name_in_zip.to_string();

            let result: Result<(), PyErr> =
                file_entries
                    .par_iter()
                    .with_max_len(8)
                    .try_for_each(|entry| -> PyResult<()> {
                        let path = entry.path();
                        let rel_path = match path.strip_prefix(&src_path_clone) {
                            Ok(p) => p,
                            Err(_) => return Ok(()), // Should not happen if walkdir is correct
                        };
                        let item_rel_to_src_path_str = rel_path.to_str().unwrap_or("").to_string();

                        if item_rel_to_src_path_str.is_empty() {
                            return Ok(()); // Skip the entry for the source directory itself
                        }

                        let archive_path_for_item = if top_level_dir_name_in_zip_clone.is_empty()
                            || top_level_dir_name_in_zip_clone == "."
                        {
                            item_rel_to_src_path_str.clone()
                        } else {
                            format!(
                                "{}/{}",
                                top_level_dir_name_in_zip_clone, item_rel_to_src_path_str
                            )
                        };

                        let metadata = std::fs::metadata(path)
                            .map_err(|e| PyIOError::new_err(e.to_string()))?;
                        let permissions = metadata.permissions().mode();

                        if path.is_dir() {
                            // Directories are collected and added sequentially later to ensure correct order and permissions.
                            // The `dir_entry_name` calculation here was unused.
                            Ok(())
                        } else if path.is_file() {
                            let content = std::fs::read(path)
                                .map_err(|e| PyIOError::new_err(e.to_string()))?;
                            sender
                                .send((archive_path_for_item, content, permissions))
                                .map_err(|e| {
                                    PyIOError::new_err(format!("Channel send error: {}", e))
                                })?;
                            Ok(())
                        } else {
                            Ok(())
                        }
                    });

            result?;

            let mut sub_dirs_to_add: Vec<(String, u32)> = Vec::new();
            // Recapture top_level_dir_name_in_zip for this loop as well
            let top_level_dir_name_in_zip_for_subdir_pass = top_level_dir_name_in_zip.to_string();
            for entry in walkdir::WalkDir::new(&src_path)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let path = entry.path();
                if path.is_dir() {
                    let rel_path = match path.strip_prefix(&src_path) {
                        Ok(p) => p,
                        Err(_) => continue,
                    };
                    let item_rel_to_src_path_str = rel_path.to_str().unwrap_or("").to_string();

                    if !item_rel_to_src_path_str.is_empty() {
                        let metadata =
                            fs::metadata(path).map_err(|e| PyIOError::new_err(e.to_string()))?;
                        let permissions = metadata.permissions().mode();

                        let mut archive_path_for_subdir =
                            if top_level_dir_name_in_zip_for_subdir_pass.is_empty()
                                || top_level_dir_name_in_zip_for_subdir_pass == "."
                            {
                                item_rel_to_src_path_str.clone()
                            } else {
                                format!(
                                    "{}/{}",
                                    top_level_dir_name_in_zip_for_subdir_pass,
                                    item_rel_to_src_path_str
                                )
                            };

                        if !archive_path_for_subdir.ends_with('/') {
                            archive_path_for_subdir.push('/');
                        }
                        // Avoid adding the top-level directory again if it's effectively the same path
                        if top_level_dir_name_in_zip_for_subdir_pass != "."
                            && archive_path_for_subdir
                                == format!("{}/", top_level_dir_name_in_zip_for_subdir_pass)
                        {
                            // This case is when item_rel_to_src_path_str was empty and top_level_dir_name_in_zip_for_subdir_pass was not "." or empty.
                            // It's already handled by the initial add_directory or skipped if "." / empty.
                            // The item_rel_to_src_path_str.is_empty() check above should prevent this.
                        } else {
                            sub_dirs_to_add.push((archive_path_for_subdir, permissions));
                        }
                    }
                }
            }

            drop(sender);

            // Sort directories by path to ensure parent directories are created before children, if not already.
            // This is mostly a safeguard; add_directory should handle intermediate directory creation.
            sub_dirs_to_add.sort_by(|a, b| a.0.cmp(&b.0));
            // Deduplicate, as walkdir might yield a dir and then its contents, leading to multiple adds if not careful.
            sub_dirs_to_add.dedup_by(|a, b| a.0 == b.0);

            for (dir_path_in_zip, perms) in sub_dirs_to_add {
                // Skip adding the root dir ("./" or "/") if that's what dir_path_in_zip evaluates to and top_level_dir_name_in_zip implies it
                if (top_level_dir_name_in_zip == "." && dir_path_in_zip == "./")
                    || (top_level_dir_name_in_zip.is_empty() && dir_path_in_zip == "/")
                {
                    continue;
                }
                // Also skip if it's the main directory we already added (e.g. "subdir/")
                if !top_level_dir_name_in_zip.is_empty()
                    && top_level_dir_name_in_zip != "."
                    && dir_path_in_zip == format!("{}/", top_level_dir_name_in_zip)
                {
                    continue;
                }
                zip.add_directory(
                    &dir_path_in_zip,
                    FileOptions::<()>::default().unix_permissions(perms),
                )
                .map_err(|e| PyIOError::new_err(e.to_string()))?;
            }

            for (archive_path, content, permissions) in receiver {
                add_file_to_zip_with_permissions(&mut zip, &archive_path, permissions, content)?;
            }
        }
    }

    // Finalize the zip archive to ensure all metadata is written
    zip.finish()
        .map_err(|e| PyIOError::new_err(e.to_string()))?;
    Ok(())
}

// Helper function to add a file to the zip archive with permissions
fn add_file_to_zip_with_permissions<W: std::io::Write + std::io::Seek>(
    zip: &mut ZipWriter<W>,
    archive_path: &str,
    permissions: u32,
    content: Vec<u8>,
) -> PyResult<()> {
    let file_options = SimpleFileOptions::default().unix_permissions(permissions);

    zip.start_file(archive_path, file_options)
        .map_err(|e| PyIOError::new_err(e.to_string()))?;

    zip.write_all(&content)
        .map_err(|e| PyIOError::new_err(e.to_string()))?;

    Ok(())
}

// Helper function to add a file from filesystem to zip with permissions
fn add_file_from_path_to_zip_with_permissions<W: std::io::Write + std::io::Seek>(
    zip: &mut ZipWriter<W>,
    file_path: &Path,
    archive_path: &str,
    permissions: u32,
) -> PyResult<()> {
    // Read the entire file content first
    let content = std::fs::read(file_path).map_err(|e| PyIOError::new_err(e.to_string()))?;
    add_file_to_zip_with_permissions(zip, archive_path, permissions, content)
}

/// A Python module implemented in Rust.
#[pymodule]
fn ziprs(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Register the zip_files function as a Python-callable function
    m.add_function(wrap_pyfunction!(zip_files, m)?)?;
    // Register the unzip_files function as a Python-callable function
    m.add_function(wrap_pyfunction!(unzip::unzip_files, m)?)?;
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

    #[test]
    fn test_zip_preserves_permissions() {
        use std::os::unix::fs::PermissionsExt;

        // Create a temporary directory
        let dir = tempdir().unwrap();
        let executable_file = dir.path().join("executable.sh");
        let readonly_file = dir.path().join("readonly.txt");
        let subdir_path = dir.path().join("subdir");
        let subfile_path = subdir_path.join("subfile.txt");
        let zip_path = dir.path().join("permissions_test.zip");

        // Create files with specific permissions
        fs::write(&executable_file, b"#!/bin/bash\necho 'hello'").unwrap();
        fs::write(&readonly_file, b"read only content").unwrap();

        // Create subdirectory and file
        fs::create_dir(&subdir_path).unwrap();
        fs::write(&subfile_path, b"sub file content").unwrap();

        // Set specific permissions
        let mut perms = fs::metadata(&executable_file).unwrap().permissions();
        perms.set_mode(0o755); // rwxr-xr-x (executable)
        fs::set_permissions(&executable_file, perms).unwrap();

        let mut perms = fs::metadata(&readonly_file).unwrap().permissions();
        perms.set_mode(0o444); // r--r--r-- (readonly)
        fs::set_permissions(&readonly_file, perms).unwrap();

        let mut perms = fs::metadata(&subfile_path).unwrap().permissions();
        perms.set_mode(0o600); // rw------- (owner only)
        fs::set_permissions(&subfile_path, perms).unwrap();

        // Call the zip_files function
        let srcs = vec![
            executable_file.to_str().unwrap().to_string(),
            readonly_file.to_str().unwrap().to_string(),
            subdir_path.to_str().unwrap().to_string(),
        ];
        let result = zip_files(zip_path.to_str().unwrap().to_string(), srcs);
        assert!(result.is_ok());

        // Check that the zip file exists
        assert!(fs::metadata(&zip_path).unwrap().is_file());

        // Open the zip and verify permissions are preserved
        let zip_file = File::open(&zip_path).unwrap();
        let mut archive = zip::ZipArchive::new(zip_file).unwrap();

        for i in 0..archive.len() {
            let file = archive.by_index(i).unwrap();
            let name = file.name();
            let permissions = file.unix_mode().unwrap_or(0);

            match name {
                "executable.sh" => {
                    // Should be executable (0o755)
                    assert_eq!(
                        permissions & 0o777,
                        0o755,
                        "executable.sh should have 755 permissions, got {:o}",
                        permissions & 0o777
                    );
                }
                "readonly.txt" => {
                    // Should be readonly (0o444)
                    assert_eq!(
                        permissions & 0o777,
                        0o444,
                        "readonly.txt should have 444 permissions, got {:o}",
                        permissions & 0o777
                    );
                }
                "subfile.txt" => {
                    // Should be owner only (0o600)
                    assert_eq!(
                        permissions & 0o777,
                        0o600,
                        "subfile.txt should have 600 permissions, got {:o}",
                        permissions & 0o777
                    );
                }
                _ => {} // Ignore other files/directories
            }
        }
    }
}
