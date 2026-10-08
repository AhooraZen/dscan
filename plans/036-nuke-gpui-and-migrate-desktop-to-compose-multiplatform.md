# Plan 036: Migrate Desktop GUI to Compose Multiplatform and Unify Shared JNI

> **Executor instructions**: Follow this plan step by step. Run every verification command and confirm the expected result before moving to the next step. If anything in the "STOP conditions" section occurs, stop and report — do not improvise. When done, update the status row for this plan in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat ba016d0..HEAD -- Cargo.toml crates/dscan-android crates/dscan-gui android`
> If any in-scope file changed since this plan was written, compare the "Current state" excerpts against the live code before proceeding; on a mismatch, treat it as a STOP condition.

## Status
- **Priority**: P1
- **Effort**: L
- **Risk**: MED
- **Depends on**: `plans/032-android-native-apk-jni-and-jetpack-compose.md`
- **Category**: migration
- **Planned at**: commit `ba016d0`, 2026-10-08
- **Issue**: N/A

## Why this matters
The GPUI desktop visualizer in `crates/dscan-gui` relies on heavy platform-specific graphics dependencies and complex display-server toolchains. Meanwhile, the Android application already implements a modern Jetpack Compose UI backed by `dscan-core` through JNI. Migrating the desktop application to Compose Multiplatform unifies mobile and desktop onto a single 100% shared Kotlin Compose UI codebase, delivers high-performance Skia Canvas cushion treemap rendering, and keeps the core engine pure Rust.

## Current state
- `Cargo.toml` — Workspace manifest includes `dscan-gui` and `dscan-android`:
  ```toml
  [workspace]
  resolver = "3"
  members = [
      "crates/dscan-core",
      "crates/dscan-cli",
      "crates/dscan-gui",
      "crates/dscan-android",
  ]
  ```
- `crates/dscan-android/src/lib.rs` — JNI bindings exposing scan session lifecycle, progress polling, and treemap DTO serialization to `com.dscan.app.DscanBridge`.
- `crates/dscan-gui/` — GPUI desktop visualizer implementing treemap layout and system helper routines.
- `android/` — Android Gradle project containing Compose UI components:
  - `DscanBridge.kt` — JNI binding declarations and JSON DTO parsers.
  - `MainActivity.kt` — Activity host and lifecycle management.
  - `ui/MainScreen.kt`, `ui/DirectoryList.kt`, `ui/Theme.kt`, `ui/Format.kt`, `ui/ScanGauge.kt`, `ui/SettingsScreen.kt`.

### Parch Linux Visual Design Tokens
- Dark Mode Base Canvas: `#0c1017`
- Card and Panel Surface: `#151c28`
- Structural Border: `#212c3d`
- Primary Brand Blue: `#0080FF`
- Secondary Accent Cyan: `#00EAFF`
- Action Accent Green: `#00FF80`
- Light Mode Base Canvas: `#f8fafc`, Surface: `#ffffff`, Border: `#e2e8f0`, Text: `#0f172a`
- Surface Principle: Solid flat modern surfaces only. No blur effects or glassmorphism.

## Commands you will need
| Purpose | Command | Expected on success |
|---|---|---|
| Cargo Workspace Check | `cargo check --workspace` | exit 0, zero warnings |
| Cargo Unit Tests | `cargo test --workspace` | all tests pass |
| Build Shared JNI Library | `cargo build -p dscan-jni --release` | exit 0, produces shared library |
| Gradle Build Desktop App | `cd android && ./gradlew :desktopApp:assemble` | exit 0, BUILD SUCCESSFUL |
| Gradle Run Desktop App | `cd android && ./gradlew :desktopApp:run` | launches Compose Desktop app |
| Gradle Build Android APK | `cd android && ./gradlew :androidApp:assembleDebug` | exit 0, BUILD SUCCESSFUL |

## Scope
**In scope**:
- Remove `crates/dscan-gui` from `Cargo.toml` and delete the directory.
- Rename `crates/dscan-android` to `crates/dscan-jni` and configure cross-platform dynamic library output (`.so`, `.dll`, `.dylib`).
- Add helper JNI functions in `crates/dscan-jni/src/lib.rs` for drive detection, safe file operations, and system file manager opening.
- Refactor `android/` into a Compose Multiplatform Gradle project with three modules:
  - `:shared` — Common Kotlin Compose UI, state management, Treemap Canvas renderer, and `DscanBridge`.
  - `:desktopApp` — JVM Desktop entry point (`Main.kt`), window frame, menu bar, drag-and-drop, and shortcuts.
  - `:androidApp` — Android APK entry point (`MainActivity.kt`) and platform permission handlers.
