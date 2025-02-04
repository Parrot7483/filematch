use filematch::compare_directories;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn test_general() -> Result<(), Box<dyn std::error::Error>> {
    // Create a temporary base directory
    let base_dir = std::env::temp_dir().join("test_dirs");
    let dir1 = base_dir.join("dir1");
    let dir2 = base_dir.join("dir2");

    // Create directories and subdirectories
    fs::create_dir_all(dir1.join("subdir"))?;
    fs::create_dir_all(dir2.join("subdir"))?;

    // Add empty directories
    fs::create_dir_all(dir1.join("emptydir"))?;
    fs::create_dir_all(dir2.join("emptydir"))?;

    // Create overlapping files in both dir1 and dir2
    let common1 = create_file(&dir1.join("common1.txt"), "Common file content")?;
    let common2 = create_file(&dir2.join("common2.txt"), "Common file content")?;
    let common3 = create_file(&dir1.join("subdir/common3.txt"), "Another common file")?;
    let common4 = create_file(&dir2.join("subdir/common4.txt"), "Another common file")?;

    // Create unique files in dir1
    let unique1 = create_file(&dir1.join("unique1.txt"), "Unique file in dir1")?;
    let unique_sub1 = create_file(
        &dir1.join("subdir/unique_sub1.txt"),
        "Unique file in dir1/subdir1",
    )?;

    // Create duplicate file in dir1
    let duplicate1 = create_file(&dir1.join("duplicate1.txt"), "duplicate")?;
    let duplicate2 = create_file(&dir1.join("duplicate2.txt"), "duplicate")?;

    // Create unique files in dir2
    let unique2 = create_file(&dir2.join("unique2.txt"), "Unique file in dir2")?;
    let unique_sub2 = create_file(
        &dir2.join("subdir/unique_sub2.txt"),
        "Unique file in dir2/subdir2",
    )?;

    // Simulate expected paths in each category
    let expected_intersection: HashSet<PathBuf> = vec![common1, common2, common3, common4]
        .into_iter()
        .collect();

    let expected_unique_dir1: HashSet<PathBuf> = vec![unique1, unique_sub1, duplicate1, duplicate2]
        .into_iter()
        .collect();

    let expected_unique_dir2: HashSet<PathBuf> = vec![unique2, unique_sub2].into_iter().collect();

    // Call the `compare_directories` function
    let (intersection_paths, unique_dir1_paths, unique_dir2_paths) =
        compare_directories(&dir1, &dir2, false, false, false, false);

    // Convert results to HashSet for comparison
    let intersection_set: HashSet<_> = intersection_paths.into_iter().collect();
    let unique_dir1_set: HashSet<_> = unique_dir1_paths.into_iter().collect();
    let unique_dir2_set: HashSet<_> = unique_dir2_paths.into_iter().collect();

    // Assertions
    assert_eq!(
        intersection_set, expected_intersection,
        "Intersection paths mismatch"
    );
    assert_eq!(
        unique_dir1_set, expected_unique_dir1,
        "Unique dir1 paths mismatch"
    );
    assert_eq!(
        unique_dir2_set, expected_unique_dir2,
        "Unique dir2 paths mismatch"
    );

    Ok(())
}

#[test]
fn test_hidden() -> Result<(), Box<dyn std::error::Error>> {
    // Create a temporary base directory
    let base_dir = std::env::temp_dir().join("test_dirs_hidden");
    let dir1 = base_dir.join("dir1");
    let dir2 = base_dir.join("dir2");

    // Create directories and subdirectories
    fs::create_dir_all(&dir1)?;
    fs::create_dir_all(&dir2)?;

    // Create overlapping files in both dir1 and dir2
    let common1 = create_file(&dir1.join("common1.txt"), "Common file content")?;
    let common2 = create_file(&dir2.join("common2.txt"), "Common file content")?;
    let _ = create_file(&dir1.join(".common3.txt"), "Another common file")?;
    let _ = create_file(&dir2.join(".common4.txt"), "Another common file")?;

    // Add hidden directory
    fs::create_dir_all(dir1.join(".hidden"))?;
    fs::create_dir_all(dir2.join(".hidden"))?;

    let _ = create_file(&dir1.join(".hidden/common5.txt"), "More common file")?;
    let _ = create_file(&dir2.join(".hidden/common5.txt"), "More common file")?;

    // Create unique files in dir1
    let unique1 = create_file(&dir1.join("unique1.txt"), "Unique file in dir1")?;

    // Create duplicate file in dir1
    let duplicate1 = create_file(&dir1.join("duplicate1.txt"), "duplicate")?;
    let duplicate2 = create_file(&dir1.join("duplicate2.txt"), "duplicate")?;

    // Create unique files in dir2
    let unique2 = create_file(&dir2.join("unique2.txt"), "Unique file in dir2")?;

    // Simulate expected paths in each category
    let expected_intersection: HashSet<_> = vec![common1, common2].into_iter().collect();

    let expected_unique_dir1: HashSet<_> =
        vec![unique1, duplicate1, duplicate2].into_iter().collect();

    let expected_unique_dir2: HashSet<_> = vec![unique2].into_iter().collect();

    // Call the `compare_directories` function
    println!("{:?}", expected_intersection);
    let (intersection_paths, unique_dir1_paths, unique_dir2_paths) =
        compare_directories(&dir1, &dir2, false, true, false, false);

    // Convert results to HashSet for comparison
    let intersection_set: HashSet<_> = intersection_paths.into_iter().collect();
    let unique_dir1_set: HashSet<_> = unique_dir1_paths.into_iter().collect();
    let unique_dir2_set: HashSet<_> = unique_dir2_paths.into_iter().collect();

    // Assertions
    assert_eq!(
        intersection_set, expected_intersection,
        "Intersection paths mismatch"
    );
    assert_eq!(
        unique_dir1_set, expected_unique_dir1,
        "Unique dir1 paths mismatch"
    );
    assert_eq!(
        unique_dir2_set, expected_unique_dir2,
        "Unique dir2 paths mismatch"
    );

    Ok(())
}

/// Helper function to create a file with specified content
pub fn create_file(path: &Path, content: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut file = fs::File::create(path)?;
    use std::io::Write;
    file.write_all(content.as_bytes())?;
    Ok(path.to_path_buf())
}
