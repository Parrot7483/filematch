# filematch

Filematch compares the content of files in two directories by calculating their hash. This approach helps you find duplicates or check if files have changed.

## Usage

```
Compares files between two directories by hash

Usage: filematch [OPTIONS] <directory1> <directory2>

Arguments:
  <directory1>  The first directory to compare
  <directory2>  The second directory to compare

Options:
      --sort         Sort output paths
      --skip-hidden  Skip hidden files and directories
      --relative     Display output paths relative to argument directory
  -h, --help         Print help
  -V, --version      Print version
```

## Technical Overview

Filematch is developed in Rust and utilizes the following crates:

- ![Crossbeam](https://github.com/crossbeam-rs/crossbeam): Enables efficient multi-threading.
- ![BLAKE3](https://github.com/BLAKE3-team/BLAKE3): Modern cryptographic hashing algorithm
- ![clap](https://github.com/clap-rs/clap): Provides command-line argument parsing and option handling.
- ![walkdir](https://github.com/BurntSushi/walkdir): Facilitates recursive directory traversal.

To compile the program, simply run `cargo build --release` or `nix build`.

## Planned improvements and features:

### Packaging
- [ ] Package for nixpkgs
- [ ] CI/CD

### Development 
- [x] Implement benchmark testing to evaluate and optimize performance.

### Logging and Debugging
- [ ] Add verbose logging to provide detailed information about the comparison process, 
- [ ] Progress Bar.

### Hashing
- Add support for selecting different hash algorithms:
  - [x] **Cryptographic hashing**: Add cryptographic for scenarios where stronger hashing is required
  - [ ] **Partial hashing**: Implement an algorithm that compares only the first page of each file for quicker approximations.

### Output Customization
- Introduce options to filter output:
  - [ ] Display only the intersection (common files between directories).
  - [ ] Display only unique files in `directory1` or `directory2`.
  - [ ] Ouput as JSON

## Example 1: Basic
Directory structure:
```
files_test
├── source_dir
│   ├── common1.txt
│   ├── duplicate1.txt
│   ├── duplicate2.txt
│   ├── emptydir
│   ├── largefile.txt
│   ├── subdir
│   │   ├── common3.txt
│   │   └── unique_sub1.txt
│   └── unique1.txt
└── target_dir
    ├── common2.txt
    ├── emptydir
    ├── largefile.txt
    ├── subdir
    │   ├── common4.txt
    │   └── unique_sub2.txt
    └── unique2.txt

7 directories, 12 files
```
Run the tool:
```
filematch --sort files_test/dir1 files_test/dir2
```
Output: 
```
Files both in 'files_test/dir1/' and 'files_test/dir2/':
files_test/dir1/common1.txt
files_test/dir1/largefile.txt
files_test/dir1/subdir/common3.txt
files_test/dir2/common2.txt
files_test/dir2/largefile.txt
files_test/dir2/subdir/common4.txt

Files unique in 'files_test/dir1/':
files_test/dir1/duplicate1.txt
files_test/dir1/duplicate2.txt
files_test/dir1/subdir/unique_sub1.txt
files_test/dir1/unique1.txt

Files unique in 'files_test/dir2/':
files_test/dir2/subdir/unique_sub2.txt
files_test/dir2/unique2.txt
```

## Example 2: Skipping Hidden Files and Relative Paths
Directory structure:
```
hidden_files_test
├── source_dir
│   ├── common1.txt
│   ├── .common3.txt
│   ├── duplicate1.txt
│   ├── duplicate2.txt
│   ├── .hidden
│   │   └── common5.txt
│   └── unique1.txt
└── target_dir
    ├── common2.txt
    ├── .common4.txt
    ├── .hidden
    │   └── common5.txt
    └── unique2.txt

5 directories, 10 files
```
Run the tool with `--skip-hidden` and `--relative`:
```
filematch --skip-hidden --relative hidden_files_test/dir1 hidden_files_test/dir2
```
Example output:
```
Files both in 'hidden_files_test/dir1' and 'hidden_files_test/dir2':
common1.txt
common2.txt

Files unique in 'hidden_files_test/source_dir':
duplicate1.txt
duplicate2.txt
unique1.txt

Files unique in 'hidden_files_test/target_dir':
unique2.txt
```
Hidden files like `.common3.txt` and `.common4.txt` and directories such as `.hidden` are ignored in this output.

## Contributing
Feel free to open issues or create pull requests if you would like to improve this tool.

## License
This project is available under the GNU General Public License v3. See the ![LICENSE](https://github.com/Parrot7483/filematch/blob/main/LICENSE) file for details.