- Implement Skia Canvas Cushion Treemap visualizer with squarified layout, hover tooltips, click drilldown, search highlighting, and zoom breadcrumbs.
- Implement dynamic native library loading in `DscanBridge.kt` supporting bundled JVM resources and system paths.

**Out of scope**:
- `crates/dscan-core/` — Core scanning algorithms, work-stealing queues, and SIMD routines stay untouched.
- `crates/dscan-cli/` — CLI application remains independent and zero-dependency.

## Git workflow
- Branch: `advisor/036-nuke-gpui-compose-multiplatform`
- Conventional commit per logical step: `refactor: ...`, `feat: ...`, `build: ...`
- Do NOT push or open PR unless instructed.

## Steps

### Step 1: Remove GPUI Crate from Cargo Workspace
Update root `Cargo.toml` to remove `crates/dscan-gui` and remove the crate directory.

1. Modify `/home/ahoura/dscan/Cargo.toml`:
   - Remove `"crates/dscan-gui"` from `members`.
   - Update `members` to `["crates/dscan-core", "crates/dscan-cli", "crates/dscan-jni"]`.
2. Delete directory `crates/dscan-gui/`.

**Verify**: `cargo check --workspace` → exit 0, `dscan-gui` absent.

---

### Step 2: Rename `crates/dscan-android` to `crates/dscan-jni` and Generalize JNI Bridge
Rename crate to `dscan-jni` and support Linux, macOS, Windows, and Android dynamic library compilation.

1. Rename `crates/dscan-android/` to `crates/dscan-jni/`.
2. Update `crates/dscan-jni/Cargo.toml`:
   - Package name: `dscan-jni`.
   - Library name: `dscan`, crate types `["cdylib", "rlib"]`.
   - Dependencies: `dscan-core`, `jni = { version = "0.21", default-features = false }`, `sysinfo = "0.33"`, `trash = "5.2"`, `open = "5.3"`.
3. In `crates/dscan-jni/src/lib.rs`:
   - Retain existing scan session lifecycle bindings (`startScan`, `pollProgress`, `getTreemapNodes`, `getExtensionBreakdown`, `cancelScan`, `stopScan`).
   - Add `Java_com_dscan_app_DscanBridge_detectDrives` returning JSON array of available storage mounts.
   - Add `Java_com_dscan_app_DscanBridge_revealInFileManager` delegating to platform file manager.
   - Add `Java_com_dscan_app_DscanBridge_moveToTrash` performing safe deletion through trash subsystem with path guards.

**Verify**: `cargo build -p dscan-jni --release` → exit 0, compiles `target/release/libdscan.so`.
`cargo test -p dscan-jni` → all tests pass.

---

### Step 3: Restructure Gradle Project into Multiplatform Modules
Transform `android/` into a Compose Multiplatform Gradle project with `:shared`, `:androidApp`, and `:desktopApp`.

1. Update `android/gradle/libs.versions.toml`:
   - Add `compose-multiplatform = "1.7.3"`.
   - Add `kotlinx-coroutines-swing = { group = "org.jetbrains.kotlinx", name = "kotlinx-coroutines-swing", version = "1.10.1" }`.
   - Register Compose Multiplatform plugin: `compose-multiplatform = { id = "org.jetbrains.compose", version.ref = "compose-multiplatform" }`.
2. Update `android/settings.gradle.kts`:
   - Register modules:
     ```kotlin
     include(":shared")
     include(":androidApp")
     include(":desktopApp")
     ```
3. Update root `android/build.gradle.kts` to declare Compose Multiplatform and Android plugins with `apply false`.
4. Create `:shared/build.gradle.kts`:
   - Apply `kotlin("multiplatform")`, `id("com.android.library")`, `id("org.jetbrains.compose")`, `id("org.jetbrains.kotlin.plugin.compose")`.
   - Define source sets: `commonMain` (Compose runtime, foundation, material3, material-icons-extended, coroutines), `androidMain`, `jvmMain`.
5. Create `:androidApp/build.gradle.kts`:
   - Apply `id("com.android.application")`, `id("org.jetbrains.kotlin.android")`, `id("org.jetbrains.kotlin.plugin.compose")`.
   - Depend on `implementation(project(":shared"))`.
