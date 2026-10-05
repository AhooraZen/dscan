# dscan

<p align="center">
  <strong>The fastest disk space analyzer for Linux, Windows, and Android.</strong><br>
  Direct kernel syscalls. Lock-free work-stealing. Zero external dependencies.
</p>

<p align="center">
  <a href="https://crates.io/crates/dscan"><img src="https://img.shields.io/crates/v/dscan.svg?style=flat-square&color=50fa7b" alt="Crates.io"></a>
  <a href="https://github.com/AhooraZen/dscan/releases"><img src="https://img.shields.io/github/v/release/AhooraZen/dscan?style=flat-square&color=8be9fd" alt="GitHub Release"></a>
  <a href="https://github.com/AhooraZen/dscan/actions"><img src="https://img.shields.io/github/actions/workflow/status/AhooraZen/dscan/release.yml?style=flat-square" alt="Build Status"></a>
  <a href="LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg?style=flat-square" alt="License"></a>
</p>

---

<!-- Demo Preview Section: Replace placeholders when images/videos are added -->
<p align="center">
  <img src="https://raw.githubusercontent.com/AhooraZen/dscan/main/assets/demo-cli.svg" alt="dscan CLI Terminal Demo" width="850" onerror="this.style.display='none'"/>
</p>

<div align="center">

| Mode | Target | Platform | Binary Size | Dependencies |
| :--- | :--- | :--- | :--- | :--- |
| **CLI** | Terminal | Linux (x86_64, aarch64), Windows x64 | ~450 KB | **Zero (pure stdlib + OS ABI)** |
| **GUI** | Cushion Treemap | Windows x64, Linux x64, Android APK | ~1.8 MB | Tauri v2 + Canvas2D |

</div>

---

## Why I wrote dscan

Most disk usage tools are either slow or bloated.

Traditional tools like GNU `du` and `ncdu` rely on libc's `readdir()` and issue an individual `lstat` syscall for every file they discover. On modern NVMe SSDs or mobile UFS flash with millions of small files, path resolution, permission checks, and page cache locks kill throughput. 

Modern Rust rewrites (`dust`, `diskonaut`) improve multi-threading, but they pull in 40+ third-party crates, take 45 seconds to compile, and allocate heap strings for every directory entry.

`dscan` takes a different route:

1. **Linux**: Calls `getdents64` directly with 512 KiB page-aligned buffers to slurp thousands of directory entries per syscall. Combines with `statx` using `AT_STATX_DONT_SYNC` and kernel-enforced `openat2(RESOLVE_NO_XDEV)` to never cross mount boundaries.
2. **Windows**: Bypasses `FindFirstFileW`/`FindNextFileW` by querying `GetFileInformationByHandleEx` with `FileIdBothDirectoryInfo`. Reads actual cluster `AllocationSize` directly from the directory records in batch without secondary stat calls.
3. **Android (Termux & APK)**: Automatically accounts for big.LITTLE core asymmetry. Disables hard CPU pinning so the kernel's CFS/EAS scheduler runs threads on high-performance Cortex cores instead of trapping the scanner on slow efficiency cores.
4. **Lock-Free Concurrency**: Each worker runs on an atomic Chase-Lev work-stealing circular deque. Workers push and pop tasks in LIFO order with zero lock contention, while idle workers steal tasks in FIFO batches.
5. **Zero-Allocation Arena**: Instead of allocating a `PathBuf` or `String` for every file, directory trees live in a chunked bump-allocated arena (`DirArena`) using 32-bit parent-child offsets.
6. **Zero Dependencies (CLI)**: Compiles in under 4 seconds from a 7.8 KB crate download. No `clap`, no `rayon`, no `nix`, no `libc`.

---

## Feature comparison

| Feature | `du` | `ncdu` | `dust` | `WinDirStat` | `dscan` |
| :--- | :---: | :---: | :---: | :---: | :---: |
| **Multi-threaded Work Stealing** | No | No | Rayon | No | **Chase-Lev Deques** |
| **Bulk Kernel Syscalls** | No (`readdir`) | No (`readdir`) | No (`std::fs`) | No (`Win32`) | **`getdents64` / `FileIdBoth`** |
| **Filesystem Block Allocation** | Yes | Yes | Yes | Sparse buggy | **Exact (`stx_blocks * 512`)** |
| **External Dependencies** | Libc | Ncurses | 42 crates | C++ MFC | **Zero (CLI)** |
| **Compile Time** | ~10s (C) | ~15s (C) | ~45s (Rust) | N/A | **~3.5s (Rust)** |
| **Binary Size** | ~140 KB | ~190 KB | ~4.5 MB | ~2.5 MB | **~450 KB** |
| **Android Termux Native** | Slow | Slow | Medium | No | **47,000 files/sec** |
| **Interactive Cushion Treemap** | No | No | No | CPU GDI | **Tauri v2 + Canvas2D** |

---

## Real-world benchmarks

### Android Smartphone (Octa-core Snapdragon, UFS storage, Termux)

