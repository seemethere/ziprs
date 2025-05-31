use clap::ValueEnum;
use pyo3::exceptions::PyIOError;
use pyo3::prelude::*;
use rayon::prelude::*;
use std::fs::{self, File};
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use zip::{write::FileOptions, CompressionMethod as ZipCompressionMethod, ZipWriter};

// Type alias for simpler usage of FileOptions with default parameters
type SimpleFileOptions = FileOptions<'static, ()>;

#[derive(Clone, Copy, Debug, ValueEnum, Default)]
pub enum Compression {
    Stored,
    #[default]
    Deflate,
    Bzip2,
    Zstd,
}

impl Compression {
    fn to_zip_compression_method(self) -> ZipCompressionMethod {
        match self {
            Compression::Stored => ZipCompressionMethod::Stored,
            Compression::Deflate => ZipCompressionMethod::Deflated,
            Compression::Bzip2 => ZipCompressionMethod::Bzip2,
            Compression::Zstd => ZipCompressionMethod::Zstd,
        }
    }

    fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "stored" => Ok(Compression::Stored),
            "deflate" | "deflated" => Ok(Compression::Deflate),
            "bzip2" => Ok(Compression::Bzip2),
            "zstd" => Ok(Compression::Zstd),
            _ => Err(format!("Unsupported compression method: {}", s)),
        }
    }
}

