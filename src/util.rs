use blake3::Hash;
use blake3::Hasher as BlakeHasher;
use crossbeam_channel::Sender;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use walkdir::{DirEntry, WalkDir};

/// Computes the BLAKE3 hash of the file at the given path.
///
/// Opens the file, reads it in chunks, and feeds the data to the hasher.
///
/// # Parameters
/// - `path`: The file path to hash.
///
/// # Returns
/// - `Ok(Hash)` containing the computed hash of the file if successful.
/// - `Err(io::Error)` if there was an error opening the file or reading its contents.
///
/// # Errors
/// This function returns an `io::Error` if the file cannot be opened or read.
pub fn calculate_file_hash(path: &Path) -> io::Result<Hash> {
    let mut file = File::open(path)?;
    let mut hasher = BlakeHasher::default();
    let mut buffer = vec![0; 64 * 1024];

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(hasher.finalize())
}

/// Determines if the given file or directory is hidden.
///
/// Checks if the name starts with a dot.
///
/// # Parameters
/// - `entry`: The directory entry to check.
///
/// # Returns
/// True if the entry is hidden, false otherwise.
fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .is_some_and(|s| s.starts_with('.'))
}

/// Recursively sends all file paths from a directory through a channel.
///
/// Walks the directory tree and sends file paths if they are not hidden (when `skip_hidden` is true).
///
/// # Parameters
/// - `directory`: The root directory to scan.
/// - `sender`: The channel sender to pass file paths.
/// - `skip_hidden`: If true, skips hidden files.
///
/// # Panics
/// This function may panic if the `sender.send()` call fails.
///
/// # Errors
/// This function does not return any errors directly, but it may panic if the `unwrap()` call fails.
pub fn send_file_paths(directory: &Path, sender: &Sender<PathBuf>, skip_hidden: bool) {
    for entry in WalkDir::new(directory)
        .into_iter()
        .filter_entry(|e| !skip_hidden || !is_hidden(e))
        .filter_map(Result::ok)
    {
        if entry.path().is_file() {
            sender.send(entry.path().to_path_buf()).unwrap();
        }
    }
}

/// Computes a file's hash and records its (possibly relative) path in the given map.
///
/// This function computes the file's hash and converts the file's path to a relative path if a
/// base directory is provided. It then inserts the final path into the
/// hash map under the computed hash.
///
/// # Parameters
/// - `map`: A mutable reference to a hash map that groups file paths by their computed hash.
/// - `path`: The file path to process.
/// - `base`: An optional base directory. If provided, the file path is converted to a relative path
///           based on this directory.
///
/// # Returns
/// A Result indicating success or an `io::Error`.
///
/// # Errors
/// This function returns an `io::Error` if there is an issue reading the file to compute its hash.
#[allow(clippy::implicit_hasher)]
pub fn compute_file_hash_and_insert_path(
    map: &mut HashMap<Hash, Vec<PathBuf>>,
    path: PathBuf,
    base: Option<&PathBuf>,
) -> Result<(), io::Error> {
    let hash = calculate_file_hash(&path)?;
    let final_path = match base {
        Some(base_dir) => path
            .strip_prefix(base_dir)
            .map_or_else(|_| path.clone(), Path::to_path_buf),
        None => path,
    };
    map.entry(hash).or_default().push(final_path);
    Ok(())
}
