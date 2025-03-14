use blake3::Hash;
use crossbeam_channel::{select, unbounded, Receiver};
use std::collections::{HashMap, HashSet};
use std::io::{self};
use std::path::{Path, PathBuf};
use std::thread;

use crate::util::{compute_file_hash_and_insert_path, send_file_paths};

/// Partitions values from two hash maps based on key occurrence.
///
/// This function compares two hash maps where each key maps to a vector of values.
/// It partitions the values into three groups:
/// 1. Values from keys that appear in both maps.
/// 2. Values from keys that are unique to the first map.
/// 3. Values from keys that are unique to the second map.
///
/// The function accepts boolean flags to control which groups are computed.
/// If a flag is false, the corresponding result is returned as None.
///
/// # Parameters
/// - `map1`: The first hash map.
/// - `map2`: The second hash map.
/// - `include_intersection`: When true, includes values from keys present in both maps.
/// - `include_unique_dir1`: When true, includes values from keys only in `map1`.
/// - `include_unique_dir2`: When true, includes values from keys only in `map2`.
///
/// # Returns
/// A tuple of three optional vectors:
/// - The first vector holds values for keys common to both maps (if requested).
/// - The second vector holds values unique to `map1` (if requested).
/// - The third vector holds values unique to `map2` (if requested).
#[allow(clippy::type_complexity)]
fn partition_map_values<K: Eq + std::hash::Hash + Clone, V: Clone>(
    map1: &HashMap<K, Vec<V>>,
    map2: &HashMap<K, Vec<V>>,
    include_intersection: bool,
    include_unique_dir1: bool,
    include_unique_dir2: bool,
) -> (Option<Vec<V>>, Option<Vec<V>>, Option<Vec<V>>) {
    let keys1: HashSet<_> = map1.keys().cloned().collect();
    let keys2: HashSet<_> = map2.keys().cloned().collect();
    let keys_intersection: HashSet<_> = keys1.intersection(&keys2).cloned().collect();

    let intersection = include_intersection.then(|| {
        keys_intersection
            .into_iter()
            .flat_map(|key| {
                map1.get(&key)
                    .into_iter()
                    .chain(map2.get(&key))
                    .flat_map(|values| values.iter().cloned())
            })
            .collect()
    });

    let unique_dir1 = include_unique_dir1.then(|| {
        map1.iter()
            .filter(|(key, _)| !keys2.contains(*key))
            .flat_map(|(_, values)| values.iter().cloned())
            .collect()
    });

    let unique_dir2 = include_unique_dir2.then(|| {
        map2.iter()
            .filter(|(key, _)| !keys1.contains(*key))
            .flat_map(|(_, values)| values.iter().cloned())
            .collect()
    });

    (intersection, unique_dir1, unique_dir2)
}

/// Receives file paths from two channels, computes their hash, and groups them by hash.
///
/// This function listens on two channels, each providing file paths. File paths from the first channel
/// are grouped into the first hash map, while file paths from the second channel are grouped into the
/// second hash map. When one channel is closed, it drains the other channel.
///
/// # Parameters
/// - `r1`: Receiver for file paths for the first group.
/// - `r2`: Receiver for file paths for the second group.
/// - `base1`: An optional base directory for file paths from the first channel.
/// - `base2`: An optional base directory for file paths from the second channel.
///
/// # Returns
/// A Result containing a tuple of two hash maps:
/// - The first hash map groups file paths (from `r1`) by their computed hash.
/// - The second hash map groups file paths (from `r2`) by their computed hash.
#[allow(clippy::type_complexity)]
#[allow(clippy::needless_pass_by_value)] // TODO: This can most likely be fixed
fn group_files_by_hash(
    r1: &Receiver<PathBuf>,
    r2: &Receiver<PathBuf>,
    base1: Option<PathBuf>,
    base2: Option<PathBuf>,
) -> Result<(HashMap<Hash, Vec<PathBuf>>, HashMap<Hash, Vec<PathBuf>>), io::Error> {
    let mut map1 = HashMap::new();
    let mut map2 = HashMap::new();

    loop {
        select! {
            recv(r1) -> msg => {
                if let Ok(path) = msg {
                    compute_file_hash_and_insert_path(&mut map1, path, base1.as_ref())?;
                } else {
                    for path in r2 {
                        compute_file_hash_and_insert_path(&mut map2, path, base2.as_ref())?;
                    }
                    break;
                }
            },
            recv(r2) -> msg => {
                if let Ok(path) = msg {
                    compute_file_hash_and_insert_path(&mut map2, path, base2.as_ref())?;
                } else {
                    for path in r1 {
                        compute_file_hash_and_insert_path(&mut map1, path, base1.as_ref())?;
                    }
                    break;
                }
            }
        }
    }

    Ok((map1, map2))
}