// Core zipping logic, callable from both CLI and Python wrapper
pub fn zip_files(dst: &Path, srcs: &[PathBuf], compression: Compression) -> io::Result<()> {
    let file = File::create(dst)?;
    let mut zip = ZipWriter::new(file);
    let compression_method = compression.to_zip_compression_method();

    for src_path in srcs {
        if src_path.is_file() {
            let metadata = fs::metadata(src_path)?;
            let permissions = metadata.permissions().mode();
            let file_name_in_archive = src_path
                .file_name()
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "Source path has no filename")
                })?
                .to_str()
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "Filename is not valid UTF-8")
                })?;

            let content = fs::read(src_path)?;
            add_file_to_zip_with_permissions(
                &mut zip,
                file_name_in_archive,
                permissions,
                content,
                compression_method,
            )?;
        } else if src_path.is_dir() {
            let dir_metadata = fs::metadata(src_path)?;
            let dir_permissions = dir_metadata.permissions().mode();

            let top_level_dir_name_in_zip = src_path
                .file_name()
                .unwrap_or_default() // . (current dir) or actual name
                .to_str()
                .unwrap_or(""); // Should be valid UTF-8

            // If zipping a directory, and it's not the current directory ("."),
            // create an explicit directory entry in the zip for this top-level directory.
            if !top_level_dir_name_in_zip.is_empty() && top_level_dir_name_in_zip != "." {
                let proper_dir_name = format!("{}/", top_level_dir_name_in_zip);
                zip.add_directory(
                    proper_dir_name,
                    SimpleFileOptions::default()
                        .unix_permissions(dir_permissions)
                        .compression_method(compression_method), // Apply to directory entry options as well
                )?;
            }

            // Collect all file entries first to enable parallel processing.
            let file_entries: Vec<_> = walkdir::WalkDir::new(src_path)
                .into_iter()
                .filter_map(|e| e.ok())
                .collect();

            if file_entries.is_empty() {
                continue;
            }

            // Parallel processing part needs careful error handling conversion
            let (sender, receiver) = mpsc::channel::<(String, Vec<u8>, u32)>();
            let src_path_clone = src_path.clone();
            let top_level_dir_name_in_zip_clone = top_level_dir_name_in_zip.to_string();
            let current_compression_method = compression_method; // Capture for parallel closure

            // Rayon parallel iteration: Read file contents and gather metadata.
            // Sends data (archive path, content, permissions) to a channel for sequential writing to the zip.
            // This avoids holding the ZipWriter mutex for the entire file reading duration.
            let result: Result<(), io::Error> = file_entries
                .par_iter()
                .with_max_len(8)
                .try_for_each(|entry| -> io::Result<()> {
                    let path = entry.path();
                    let rel_path = match path.strip_prefix(&src_path_clone) {
                        Ok(p) => p,
                        Err(_) => return Ok(()), // Should not happen
                    };
                    let item_rel_to_src_path_str = rel_path.to_str().unwrap_or("").to_string();

                    if item_rel_to_src_path_str.is_empty() {
                        return Ok(());
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

                    let metadata = fs::metadata(path)?;
                    let permissions = metadata.permissions().mode();

                    if path.is_dir() {
                        // Defer directory creation
                        Ok(())
                    } else if path.is_file() {
                        let content = fs::read(path)?;
                        sender
                            .send((archive_path_for_item, content, permissions))
                            .map_err(|e| io::Error::other(format!("Channel send error: {}", e)))?;
                        Ok(())
                    } else {
                        Ok(())
                    }
                });
            result?; // Propagate potential error from parallel processing
            drop(sender); // Close sender before collecting from receiver; signals receiver that no more messages are coming.

            // After processing files, explicitly create all directory entries in the zip.
            // This ensures directories are listed even if they are empty or processed after their files.
            let mut sub_dirs_to_add: Vec<(String, u32)> = Vec::new();
            let top_level_dir_name_in_zip_for_subdir_pass = top_level_dir_name_in_zip.to_string();

            for entry in walkdir::WalkDir::new(src_path)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let path = entry.path();
                if path.is_dir() {
                    let rel_path = match path.strip_prefix(src_path) {
                        Ok(p) => p,
                        Err(_) => continue,
                    };
                    let item_rel_to_src_path_str = rel_path.to_str().unwrap_or("").to_string();

                    if !item_rel_to_src_path_str.is_empty() {
                        let metadata = fs::metadata(path)?;
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
                        if top_level_dir_name_in_zip_for_subdir_pass != "."
                            && archive_path_for_subdir
                                == format!("{}/", top_level_dir_name_in_zip_for_subdir_pass)
                        {
                            // Already handled
                        } else {
                            sub_dirs_to_add.push((archive_path_for_subdir, permissions));
                        }
                    }
                }
            }

            // Sort and deduplicate directory paths to ensure correct order and avoid duplicate entries.
            sub_dirs_to_add.sort_by(|a, b| a.0.cmp(&b.0));
            sub_dirs_to_add.dedup_by(|a, b| a.0 == b.0);

            for (dir_path_in_zip, perms) in sub_dirs_to_add {
                // Skip adding the current directory ("." or "") or the top-level directory itself if already handled.
                if (top_level_dir_name_in_zip == "." && dir_path_in_zip == "./")
                    || (top_level_dir_name_in_zip.is_empty() && dir_path_in_zip == "/")
                {
                    continue;
                }
                if !top_level_dir_name_in_zip.is_empty()
                    && top_level_dir_name_in_zip != "."
                    && dir_path_in_zip == format!("{}/", top_level_dir_name_in_zip)
                {
                    continue;
                }
                zip.add_directory(
                    &dir_path_in_zip,
                    SimpleFileOptions::default()
                        .unix_permissions(perms)
                        .compression_method(current_compression_method),
                )?;
            }

            // Now, write all file contents (received from parallel processing) to the zip archive.
            for (archive_path, content, permissions) in receiver {
                add_file_to_zip_with_permissions(
                    &mut zip,
                    &archive_path,
                    permissions,
                    content,
                    current_compression_method,
                )?;
            }
        }
    }
    zip.finish()?;
    Ok(())
}

// PyO3 wrapper function
#[pyfunction]
#[pyo3(name = "zip_files", signature = (dst_py, srcs_py, compression_method_py = None))]
pub fn zip_files_pywrapper(
    dst_py: String,
    srcs_py: Vec<String>,
    compression_method_py: Option<String>,
) -> PyResult<()> {
    let dst_path = PathBuf::from(dst_py);
    let src_paths: Vec<PathBuf> = srcs_py.into_iter().map(PathBuf::from).collect();

    let compression = match compression_method_py {
        Some(method_str) => Compression::from_str(&method_str)
            .map_err(|e| PyIOError::new_err(format!("Invalid compression method: {}", e)))?,
        None => Compression::default(),
    };

    zip_files(&dst_path, &src_paths, compression).map_err(|e| PyIOError::new_err(e.to_string()))
}

