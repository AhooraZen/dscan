# dscan

<p align="center">
  <strong>Fast disk space analyzer for Linux, Windows, macOS, and Android.</strong><br>
  Direct kernel syscalls. Lock-free work-stealing. Zero external dependencies in core.
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
| **CLI** | Terminal | Linux (x86_64, aarch64), Windows x64 | ~450 KB | Zero (pure Rust stdlib and kernel ABI) |
| **Desktop** | Skia cushion treemap | Linux, Windows, macOS | ~30 MB | Compose Multiplatform (Kotlin + Skia + Rust JNI) |
| **Android** | Jetpack Compose | Android 8+ (ARM64, x86_64, ARMv7) | ~6 MB | Compose Multiplatform + Rust JNI (`libdscan.so`) |

</div>

---

## Why dscan?

Most disk usage tools are either slow or bloated.

GNU `du` and `ncdu` call libc `readdir()` and issue an `lstat` for every single file. On modern NVMe SSDs with millions of files, path resolution, permission checks, and page cache locks slow traversal to a crawl.

Other tools bring multi-threading, but drag in dozens of crates, take minutes to compile, and allocate heap strings for every directory entry.

`dscan` bypasses libc overhead:

1. **Linux**: Uses raw `getdents64` with page-aligned buffers to read entries in bulk. Pairs with `statx` (`AT_STATX_DONT_SYNC`, `STATX_BLOCKS`) and `openat2(RESOLVE_NO_XDEV)` so it never walks off the current filesystem mount.
2. **Windows**: Calls `NtQueryDirectoryFileEx` with `FileIdBothDirectoryInfo`. Reads cluster `AllocationSize` directly out of directory records without extra stat calls.
3. **Lock-free concurrency**: Workers run on atomic Chase-Lev work-stealing circular deques. Threads push and pop tasks in LIFO order with zero lock contention. Idle workers steal tasks in single-CAS batches.
4. **Zero-allocation bump arena**: No `PathBuf` or `String` allocations per file. Trees live in a chunked bump arena (`DirArena`) linked by 32-bit parent-child offsets.
5. **Adaptive thread scaling**: Checks whether target storage is rotational HDD or SSD via Linux sysfs (`queue/rotational`) or Windows `IOCTL_STORAGE_QUERY_PROPERTY`. Clamps to 2-4 threads on HDDs to eliminate seek thrashing, and respects container cgroup CPU limits (`/sys/fs/cgroup/cpu.max`).
6. **Zero core dependencies**: Compiles in 2 seconds. No `clap`, no `rayon`, no `nix`, no `libc`.

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
| **Interactive cushion treemap** | No | No | No | CPU GDI | **Compose Desktop (120 FPS Skia)** |
| **Safe trash and reveal** | No | Delete | Delete | Explorer | **`trash` subsystem with safety guards** |
| **Container cgroup CPU scaling** | No | No | No | No | **Reads cgroups v1/v2 CPU limits** |
| **Android app** | No | No | No | No | **Compose Multiplatform + Rust JNI** |

---

## Benchmarks