/// Compares two directories by grouping files according to their hashes.
///
/// This function scans two directories concurrently, computes the hash of each file, and
/// groups the file paths based on their hash values. It then compares the two groups to determine:
/// - File paths common to both directories.
/// - File paths unique to the first directory.
/// - File paths unique to the second directory.
///
/// The caller may choose whether to return paths as relative to the provided directories,
/// skip hidden files, or sort the results.
///
/// # Parameters
/// - `dir1`: The first directory to compare.
/// - `dir2`: The second directory to compare.
/// - `relative`: If true, returns file paths relative to the respective directory.
/// - `skip_hidden`: If true, skips hidden files.
/// - `sort`: If true, sorts the resulting file paths.
/// - `include_intersection`: If true, includes file paths common to both directories.
/// - `include_unique_dir1`: If true, includes file paths unique to `dir1`.
/// - `include_unique_dir2`: If true, includes file paths unique to `dir2`.
///
/// # Returns
/// A tuple containing three optional vectors:
/// - The first vector holds file paths present in both directories (if requested).
/// - The second vector holds file paths unique to `dir1` (if requested).
/// - The third vector holds file paths unique to `dir2` (if requested).
///
/// # Panics
/// This function may panic if a thread panics or when the channel sends a message.
/// 
/// # Errors
/// This function does not return any errors directly but may panic.
#[allow(clippy::fn_params_excessive_bools)]
#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
#[must_use]
pub fn compare_two_directories(
    dir1: &Path,
    dir2: &Path,
    relative: bool,
    skip_hidden: bool,
    sort: bool,
    include_intersection: bool,
    include_unique_dir1: bool,
    include_unique_dir2: bool,
) -> (
    Option<Vec<PathBuf>>,
    Option<Vec<PathBuf>>,
    Option<Vec<PathBuf>>,
) {
    // Determine the number of threads based on available physical cores.
    let num_threads = num_cpus::get_physical();
    let mut handles = Vec::with_capacity(num_threads);

    // Create channels for sending file paths from both directories.
    let (sender1, receiver1) = unbounded();
    let (sender2, receiver2) = unbounded();

    let base1: Option<PathBuf> = if relative {
        Some(dir1.to_path_buf())
    } else {
        None
    };
    
    let base2: Option<PathBuf> = if relative {
        Some(dir2.to_path_buf())
    } else {
        None
    };

    // Spawn threads.
    for _ in 0..num_threads {
        let r1 = receiver1.clone();
        let r2 = receiver2.clone();
        let b1 = base1.clone();
        let b2 = base2.clone();

        let handle = thread::spawn(move || group_files_by_hash(&r1, &r2, b1, b2));
        handles.push(handle);
    }

    // Send file paths from each directory into the respective channels.
    send_file_paths(dir1, &sender1, skip_hidden);
    send_file_paths(dir2, &sender2, skip_hidden);

    // Close the channels so that threads can finish processing.
    drop(sender1);
    drop(sender2);

    // Combine the results from all threads.
    let mut combined1: HashMap<Hash, Vec<PathBuf>> = HashMap::new();
    let mut combined2: HashMap<Hash, Vec<PathBuf>> = HashMap::new();

    for handle in handles {
        let (map1, map2) = handle.join().expect("Thread panicked").unwrap();

        for (key, paths) in map1 {
            combined1.entry(key).or_default().extend(paths);
        }
        for (key, paths) in map2 {
            combined2.entry(key).or_default().extend(paths);
        }
    }

    // Partition the file paths into intersection and unique groups.
    let (mut intersection_paths, mut unique_dir1_paths, mut unique_dir2_paths) =
        partition_map_values(
            &combined1,
            &combined2,
            include_intersection,
            include_unique_dir1,
            include_unique_dir2,
        );

    // Optionally sort the file paths.
    if sort {
        if let Some(ref mut paths) = intersection_paths {
            paths.sort();
        }
        if let Some(ref mut paths) = unique_dir1_paths {
            paths.sort();
        }
        if let Some(ref mut paths) = unique_dir2_paths {
            paths.sort();
        }
    }

    (intersection_paths, unique_dir1_paths, unique_dir2_paths)
}
