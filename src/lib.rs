use blake3::Hash;
use blake3::Hasher as BlakeHasher;
use crossbeam_channel::{select, unbounded, Receiver};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::thread;
use walkdir::{DirEntry, WalkDir};

/// A function that calculates a file hash using BLAKE3
#[inline(always)]
fn calculate_file_hash(path: &Path) -> io::Result<Hash> {
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

/// Compares two `HashMap`s and computes the intersection, values unique to the first map,
/// and items values to the second map based on the key.
///
/// # Arguments
/// * `map1` - The first `HashMap`.
/// * `map2` - The second `HashMap`.
///
/// # Returns
/// A tuple of three `Vec`s:
/// 1. A `Vec` containing all values for the keys common to both maps.
/// 2. A `Vec` containing all values unique to `map1`.
/// 3. A `Vec` containing all values unique to `map2`.
#[allow(clippy::type_complexity)]
pub fn compare_hashmaps<K: Eq + std::hash::Hash + Clone, V: Clone>(
    map1: &HashMap<K, Vec<V>>,
    map2: &HashMap<K, Vec<V>>,
    include_intersection: bool,
    include_unique_dir1: bool,
    include_unique_dir2: bool,
) -> (Option<Vec<V>>, Option<Vec<V>>, Option<Vec<V>>) {
    let set1: HashSet<_> = map1.keys().cloned().collect();
    let set2: HashSet<_> = map2.keys().cloned().collect();
    let intersection_keys: HashSet<_> = set1.intersection(&set2).cloned().collect();

    let intersection = include_intersection.then(|| {
        intersection_keys
            .into_iter()
            .flat_map(|key| {
                map1.get(&key)
                    .into_iter()
                    .chain(map2.get(&key))
                    .flat_map(|values| values.iter().cloned())
            })
            .collect()
    });

    let only_in_a = include_unique_dir1.then(|| {
        map1.iter()
            .filter(|(key, _)| !set2.contains(*key))
            .flat_map(|(_, values)| values.iter().cloned())
            .collect()
    });

    let only_in_b = include_unique_dir2.then(|| {
        map2.iter()
            .filter(|(key, _)| !set1.contains(*key))
            .flat_map(|(_, values)| values.iter().cloned())
            .collect()
    });

    (intersection, only_in_a, only_in_b)
}

/// Receives file paths from two channels, calculates their hash, and groups them into two maps.
/// File paths from r1 go into map1 and from r2 into map2. When one channel closes, it drains the other.
#[allow(clippy::type_complexity)]
fn worker(
    r1: Receiver<PathBuf>,
    r2: Receiver<PathBuf>,
) -> Result<(HashMap<Hash, Vec<PathBuf>>, HashMap<Hash, Vec<PathBuf>>), io::Error> {
    let mut map1: HashMap<Hash, Vec<PathBuf>> = HashMap::new();
    let mut map2: HashMap<Hash, Vec<PathBuf>> = HashMap::new();

    loop {
        select! {
            recv(r1) -> msg => {
                match msg {
                    Ok(path) => {
                        let hash = calculate_file_hash(&path)?;
                        map1.entry(hash).or_default().push(path);
                    }
                    Err(_) => {
                        // r1 closed; drain r2 and exit.
                        for path in r2.iter() {
                            let hash = calculate_file_hash(&path)?;
                            map2.entry(hash).or_default().push(path);
                        }
                        break;
                    }
                }
            },
            recv(r2) -> msg => {
                match msg {
                    Ok(path) => {
                        let hash = calculate_file_hash(&path)?;
                        map2.entry(hash).or_default().push(path);
                    }
                    Err(_) => {
                        // r2 closed; drain r2 and exit.
                        for path in r1.iter() {
                            let hash = calculate_file_hash(&path)?;
                            map2.entry(hash).or_default().push(path);
                        }
                        break;
                    }
                }
            }
        }
    }

    Ok((map1, map2))
}

/// Check if a File or Directory is hidden
fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with("."))
        .unwrap_or(false)
}

/// Compares two directories and returns their file paths.
/// Returns a tuple containing:
/// 1. Files present in both directories.
/// 2. Files unique to the first directory.
/// 3. Files unique to the second directory.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
pub fn compare_directories(
    dir1: &Path,
    dir2: &Path,
    sort: bool,
    skip_hidden: bool,
    relative: bool,
    include_intersection: bool,
    include_unique_dir1: bool,
    include_unique_dir2: bool,
) -> (
    Option<Vec<PathBuf>>,
    Option<Vec<PathBuf>>,
    Option<Vec<PathBuf>>,
) {
    let num_threads = num_cpus::get_physical();

    // Create two channels.
    let (sender1, receiver1) = unbounded();
    let (sender2, receiver2) = unbounded();

    let mut handles = Vec::new();

    // TODO: This is very usefull on SSD but multithreading is useless on HDD
    // Spawn threads.
    for _ in 0..num_threads {
        let r1 = receiver1.clone();
        let r2 = receiver2.clone();
        let handle = thread::spawn(move || worker(r1, r2));
        handles.push(handle);
    }

    // Send messages into both channels.
    for entry in WalkDir::new(dir1)
        .into_iter()
        .filter_entry(|entry| !skip_hidden || !is_hidden(entry))
        .filter_map(Result::ok)
    {
        if entry.path().is_file() {
            sender1.send(entry.path().to_path_buf()).unwrap();
        }
    }

    for entry in WalkDir::new(dir2)
        .into_iter()
        .filter_entry(|entry| !skip_hidden || !is_hidden(entry))
        .filter_map(Result::ok)
    {
        if entry.path().is_file() {
            sender2.send(entry.path().to_path_buf()).unwrap();
        }
    }

    // Close the channels.
    drop(sender1);
    drop(sender2);

    // Collect and combine results.
    let mut combined1: HashMap<Hash, Vec<PathBuf>> = HashMap::new();
    let mut combined2: HashMap<Hash, Vec<PathBuf>> = HashMap::new();

    for handle in handles {
        let (map1, map2) = handle.join().expect("Thread panicked").unwrap();

        for (key, mut vec) in map1 {
            if relative {
                vec = vec
                    .into_iter()
                    .map(|path| path.strip_prefix(dir1).unwrap().to_path_buf())
                    .collect();
            };
            combined1.entry(key).or_default().extend(vec);
        }

        for (key, mut vec) in map2 {
            if relative {
                vec = vec
                    .into_iter()
                    .map(|path| path.strip_prefix(dir2).unwrap().to_path_buf())
                    .collect();
            };
            combined2.entry(key).or_default().extend(vec);
        }
    }

    // Compute intersection, files unique to dir1, files unique to dir2
    let (mut intersection_paths, mut unique_dir1_paths, mut unique_dir2_paths) = compare_hashmaps(
        &combined1,
        &combined2,
        include_intersection,
        include_unique_dir1,
        include_unique_dir2,
    );

    if sort {
        if let Some(ref mut v) = intersection_paths {
            v.sort();
        }
        if let Some(ref mut v) = unique_dir1_paths {
            v.sort();
        }
        if let Some(ref mut v) = unique_dir2_paths {
            v.sort();
        }
    }

    (intersection_paths, unique_dir1_paths, unique_dir2_paths)
}