Benchmarked with [`hyperfine`](https://github.com/sharkdp/hyperfine) on Arch Linux (Linux 6.12, AMD Ryzen, NVMe SSD, Btrfs) scanning a real-world directory with **1,121,462 files** across **148,230 directories**:

| Command | Wall time (mean ± σ) | Total CPU time | Accounting depth | Binary size | Dependencies |
| :--- | :---: | :---: | :--- | :---: | :---: |
| **`dscan ~ --top 20`** | **1.218 s ± 0.024 s** | **3.82 s** | **Top 20 files + Top 25 dirs + Extension breakdown + Arena** | **~450 KB** | **Zero (pure stdlib)** |
| `dust ~` | 1.333 s ± 0.031 s | 7.73 s (+102% CPU) | Sized directory bars | ~3 MB | 40+ crates |
| `du -sh ~` | 2.210 s ± 0.011 s | 2.20 s | Single total size | ~50 KB | libc |

> `dscan` completes a 1.12-million file scan in **1.21 seconds** while calculating top-file ranking, file extension aggregation, and 32-bit bump arena tree rollup, using **50% less CPU** than `dust` with zero external crates.

---

## Desktop Visualizer

A unified cross-platform desktop visualizer built on Compose Multiplatform (Kotlin + Skia) backed by `dscan-core` via JNI.

- **120 FPS Skia cushion treemap**: Implements van Wijk's cushion shading algorithm with squarified layouts.
- **Dark and light themes**: Modern high-contrast palettes adhering to Parch Linux design guidelines.
- **Interactive drilldown**: Left-click to zoom into directories with live breadcrumb trails; hover for instant file details.
- **Search and filter**: Press `Ctrl+F` to search file paths in real time and highlight matching nodes in the treemap.
- **Native file actions**: Right-click to reveal in system file manager or move to system trash with boundary guards.

---

## Android App

Android disk analyzer sharing 100% of Compose UI components with the desktop app.

- **Native JNI engine**: `dscan-core` compiled as `libdscan.so`.
- **Storage dashboard**: Instant space breakdown with throughput metrics, elapsed duration, and top file categories.
- **Directory explorer**: Breadcrumb navigation with proportional size bars and folder drilldown.
- **Storage hogs**: Leaderboard of the largest space consumers across your storage.
- **Folder picker**: Storage Access Framework (SAF) folder selection plus quick chips (`/sdcard`, `Downloads`, `DCIM`, `WhatsApp`, `Android/data`).

---

## Installation

### Package managers

#### Arch Linux (AUR)

```bash
# Pre-built binary
paru -S dscan-bin

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
scoop install dscan
```

### From crates.io (CLI)

```bash
cargo install dscan
```

### Pre-built binaries (GitHub Releases)

Download pre-compiled builds from [GitHub Releases](https://github.com/AhooraZen/dscan/releases/latest):

- **Linux x86_64 CLI**: `dscan-linux-x86_64.tar.gz`
- **Linux aarch64 CLI**: `dscan-linux-aarch64.tar.gz`
- **Windows x64 CLI**: `dscan-windows-x64.zip`
- **Android APK**: `dscan-android-release.apk`
- **Desktop Visualizer**: `dscan-desktop.jar`

### Build from source

```bash
git clone https://github.com/AhooraZen/dscan.git
cd dscan

# Build CLI
cargo build --release -p dscan

# Run Desktop Visualizer
cd android && ./gradlew :desktopApp:run

# Build Android APK
cd android && ./gradlew :androidApp:assembleRelease
```

---

## Usage

### Quick scan

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
│   │   ├── src/scanner/  # Modular scanner (linux.rs, windows.rs, rollup.rs, buffer.rs, state.rs)
│   │   ├── src/arena.rs  # Zero-allocation bump directory arena with subtree grafting
│   │   ├── src/snapshot.rs # Treemap node builder and scan session manager
│   │   ├── src/simd.rs   # AVX2/NEON SIMD string scanners
│   │   └── src/work_stealing.rs # Chase-Lev lock-free task deques
│   ├── dscan-cli/        # Neon ANSI terminal UI, JSON serializer, zero dependencies
│   └── dscan-jni/        # Cross-platform shared C-ABI library for Compose Multiplatform
├── android/              # Compose Multiplatform project (Kotlin + Skia)
│   ├── shared/           # 100% shared Compose UI, Skia cushion treemap, DscanBridge
│   ├── desktopApp/       # Desktop entry point (JVM Skia desktop window)
│   └── androidApp/       # Android APK entry point (SAF, Activity)
└── plans/                # Implementation plans (37 executed, audit trail)
```

---

## License

Dual-licensed under either:

- MIT License ([LICENSE-MIT](LICENSE-MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))

at your option.
