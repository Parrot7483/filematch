use filematch::compare_directories;
use rand_xoshiro::rand_core::{RngCore, SeedableRng};
use rand_xoshiro::Xoshiro256Plus;
use std::fs::{self, File};
use std::io::{self, BufWriter, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn generate_deterministic_file(path: &Path, size: u64, init: u64) -> std::io::Result<()> {
    // Create a buffered writer to reduce the overhead of multiple small writes
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);

    // Initialize the Xoshiro256Plus RNG with the given seed
    let mut rng = Xoshiro256Plus::seed_from_u64(init);

    // Use a larger buffer to write fewer times
    let mut buffer = vec![0_u8; 64 * 1024]; // 64 KiB
    let mut remaining = size;

    while remaining > 0 {
        // Determine the chunk size to avoid overrun
        let chunk_size = std::cmp::min(remaining, buffer.len() as u64) as usize;

        // Fill the buffer slice
        rng.fill_bytes(&mut buffer[..chunk_size]);

        // Write the chunk
        writer.write_all(&buffer[..chunk_size])?;

        remaining -= chunk_size as u64;
    }

    // Ensure all data is flushed
    writer.flush()?;
    Ok(())
}

fn generate_deterministic_file_in_dir(
    path: &Path,
    size: u64,
    init: u64,
) -> std::io::Result<PathBuf> {
    if !path.is_dir() {
        return Err(std::io::Error::new(
            ErrorKind::InvalidInput,
            "Path must be a directory",
        ));
    }

    // Generate the file name using hexadecimal formatting with a .bin extension.
    let file_name = format!("random_{:016x}.bin", init);
    let file_path = path.join(file_name);

    if !file_path.exists() {
        generate_deterministic_file(&file_path, size, init)?;
    }

    Ok(file_path)
}

fn generate_deterministic_numbers(seed: u64, len: usize) -> Vec<u64> {
    // Initialize the generator with the given seed.
    let mut rng = Xoshiro256Plus::seed_from_u64(seed);
    let mut result = Vec::with_capacity(len);

    // Generate len random u64 values.
    for _ in 0..len {
        result.push(rng.next_u64());
    }

    result
}

fn generate_deterministic_files(
    dir: &Path,
    size: u64,
    seed: u64,
    count: usize,
) -> std::io::Result<Vec<PathBuf>> {
    // Generate file seeds deterministically
    let seeds = generate_deterministic_numbers(seed, count);
    let mut files = Vec::with_capacity(count);
    for seed in seeds {
        // Create a file for each seed.
        let path = generate_deterministic_file_in_dir(dir, size, seed)?;
        files.push(path);
    }
    Ok(files)
}

