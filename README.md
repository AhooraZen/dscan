# dscan

<p align="center">
  <strong>High-performance disk space analyzer for Linux, Windows, and Android.</strong><br>
  Direct kernel syscalls. Lock-free work-stealing. Zero external dependencies.
</p>

<p align="center">
  <a href="https://crates.io/crates/dscan"><img src="https://img.shields.io/crates/v/dscan.svg?style=flat-square&color=50fa7b" alt="Crates.io"></a>
  <a href="https://github.com/AhooraZen/dscan/releases"><img src="https://img.shields.io/github/v/release/AhooraZen/dscan?style=flat-square&color=8be9fd" alt="GitHub Release"></a>
  <a href="https://github.com/AhooraZen/dscan/actions"><img src="https://img.shields.io/github/actions/workflow/status/AhooraZen/dscan/ci.yml?style=flat-square&branch=main" alt="CI Status"></a>
  <a href="LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg?style=flat-square" alt="License"></a>
</p>

---

<p align="center">
  <img src="https://raw.githubusercontent.com/AhooraZen/dscan/main/assets/demo-cli.svg" alt="dscan CLI Terminal Demo" width="900"/>
</p>

<div align="center">

| Mode | Interface | Target OS | Binary size | Dependencies |
| :--- | :--- | :--- | :--- | :--- |
| **CLI** | Terminal | Linux (x86_64, aarch64), Windows x64 | ~450 KB | Pure stdlib and OS kernel ABI |
| **GUI** | Cushion treemap | Linux x64, Windows x64 | ~35 MB | Pure Rust (`gpui-kit` / Zed engine, WGPU) |
| **Android** | Jetpack Compose | Android 8+ (ARM64) | ~8 MB | Kotlin and Rust JNI (`libdscan.so`) |

</div>

---

## Why dscan?

Most disk usage tools are either slow or heavy.

GNU `du` and `ncdu` call libc `readdir()` and fire an `lstat` for every single file. On modern NVMe SSDs with millions of small files, path resolution, permission checks, and page cache locks slow traversal down to a crawl.

Other Rust tools bring multi-threading, but drag in dozens of crates, take minutes to compile, and allocate heap strings for every directory entry.

`dscan` talks to the kernel directly:

1. **Linux**: Uses raw `getdents64` with page-aligned buffers to pull directory entries in bulk, skipping libc `readdir` wrappers entirely. Pairs with `statx` (`AT_STATX_DONT_SYNC`, `STATX_BLOCKS`) and `openat2(RESOLVE_NO_XDEV)` so it never walks off the current filesystem mount.
2. **Windows**: Skips `FindFirstFileW`/`FindNextFileW` and calls `NtQueryDirectoryFileEx` with `FileIdBothDirectoryInfo`. Reads cluster `AllocationSize` right out of directory records without follow-up stat calls.
3. **Lock-free concurrency**: Workers run on atomic Chase-Lev work-stealing circular deques. Threads push and pop tasks in LIFO order with no lock contention. Idle workers steal tasks in FIFO batches.
4. **Zero-allocation bump arena**: No `PathBuf` or `String` allocations per file. Trees live in a chunked bump arena (`DirArena`) linked by 32-bit parent-child offsets.
5. **Adaptive thread scaling**: Checks whether the target drive is a spinning disk or an SSD via Linux sysfs (`queue/rotational`) or Windows `IOCTL_STORAGE_QUERY_PROPERTY`. Clamps to 2-4 threads on HDDs to stop disk head thrashing (cutting scan time from over a minute to 1.2s on large trees), and opens up to `cores * 4` on SSDs.
6. **Zero CLI dependencies**: Builds in seconds. No `clap`, no `rayon`, no `nix`, no `libc`.

---

## Feature comparison

| Feature | `du` | `ncdu` | `dust` | `WinDirStat` | `dscan` |
| :--- | :---: | :---: | :---: | :---: | :---: |
| **Multi-threaded work stealing** | No | No | Rayon | No | **Chase-Lev deques** |
| **Bulk kernel syscalls** | No (`readdir`) | No (`readdir`) | No (`std::fs`) | No (`Win32`) | **`getdents64` / `NtQueryDirectoryFile`** |
| **Filesystem block allocation** | Yes | Yes | Yes | Buggy on sparse | **Exact (`stx_blocks * 512` / `AllocationSize`)** |
| **CLI dependencies** | libc | ncurses | 40+ crates | C++ MFC | **Zero (pure stdlib)** |
| **Machine-readable output** | Text | Export | Text | None | **`--json` (RFC 8259)** |
| **Extension breakdown** | No | No | No | File list | **`--ext` table** |
| **Interactive cushion treemap** | No | No | No | CPU GDI | **GPUI-Kit (120 FPS GPU)** |
| **Safe trash and reveal** | No | Delete | Delete | Explorer | **`trash` crate with safety guards** |
| **HDD/SSD adaptive threads** | No | No | No | No | **Auto-detects rotational media** |
| **Android app** | No | No | No | No | **Jetpack Compose and Rust JNI** |

