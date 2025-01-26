use crossbeam_channel::{select, unbounded, Receiver};
use gxhash::GxHasher;
use num_cpus;
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::hash::Hasher;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::thread;
use walkdir::{DirEntry, WalkDir};

// TODO: Options to only display intersection, first or second (with no title)
// TODO: CI/CD
// TODO: Options to add count
// TODO: Verbose log
// TODO: Crypto hash algo option
// TODO: Use GxHasher vaes avx512 when available
// TODO: Benchmark calcualte_file_hash

/// A generic function that calculates a file hash using the provided hasher type.
#[inline(always)]
fn calculate_file_hash_generic<H>(path: &Path) -> io::Result<u64>
where
    H: Hasher + Default,
{
    let mut file = File::open(path)?;
    let mut hasher = H::default();
    let mut buffer = vec![0; 64 * 1024];

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.write(&buffer[..bytes_read]);
    }

    Ok(hasher.finish())
}

/// A function that calculates a file hash either using the
/// DefaultHasher or GxHasher based on if aes or sse2 is available
fn calculate_file_hash(path: &Path) -> io::Result<u64> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if std::is_x86_feature_detected!("aes") {
            calculate_file_hash_generic::<GxHasher>(path)
        } else {
            calculate_file_hash_generic::<DefaultHasher>(path)
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        if std::is_aarch64_feature_detected!("sse2") {
            calculate_file_hash_generic::<GxHasher>(path)
        } else {
            calculate_file_hash_generic::<DefaultHasher>(path)
        }
    }

    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64", target_arch = "aarch64")))]
    {
        calculate_file_hash_generic::<DefaultHasher>(path)
    }
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
fn compare_hashmaps<K: Eq + std::hash::Hash + std::clone::Clone, V: Clone>(
    map1: &HashMap<K, Vec<V>>,
    map2: &HashMap<K, Vec<V>>,
) -> (Vec<V>, Vec<V>, Vec<V>) {
    // Convert the keys of both maps into `HashSet`s
    let set1: HashSet<_> = map1.keys().cloned().collect();
    let set2: HashSet<_> = map2.keys().cloned().collect();

    // Compute the intersection keys
    let intersection_keys: HashSet<_> = set1.intersection(&set2).cloned().collect();

    // Create the `intersection` Vec by flattening all values for the intersecting keys
    let intersection: Vec<V> = intersection_keys
        .into_iter()
        .flat_map(|key| {
            map1.get(&key)
                .into_iter()
                .chain(map2.get(&key))
                .flat_map(|v| v.iter().cloned())
        })
        .collect();

    // Create the `only_in_a` Vec by flattening all values for the keys unique to `map1`
    let only_in_a: Vec<V> = map1
        .iter()
        .filter(|(key, _)| !set2.contains(*key))
        .flat_map(|(_, value)| value.iter().cloned())
        .collect();

    // Create the `only_in_b` Vec by flattening all values for the keys unique to `map2`
    let only_in_b: Vec<V> = map2
        .iter()
        .filter(|(key, _)| !set1.contains(*key))
        .flat_map(|(_, value)| value.iter().cloned())
        .collect();

    (intersection, only_in_a, only_in_b)
}

/// Check if a File or Directory is hidden
fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with("."))
        .unwrap_or(false)
}

fn worker(
    r1: Receiver<PathBuf>,
    r2: Receiver<PathBuf>,
) -> Result<(HashMap<u64, Vec<PathBuf>>, HashMap<u64, Vec<PathBuf>>), io::Error> {
    let mut map1: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    let mut map2: HashMap<u64, Vec<PathBuf>> = HashMap::new();

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

/// Compares two directories and returns their file paths.
/// Returns a tuple containing:
/// 1. Files present in both directories.
/// 2. Files unique to the first directory.
/// 3. Files unique to the second directory.
pub fn compare_directories(
    dir1: &Path,
    dir2: &Path,
    sort: bool,
    skip_hidden: bool,
    relative: bool,
) -> (Vec<PathBuf>, Vec<PathBuf>, Vec<PathBuf>) {
    let num_threads = num_cpus::get_physical();

    // Create two channels.
    let (sender1, receiver1) = unbounded();
    let (sender2, receiver2) = unbounded();

    let mut handles = Vec::new();

    // Spawn four threads.
    for _ in 0..num_threads {
        let r1 = receiver1.clone();
        let r2 = receiver2.clone();
        let handle = thread::spawn(move || worker(r1, r2));
        handles.push(handle);
    }

    // Send messages into both channels.
    WalkDir::new(dir1)
        .into_iter()
        .filter_entry(|e| !skip_hidden || !is_hidden(e))
        .filter_map(|entry| match entry {
            Ok(entry) if entry.path().is_file() => Some(Ok(entry.path().to_path_buf())),
            Ok(_) => None,
            Err(e) => Some(Err(io::Error::new(io::ErrorKind::Other, e))),
        })
        .for_each(|result| match result {
            Ok(path) => sender1.send(path).unwrap(),
            Err(e) => panic!("Error retrieving path: {}", e),
        });

    WalkDir::new(dir2)
        .into_iter()
        .filter_entry(|e| !skip_hidden || !is_hidden(e))
        .filter_map(|entry| match entry {
            Ok(entry) if entry.path().is_file() => Some(Ok(entry.path().to_path_buf())),
            Ok(_) => None,
            Err(e) => Some(Err(io::Error::new(io::ErrorKind::Other, e))),
        })
        .for_each(|result| match result {
            Ok(path) => sender2.send(path).unwrap(),
            Err(e) => panic!("Error retrieving path: {}", e),
        });

    // Close the channels.
    drop(sender1);
    drop(sender2);

    // Collect and combine results.
    let mut combined1: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    let mut combined2: HashMap<u64, Vec<PathBuf>> = HashMap::new();

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
    let (mut intersection_paths, mut unique_dir1_paths, mut unique_dir2_paths) =
        compare_hashmaps(&combined1, &combined2);

    if sort {
        intersection_paths.sort();
        unique_dir1_paths.sort();
        unique_dir2_paths.sort();
    }

    (intersection_paths, unique_dir1_paths, unique_dir2_paths)
}
