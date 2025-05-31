# ziprs: High-Performance Zip Archiving Utility

## Overview

ZipRs is a Rust-based utility designed for efficient creation of ZIP archives. It provides both a Rust library and Python bindings (via PyO3) for seamless integration into various workflows. The core focus is on performance, leveraging parallel processing for directory traversal and compression where applicable, and correctness, including preserving file permissions.

> [!NOTE]
> For anyone looking at this as a serious project just know that I'm normally a Python developer who 
> vibe coded most of this. I think it works and plan to refine it as time goes on but wanted to
> have a disclaimer ahead of time

## Features

*   **Fast Archiving**: Built in Rust for speed and safety.
*   **Parallel Processing**: Utilizes Rayon for concurrent processing of directory contents, speeding up the archiving of large directories.
*   **Permission Preservation**: Retains Unix file permissions in the ZIP archive.
*   **Python Bindings**: Easy to use from Python thanks to PyO3, allowing integration into Python applications and scripts.
*   **File Unzipping**: Supports extracting files and directories from ZIP archives, preserving permissions.

## Usage

### Rust

```rust
// Example (conceptual) - Zipping
use ziprs::zip_files;
use std::path::{Path, PathBuf};

fn main() -> std::io::Result<()> {
    let destination = Path::new("archive.zip");
    let sources = vec![PathBuf::from("file1.txt"), PathBuf::from("my_directory")];
    zip_files(destination, &sources)?;
    Ok(())
}
```

```rust
// Example (conceptual) - Unzipping
use ziprs::unzip_files;
use std::path::Path;

fn main() -> std::io::Result<()> {
    let archive_path = Path::new("archive.zip");
    let destination_directory = Path::new("output_folder");
    unzip_files(archive_path, destination_directory)?;
    Ok(())
}
```

### Python

The Python module provides `zip_files` and `unzip_files` functions.

```python
# Example for zipping
from ziprs import zip_files # Assuming the package is named ziprs

try:
    zip_files("archive.zip", ["file1.txt", "my_directory/"])
    print("Archive created successfully!")
except IOError as e:
    print(f"Error creating archive: {e}")

```

```python
# Example for unzipping
from ziprs import unzip_files # Assuming the package is named ziprs

try:
    unzip_files("archive.zip", "output_directory/")
    print("Archive extracted successfully!")
except IOError as e:
    print(f"Error extracting archive: {e}")

```

## Building from Source

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
uvx maturin build --release
```

## Testing

1. Running rust tests
```bash
cargo test
```

2. Running Python tests
```bash
uvx maturin develop
python3 test_ziprs.py
```

## Contributing

Contributions are welcome! Please feel free to submit pull requests, report bugs, or suggest features.

## License

This project is licensed under the terms of the MIT license and Apache License 2.0.