use clap::Parser;
use filematch::compare_directories;
use serde_json::json;
use std::path::PathBuf;

// Compares files between two directories by hash
#[derive(Parser)]
#[command(
    version = env!("CARGO_PKG_VERSION"),
    author = env!("CARGO_PKG_AUTHORS"),
    about = "Compares files between two directories by hash",
    after_help = "If none of --intersection, --dir1, or --dir2 are set, then all are displayed",
)]
struct Cli {
    /// The first directory to compare
    #[arg(required = true)]
    directory1: PathBuf,

    /// The second directory to compare
    #[arg(required = true)]
    directory2: PathBuf,

    /// Sort output paths
    #[arg(long, action = clap::ArgAction::SetTrue)]
    sort: bool,

    /// Skip hidden files and directories
    #[arg(long, action = clap::ArgAction::SetTrue)]
    skip_hidden: bool,

    /// Display output paths relative to argument directory
    #[arg(long, action = clap::ArgAction::SetTrue)]
    relative: bool,

    /// Display as json
    #[arg(long, action = clap::ArgAction::SetTrue)]
    json: bool,

    /// Display files both in directory1 and directory2
    #[arg(long, action = clap::ArgAction::SetTrue)]
    intersection: bool,

    /// Display unique files in dir1
    #[arg(long, action = clap::ArgAction::SetTrue)]
    dir1: bool,

    /// Display unique files in dir2
    #[arg(long, action = clap::ArgAction::SetTrue)]
    dir2: bool,
}

fn main() {
    let args = Cli::parse();

    // Validate directories
    if !args.directory1.is_dir() {
        eprintln!(
            "Error: '{}' does not exist or is not a directory.",
            args.directory1.display()
        );
        std::process::exit(1);
    }
    if !args.directory2.is_dir() {
        eprintln!(
            "Error: '{}' does not exist or is not a directory.",
            args.directory2.display()
        );
        std::process::exit(1);
    }

    // If no selective directory is set all are true
    let all = !args.intersection && !args.dir1 && !args.dir2;
    let intersection = all || args.intersection;
    let dir1 = all || args.dir1;
    let dir2 = all || args.dir2;

    // Call the function to compare directories
    let (intersection_paths, unique_dir1_paths, unique_dir2_paths) = compare_directories(
        &args.directory1,
        &args.directory2,
        args.sort,
        args.skip_hidden,
        args.relative,
        intersection,
        dir1,
        dir2,
    );

    if args.json {
        // Create a JSON value with string representations of the paths.
        let mut result = serde_json::Map::new();

        if intersection {
            result.insert(
                "intersection".to_string(),
                json!(intersection_paths
                    .unwrap()
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()),
            );
        }

        if dir1 {
            result.insert(
                "directory1".to_string(),
                json!(unique_dir1_paths
                    .unwrap()
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()),
            );
        }

        if dir2 {
            result.insert(
                "directory2".to_string(),
                json!(unique_dir2_paths
                    .unwrap()
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()),
            );
        }

        println!("{}", serde_json::to_string_pretty(&result).unwrap());
    } else {
        // Print the results
        if intersection {
            println!(
                "Files both in '{}' and '{}':",
                args.directory1.display(),
                args.directory2.display()
            );
            for path in intersection_paths.unwrap() {
                println!("{}", path.display());
            }
        }

        if intersection && dir1 {
            println!();
        }

        if dir1 {
            println!("Files unique in '{}':", &args.directory1.display());
            for path in unique_dir1_paths.unwrap() {
                println!("{}", path.display());
            }
        }

        if (intersection || dir1) && dir2 {
            println!();
        }

        if dir2 {
            println!("Files unique in '{}':", &args.directory2.display());
            for path in unique_dir2_paths.unwrap() {
                println!("{}", path.display());
            }
        }
    }
}
