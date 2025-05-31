use clap::Parser;
use std::path::PathBuf;

// This will refer to the library part of your crate
// We call the internal functions directly from their modules
use ziprs::{unzip::do_unzip_internal, zip::do_zip_internal};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Parser, Debug)]
enum Commands {
    /// Zips specified files into an archive
    Zip {
        /// List of input files or directories to zip
        #[clap(required = true, num_args = 1..)]
        input_paths: Vec<PathBuf>,

        /// Output zip file path
        #[clap(short, long)]
        output_path: PathBuf,
    },
    /// Unzips a specified archive
    Unzip {
        /// Path to the zip file to unzip
        #[clap(required = true)]
        zip_path: PathBuf,

        /// Directory to extract files to
        #[clap(short, long)]
        output_dir: PathBuf,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Zip {
            input_paths,
            output_path,
        } => {
            println!("Zipping {:?} to {:?}...", input_paths, output_path);
            do_zip_internal(&output_path, &input_paths)
                .map_err(|e| format!("Failed to zip files: {}", e))?;
            println!("Successfully zipped files to {}.\n", output_path.display());
        }
        Commands::Unzip {
            zip_path,
            output_dir,
        } => {
            println!("Unzipping {:?} to {:?}...", zip_path, output_dir);
            do_unzip_internal(&zip_path, &output_dir)
                .map_err(|e| format!("Failed to unzip archive: {}", e))?;
            println!(
                "Successfully unzipped archive {} to {}.\n",
                zip_path.display(),
                output_dir.display()
            );
        }
    }

    Ok(())
}