6. Create `:desktopApp/build.gradle.kts`:
   - Apply `kotlin("jvm")`, `id("org.jetbrains.compose")`, `id("org.jetbrains.kotlin.plugin.compose")`.
   - Depend on `implementation(project(":shared"))` and `implementation(compose.desktop.currentOs)`.
   - Configure desktop packaging and main class `com.dscan.desktop.MainKt`.

**Verify**: `cd android && ./gradlew tasks` → exit 0, all three subprojects listed.

---

### Step 4: Implement Native Library Loader and Data Models in `:shared`
Implement dynamic library loading in `shared/src/commonMain/kotlin/com/dscan/app/DscanBridge.kt`.

1. Implement dynamic library resolution in `DscanBridge.kt`:
   - First attempt `System.loadLibrary("dscan")`.
   - Fallback on JVM Desktop: extract bundled native binary for current OS and architecture from JAR resources to temporary cache directory and call `System.load(path)`.
   - Development fallback: inspect `../target/release/` and `../target/debug/` for `libdscan.so` / `dscan.dll` / `libdscan.dylib`.
2. Declare external functions in `DscanBridge`:
   - `startScan(path: String, threads: Int): Long`
   - `pollProgress(sessionPtr: Long): String`
   - `getTreemapNodes(sessionPtr: Long, maxDepth: Int, maxNodes: Int): String`
   - `getExtensionBreakdown(sessionPtr: Long, limit: Int): String`
   - `cancelScan(sessionPtr: Long)`
   - `stopScan(sessionPtr: Long)`
   - `detectDrives(): String`
   - `revealInFileManager(path: String): Boolean`
   - `moveToTrash(path: String, rootPath: String): String?`
3. Define data classes and parsers: `ScanProgress`, `TreemapNode`, `ExtensionStat`, `DriveInfo`.

**Verify**: `cd android && ./gradlew :shared:compileKotlinJvm` → exit 0.

---

### Step 5: Implement Skia Canvas Cushion Treemap in `:shared`
Implement high-performance interactive treemap component in `shared/src/commonMain/kotlin/com/dscan/app/ui/TreemapView.kt`.

1. **Squarify Layout Engine**:
   - Compute proportional rectangle bounds $(x, y, w, h)$ recursively from hierarchical `TreemapNode` lists.
   - Maintain aspect ratios near 1.0 using standard squarified partitioning.
2. **Skia Canvas Shading**:
   - Draw leaf rectangles using file type palette colors from `Theme.kt`.
   - Apply top-left highlight and bottom-right shadow linear gradients to produce the cushion surface effect.
   - Render crisp 1px borders with contrast outlines (`#212c3d` / `#e2e8f0`).
3. **Interactive Capabilities**:
   - Pointer hover tracking: determine node under cursor and display floating tooltip with file name, formatted size, relative share, and path.
   - Click navigation: left-click directory node to zoom/drill down into subtree; update breadcrumbs.
   - Context menu trigger: right-click file/directory to show actions (Reveal in File Manager, Move to Trash, Copy Path).
   - Search filtering: highlight matching node rectangles with cyan border (`#00EAFF`) and dim non-matching nodes.
   - Minimum size cutoff filter: skip rendering nodes smaller than user-selected pixel or byte threshold.

**Verify**: `cd android && ./gradlew :shared:compileKotlinJvm` → exit 0.

---

### Step 6: Implement Desktop Split-Pane Layout & Polish in `:shared`
Implement desktop-optimized layout in `shared/src/commonMain/kotlin/com/dscan/app/ui/DesktopMainView.kt` applying Parch Linux design system.

1. **Top Header**:
   - Brand indicator `⚡ dscan` with version badge.
   - Target path input field, Browse button, Scan / Cancel button.
   - Search bar (`Ctrl+F` hotkey), theme toggle (Dark/Light), settings dialog toggle.
2. **Main Split View**:
   - Left Sidebar (resizable, 340dp default width):
     - Tabs for Directory Tree, Storage Hogs (top largest files), and File Extension Legend.
     - Proportional size bars and item counts.
   - Right Main Area:
     - Zoom breadcrumbs navigation trail with clickable path segments.
     - Interactive Cushion Treemap Canvas (`TreemapView`).