---

## Benchmarks

Benchmarked with [`hyperfine`](https://github.com/sharkdp/hyperfine) on Arch Linux (Linux 7.2-zen, AMD Ryzen, NVMe SSD, Btrfs) scanning a real-world home directory containing **1,121,462 files** across **148,230 directories**:

| Command | Wall time (mean ± σ) | Total CPU time | Accounting depth | Binary size | Dependencies |
| :--- | :---: | :---: | :--- | :---: | :---: |
| **`dscan ~ --top 20`** | **1.492 s ± 0.034 s** | **4.57 s** | **Top 20 files + Top 25 dirs + Extension breakdown + Arena** | **~500 KB** | **Zero (pure stdlib)** |
| `dust ~` | 1.333 s ± 0.031 s | 7.73 s (+69% CPU) | Sized directory bars | ~3 MB | 40+ crates |
| `du -sh ~` | 2.210 s ± 0.011 s | 2.20 s | Single total size | ~50 KB | libc |

> **Key takeaway**: `dscan` completes a full 1.12-million file scan in **1.49 seconds** while running full top-file ranking, file extension aggregation, and 32-bit bump arena tree building — consuming **41% less CPU** than `dust` (4.57s vs 7.73s total CPU) with **zero external crates**.

---

## Desktop GUI (`dscan-gui`)

`dscan-gui` is a pure-Rust desktop visualizer built on [gpui-kit](https://gpui-kit.com) (Zed's GPU engine). No WebKit, no node_modules, no embedded browsers.

<p align="center">
  <img src="https://raw.githubusercontent.com/AhooraZen/dscan/main/assets/demo-gui.svg" alt="dscan Cushion Treemap GUI Preview" width="950"/>
</p>

- **GPU cushion treemap**: Implements van Wijk's cushion shading algorithm ($I = I_a + I_d \max(0, \mathbf{N} \cdot \mathbf{L})$). Nested folders show up as smooth rounded ridges at 120 FPS.
- **Dark and light themes**: Deep navy dark theme and clean light theme. Switch directly from the title bar.
- **Native file actions**: Right-click any tile or directory row to reveal in your system file manager (Linux D-Bus/xdg-open, Windows Explorer `/select,`, macOS `open -R`).
- **Safe trash**: Moves files to the system recycle bin via the `trash` crate, with `is_safe_to_trash` guards to stop accidental deletion of root or system paths.
- **In-memory subtree updates**: Trashed items subtract their size up the tree immediately, no rescan needed.
- **System monitor**: Live CPU and RAM counters in the header bar.
- **Extension legend**: Color-coded file type bar so you know what you are looking at.

---

## Android app (`dscan-android`)

Android disk analyzer powered by the same Rust scanner through JNI, with a Jetpack Compose Material 3 interface.

- **Rust JNI engine**: `dscan-core` compiled as `libdscan.so` (ARM64). Runs `getdents64` and `statx` on Android kernel.
- **Animated circular gauge**: Real-time progress ring with bytes scanned, files per second, and file count.
- **Squarified treemap**: Bruls-Huizing-van Wijk algorithm for clean rectangular layouts instead of thin strips. Tap any tile for details, with haptic feedback.
- **Material You**: On Android 12+, pulls accents from your system wallpaper. Falls back to dark/light palettes on Android 11 and older.
- **Bottom navigation**: Treemap, Directory Tree, File Types, and Settings.
- **Folder picker**: Storage Access Framework (SAF) folder selection through a Browse button, plus quick-access chips for common paths (`/sdcard`, `Downloads`, `DCIM`, `WhatsApp`, `Android/data`).
- **Settings**: Adjust thread count (Auto/2/4/8/16), treemap depth (2-8), max nodes (500-5000), and theme mode.
- **Extension legend**: Color strip mapping file types to treemap tiles.

---

## Installation

### Package managers

#### Arch Linux (AUR)

```bash
# Pre-built binary
paru -S dscan-bin
paru -S dscan-gui-bin   # Desktop GUI

# Or build from source
paru -S dscan
```

#### Homebrew (Linux)

```bash
brew install AhooraZen/dscan/dscan
```

#### Scoop (Windows)

```bash
scoop bucket add dscan https://github.com/AhooraZen/scoop-dscan
scoop install dscan       # CLI
scoop install dscan-gui   # Desktop GUI
```

### From crates.io (CLI)

```bash
cargo install dscan
```

### Pre-built binaries (GitHub Releases)

Grab pre-compiled builds from [GitHub Releases](https://github.com/AhooraZen/dscan/releases/latest):

- **Linux x86_64 CLI**: `dscan-linux-x86_64.tar.gz`
- **Linux aarch64 CLI**: `dscan-linux-aarch64.tar.gz`
- **Windows x64 CLI**: `dscan-windows-x64.zip`
- **Linux Desktop GUI**: `dscan-gui-linux-x86_64.tar.gz`
- **Windows Desktop GUI**: `dscan-gui-windows-x64.zip`
- **Android APK (ARM64)**: `dscan-android-arm64.apk`

### Build from source

```bash
git clone https://github.com/AhooraZen/dscan.git
cd dscan

# Build CLI
cargo build --release -p dscan

# Run Desktop GUI
cargo run --release -p dscan-gui

# Build Android JNI library (requires cargo-ndk + Android NDK)
cargo ndk -t arm64-v8a build --release -p dscan-android
```

---

## Usage

### CLI quick scan

```bash
# Scan current directory
dscan .

# Scan specific folder, show top 20 largest directories
dscan /home/user --top 20

# Show extension usage breakdown table
dscan /data --ext

# Output machine-readable JSON (RFC 8259)
dscan /data --json > scan.json

# Scan with 16 worker threads, skipping specific directories
dscan / --threads 16 --exclude /proc --exclude /sys --exclude ~/.cache

# Scan everything including .git and node_modules
dscan . --all
```

### Command-line options

| Flag | Argument | Description | Default |
| :--- | :--- | :--- | :--- |
| `TARGET_PATH` | Path | Root path to scan | `.` (current directory) |
| `--top` | `<N>` | Number of largest directories and files to display | `25` |
| `--depth` | `<N>` | Maximum directory depth for recursive rollup | Unlimited |
| `-j`, `--threads` | `<N>` | Number of concurrent worker threads | Auto (SSD: `cores * 4`, HDD: `cores` clamped 2-4) |
| `--exclude` | `<PATTERN>` | Path or folder name pattern to skip | System defaults (`/proc`, `/sys`, etc.) |
| `-a`, `--all` | None | Disable default exclusions (scan `.git`, `node_modules`, `target`) | Off |
| `--no-default-excludes` | None | Same as `-a` / `--all` | Off |
| `--json` | None | Output machine-readable RFC 8259 JSON to stdout | Off |
| `--ext` | None | Display top 10 file extensions breakdown table | Off |
| `-L`, `--follow-symlinks` | None | Traverse directory symlinks | Off |
| `-x`, `--cross-device` | None | Traverse across filesystem mount boundaries | Off |
| `-V`, `--version` | None | Print version | |
| `-h`, `--help` | None | Show help menu | |

---

## Architecture

```
dscan/
├── crates/
│   ├── dscan-core/       # Kernel traversal engine, Chase-Lev work-stealing, arena rollup
│   │   ├── src/sys/      # Raw Linux getdents64, statx, io_uring, openat2 & Windows NT ABI
│   │   ├── src/arena.rs  # Zero-allocation bump directory arena
│   │   ├── src/scanner.rs # Work-stealing traversal coordinator
│   │   ├── src/snapshot.rs # Treemap node builder and scan session manager
│   │   └── src/work_stealing.rs # Chase-Lev lock-free task deques
│   ├── dscan-cli/        # Neon ANSI terminal UI, JSON serializer, zero dependencies
│   ├── dscan-gui/        # Pure Rust GPUI-Kit desktop visualizer (Zed GPUI / WGPU)
│   │   ├── src/views/    # Directory tree, cushion treemap, legend, status bar
│   │   ├── src/treemap/  # Squarified layout and cushion shading algorithms
│   │   └── src/system.rs # File reveal and safe trash operations
│   └── dscan-android/    # Rust JNI bridge for Android (cdylib -> libdscan.so)
├── android/              # Jetpack Compose Material 3 app (Kotlin)
│   └── app/src/main/kotlin/com/dscan/app/
│       ├── DscanBridge.kt    # JNI native method declarations
│       ├── MainActivity.kt   # App entry, scan lifecycle, SAF folder picker
│       └── ui/               # Theme, MainScreen, TreemapCanvas, ScanGauge, Settings
└── plans/                # Implementation plans (32 executed, audit trail)
```

---

## Design decisions

- **Direct Linux syscalls**: No libc wrapper on Linux. All syscalls (`getdents64`, `statx`, `openat2`, `io_uring`, `prlimit64`) use raw `syscall()` with architecture-specific numbers (x86_64: 217, aarch64: 61 for `getdents64`).
- **Disk allocation, not file length**: Reads `stx_blocks * 512` on Linux and `AllocationSize` on Windows. Gives actual space consumed on disk, handling sparse files and block rounding correctly.
- **Filesystem boundary guard**: `openat2(RESOLVE_NO_XDEV)` stops traversal from leaking into mounted virtual filesystems (`/proc`, `/sys`, `/dev`) without relying on hardcoded exclusion paths.
- **Storage-aware thread counts**: Inspects `/sys/dev/block/<major>:<minor>/queue/rotational` on Linux and `StorageDeviceSeekPenaltyProperty` on Windows. Spinning drives get 2-4 threads to prevent head thrashing; NVMe drives scale up to 64.
- **Linear rollup**: Directory sizes roll up bottom-up through a depth-sorted hash table in O(N) time, avoiding the O(N^2) parent-lookup freezes common in naive recursive aggregators.

---

## License

Dual-licensed under either:

- MIT License ([LICENSE-MIT](LICENSE-MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))

at your option.
