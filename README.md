# ZipRs: High-Performance Zip Archiving Utility

## Overview

ZipRs is a Rust-based utility designed for efficient creation of ZIP archives. It provides both a Rust library and Python bindings (via PyO3) for seamless integration into various workflows. The core focus is on performance, leveraging parallel processing for directory traversal and compression where applicable, and correctness, including preserving file permissions.

## Features

*   **Fast Archiving**: Built in Rust for speed and safety.
*   **Parallel Processing**: Utilizes Rayon for concurrent processing of directory contents, speeding up the archiving of large directories.
*   **Permission Preservation**: Retains Unix file permissions in the ZIP archive.
*   **Python Bindings**: Easy to use from Python thanks to PyO3, allowing integration into Python applications and scripts.
*   **Handles Files and Directories**: Can archive individual files or entire directory structures.
*   **Correctly Handles Empty Directories**: Ensures empty directories are represented in the archive.
*   **Robust Error Handling**: Designed to handle I/O errors and invalid path issues gracefully.

## Usage

### Rust

(Details on how to use the Rust library will be added here. This would typically involve adding `ziprs` as a dependency in your `Cargo.toml` and using the `do_zip_internal` function.)

```rust
// Example (conceptual)
// use ziprs::do_zip_internal;
// use std::path::{Path, PathBuf};
//
// fn main() -> std::io::Result<()> {
//     let destination = Path::new("archive.zip");
//     let sources = vec![PathBuf::from("file1.txt"), PathBuf::from("my_directory")];
//     do_zip_internal(destination, &sources)?;
//     Ok(())
// }
```

### Python

The Python module provides a `zip_files` function.

```python
# Example
# from ziprs import zip_files # Assuming the package is named ziprs
#
# try:
#     zip_files("archive.zip", ["file1.txt", "my_directory/"])
#     print("Archive created successfully!")
# except IOError as e:
#     print(f"Error creating archive: {e}")
#
```

## Building from Source

(Instructions on how to build the project from source, including any prerequisites like Rust, Cargo, and potentially Maturin for the Python bindings, will go here.)

1.  Clone the repository:
    ```bash
    git clone <repository-url>
    cd ziprs
    ```
2.  Build the Rust library:
    ```bash
    cargo build --release
    ```
3.  Build the Python wheel (if applicable, using Maturin):
    ```bash
    # Example with maturin
    # maturin build --release
    ```

## Contributing

Contributions are welcome! Please feel free to submit pull requests, report bugs, or suggest features.

(You might want to add guidelines for contributing, code of conduct, etc.)

## License

This project is licensed under the terms of the MIT license and Apache License 2.0.

(Or specify the chosen license, e.g., "This project is licensed under the MIT License.") 