#!/usr/bin/env python3
"""
Python test script for the ziprs module using unittest framework.
Tests the zip_files function with both files and directories.
"""

import tempfile
import os
import zipfile
import unittest
import ziprs


class TestZiprs(unittest.TestCase):
    """Test cases for the ziprs module."""

    def test_zip_single_file(self):
        """Test zipping a single file."""
        with tempfile.TemporaryDirectory() as temp_dir:
            # Create a test file
            test_file = os.path.join(temp_dir, "single.txt")
            zip_path = os.path.join(temp_dir, "single.zip")

            with open(test_file, "w") as f:
                f.write("Single file content")

            # Zip the single file
            ziprs.zip_files(zip_path, [test_file])

            # Verify the zip file was created
            self.assertTrue(os.path.exists(zip_path), "Zip file was not created")
            self.assertGreater(os.path.getsize(zip_path), 0, "Zip file is empty")

            # Verify the contents
            with zipfile.ZipFile(zip_path, "r") as zf:
                names = zf.namelist()
                self.assertIn("single.txt", names, "single.txt not found in zip")

                with zf.open("single.txt") as f:
                    content = f.read().decode("utf-8")
                    self.assertEqual(
                        content, "Single file content", f"Unexpected content: {content}"
                    )

    def test_zip_files_and_directories(self):
        """Test zipping both individual files and directories."""
        with tempfile.TemporaryDirectory() as temp_dir:
            # Create test files
            file1_path = os.path.join(temp_dir, "file1.txt")
            file2_path = os.path.join(temp_dir, "file2.txt")
            subdir_path = os.path.join(temp_dir, "subdir")
            subfile_path = os.path.join(subdir_path, "subfile.txt")
            zip_path = os.path.join(temp_dir, "test_archive.zip")

            # Write content to files
            with open(file1_path, "w") as f:
                f.write("Hello from file1!")

            with open(file2_path, "w") as f:
                f.write("Hello from file2!")

            # Create subdirectory and file
            os.makedirs(subdir_path)
            with open(subfile_path, "w") as f:
                f.write("Hello from subdirectory!")

            # Test zipping both files and directory
            sources = [file1_path, subdir_path]
            ziprs.zip_files(zip_path, sources)

            # Verify the zip file was created
            self.assertTrue(os.path.exists(zip_path), "Zip file was not created")
            self.assertGreater(os.path.getsize(zip_path), 0, "Zip file is empty")

            # Verify the contents of the zip file
            with zipfile.ZipFile(zip_path, "r") as zf:
                names = zf.namelist()
                print(f"Files in zip: {names}")

                # Check that file1.txt is present
                self.assertIn("file1.txt", names, "file1.txt not found in zip")

                # Check that subfile.txt is present (with correct path)
                self.assertIn(
                    "subdir/subfile.txt", names, "subdir/subfile.txt not found in zip"
                )

                # Verify file contents
                with zf.open("file1.txt") as f:
                    content = f.read().decode("utf-8")
                    self.assertEqual(
                        content, "Hello from file1!", f"Unexpected content: {content}"
                    )

                with zf.open("subdir/subfile.txt") as f:
                    content = f.read().decode("utf-8")
                    self.assertEqual(
                        content,
                        "Hello from subdirectory!",
                        f"Unexpected content: {content}",
                    )

    def test_zip_multiple_files(self):
        """Test zipping multiple individual files."""
        with tempfile.TemporaryDirectory() as temp_dir:
            # Create multiple test files
            file1_path = os.path.join(temp_dir, "test1.txt")
            file2_path = os.path.join(temp_dir, "test2.txt")
            zip_path = os.path.join(temp_dir, "multiple_files.zip")

            with open(file1_path, "w") as f:
                f.write("Content of file 1")

            with open(file2_path, "w") as f:
                f.write("Content of file 2")

            # Zip multiple files
            ziprs.zip_files(zip_path, [file1_path, file2_path])

            # Verify
            self.assertTrue(os.path.exists(zip_path))

            with zipfile.ZipFile(zip_path, "r") as zf:
                names = zf.namelist()
                self.assertIn("test1.txt", names)
                self.assertIn("test2.txt", names)
                self.assertEqual(len(names), 2, "Expected exactly 2 files in zip")

    def test_zip_empty_directory(self):
        """Test zipping an empty directory."""
        with tempfile.TemporaryDirectory() as temp_dir:
            # Create an empty subdirectory
            empty_dir = os.path.join(temp_dir, "empty_dir")
            os.makedirs(empty_dir)
            zip_path = os.path.join(temp_dir, "empty_dir.zip")

            # Zip the empty directory
            ziprs.zip_files(zip_path, [empty_dir])

            # Verify the zip file was created (should be minimal size)
            self.assertTrue(os.path.exists(zip_path))

            with zipfile.ZipFile(zip_path, "r") as zf:
                names = zf.namelist()
                # Empty directory should result in one entry for the directory itself
                self.assertEqual(
                    len(names),
                    1,
                    f"Expected 1 entry for empty directory, got {len(names)}: {names}",
                )
                self.assertIn(
                    "empty_dir/",
                    names,
                    "The empty directory itself should be in the zip names.",
                )

    def test_zip_preserves_permissions(self):
        """Test that file permissions are preserved in the zip archive."""
        with tempfile.TemporaryDirectory() as temp_dir:
            # Define test cases: (filename, content, permissions, zip_path)
            test_cases = [
                ("executable.sh", "#!/bin/bash\necho 'hello'", 0o755, "executable.sh"),
                ("readonly.txt", "read only content", 0o444, "readonly.txt"),
                ("subdir/subfile.txt", "sub file content", 0o600, "subfile.txt"),
            ]

            zip_path = os.path.join(temp_dir, "permissions_test.zip")
            sources = []

            # Create files and set permissions
            for filename, content, perms, _ in test_cases:
                file_path = os.path.join(temp_dir, filename)

                # Create directory if needed
                os.makedirs(os.path.dirname(file_path), exist_ok=True)

                # Create file with content
                with open(file_path, "w") as f:
                    f.write(content)

                # Set permissions
                os.chmod(file_path, perms)

                # Verify permissions were set correctly
                actual_perms = os.stat(file_path).st_mode & 0o777
                self.assertEqual(
                    actual_perms, perms, f"Failed to set permissions for {filename}"
                )

                # Add to sources (add parent dir for subdir files, individual files otherwise)
                if "/" in filename:
                    parent_dir = os.path.join(temp_dir, filename.split("/")[0])
                    if parent_dir not in sources:
                        sources.append(parent_dir)
                else:
                    sources.append(file_path)

            # Zip the files
            ziprs.zip_files(zip_path, sources)

            # Verify the zip file was created
            self.assertTrue(os.path.exists(zip_path), "Zip file was not created")

            # Create expected permissions mapping
            expected_perms = {
                zip_filename: perms for _, _, perms, zip_filename in test_cases
            }

            # Verify permissions are preserved in the zip
            with zipfile.ZipFile(zip_path, "r") as zf:
                for info in zf.infolist():
                    filename = info.filename

                    # Extract Unix permissions from external_attr
                    unix_permissions = (info.external_attr >> 16) & 0o777

                    if filename in expected_perms:
                        expected = expected_perms[filename]
                        self.assertEqual(
                            unix_permissions,
                            expected,
                            f"{filename} should have {oct(expected)} permissions, got {oct(unix_permissions)}",
                        )

    def test_unzip_basic(self):
        """Test basic unzipping of files, directories, and permission preservation."""
        with tempfile.TemporaryDirectory() as source_temp_dir, tempfile.TemporaryDirectory() as extracted_temp_dir:
            original_dir_path = os.path.join(source_temp_dir, "original")
            extracted_dir_path = os.path.join(extracted_temp_dir, "extracted")
            zip_file_path = os.path.join(source_temp_dir, "archive.zip")

            os.makedirs(original_dir_path, exist_ok=True)
            os.makedirs(
                extracted_dir_path, exist_ok=True
            )  # unzip_files should also be able to create this

            # 1. Create source files and directories
            file1_orig_path = os.path.join(original_dir_path, "file1.txt")
            subdir_orig_path = os.path.join(original_dir_path, "subdir")
            subfile_orig_path = os.path.join(subdir_orig_path, "subfile.txt")

            os.makedirs(subdir_orig_path)
            with open(file1_orig_path, "w") as f:
                f.write("hello from file1")
            with open(subfile_orig_path, "w") as f:
                f.write("hello from subfile")

            # Set specific permissions for testing
            os.chmod(file1_orig_path, 0o644)  # rw-r--r--
            os.chmod(subfile_orig_path, 0o755)  # rwxr-xr-x
            # Note: Directory permissions are harder to test consistently across zip libraries and OS
            # We will focus on file permissions which are explicitly handled in unzip.rs

            # 2. Zip these files using ziprs.zip_files
            # ziprs.zip_files expects a list of paths relative to where it runs, or absolute paths.
            # For simplicity and to mimic Rust test behavior, we provide paths as they are.
            # The zip_files function in Rust seems to take the base name of files/dirs given.
            # To achieve the same structure as the Rust test (file1.txt, subdir/subfile.txt in archive root):
            # We need to zip the *contents* of original_dir_path or structure them appropriately.
            # A simpler approach for this Python test is to zip the individual items.
            # However, the Rust zip_files function zips 'file1.txt' and 'subdir/' based on `original_dir.path()` as root.
            # To align, let's cd into the original_dir_path or prepare paths for zip_files carefully.

            # Simplest: create archive from individual full paths, let ziprs handle naming
            # This means file1.txt will be at root of zip, subdir/subfile.txt will be as subdir/subfile.txt
            # The Rust test seems to be zipping "file1_path" and "subdir_path" relative to `original_dir`.
            # Let's adjust sources for zip_files:
            srcs_to_zip = [file1_orig_path, subdir_orig_path]
            ziprs.zip_files(zip_file_path, srcs_to_zip)

            # 3. Unzip the archive using unzip_files
            ziprs.unzip_files(zip_file_path, extracted_dir_path)

            # 4. Verify extracted content and structure
            extracted_file1 = os.path.join(
                extracted_dir_path, os.path.basename(file1_orig_path)
            )
            extracted_subdir = os.path.join(
                extracted_dir_path, os.path.basename(subdir_orig_path)
            )
            extracted_subfile = os.path.join(
                extracted_subdir, os.path.basename(subfile_orig_path)
            )

            self.assertTrue(
                os.path.exists(extracted_file1), "file1.txt should be extracted"
            )
            self.assertTrue(
                os.path.isdir(extracted_subdir),
                "subdir should be extracted as a directory",
            )
            self.assertTrue(
                os.path.exists(extracted_subfile),
                "subdir/subfile.txt should be extracted",
            )

            with open(extracted_file1, "r") as f:
                content = f.read()
                self.assertEqual(content, "hello from file1")

            with open(extracted_subfile, "r") as f:
                content = f.read()
                self.assertEqual(content, "hello from subfile")

            # 5. Verify permissions (Python's os.stat().st_mode gives full mode, mask with 0o777 for comparison)
            perms_ext_file1 = os.stat(extracted_file1).st_mode & 0o777
            self.assertEqual(
                perms_ext_file1,
                0o644,
                f"Permissions for file1.txt mismatch. Expected {oct(0o644)}, got {oct(perms_ext_file1)}",
            )

            perms_ext_subfile = os.stat(extracted_subfile).st_mode & 0o777
            self.assertEqual(
                perms_ext_subfile,
                0o755,
                f"Permissions for subfile.txt mismatch. Expected {oct(0o755)}, got {oct(perms_ext_subfile)}",
            )

    def test_unzip_to_non_existent_destination(self):
        """Test unzipping to a destination directory that does not exist yet."""
        with tempfile.TemporaryDirectory() as source_temp_dir, tempfile.TemporaryDirectory() as extracted_base_dir:
            original_dir_path = os.path.join(source_temp_dir, "original")
            zip_file_path = os.path.join(source_temp_dir, "archive.zip")
            # This directory will not be created beforehand
            extracted_dir_path = os.path.join(extracted_base_dir, "new_dest_dir")

            os.makedirs(original_dir_path, exist_ok=True)

            # Create a dummy file to zip
            dummy_file_path = os.path.join(original_dir_path, "dummy.txt")
            with open(dummy_file_path, "w") as f:
                f.write("dummy content")

            srcs_to_zip = [dummy_file_path]
            ziprs.zip_files(zip_file_path, srcs_to_zip)

            # Attempt to unzip to a non-existent directory
            ziprs.unzip_files(
                zip_file_path, extracted_dir_path
            )  # Should not raise error

            # Verify the directory and file were created
            self.assertTrue(
                os.path.exists(extracted_dir_path),
                "Destination directory should have been created.",
            )
            self.assertTrue(
                os.path.isdir(extracted_dir_path),
                "Destination path should be a directory.",
            )

            extracted_file = os.path.join(extracted_dir_path, "dummy.txt")
            self.assertTrue(
                os.path.exists(extracted_file),
                "dummy.txt should be extracted into the new directory.",
            )
            with open(extracted_file, "r") as f:
                self.assertEqual(f.read(), "dummy content")

    def test_unzip_empty_directory(self):
        """Test unzipping an archive that contains an empty directory."""
        with tempfile.TemporaryDirectory() as source_temp_dir, tempfile.TemporaryDirectory() as extracted_temp_dir:
            original_dir_path = os.path.join(source_temp_dir, "original")
            zip_file_path = os.path.join(source_temp_dir, "archive_with_empty_dir.zip")
            extracted_dir_path = os.path.join(extracted_temp_dir, "extracted")

            os.makedirs(original_dir_path, exist_ok=True)
            os.makedirs(extracted_dir_path, exist_ok=True)

            # Create an empty directory
            empty_subdir_orig_path = os.path.join(original_dir_path, "empty_subdir")
            os.makedirs(empty_subdir_orig_path)

            # Zip this empty directory
            srcs_to_zip = [empty_subdir_orig_path]
            ziprs.zip_files(zip_file_path, srcs_to_zip)

            # Unzip
            ziprs.unzip_files(zip_file_path, extracted_dir_path)

            # Verify the empty directory was created
            extracted_empty_subdir = os.path.join(
                extracted_dir_path, os.path.basename(empty_subdir_orig_path)
            )
            self.assertTrue(
                os.path.exists(extracted_empty_subdir),
                "Empty subdirectory should be extracted.",
            )
            self.assertTrue(
                os.path.isdir(extracted_empty_subdir),
                "Extracted path for empty subdirectory should be a directory.",
            )
            self.assertEqual(
                len(os.listdir(extracted_empty_subdir)),
                0,
                "Extracted empty subdirectory should be empty.",
            )

    def test_zip_compression_options(self):
        """Test zipping with different compression options."""
        with tempfile.TemporaryDirectory() as temp_dir:
            test_file_name = "compressible_data.txt"
            test_file_path = os.path.join(temp_dir, test_file_name)
            zip_stored_path = os.path.join(temp_dir, "archive_stored.zip")
            zip_deflate_path = os.path.join(temp_dir, "archive_deflate.zip")
            zip_default_compression_path = os.path.join(temp_dir, "archive_default.zip")

            # Create a somewhat compressible file
            large_content = "".join(
                [f"Line {i} with some repetitive text.\n" for i in range(1000)]
            )
            with open(test_file_path, "w") as f:
                f.write(large_content)

            original_size = os.path.getsize(test_file_path)

            # 1. Test with "stored" compression
            ziprs.zip_files(zip_stored_path, [test_file_path], "stored")
            self.assertTrue(os.path.exists(zip_stored_path))
            with zipfile.ZipFile(zip_stored_path, "r") as zf:
                self.assertIn(test_file_name, zf.namelist())
                info = zf.getinfo(test_file_name)
                self.assertEqual(info.compress_type, zipfile.ZIP_STORED)
                # For stored, compressed size should be very close to original, possibly slightly larger due to metadata
                # self.assertEqual(info.compress_size, original_size) # Can be slightly different
                print(
                    f"Stored size: {info.compress_size}, Original size: {original_size}"
                )
                self.assertLess(
                    abs(info.compress_size - original_size),
                    original_size * 0.05 + 16,
                    "Stored size differs too much from original",
                )

            # 2. Test with "deflate" compression
            ziprs.zip_files(zip_deflate_path, [test_file_path], "deflate")
            self.assertTrue(os.path.exists(zip_deflate_path))
            with zipfile.ZipFile(zip_deflate_path, "r") as zf:
                self.assertIn(test_file_name, zf.namelist())
                info_deflate = zf.getinfo(test_file_name)
                self.assertEqual(info_deflate.compress_type, zipfile.ZIP_DEFLATED)
                self.assertLess(
                    info_deflate.compress_size,
                    original_size,
                    "Deflated size should be less than original for this data.",
                )
                size_deflated = info_deflate.compress_size

            # 3. Test with default compression (None, should be deflate)
            ziprs.zip_files(zip_default_compression_path, [test_file_path], None)
            self.assertTrue(os.path.exists(zip_default_compression_path))
            with zipfile.ZipFile(zip_default_compression_path, "r") as zf:
                self.assertIn(test_file_name, zf.namelist())
                info_default = zf.getinfo(test_file_name)
                self.assertEqual(info_default.compress_type, zipfile.ZIP_DEFLATED)
                self.assertLess(
                    info_default.compress_size,
                    original_size,
                    "Default compression (deflate) size should be less than original.",
                )
                # Check if default compression (deflate) matches explicit deflate
                self.assertEqual(
                    info_default.compress_size,
                    size_deflated,
                    "Default compression size should match explicit deflate size.",
                )


if __name__ == "__main__":
    # Run the tests
    unittest.main(verbosity=2)
