# Plan 032: Android Native APK Architecture with Rust JNI Bridge and Jetpack Compose

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.

## Status
- **Priority**: P1
- **Effort**: L
- **Risk**: LOW (purely additive mobile package; zero breaking changes to desktop/CLI crates)
- **Depends on**: plans/031-fix-path-normalization-and-gui-relative-root.md
- **Category**: feature
- **Planned at**: commit `03f3bbd`, 2026-10-06

## Architectural Design
To deliver the fastest disk space analyzer on Android without sacrificing mobile UX or safety:
1. **Engine Layer (`crates/dscan-android`)**:
   - Compiles `dscan-core` as a high-performance native C shared library (`libdscan.so`) targeting `aarch64-linux-android` (ARM64) and `x86_64-linux-android` (Emulators).
   - Utilizes direct Linux kernel `getdents64` (syscall 61 on aarch64), `statx`, and lock-free work-stealing concurrency.
   - Preserves battery & thermal thresholds on Android (`(cores * 2).clamp(4, 16)`).
   - Exposes safe JNI bindings under `com.dscan.app.DscanBridge`:
     - `startScan(path: String, threads: Int): Long` -> returns opaque pointer to `ScanSession`
     - `pollProgress(sessionPtr: Long): String` -> JSON payload of current files, bytes, speed, status
     - `getTreemapNodes(sessionPtr: Long, maxDepth: Int, maxNodes: Int): String` -> JSON array of treemap node DTOs
     - `stopScan(sessionPtr: Long)` -> cancels and frees session
2. **UI Layer (`android/`)**:
   - Modern Kotlin + Jetpack Compose app with Material 3 (Material You dynamic theming).
   - Permissions: `MANAGE_EXTERNAL_STORAGE` (`ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION`) to inspect all shared storage directories (`/sdcard`, `/storage/emulated/0`, Termux, Download, DCIM, Android).
   - Compose Components:
     - Storage Root Selector & Quick Chips (`/sdcard`, `Downloads`, `DCIM`, `WhatsApp`, `Android/data`)
     - Animated Real-Time Circular Gauge & Speed Counter
     - GPU Canvas Cushion Treemap Visualizer with zoom/pan gesture support
     - Hierarchical Directory Drilldown List with file type color badges
3. **Build Automation**:
   - Gradle build scripts (`build.gradle.kts`) with NDK support that automatically compiles the Rust JNI library or packages prebuilt `.so` libraries into the final `.apk`.

## Implementation Steps

### Step 1: Create `crates/dscan-android`
Add `crates/dscan-android/Cargo.toml`:
```toml
[package]
name = "dscan-android"
version = "0.4.3"
edition = "2024"
authors = ["AhooraZen <ahoora935137@gmail.com>"]
description = "Native Android JNI bridge for dscan disk space analyzer"
license = "MIT OR Apache-2.0"

[lib]
name = "dscan"
crate-type = ["cdylib", "rlib"]

[dependencies]
dscan-core = { version = "0.4.3", path = "../dscan-core" }
jni = { version = "0.21", default-features = false }
```
Add `crates/dscan-android/src/lib.rs` implementing safe JNI bridge with zero memory leaks and proper error trapping.

### Step 2: Add `crates/dscan-android` to workspace in root `Cargo.toml`
Add `"crates/dscan-android"` to `members`.

### Step 3: Scaffold Android Application in `android/`
Create complete Android project layout:
- `android/settings.gradle.kts`
- `android/build.gradle.kts`
- `android/app/build.gradle.kts`
- `android/app/src/main/AndroidManifest.xml`
- `android/app/src/main/kotlin/com/dscan/app/DscanBridge.kt`
- `android/app/src/main/kotlin/com/dscan/app/MainActivity.kt`
- `android/app/src/main/kotlin/com/dscan/app/ui/` (Theme, TreemapCanvas, DirectoryList, StoragePicker)
- `android/app/src/main/res/` (Icons, strings, themes)

### Step 4: Verification & Build
- Verify `cargo check -p dscan-android` compiles cleanly.
- Verify `cargo test -p dscan-core -p dscan` remains 100% green.
- Add GitHub Actions Android build job in `.github/workflows/release.yml` so every release automatically attaches `dscan-android-arm64.apk`.
- Update `plans/README.md` to DONE.
