# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build, Test, and Quality Commands

```bash
# Build
cargo build
cargo build --release

# Run
cargo run -- .
cargo run -- /path/to/scan --top 20 --depth 2 --threads 16

# Run tests
cargo test
cargo test <test_name>
cargo test -- --nocapture

# Lint and format
cargo check
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo fmt
```

## Architecture and Design

`dscan` is a zero-external-dependency, multi-threaded disk space analyzer for Linux. It achieves high throughput by pairing direct Linux kernel syscalls (`getdents64`) with work-stealing parallelism and thread-local aggregation.

### Key Modules

- **`src/sys.rs`**: Raw Linux ABI definitions. Declares `SYS_GETDENTS64` (architecture-specific: 217 on x86_64, 61 on aarch64), `LinuxDirent64` C struct layout, and POSIX `open`/`close`/`syscall` externs. `open_dir` opens directories with `O_RDONLY | O_DIRECTORY | O_CLOEXEC | O_NOATIME`.
- **`src/scanner.rs`**: Core traversal and synchronization engine:
  - **Raw Directory Traversal (`scan_directory_tree`)**: Reads directory entries in bulk using `getdents64` into a 64 KiB buffer, bypassing libc `readdir` overhead. Falls back to `std::fs::read_dir` if syscall fails.
  - **Filesystem Boundary Guard**: Compares `meta.dev() == root_dev` (`MetadataExt`) to stay on the origin device and prevent traversing virtual or network mounts.
  - **Space Calculation**: Uses `meta.blocks() * 512` to measure actual allocated disk space (accounting for sparse files and filesystem block allocation).
  - **Work-Stealing Concurrency**: Global `QueueState` (`VecDeque<PathBuf>`) protected by `Mutex` and `Condvar`. If a worker finds multiple subdirectories and idle workers exist (`active_workers < threads`), it splits its subdirectory list and pushes half to the global queue.
  - **Contention-Free Aggregation**: Workers accumulate directory sizes up to `--depth` in thread-local `HashMap<PathBuf, u64>` and track largest files in a thread-local `BinaryHeap<Reverse<(u64, PathBuf)>>` min-heap. Results are merged on the main thread after all workers join.
  - **Progress Reporter**: Separate background thread renders an animated spinner and scans status every 60ms by sampling atomics (`total_bytes`, `total_files`, `active_workers`).
- **`src/cli.rs`**: Handcrafted command-line argument parser with zero external crates. Handles `--exclude`, `--top`, `--depth`, `--threads`/`-j`, `-h`/`--help`, and `-V`/`--version`. Excludes `/proc`, `/sys`, `/dev`, `/run`, `/tmp`, and `.git` by default.
- **`src/ui.rs`**: Neon/cyberpunk ANSI terminal formatting. Handles dynamic column width via `ioctl(TIOCGWINSZ)`, Unicode progress bar generation (`make_bar`), spinner updates, and path truncation.
- **`src/format.rs`**: Human-readable binary byte formatting (B, KiB, MiB, GiB, TiB, PiB).

### Invariants & Platform Requirements

- Target OS is Linux. Syscall numbers and `LinuxDirent64` structure layouts depend on Linux kernel interfaces.
- The project intentionally avoids third-party crates (like `clap`, `rayon`, or `nix`) to maintain minimal binary size, instant compilation, and zero runtime dependencies.