// Helper function to add a file to the zip archive with permissions
// Changed to return io::Result
fn add_file_to_zip_with_permissions<W: std::io::Write + std::io::Seek>(
    zip: &mut ZipWriter<W>,
    archive_path: &str,
    permissions: u32,
    content: Vec<u8>,
    compression_method: ZipCompressionMethod,
) -> io::Result<()> {
    // Changed PyResult to io::Result
    let file_options = SimpleFileOptions::default()
        .unix_permissions(permissions)
        .compression_method(compression_method);
    zip.start_file(archive_path, file_options)?;
    zip.write_all(&content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*; // Imports zip_files and the pyfunction zip_files
    use std::fs::{self, File};
    use std::io::Read;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    // Helper to call the Python-wrapped version for tests that expect PyResult
    fn zip_files_py_wrapper(
        dst: String,
        srcs: Vec<String>,
        compression: Option<String>,
    ) -> PyResult<()> {
        super::zip_files_pywrapper(dst, srcs, compression)
    }

    // Or, a helper to call internal if tests want to use io::Result
    fn zip_files_internal_wrapper(
        dst: &Path,
        srcs: &[PathBuf],
        compression: Compression,
    ) -> io::Result<()> {
        super::zip_files(dst, srcs, compression)
    }

    #[test]
    fn test_zip_files_creates_zip() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("file1.txt");
        fs::write(&file_path, "hello").unwrap();

        let zip_file_path_str = dir.path().join("archive.zip").to_str().unwrap().to_string();
        let srcs_str = vec![file_path.to_str().unwrap().to_string()];

        // Test the PyO3 wrapper
        zip_files_py_wrapper(zip_file_path_str.clone(), srcs_str.clone(), None).unwrap();
        let mut zip_file = File::open(dir.path().join("archive.zip")).unwrap();
        let mut archive = zip::ZipArchive::new(&mut zip_file).unwrap();
        assert_eq!(archive.len(), 1);
        let mut file_in_zip = archive.by_name("file1.txt").unwrap();
        let mut contents = String::new();
        file_in_zip.read_to_string(&mut contents).unwrap();
        assert_eq!(contents, "hello");

        // Optionally, test the internal function directly
        let zip_file_path_internal = dir.path().join("archive_internal.zip");
        let src_path_bufs = vec![file_path.clone()];
        zip_files_internal_wrapper(
            &zip_file_path_internal,
            &src_path_bufs,
            Compression::default(),
        )
        .unwrap();
        let mut zip_file_internal = File::open(&zip_file_path_internal).unwrap();
        let archive_internal = zip::ZipArchive::new(&mut zip_file_internal).unwrap();
        assert_eq!(archive_internal.len(), 1);
        // Further checks for internal version...
    }

    #[test]
    fn test_zip_files_and_directories() {
        let dir = tempdir().unwrap();
        let file1_path = dir.path().join("file1.txt");
        let subdir_path = dir.path().join("subdir");
        let subfile_path = subdir_path.join("subfile.txt");

        fs::write(&file1_path, "hello from file1").unwrap();
        fs::create_dir(&subdir_path).unwrap();
        fs::write(&subfile_path, "hello from subfile").unwrap();

        let zip_file_path_str = dir.path().join("archive.zip").to_str().unwrap().to_string();
        let srcs_str = vec![
            file1_path.to_str().unwrap().to_string(),
            subdir_path.to_str().unwrap().to_string(),
        ];

        zip_files_py_wrapper(zip_file_path_str, srcs_str, None).unwrap();

        let mut zip_file = File::open(dir.path().join("archive.zip")).unwrap();
        let mut archive = zip::ZipArchive::new(&mut zip_file).unwrap();

        // Expected entries: file1.txt, subdir/, subdir/subfile.txt
        // Depending on how WalkDir iterates and how "." is handled, count might vary.
        // Let's check for specific entries.

        let file1_in_zip = archive.by_name("file1.txt").is_ok();
        assert!(file1_in_zip, "file1.txt should be in the zip");

        let subdir_in_zip = archive.by_name("subdir/").is_ok();
        assert!(subdir_in_zip, "subdir/ should be in the zip");

        let subfile_in_zip = archive.by_name("subdir/subfile.txt").is_ok();
        assert!(subfile_in_zip, "subdir/subfile.txt should be in the zip");

        let mut file_in_zip = archive.by_name("subdir/subfile.txt").unwrap();
        let mut contents = String::new();
        file_in_zip.read_to_string(&mut contents).unwrap();
        assert_eq!(contents, "hello from subfile");
    }

    #[test]
    fn test_zip_preserves_permissions() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("executable.sh");
        fs::write(&file_path, "#!/bin/bash\\necho hello").unwrap();

        #[cfg(unix)]
        {
            let mut perms = fs::metadata(&file_path).unwrap().permissions();
            perms.set_mode(0o755); // rwxr-xr-x
            fs::set_permissions(&file_path, perms).unwrap();
        }

        let zip_file_path_str = dir.path().join("archive.zip").to_str().unwrap().to_string();
        let srcs_str = vec![file_path.to_str().unwrap().to_string()];

        zip_files_py_wrapper(zip_file_path_str, srcs_str, None).unwrap();

        let mut zip_file = File::open(dir.path().join("archive.zip")).unwrap();
        let mut archive = zip::ZipArchive::new(&mut zip_file).unwrap();
        let file_in_zip = archive.by_name("executable.sh").unwrap();

        #[cfg(unix)]
        {
            assert_eq!(
                file_in_zip.unix_mode().unwrap() & 0o777, // Mask to compare only permission bits
                0o755,
                "Permissions not preserved"
            );
        }
        // On non-Unix, this test might not be as meaningful for mode,
        // but it ensures the zipping process itself doesn't fail.
        assert!(file_in_zip.size() > 0);
    }

    #[test]
    fn test_zip_directory_with_dot() {
        let base_dir = tempdir().unwrap();
        let project_dir = base_dir.path().join("my_project");
        fs::create_dir_all(&project_dir).unwrap();

        let file_in_project = project_dir.join("file.txt");
        fs::write(&file_in_project, "content").unwrap();

        let subdir_in_project = project_dir.join("data");
        fs::create_dir_all(&subdir_in_project).unwrap();
        let file_in_subdir = subdir_in_project.join("notes.txt");
        fs::write(&file_in_subdir, "notes").unwrap();

        let zip_file_path = base_dir.path().join("project_archive.zip");

        // Scenario 1: Zip the directory itself ("my_project")
        // We pass the path to "my_project"
        zip_files_internal_wrapper(
            &zip_file_path,
            &[project_dir.clone()],
            Compression::default(),
        )
        .unwrap();

        let mut zip_file = File::open(&zip_file_path).unwrap();
        let mut archive = zip::ZipArchive::new(&mut zip_file).unwrap();

        assert!(
            archive.by_name("my_project/").is_ok(),
            "Archive should contain my_project/ directory entry"
        );
        assert!(archive.by_name("my_project/file.txt").is_ok());
        assert!(archive.by_name("my_project/data/").is_ok());
        assert!(archive.by_name("my_project/data/notes.txt").is_ok());

        // Clean up for next scenario
        fs::remove_file(&zip_file_path).unwrap();

        // Scenario 2: cd into "my_project" and zip "."
        // Simulating this by providing "." as a source and changing current directory for WalkDir logic
        // For the internal function, we need to provide absolute paths or paths relative to where it *thinks* it is.
        // The internal function itself doesn't know about "current directory" in the shell sense.
        // What the user often means by `zip -r archive.zip .` is "zip everything in the current directory,
        // with paths relative to the current directory, and without the current directory's name as a prefix".

        // To simulate zipping "." from within "my_project":
        // The `srcs` for `do_zip_internal` would be `[PathBuf::from("file.txt"), PathBuf::from("data")]`
        // IF `do_zip_internal` was also given `my_project` as a base path to strip.
        // Our current `do_zip_internal` expects full paths for `srcs` if they are top-level items.
        // If we pass `PathBuf::from(".")` as a src, `file_name()` is `.`
        // Let's test current behavior with PathBuf::from(".")
        // This requires creating a "." directory, which is not typical.
        // The more realistic way is that the calling code (CLI) resolves "." to the actual path.

        // Let's test zipping specific files/dirs that are inside my_project,
        // as if we were in my_project and did `zip ../archive.zip file.txt data`
        let zip_file_path_rel = base_dir.path().join("project_archive_relative.zip");
        let sources_relative = vec![file_in_project.clone(), subdir_in_project.clone()];
        zip_files_internal_wrapper(
            &zip_file_path_rel,
            &sources_relative,
            Compression::default(),
        )
        .unwrap();

        let mut zip_file_rel = File::open(&zip_file_path_rel).unwrap();
        let mut archive_rel = zip::ZipArchive::new(&mut zip_file_rel).unwrap();
        // Expects file.txt, data/, data/notes.txt at the root of the zip
        assert!(archive_rel.by_name("file.txt").is_ok());
        assert!(archive_rel.by_name("data/").is_ok());
        assert!(archive_rel.by_name("data/notes.txt").is_ok());
        assert!(
            archive_rel.by_name("my_project/").is_err(),
            "Should not include my_project prefix when zipping contents directly"
        );
    }

    #[test]
    fn test_zip_empty_directory() {
        let dir = tempdir().unwrap();
        let empty_subdir_path = dir.path().join("empty_dir");
        fs::create_dir(&empty_subdir_path).unwrap();

        let zip_file_path_str = dir.path().join("archive.zip").to_str().unwrap().to_string();
        let srcs_str = vec![empty_subdir_path.to_str().unwrap().to_string()];

        zip_files_py_wrapper(zip_file_path_str, srcs_str, None).unwrap();

        let mut zip_file = File::open(dir.path().join("archive.zip")).unwrap();
        let mut archive = zip::ZipArchive::new(&mut zip_file).unwrap();

        // Should contain an entry for "empty_dir/"
        assert_eq!(
            archive.len(),
            1,
            "Zip should contain one entry for the empty directory"
        );
        let entry = archive.by_name("empty_dir/").unwrap();
        assert!(entry.is_dir());
    }

    #[test]
    fn test_zip_compression_methods() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("compressible_data.txt");
        // Create a somewhat compressible file
        let mut large_content = String::new();
        for i in 0..1000 {
            large_content.push_str(&format!("Line {} with some repetitive text. ", i));
        }
        fs::write(&file_path, large_content).unwrap();

        let src_path_bufs = vec![file_path.clone()];
        let srcs_str = vec![file_path.to_str().unwrap().to_string()];

        // Test with Stored (no compression)
        let zip_stored_path = dir.path().join("archive_stored.zip");
        zip_files_internal_wrapper(&zip_stored_path, &src_path_bufs, Compression::Stored).unwrap();

        let mut zip_file_stored = File::open(&zip_stored_path).unwrap();
        let mut archive_stored = zip::ZipArchive::new(&mut zip_file_stored).unwrap();
        let file_in_zip_stored = archive_stored.by_name("compressible_data.txt").unwrap();
        let stored_size = file_in_zip_stored.compressed_size();
        assert_eq!(
            file_in_zip_stored.compression(),
            ZipCompressionMethod::Stored
        );

        // Test with Deflate (default compression) using the Python wrapper
        let zip_deflate_path_str = dir
            .path()
            .join("archive_deflate.zip")
            .to_str()
            .unwrap()
            .to_string();
        zip_files_py_wrapper(
            zip_deflate_path_str.clone(),
            srcs_str.clone(),
            Some("deflate".to_string()),
        )
        .unwrap();

        let mut zip_file_deflate = File::open(dir.path().join("archive_deflate.zip")).unwrap();
        let mut archive_deflate = zip::ZipArchive::new(&mut zip_file_deflate).unwrap();
        let file_in_zip_deflate = archive_deflate.by_name("compressible_data.txt").unwrap();
        let deflated_size = file_in_zip_deflate.compressed_size();
        assert_eq!(
            file_in_zip_deflate.compression(),
            ZipCompressionMethod::Deflated
        );

        // Assert that deflated size is smaller than stored size for compressible data
        // This might not hold for very small or already compressed files, but should for our test data.
        println!(
            "Stored size: {}, Deflated size: {}",
            stored_size, deflated_size
        );
        assert!(
            deflated_size < stored_size,
            "Deflated size should be less than stored size for this data."
        );

        // Test with Bzip2 if feature is enabled (requires bzip2 feature in zip crate)
        // For now, let's assume it might not be and skip, or conditionally compile.
        // We can add a specific test for Bzip2 if we ensure the Cargo.toml enables it.
        // zip_files_internal_wrapper(&dir.path().join("archive_bzip2.zip"), &src_path_bufs, Compression::Bzip2).unwrap();
        // ... then verify ...

        // Test with Zstd if feature is enabled (requires zstd feature in zip crate)
        // zip_files_internal_wrapper(&dir.path().join("archive_zstd.zip"), &src_path_bufs, Compression::Zstd).unwrap();
        // ... then verify ...
    }
}
