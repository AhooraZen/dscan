# dscan

Fast, multi-threaded disk usage analyzer for Linux and Windows. It scans directory trees using raw operating system syscalls and displays directory sizes with accurate recursive rollups in your terminal.

Zero external dependencies, fast compilation, and faster than GNU `du` on large directory trees.

---

## Why dscan

Most disk space tools either wrap standard library `readdir` or pull in heavy dependency trees. Standard POSIX or Win32 traversal issues individual `stat` or `GetFileAttributesExW` calls for every file, burning time in path resolution, permission checks, and page cache locks.

`dscan` bypasses standard library overhead:
- **Linux**: Direct `getdents64` calls with 512 KiB page-aligned buffers to read directory entries in bulk.
- **Linux**: Kernel `statx` with `AT_STATX_DONT_SYNC` relative to directory file descriptors, fetching block counts without filesystem sync stalls.
- **Windows**: Bulk directory queries via `GetFileInformationByHandleEx` with `FileIdBothDirectoryInfo`, reading `AllocationSize` directly from directory records with zero secondary stat calls.
- **Windows**: Automatic reparse point detection to avoid recursive junction loops.
- **Concurrency**: Lock-free Chase-Lev work-stealing circular deques per worker thread, eliminating mutex lock contention across cores.
- **Accuracy**: Bottom-up hierarchical size rollup so parent directory totals always reflect their full subtrees.
- **Terminal UI**: Responsive ANSI layout that dynamically sizes borders, progress bars, and path truncations to fit anything from phone screens (Termux) to wide monitors without line wrapping.
- **Zero Dependencies**: Built entirely with the Rust standard library and raw OS platform interfaces.

---

## Installation

From crates.io:

```bash
cargo install dscan
```

Or build from source:

```bash
git clone https://github.com/parchlinux/dscan.git
cd dscan
cargo build --release
```

Binary will be at `target/release/dscan` (or `target/release/dscan.exe` on Windows).

---

## Usage

```bash
# Scan current directory
dscan .

# Scan root filesystem, showing top 30 offenders
sudo dscan / --top 30 --exclude /var/lib/docker

# Scan Termux home on Android, traversing storage symlinks and mount points
dscan ~ -L -x

# Limit directory rollup depth to 2 levels
dscan /home/user --depth 2 --top 15

# Use 8 worker threads
dscan /data -j 8
```

### Options

| Flag | Description | Default |
| --- | --- | --- |
| `TARGET_PATH` | Path to scan | `.` (current directory) |
| `--exclude <PATTERN>` | Path or folder name to skip (e.g. `--exclude .cache`) | System defaults (`/proc`, `/sys`, etc. on Linux; `$Recycle.Bin`, etc. on Windows) |
| `--top <N>` | Number of largest directories and files to show | `25` |
| `--depth <N>` | Maximum directory depth to display (`0` or omitted = unlimited) | unlimited |
| `-j`, `--threads <N>` | Number of parallel worker threads | CPU core count (max 32) |
| `-L`, `--follow-symlinks` | Follow directory symlinks (e.g. Android Termux `~/storage`) | off |
| `-x`, `--cross-device` | Cross filesystem mount boundaries | off |
| `-V`, `--version` | Print version information | |
| `-h`, `--help` | Print help | |

---

## License

Dual-licensed under MIT or Apache 2.0 at your option.
