use clap::{Arg, Command};
use filematch::compare_directories;
use std::path::PathBuf;

fn main() {
    // Define the CLI with `clap`
    let matches = Command::new("filematch")
        .version(env!("CARGO_PKG_VERSION")) // Use the version from Cargo.toml
        .author(env!("CARGO_PKG_AUTHORS")) // Use the authors from Cargo.toml
        .about("Compares files between two directories by hash")
        .arg(
            Arg::new("directory1")
                .help("The first directory to compare")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::new("directory2")
                .help("The second directory to compare")
                .required(true)
                .index(2),
        )
        .arg(
            Arg::new("sort")
                .help("Sort output paths")
                .long("sort")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("skip-hidden")
                .help("Skip hidden files and directories")
                .long("skip-hidden")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("relative")
                .help("Display output paths relative to argument directory")
                .long("relative")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("blake")
                .help("Use BLAKE3 cryptographic hash function")
                .long("blake")
                .action(clap::ArgAction::SetTrue),
        )
        .get_matches();

    // Extract required positional arguments
    let dir1 = PathBuf::from(matches.get_one::<String>("directory1").unwrap());
    let dir2 = PathBuf::from(matches.get_one::<String>("directory2").unwrap());

    // Validate directories
    if !dir1.is_dir() {
        eprintln!(
            "Error: '{}' does not exist or is not a directory.",
            dir1.display()
        );
        std::process::exit(1);
    }
    if !dir2.is_dir() {
        eprintln!(
            "Error: '{}' does not exist or is not a directory.",
            dir2.display()
        );
        std::process::exit(1);
    }

    // Extract flags
    let sort = *matches.get_one::<bool>("sort").unwrap_or(&false);
    let skip_hidden = *matches.get_one::<bool>("skip-hidden").unwrap_or(&false);
    let relative = *matches.get_one::<bool>("relative").unwrap_or(&false);
    let blake = *matches.get_one::<bool>("blake").unwrap_or(&false);

    // Call the function to compare directories
    let (intersection_paths, unique_dir1_paths, unique_dir2_paths) =
        compare_directories(&dir1, &dir2, sort, skip_hidden, relative, blake);

    // Print the results
    println!(
        "Files both in '{}' and '{}':",
        dir1.display(),
        dir2.display()
    );
    for path in &intersection_paths {
        println!("{}", path.display());
    }
    println!();

    println!("Files unique in '{}':", dir1.display());
    for path in &unique_dir1_paths {
        println!("{}", path.display());
    }
    println!();

    println!("Files unique in '{}':", dir2.display());
    for path in &unique_dir2_paths {
        println!("{}", path.display());
    }
}
