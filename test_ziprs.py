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
            
            with open(test_file, 'w') as f:
                f.write("Single file content")
            
            # Zip the single file
            ziprs.zip_files(zip_path, [test_file])
            
            # Verify the zip file was created
            self.assertTrue(os.path.exists(zip_path), "Zip file was not created")
            self.assertGreater(os.path.getsize(zip_path), 0, "Zip file is empty")
            
            # Verify the contents
            with zipfile.ZipFile(zip_path, 'r') as zf:
                names = zf.namelist()
                self.assertIn("single.txt", names, "single.txt not found in zip")
                
                with zf.open("single.txt") as f:
                    content = f.read().decode('utf-8')
                    self.assertEqual(content, "Single file content", 
                                   f"Unexpected content: {content}")

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
            with open(file1_path, 'w') as f:
                f.write("Hello from file1!")
            
            with open(file2_path, 'w') as f:
                f.write("Hello from file2!")
            
            # Create subdirectory and file
            os.makedirs(subdir_path)
            with open(subfile_path, 'w') as f:
                f.write("Hello from subdirectory!")
            
            # Test zipping both files and directory
            sources = [file1_path, subdir_path]
            ziprs.zip_files(zip_path, sources)
            
            # Verify the zip file was created
            self.assertTrue(os.path.exists(zip_path), "Zip file was not created")
            self.assertGreater(os.path.getsize(zip_path), 0, "Zip file is empty")
            
            # Verify the contents of the zip file
            with zipfile.ZipFile(zip_path, 'r') as zf:
                names = zf.namelist()
                print(f"Files in zip: {names}")
                
                # Check that file1.txt is present
                self.assertIn("file1.txt", names, "file1.txt not found in zip")
                
                # Check that subfile.txt is present (with correct path)
                self.assertIn("subfile.txt", names, "subfile.txt not found in zip")
                
                # Verify file contents
                with zf.open("file1.txt") as f:
                    content = f.read().decode('utf-8')
                    self.assertEqual(content, "Hello from file1!", 
                                   f"Unexpected content: {content}")
                
                with zf.open("subfile.txt") as f:
                    content = f.read().decode('utf-8')
                    self.assertEqual(content, "Hello from subdirectory!", 
                                   f"Unexpected content: {content}")

    def test_zip_multiple_files(self):
        """Test zipping multiple individual files."""
        with tempfile.TemporaryDirectory() as temp_dir:
            # Create multiple test files
            file1_path = os.path.join(temp_dir, "test1.txt")
            file2_path = os.path.join(temp_dir, "test2.txt")
            zip_path = os.path.join(temp_dir, "multiple_files.zip")
            
            with open(file1_path, 'w') as f:
                f.write("Content of file 1")
            
            with open(file2_path, 'w') as f:
                f.write("Content of file 2")
            
            # Zip multiple files
            ziprs.zip_files(zip_path, [file1_path, file2_path])
            
            # Verify
            self.assertTrue(os.path.exists(zip_path))
            
            with zipfile.ZipFile(zip_path, 'r') as zf:
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
            
            with zipfile.ZipFile(zip_path, 'r') as zf:
                names = zf.namelist()
                # Empty directory should result in empty zip or minimal zip structure
                self.assertEqual(len(names), 0, "Empty directory should produce empty zip")


if __name__ == "__main__":
    # Run the tests
    unittest.main(verbosity=2) 