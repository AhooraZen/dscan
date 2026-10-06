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

| Mode | Interface | Target OS | Binary Size | Dependencies |
| :--- | :--- | :--- | :--- | :--- |
| **CLI** | Terminal | Linux (x86_64, aarch64), Windows x64 | ~450 KB | **Zero (pure stdlib + OS kernel ABI)** |
| **GUI** | Cushion Treemap | Linux x64, Windows x64 | ~35 MB | **Pure Rust (`gpui-kit` / Zed engine, WGPU)** |
| **Android** | Jetpack Compose | Android 8+ (ARM64) | ~8 MB | **Kotlin + Rust JNI (`libdscan.so`)** |

</div>

---

## Why dscan?

Most disk usage tools are either slow or bloated.

Traditional tools like GNU `du` and `ncdu` rely on libc's `readdir()` and issue an individual `lstat` syscall for every file they discover. On modern NVMe SSDs with millions of small files, path resolution, permission checks, and page cache locks degrade traversal throughput.

Modern Rust tools improve multi-threading, but often pull in dozens of third-party crates, take minutes to compile, and allocate heap strings for every directory entry.

`dscan` takes a direct, low-overhead approach:

1. **Linux**: Calls raw `getdents64` directly with page-aligned buffers to retrieve directory records in bulk without libc `readdir` wrapper overhead. Combines with `statx` (`AT_STATX_DONT_SYNC`, `STATX_BLOCKS`) and `openat2(RESOLVE_NO_XDEV)` to respect filesystem mount boundaries.
2. **Windows**: Bypasses slow `FindFirstFileW`/`FindNextFileW` by querying `NtQueryDirectoryFileEx` with `FileIdBothDirectoryInfo`. Reads cluster `AllocationSize` directly from directory records in batch without secondary stat calls.
3. **Lock-Free Concurrency**: Each worker thread operates on an atomic Chase-Lev work-stealing circular deque. Workers push and pop tasks in LIFO order with zero lock contention, while idle workers steal tasks in FIFO batches.
4. **Zero-Allocation Bump Arena**: Instead of allocating a `PathBuf` or `String` per file, directory trees live in a chunked bump-allocated arena (`DirArena`) using 32-bit parent-child offsets.
5. **Adaptive Thread Scaling**: Automatically detects whether storage is rotational HDD or NVMe SSD via Linux sysfs (`queue/rotational`) or Windows `IOCTL_STORAGE_QUERY_PROPERTY`. Clamps worker threads to 2–4 on HDDs (preventing I/O seek thrashing, yielding 30–80× speedups) and scales to `cores × 4` on SSDs.
6. **Zero Dependencies (CLI)**: Compiles in seconds. No `clap`, no `rayon`, no `nix`, no `libc`.

---

## Feature comparison

| Feature | `du` | `ncdu` | `dust` | `WinDirStat` | `dscan` |
| :--- | :---: | :---: | :---: | :---: | :---: |
| **Multi-threaded Work Stealing** | No | No | Rayon | No | **Chase-Lev Deques** |
| **Bulk Kernel Syscalls** | No (`readdir`) | No (`readdir`) | No (`std::fs`) | No (`Win32`) | **`getdents64` / `NtQueryDirectoryFile`** |
| **Filesystem Block Allocation** | Yes | Yes | Yes | Sparse buggy | **Exact (`stx_blocks * 512` / `AllocationSize`)** |
| **CLI Dependencies** | Libc | Ncurses | 40+ crates | C++ MFC | **Zero (Pure stdlib)** |
| **Machine-Readable Output** | Text | Export | Text | None | **`--json` (RFC 8259)** |
| **Extension Breakdown** | No | No | No | Extension list | **`--ext` table** |
| **Interactive Cushion Treemap** | No | No | No | CPU GDI | **GPUI-Kit (120 FPS GPU)** |
| **Native Safe Trash & Reveal** | No | Delete | Delete | Explorer | **`trash` integration + safety guard** |
| **HDD/SSD Adaptive Threading** | No | No | No | No | **Auto-detect rotational media** |
| **Android App** | No | No | No | No | **Jetpack Compose + Rust JNI** |

---

## Visual Desktop GUI (`dscan-gui`)