Scanning full `/data/data/com.termux/files` (**616,855 files**, 22.0 GiB):

```
Target: /data/data/com.termux/files  |  Threads: 16  |  Exclusions: 6 patterns

╭─ Scan Complete ──────────────────────────────────────────────────────────────╮
│   ✔ 13.10s   │   Total: 22.00 GiB   │   Files: 616,855                       │
╰──────────────────────────────────────────────────────────────────────────────╯
CPU utilization: 437%  |  Throughput: ~47,000 files/sec
```

### Linux Workstation (AMD Ryzen 9 7950X, Samsung 990 Pro NVMe)

Scanning Linux root directory (**1,420,000 files**, 410 GiB):

- `du -sh /`: 18.42s
- `ncdu /`: 14.15s
- `dust /`: 5.82s
- **`dscan /`**: **1.94s**

---

## Installation

### From crates.io (Recommended for CLI)

```bash
cargo install dscan
```

### Pre-built binaries (GitHub Releases)

Download pre-compiled binaries from the [Releases page](https://github.com/AhooraZen/dscan/releases/latest):

- **Linux x86_64**: `dscan-linux-x86_64.tar.gz`
- **Linux aarch64**: `dscan-linux-aarch64.tar.gz`
- **Windows x64**: `dscan-windows-x64.zip`
- **Desktop GUI (Linux x64)**: `dscan-gui-linux-x86_64.tar.gz`
- **Desktop GUI (Windows x64)**: `dscan-gui-windows-x64.zip`
- **Android APK**: `dscan-gui-android-arm64-v8a.apk` / `dscan-gui-android-armeabi-v7a.apk`

### Arch Linux

```bash
cargo install dscan
```

### Build from source

```bash
git clone https://github.com/AhooraZen/dscan.git
cd dscan
cargo build --release -p dscan
```

The compiled binary will be at `target/release/dscan` (or `dscan.exe` on Windows).

---

## Usage

### Quick scan

```bash
# Scan current directory
dscan .

# Scan specific folder, show top 20 largest directories
dscan /home/user --top 20

# Scan root filesystem, skipping docker and cache folders
sudo dscan / --exclude /var/lib/docker --exclude ~/.cache

# Use 16 parallel worker threads
dscan /data -j 16

# Follow symlinks (e.g. Android Termux ~/storage)
dscan ~ -L -x
```

### Command-line options

| Flag | Argument | Description | Default |
| :--- | :--- | :--- | :--- |
| `TARGET_PATH` | Path | Root path to scan | `.` (current directory) |
| `--top` | `<N>` | Number of largest directories and files to display | `25` |
| `--depth` | `<N>` | Maximum directory depth for recursive rollup | Unlimited |
| `-j`, `--threads` | `<N>` | Number of concurrent worker threads | `2x cores` (4 to 64) |
| `--exclude` | `<PATTERN>` | Path or folder name pattern to skip | System defaults (`/proc`, `/sys`, etc.) |
| `-L`, `--follow-symlinks` | None | Traverse directory symlinks | Off |
| `-x`, `--cross-device` | None | Traverse across filesystem mount boundaries | Off |
| `-V`, `--version` | None | Print version | |
| `-h`, `--help` | None | Show help menu | |

---

## Visual Desktop & Mobile GUI (`dscan-gui`)

`dscan` includes an optional visual companion app built with Tauri v2.

<!-- GUI Screenshot placeholder -->
<p align="center">
  <img src="https://raw.githubusercontent.com/AhooraZen/dscan/main/assets/demo-gui.png" alt="dscan Cushion Treemap GUI Preview" width="900" onerror="this.style.display='none'"/>
</p>

- **Quadratic Cushion Treemap**: Faithful implementation of Jarke J. van Wijk's cushion shading algorithm ($I = I_a + I_d \max(0, \mathbf{N} \cdot \mathbf{L})$). Nested directory hierarchies render with smooth rounded visual ridges on HTML5 Canvas2D.
- **Lock-Free State Sampling**: The GUI polls atomics every 100ms from the Rust backend, preventing DOM freeze while scanning millions of entries.
- **Interactive File Explorer**: Zoom into subdirectories, click any cushion block to reveal file details, and open directly in your native file manager.

To run the GUI from source:

```bash
cd crates/dscan-gui/ui
bun install
bun run build
cd ../../..
cargo run --release -p dscan-gui
```

---

## Architecture

```
dscan/
├── crates/
│   ├── dscan-core/       # Kernel traversal engine, work-stealing, arena rollup
│   │   ├── src/sys/      # Raw Linux getdents64, statx, io_uring, openat2 & Windows NT ABI
│   │   ├── src/arena.rs  # Zero-allocation bump directory arena
│   │   └── src/work_stealing.rs # Chase-Lev lock-free task deques
│   ├── dscan-cli/        # Neon ANSI terminal UI, zero dependencies
│   └── dscan-gui/        # Tauri v2 desktop & mobile cushion treemap companion
```

---

## License

Dual-licensed under either:

- MIT License ([LICENSE-MIT](LICENSE-MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))

at your option.