/// Sets up a benchmark file structure in two directories (A and B) based on a given total size.
/// The total size is divided equally among up to four file groups (1GB, 100MB, 10MB, and 1MB).
/// For each group, files are split into three categories:
/// - Files that reside only in directory A.
/// - Files that reside only in directory B.
/// - Files that exist in both directories (generated in A and then copied to B).
/// The number of files in each group is determined dynamically based on the total size.
/// The provided base_seed guarantees deterministic file names and content.
///
/// Returns a tuple of three vectors containing the file paths generated only in A, only in B,
/// and in both directories.
fn setup_benchmark_files(
    dir_a: &Path,
    dir_b: &Path,
    total: u64,
    base_seed: u64,
) -> io::Result<(Vec<PathBuf>, Vec<PathBuf>, Vec<PathBuf>)> {
    // Define file sizes in bytes.
    const SIZE_1GB: u64 = 1 * 1024 * 1024 * 1024;
    const SIZE_100MB: u64 = 100 * 1024 * 1024;
    const SIZE_10MB: u64 = 10 * 1024 * 1024;
    const SIZE_1MB: u64 = 1 * 1024 * 1024;

    // Ensure the directories exist.
    fs::create_dir_all(dir_a)?;
    fs::create_dir_all(dir_b)?;

    let mut dir_ab_files = vec![];
    let mut dir_a_files = vec![];
    let mut dir_b_files = vec![];

    // Divide the total equally into 4 groups.
    let group_total = total / 4;

    // Define potential groups (from largest to smallest) along with labels (labels unused here).
    let potential_groups = [
        (SIZE_1GB, "1GB"),
        (SIZE_100MB, "100MB"),
        (SIZE_10MB, "10MB"),
        (SIZE_1MB, "1MB"),
    ];

    // Only include groups where group_total can accommodate at least one file.
    let mut groups: Vec<(u64, usize)> = potential_groups
        .iter()
        .filter_map(|&(size, _)| {
            if group_total >= size {
                Some((size, (group_total / size) as usize))
            } else {
                None
            }
        })
        .collect();

    // If no group qualifies, default to the smallest file size.
    if groups.is_empty() {
        groups.push((SIZE_1MB, (group_total / SIZE_1MB).max(1) as usize));
    }

    // For each group, split files into three categories: only in A, only in B, and in both.
    for (group_idx, &(file_size, count)) in groups.iter().enumerate() {
        let only_a = count / 3;
        let only_b = count / 3;
        let both = count - only_a - only_b;

        // Derive unique seeds for each category.
        let seed_only_a = base_seed
            .wrapping_add((group_idx as u64) << 48)
            .wrapping_add(0);
        let seed_only_b = base_seed
            .wrapping_add((group_idx as u64) << 48)
            .wrapping_add(1);
        let seed_both = base_seed
            .wrapping_add((group_idx as u64) << 48)
            .wrapping_add(2);

        // Generate files only in subdirectory A.
        dir_a_files.extend(generate_deterministic_files(
            dir_a,
            file_size,
            seed_only_a,
            only_a,
        )?);

        // Generate files only in subdirectory B.
        dir_b_files.extend(generate_deterministic_files(
            dir_b,
            file_size,
            seed_only_b,
            only_b,
        )?);

        // Generate files that will exist in both: create in A then copy to B.
        let files_a = generate_deterministic_files(dir_a, file_size, seed_both, both)?;
        for file in files_a {
            let file_name = file.file_name().unwrap();
            let file_b = dir_b.join(file_name);
            if !file_b.exists() {
                fs::copy(&file, &file_b)?;
            }
            dir_ab_files.push(file);
            dir_ab_files.push(file_b);
        }
    }

    Ok((dir_ab_files, dir_a_files, dir_b_files))
}

// TODO Get available disk space

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create directories and subdirectories
    let base = std::env::temp_dir().join("filematch-bench");
    let dir_a_path = base.join("a");
    let dir_b_path = base.join("b");
    fs::create_dir_all(&base)?;
    fs::create_dir_all(&dir_a_path)?;
    fs::create_dir_all(&dir_b_path)?;

    print!(
        "Setting up benchmark environment in '{}'...",
        base.display()
    );
    io::stdout().flush().unwrap();
    let (mut dir_ab, mut dir_a, mut dir_b) =
        setup_benchmark_files(&dir_a_path, &dir_b_path, 32 * 1024 * 1024 * 1024, 3346523)?;
    println!(" DONE!");

    let mut best_duration = Duration::MAX;
    let times_to_run = 3;

    // Always do one warm up run
    print!("Warm up run...");
    io::stdout().flush().unwrap();
    let (_, _, _) = compare_directories(&dir_a_path, &dir_b_path, false, false, false);
    println!(" DONE!");

    for i in 0..times_to_run {
        let start = Instant::now();
        // Call your function here.
        let (mut dir_12, mut dir_1, mut dir_2) =
            compare_directories(&dir_a_path, &dir_b_path, false, false, false);

        let elapsed = start.elapsed();

        dir_a.sort();
        dir_b.sort();
        dir_ab.sort();
        dir_1.sort();
        dir_2.sort();
        dir_12.sort();

        assert_eq!(dir_ab, dir_12, "Intersection paths mismatch");

        assert_eq!(dir_a, dir_1, "Unique dir1 paths mismatch");

        assert_eq!(dir_b, dir_2, "Unique dir2 paths mismatch");

        println!("Run #{} took: {:.3?}", i + 1, elapsed);

        if elapsed < best_duration {
            best_duration = elapsed;
        }
    }

    println!("\nBest run: {:.3?}", best_duration);

    Ok(())
}
