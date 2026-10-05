# dscan

Fast, multi-threaded disk usage analyzer for Linux. It scans large filesystems using raw kernel syscalls and displays directory sizes with accurate recursive rollups in your terminal.

Zero external dependencies, instant compilation, and faster than GNU `du` on large directory trees.

---

## Why dscan

Most disk space tools either wrap standard library `readdir` or introduce heavy dependency trees. Standard POSIX traversal issues individual `stat` calls per file, which burns time in VFS path resolution and page cache sync locks.

`dscan` bypasses libc overhead completely:
- Raw `getdents64` calls with 512 KiB page-aligned buffers to read directory entries in bulk.
- Kernel `statx` with `AT_STATX_DONT_SYNC` relative to open directory file descriptors to fetch block counts without filesystem sync stalls.
- Lock-free Chase-Lev work-stealing circular deques per worker thread, eliminating mutex lock contention across cores.
- Bottom-up hierarchical size rollup so parent directory totals always reflect their full subtrees accurately.
- Responsive terminal UI that dynamically sizes borders, progress bars, and path truncations to fit anything from phone screens (Termux) to wide monitors without text wrapping.
- Zero external dependencies. Built entirely with Rust stdlib and raw Linux system calls.

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
sudo cp target/release/dscan /usr/local/bin/
```

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
| `--exclude <PATTERN>` | Path or folder name to skip (e.g. `--exclude .cache`) | `/proc`, `/sys`, `/dev`, `/run`, `/tmp`, `.git` |
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
