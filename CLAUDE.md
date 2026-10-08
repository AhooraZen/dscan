# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build, Test, and Quality Commands

```bash
# Workspace Build
cargo build
cargo build --release

# Run CLI
cargo run -p dscan -- .
cargo run -p dscan -- /path/to/scan --top 20 --depth 2 --threads 16

# Run Tests
cargo test --workspace
cargo test -p dscan-core
cargo test -p dscan
cargo test -p dscan-jni

# Lint and Format
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
cargo fmt

# Desktop Visualizer (Compose Multiplatform)
cd android && ./gradlew :desktopApp:run
cd android && ./gradlew :desktopApp:assemble

# Android APK (Jetpack Compose)
cd android && ./gradlew :androidApp:assembleDebug
cd android && ./gradlew :androidApp:assembleRelease
```

## Architecture and Design

`dscan` is an extreme-performance, zero-external-dependency multi-threaded disk space analyzer for Linux and Windows, paired with a unified 100% shared Compose Multiplatform visualizer (Desktop & Android).

### Workspace Crates

- **`crates/dscan-core`**: Core traversal and synchronization engine:
  - **Raw Directory Traversal (`scanner/linux.rs`, `scanner/windows.rs`)**: Reads directory entries in bulk using `getdents64` into a page-aligned 64 KiB buffer on Linux (`NtQueryDirectoryFile` on Windows), bypassing libc overhead.
  - **Canonical Metadata Accounting**: Uses raw `sys_statx` with `STATX_BLOCKS` (`stx_blocks * 512`) on Linux and `AllocationSize` on Windows to account for sparse files and filesystem block allocation.
  - **Lock-Free Work-Stealing**: Custom Chase-Lev deques with true single-CAS atomic batch stealing (`steal_batch`) and conditional `notify_one()` wakeups to eliminate thundering herds.
  - **Container CFS CPU Quota Scaling**: Reads cgroup v1 and v2 CPU quotas (`/sys/fs/cgroup/cpu.max`), automatically clamping thread counts to container vCPUs.
  - **Zero-Allocation Hierarchical Arena (`arena.rs`)**: 32-bit indexed bump allocator (`DirArena`). Worker subtrees are merged into master arena with `merge_subtree` and rolled up in a single O(N) reverse array pass in <5ms.
  - **SIMD String Acceleration (`simd.rs`)**: AVX2 and NEON SIMD algorithms for fast null-byte searching, exclusion prefix matching, and UTF-16/UTF-8 transcoding.
  - **Scan Session (`snapshot.rs`)**: Non-blocking background scanning with atomic progress tracking, pause/resume, and cancellation for UI consumers.
- **`crates/dscan-cli`**: Zero-dependency terminal binary with cyberpunk ANSI formatting, argument parser (`--json`, `--ext`, `--all`, `-j`), dynamic column sizing, and terminal progress bar.
- **`crates/dscan-jni`**: Cross-platform shared C-ABI library (`libdscan.so`/`.dll`/`.dylib`) exposing `dscan-core` directly to Compose Multiplatform without JNI reflection overhead.
- **`android/` (Compose Multiplatform)**:
  - **`:shared`**: 100% shared Kotlin Compose UI (`MainScreen`, `DesktopMainView`, `TreemapView`, `DirectoryList`, `Theme`, `DscanBridge`). Implements high-performance Skia Canvas Cushion Treemap rendering (120 FPS).
  - **`:desktopApp`**: Compose Desktop application with resizable split-pane layout, hover tooltips, click drilldown, search highlighting (`Ctrl+F`), native file manager reveal, safe trash deletion, and menu shortcuts.
  - **`:androidApp`**: Android APK with Storage Access Framework (SAF), foreground notifications, and mobile storage dashboards.

### Invariants & Platform Requirements

- Core engine (`dscan-core`) and CLI (`dscan-cli`) maintain strictly **zero external runtime dependencies** (no `clap`, `rayon`, `nix`, etc.).
- Linux kernel interfaces use direct syscall numbers and 256-byte UAPI `Statx` layouts across x86_64, aarch64, arm32, and riscv64.