`dscan-gui` is a pure-Rust desktop visualizer built with [gpui-kit](https://gpui-kit.com) (Zed Industries' GPU-accelerated engine). It runs without WebKit, node_modules, or webviews.

<p align="center">
  <img src="https://raw.githubusercontent.com/AhooraZen/dscan/main/assets/demo-gui.svg" alt="dscan Cushion Treemap GUI Preview" width="950"/>
</p>

- **GPU Cushion Treemap**: Faithful implementation of Jarke J. van Wijk's cushion shading algorithm ($I = I_a + I_d \max(0, \mathbf{N} \cdot \mathbf{L})$). Nested directory hierarchies render with smooth rounded ridges at 120 FPS.
- **Dark / Light Theme**: Rich deep navy dark theme with neon accents, and clean crisp light theme. Toggle from the title bar.
- **Native File Actions**: Right-click any cushion tile or directory row to **Reveal in File Manager** (Linux D-Bus/xdg-open, Windows Explorer `/select,`, macOS `open -R`).
- **Safe Move to Trash**: Integrated with system recycle bins via `trash` crate, backed by `is_safe_to_trash` guards to prevent accidental deletion of system roots or critical directories.
- **In-Memory Subtree Pruning**: Trashed or deleted directories bubble size deductions up to parent nodes instantly without requiring a full filesystem rescan.
- **System Resource Monitor**: Live CPU and RAM usage tracking directly in the header bar.
- **Extension Color Legend**: File type → color mapping with vivid, distinguishable palette.

---

## Android App (`dscan-android`)

A native Android disk space analyzer powered by the same Rust engine via JNI. Built with Jetpack Compose and Material 3 (Material You).

- **Rust JNI Engine**: The full `dscan-core` scanner compiled as `libdscan.so` (ARM64), delivering kernel-level `getdents64` + `statx` performance directly on Android.
- **Animated Circular Gauge**: Real-time scanning progress with a 270° animated arc showing total bytes, files/sec, and file count.
- **Squarified Treemap**: Bruls-Huizing-van Wijk algorithm for optimal rectangle aspect ratios. Tap any tile for details, with haptic feedback.
- **Material You Dynamic Color**: On Android 12+, adapts to your system wallpaper colors. Falls back to a custom neon dark/light theme on older devices.
- **Bottom Navigation**: Treemap | Directory Tree | File Types | Settings — with smooth animated transitions between tabs.
- **Native Folder Picker**: Android Storage Access Framework (SAF) integration via a "Browse…" button, plus quick-access chips for `/sdcard`, `Downloads`, `DCIM`, `WhatsApp`, and `Android/data`.
- **Settings Screen**: Thread count (Auto/2/4/8/16), treemap depth (2–8), treemap max nodes (500–5000), and theme mode (System/Light/Dark).
- **File Type Color Legend**: Horizontal scrollable row of colored chips mapping extensions to treemap colors.

---

## Installation

### From crates.io (CLI)

```bash
cargo install dscan
```

### Pre-built binaries (GitHub Releases)

Download pre-compiled binaries from [GitHub Releases](https://github.com/AhooraZen/dscan/releases/latest):

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

# Build CLI binary
cargo build --release -p dscan

# Build or run Desktop GUI
cargo run --release -p dscan-gui

# Build Android JNI library (requires cargo-ndk + Android NDK)
cargo ndk -t arm64-v8a build --release -p dscan-android
```

---

## Usage

### CLI Quick Scan

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
| `-j`, `--threads` | `<N>` | Number of concurrent worker threads | Auto (SSD: `cores × 4`, HDD: `cores` clamped 2–4) |
| `--exclude` | `<PATTERN>` | Path or folder name pattern to skip | System defaults (`/proc`, `/sys`, etc.) |
| `-a`, `--all` | None | Disable default exclusions (scan `.git`, `node_modules`, `target`, etc.) | Off |
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
│   └── dscan-android/    # Rust JNI bridge for Android (cdylib → libdscan.so)
├── android/              # Jetpack Compose Material 3 app (Kotlin)
│   └── app/src/main/kotlin/com/dscan/app/
│       ├── DscanBridge.kt    # JNI native method declarations
│       ├── MainActivity.kt   # App entry, scan lifecycle, SAF folder picker
│       └── ui/               # Theme, MainScreen, TreemapCanvas, ScanGauge, Settings
└── plans/                # Implementation plans (32 executed, audit trail)
```

---

## Key Design Decisions

- **No libc dependency on Linux**: All syscalls (`getdents64`, `statx`, `openat2`, `io_uring`, `prlimit64`) are issued via raw `syscall()` with architecture-specific numbers (x86_64: 217, aarch64: 61 for `getdents64`).
- **Disk space, not apparent size**: Uses `stx_blocks * 512` (Linux) and `AllocationSize` (Windows) to measure actual disk allocation, correctly handling sparse files and filesystem block boundaries.
- **Device boundary enforcement**: `openat2(RESOLVE_NO_XDEV)` prevents traversing into mounted virtual filesystems (`/proc`, `/sys`, `/dev`) without relying on path-string exclusion lists.
- **Adaptive HDD/SSD detection**: Reads `/sys/dev/block/<major>:<minor>/queue/rotational` on Linux and `IOCTL_STORAGE_QUERY_PROPERTY(StorageDeviceSeekPenaltyProperty)` on Windows to auto-tune thread count. HDDs get 2–4 threads to avoid seek thrashing; NVMe SSDs get up to 64.
- **O(N) rollup**: Directory sizes are propagated bottom-up via depth-sorted hash table, replacing the earlier O(N²) insert-based rollup that froze on large filesystems.

---

## License

Dual-licensed under either:

- MIT License ([LICENSE-MIT](LICENSE-MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))

at your option.