3. **Bottom Status Bar**:
   - Total file count, scanned size, scan throughput (files/sec, MB/sec), active threads, elapsed duration.
   - Active hover path indicator.

**Verify**: `cd android && ./gradlew :shared:compileKotlinJvm` → exit 0.

---

### Step 7: Implement `:desktopApp` Entry Point
Implement desktop application window in `desktopApp/src/jvmMain/kotlin/com/dscan/desktop/Main.kt`.

1. Configure desktop window:
   - Window title `"dscan — Disk Space Visualizer"`.
   - Minimum window dimensions: `960.dp` width, `640.dp` height. Default initial size: `1280.dp` by `800.dp`.
2. Add Desktop Menu Bar:
   - File Menu: Open Directory (`Ctrl+O`), Rescan (`Ctrl+R`), Exit (`Ctrl+Q`).
   - View Menu: Toggle Theme (`Ctrl+T`), Reset Treemap Zoom (`Ctrl+0`).
   - Help Menu: About dialog.
3. Add Keyboard Shortcuts:
   - `Ctrl+F`: focus search input.
   - `Delete`: prompt safe trash deletion for selected node.
   - `Alt+Enter`: reveal selected node in system file manager.
4. Support Drag and Drop:
   - Allow dragging folder path from OS file manager directly onto application window to initiate scan.

**Verify**: `cd android && ./gradlew :desktopApp:assemble` → exit 0.

---

### Step 8: Update `:androidApp` Entry Point
Update `androidApp/src/main/kotlin/com/dscan/app/MainActivity.kt` to consume shared modules and retain Android-specific integrations.

1. Move Android manifest, resources, and keystore into `androidApp/`.
2. In `MainActivity.kt`:
   - Retain `MANAGE_EXTERNAL_STORAGE` permission checks and SAF `OpenDocumentTree` folder picker contract.
   - Delegate UI rendering to shared `MainScreen` / `DscanTheme`.

**Verify**: `cd android && ./gradlew :androidApp:assembleDebug` → exit 0, generates debug APK.

---

## Test plan
- **Rust JNI Unit Tests**:
  - `crates/dscan-jni/src/lib.rs`: test JSON string escaping, drive list serialization, and path safety validation.
  - Command: `cargo test -p dscan-jni` → all tests pass.
- **Shared Kotlin Unit Tests**:
  - `shared/src/commonTest/kotlin/com/dscan/app/FormatTest.kt`: test `formatBytes` with 0B, KiB, MiB, GiB, TiB.
  - `shared/src/commonTest/kotlin/com/dscan/app/TreemapLayoutTest.kt`: test squarified layout aspect ratio convergence and child node bounding box containment.
  - Command: `cd android && ./gradlew :shared:allTests` → all tests pass.
- **Desktop Application Verification**:
  - Launch desktop app, initiate scan on test directory, verify treemap renders, drill down via click, verify hover tooltip, toggle dark/light theme, test search filter.

## Done criteria
- [ ] `cargo check --workspace` exits 0 with `dscan-gui` removed.
- [ ] `cargo test --workspace` exits 0.
- [ ] `cargo build -p dscan-jni --release` outputs native shared library.
- [ ] `cd android && ./gradlew :desktopApp:assemble` exits 0.
- [ ] `cd android && ./gradlew :androidApp:assembleDebug` exits 0.
- [ ] No files outside the in-scope list are modified (`git status`).
- [ ] `plans/README.md` status row updated for Plan 036.

## STOP conditions
Stop and report back if:
- `crates/dscan-core` API signatures change or do not match expected `ScanSession` interfaces.
- Kotlin Multiplatform Gradle plugin encounters incompatibility with local JVM toolchain.
- Platform JNI compilation fails on target operating system due to missing standard C runtime headers.

## Maintenance notes
- JNI native libraries for production desktop packaging can be bundled inside JAR resources under `/native/{os}-{arch}/` during release builds.
- Future enhancements may add multi-selection in the directory tree for batch trash actions.

---

### Critical Files for Implementation
- `/home/ahoura/dscan/Cargo.toml`
- `/home/ahoura/dscan/crates/dscan-jni/Cargo.toml`
- `/home/ahoura/dscan/crates/dscan-jni/src/lib.rs`
- `/home/ahoura/dscan/android/settings.gradle.kts`
- `/home/ahoura/dscan/android/shared/src/commonMain/kotlin/com/dscan/app/DscanBridge.kt`
