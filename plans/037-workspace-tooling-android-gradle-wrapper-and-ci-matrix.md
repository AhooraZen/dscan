# Plan 037: Workspace Dependency Inheritance, Android Tooling & Packaging, Multi-Platform CI Matrix, and Documentation Alignment

> **Executor instructions**: Follow this plan step by step. Run every verification command and confirm the expected result before moving to the next step. If anything in the "STOP conditions" section occurs, stop and report — do not improvise. When done, update the status row for this plan in `plans/README.md` — unless a reviewer dispatched you and told you they maintain the index.
>
> **Drift check (run first)**: `git diff --stat ba016d0..HEAD -- Cargo.toml crates/dscan-core/Cargo.toml crates/dscan-cli/Cargo.toml crates/dscan-gui/Cargo.toml crates/dscan-android/Cargo.toml android/build.gradle.kts android/app/build.gradle.kts android/gradle.properties .github/workflows/ci.yml .github/workflows/release.yml CLAUDE.md README.md plans/README.md`
> If any in-scope file changed since this plan was written, compare the "Current state" excerpts against the live code before proceeding; on a mismatch, treat it as a STOP condition.

## Status
- **Priority**: P1
- **Effort**: M
- **Risk**: LOW
- **Depends on**: none
- **Category**: dx | tech-debt | ci | docs
- **Planned at**: commit `ba016d0`, `2026-10-08`
- **Issue**: 

## Why this matters
Crate manifests duplicate package metadata (`version`, `edition`, `authors`, `license`, `repository`) and dependencies across four crates without workspace inheritance. Android build lacks Gradle wrapper scripts (`gradlew`, `gradlew.bat`, `gradle-wrapper.properties`), uses obsolete Jetifier property, embeds hardcoded keystore passwords in version control, disables R8 minification, and targets only `arm64-v8a`. CI workflows miss workspace-wide Clippy checks, skip headless tests on GUI/Android crates, and omit automated crates.io deployment. `CLAUDE.md` and `plans/README.md` document single-crate legacy structure and omit recent plans. Implementing these fixes creates reproducible builds, shrinks APK binary size, hardens CI release security, and aligns documentation across all platforms.

## Current state
- `/home/ahoura/dscan/Cargo.toml` — Workspace root manifest; defines members but lacks `[workspace.package]` and `[workspace.dependencies]` (lines 1–26).
- `/home/ahoura/dscan/crates/dscan-core/Cargo.toml:1-14` — Duplicates version `0.7.0`, edition `2024`, authors, license, repository.
- `/home/ahoura/dscan/crates/dscan-cli/Cargo.toml:1-23` — Duplicates metadata and hardcodes path `dscan-core = { version = "0.7.0", path = "../dscan-core" }`.
- `/home/ahoura/dscan/crates/dscan-gui/Cargo.toml:1-15` — Duplicates metadata, hardcodes dependency versions (`gpui-kit = "0.7"`, `open = "5.3"`, `sysinfo = "0.33"`, `trash = "5.2"`).
- `/home/ahoura/dscan/crates/dscan-gui/src/app.rs:76-84` — Contains collapsible `if` statement triggering Clippy error `-D clippy::collapsible-if`.
- `/home/ahoura/dscan/crates/dscan-android/Cargo.toml:1-16` — Duplicates metadata and dependencies (`jni = { version = "0.21", default-features = false }`).
- `/home/ahoura/dscan/android/gradle.properties:1-4` — Contains obsolete `android.enableJetifier=true`.
- `/home/ahoura/dscan/android/app/build.gradle.kts:18-51` — Hardcodes single ABI `arm64-v8a`, hardcodes signing passwords (`storePassword = "android"`, `keyPassword = "android"`), and disables R8 (`isMinifyEnabled = false`). Missing Gradle wrapper.
- `/home/ahoura/dscan/.github/workflows/ci.yml:39-41` — Runs `cargo clippy --all-targets -- -D warnings` (omits `--workspace`, masking errors in `dscan-gui`). Tests only `dscan-core` and `dscan`.
- `/home/ahoura/dscan/.github/workflows/release.yml:174-219` — Builds only `arm64-v8a` APK using system `gradle` command without wrapper; lacks automated `publish-crates-io` job.
- `/home/ahoura/dscan/CLAUDE.md:1-50` — Describes deprecated single-crate layout (`src/sys.rs`, `src/scanner.rs`) and Linux-only target.
- `/home/ahoura/dscan/plans/README.md:1-68` — Indexes only plans 001–032, contains unlinked plans (`001-gui-context-menu.md`, `002-gui-path-picker.md`, `003-package-managers.md`, `033-extreme-scanner-performance.md`).
- `/home/ahoura/dscan/README.md:247` — Reports 32 executed plans instead of 37.

## Commands you will need
| Purpose | Command | Expected on success |
|-----------|--------------------------|---------------------|
| Workspace check | `cargo check --workspace` | exit 0 |
| Workspace test | `cargo test --workspace` | all tests pass, exit 0 |
| Workspace Clippy | `cargo clippy --workspace --all-targets -- -D warnings` | exit 0, no warnings |
| Format check | `cargo fmt --check` | exit 0 |
| Android wrapper check | `cd android && ./gradlew --version` | Gradle 8.10.2 displayed, exit 0 |

## Scope
**In scope**:
- `/home/ahoura/dscan/Cargo.toml`
- `/home/ahoura/dscan/crates/dscan-core/Cargo.toml`
- `/home/ahoura/dscan/crates/dscan-cli/Cargo.toml`
- `/home/ahoura/dscan/crates/dscan-gui/Cargo.toml`
- `/home/ahoura/dscan/crates/dscan-gui/src/app.rs`
- `/home/ahoura/dscan/crates/dscan-android/Cargo.toml`
- `/home/ahoura/dscan/android/gradle/wrapper/gradle-wrapper.properties` (create)
- `/home/ahoura/dscan/android/gradlew` (create)
- `/home/ahoura/dscan/android/gradlew.bat` (create)
- `/home/ahoura/dscan/android/gradle.properties`
- `/home/ahoura/dscan/android/app/build.gradle.kts`
- `/home/ahoura/dscan/android/app/proguard-rules.pro`
- `/home/ahoura/dscan/.github/workflows/ci.yml`
- `/home/ahoura/dscan/.github/workflows/release.yml`
- `/home/ahoura/dscan/CLAUDE.md`
- `/home/ahoura/dscan/README.md`
- `/home/ahoura/dscan/plans/README.md`

**Out of scope**:
- Core scanner algorithms (`crates/dscan-core/src/scanner.rs`).
- Kotlin Compose screens (`android/app/src/main/kotlin/com/dscan/app/ui/`).

## Git workflow
- Branch: `chore/037-workspace-android-ci-docs`
- Commit per step; message style: `<conventional-commit>`
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: Workspace Manifest Inheritance in Root `Cargo.toml`
Edit `/home/ahoura/dscan/Cargo.toml` to define `[workspace.package]` and `[workspace.dependencies]`:
```toml
[workspace]
resolver = "3"
members = [
    "crates/dscan-core",
    "crates/dscan-cli",
    "crates/dscan-gui",
    "crates/dscan-android",
]
default-members = [
    "crates/dscan-core",
    "crates/dscan-cli",
]

[workspace.package]
version = "0.7.0"
edition = "2024"
authors = ["AhooraZen <ahoora935137@gmail.com>"]
license = "MIT OR Apache-2.0"
repository = "https://github.com/AhooraZen/dscan"
homepage = "https://github.com/AhooraZen/dscan"
readme = "README.md"

[workspace.dependencies]
dscan-core = { version = "0.7.0", path = "crates/dscan-core" }
gpui-kit = "0.7"
open = "5.3"
sysinfo = "0.33"
trash = "5.2"
jni = { version = "0.21", default-features = false }

[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
panic = "abort"
strip = true

[profile.dev]
debug = 1

[profile.test]
debug = 1
```

**Verify**: `cargo check -p dscan-core` → exit 0

---

### Step 2: Inherit Workspace Metadata in Member Crate Manifests
Update each crate manifest in `crates/`:

1. `/home/ahoura/dscan/crates/dscan-core/Cargo.toml`:
```toml
[package]
name = "dscan-core"
description = "High-throughput kernel directory traversal and disk space accounting engine"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
repository.workspace = true
readme = "../../README.md"

[lib]
name = "dscan_core"
path = "src/lib.rs"
```

2. `/home/ahoura/dscan/crates/dscan-cli/Cargo.toml`:
```toml
[package]
name = "dscan"
description = "Fast, zero-dependency multi-threaded disk usage analyzer for Linux and Windows"
keywords = ["disk", "du", "storage", "analyzer", "cli"]
categories = ["command-line-utilities", "filesystem"]
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
repository.workspace = true
readme = "../../README.md"

[lib]
name = "dscan_cli"
path = "src/lib.rs"

[[bin]]
name = "dscan"
path = "src/main.rs"

[dependencies]
dscan-core = { workspace = true }
```

3. `/home/ahoura/dscan/crates/dscan-gui/Cargo.toml`:
```toml
[package]
name = "dscan-gui"
description = "Native GPU-accelerated WinDirStat cushion treemap visualizer for dscan"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
repository.workspace = true

[dependencies]
dscan-core = { workspace = true }
gpui-kit = { workspace = true }
open = { workspace = true }
sysinfo = { workspace = true }
trash = { workspace = true }
```

4. `/home/ahoura/dscan/crates/dscan-android/Cargo.toml`:
```toml
[package]
name = "dscan-android"
description = "Native Android JNI bridge for dscan disk space analyzer"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
repository.workspace = true

[lib]
name = "dscan"
crate-type = ["cdylib", "rlib"]

[dependencies]
dscan-core = { workspace = true }
jni = { workspace = true }
```

**Verify**: `cargo check --workspace` → exit 0

---

### Step 3: Fix Clippy Collapsible-If in GUI Crate
In `/home/ahoura/dscan/crates/dscan-gui/src/app.rs:76-84`, collapse the nested `if let`:
```rust
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await
                && let Some(first) = paths.into_iter().next()
            {
                let _ = this.update(cx, |this, cx| {
                    this.state.target_path = first.clone();
                    this.threads = dscan_core::ScanOptions::auto_threads_for_path(&first);
                    this.start_scan(cx);
                });
            }
        })
        .detach();
```

**Verify**: `cargo clippy --workspace --all-targets -- -D warnings` → exit 0, zero warnings.

---

### Step 4: Add Gradle Wrapper & Clean Gradle Properties
1. Create directory `/home/ahoura/dscan/android/gradle/wrapper/`.
2. Create `/home/ahoura/dscan/android/gradle/wrapper/gradle-wrapper.properties`:
```properties
distributionBase=GRADLE_USER_HOME
distributionPath=wrapper/dists
distributionUrl=https\://services.gradle.org/distributions/gradle-8.10.2-bin.zip
networkTimeout=10000
validateDistributionUrl=true
zipStoreBase=GRADLE_USER_HOME
zipStorePath=wrapper/dists
```
3. Generate `/home/ahoura/dscan/android/gradlew` (standard Gradle 8 wrapper Unix shell script, mode `chmod +x 0755`) and `/home/ahoura/dscan/android/gradlew.bat` (Windows batch script).
4. In `/home/ahoura/dscan/android/gradle.properties`, delete line 2 (`android.enableJetifier=true`):
```properties
android.useAndroidX=true
org.gradle.jvmargs=-Xmx2048m -Dfile.encoding=UTF-8
```

**Verify**: `grep -rn "enableJetifier" /home/ahoura/dscan/android/` → 0 matches.

---

### Step 5: Android Multi-ABI, R8 Minification, and CI Signing Configuration
1. Edit `/home/ahoura/dscan/android/app/build.gradle.kts`:
   - Set multi-ABI filters: `abiFilters.addAll(listOf("arm64-v8a", "x86_64", "armeabi-v7a"))`.
   - Read signing configuration from environment variables with fallback to local development defaults:
     ```kotlin
     val keystoreFilePath = System.getenv("KEYSTORE_FILE") ?: "release.keystore"
     val keystorePassword = System.getenv("KEYSTORE_PASSWORD") ?: "android"
     val keyAliasVal = System.getenv("KEY_ALIAS") ?: "dscan"
     val keyPasswordVal = System.getenv("KEY_PASSWORD") ?: "android"

     signingConfigs {
         create("release") {
             if (file(keystoreFilePath).exists()) {
                 storeFile = file(keystoreFilePath)
                 storePassword = keystorePassword
                 keyAlias = keyAliasVal
                 keyPassword = keyPasswordVal
             }
             enableV1Signing = true
             enableV2Signing = true
         }
         getByName("debug") {
             enableV1Signing = true
             enableV2Signing = true
         }
     }
     ```
   - Enable R8 optimizations on release:
     ```kotlin
     buildTypes {
         release {
             isMinifyEnabled = true
             isShrinkResources = true
             signingConfig = signingConfigs.getByName("release")
             proguardFiles(
                 getDefaultProguardFile("proguard-android-optimize.txt"),
                 "proguard-rules.pro"
             )
         }
         debug {
             isDebuggable = true
             signingConfig = signingConfigs.getByName("debug")
         }
     }
     ```
2. In `/home/ahoura/dscan/android/app/proguard-rules.pro`, ensure JNI exports and Compose entrypoints are preserved under R8 minification:
```pro
-keepclassmembers class com.dscan.app.DscanBridge {
    native <methods>;
}
-keep class com.dscan.app.DscanBridge { *; }
-keepclassmembers class * {
    @androidx.compose.runtime.Composable *;
}
```

**Verify**: `grep "isMinifyEnabled = true" /home/ahoura/dscan/android/app/build.gradle.kts` → match found.

---

### Step 6: Update CI & Release Workflows
1. Edit `/home/ahoura/dscan/.github/workflows/ci.yml`:
   - In `lint-and-format`: replace line 40 with `cargo clippy --workspace --all-targets -- -D warnings`.
   - In `test-linux`: install dependencies and run `cargo test --workspace`.
   - In `test-windows`: run `cargo test --workspace`.
2. Edit `/home/ahoura/dscan/.github/workflows/release.yml`:
   - In `build-android`:
     - Set target toolchains: `targets: aarch64-linux-android,x86_64-linux-android,armv7-linux-androideabi`.
     - Build all three ABIs: `cargo ndk -t arm64-v8a -t x86_64 -t armeabi-v7a -o android/app/src/main/jniLibs build --release -p dscan-android`.
     - Build release APK: `cd android && ./gradlew assembleRelease`.
     - Upload multi-ABI APK artifact: `dscan-android-release.apk`.
   - Add `publish-crates-io` job on tags:
     ```yaml
     publish-crates-io:
       name: Publish to crates.io
       needs: [build-cli, build-gui-linux, build-gui-windows, build-android]
       runs-on: ubuntu-latest
       steps:
         - name: Checkout repository
           uses: actions/checkout@v4

         - name: Install Rust toolchain
           uses: dtolnay/rust-toolchain@stable

         - name: Publish dscan-core
           env:
             CARGO_REGISTRY_TOKEN: ${{ secrets.CARGO_REGISTRY_TOKEN }}
           run: cargo publish -p dscan-core

         - name: Wait for index propagation
           run: sleep 15

         - name: Publish dscan CLI
           env:
             CARGO_REGISTRY_TOKEN: ${{ secrets.CARGO_REGISTRY_TOKEN }}
           run: cargo publish -p dscan
     ```

**Verify**: `git diff /home/ahoura/dscan/.github/workflows/ci.yml` → confirms `--workspace` flags present.

---

### Step 7: Reconcile Plan Index in `plans/README.md`
Update `/home/ahoura/dscan/plans/README.md` to index plans 001–037 accurately:
- `033`: Extreme Filesystem Scanner Performance — Eliminating the 1.5x dust Gap
- `034`: Core Concurrency, Container CFS Quota Scaling, and Cross-Platform ABI Safety
- `035`: Zero-Copy Subtree Arena Merge, O(N) Array Rollup, and Core Scanner Modularization
- `036`: Migrate Desktop GUI to Compose Multiplatform and Unify Shared JNI
- `037`: Workspace Dependency Inheritance, Android Tooling & Packaging, Multi-Platform CI Matrix, and Documentation Alignment

Update `/home/ahoura/dscan/README.md` plan count to 37.

---

### Step 8: Update `CLAUDE.md` to Reflect Workspace Architecture
Update `/home/ahoura/dscan/CLAUDE.md` with:
1. Workspace build, test, and quality commands:
   - `cargo build --workspace`
   - `cargo test --workspace`
   - `cargo clippy --workspace --all-targets -- -D warnings`
   - `cargo fmt --check`
2. Workspace crate structure:
   - `crates/dscan-core`: Kernel traversal engine (`sys/linux`, `sys/windows`, `sys/uring`), lock-free Chase-Lev work-stealing deque (`work_stealing.rs`), zero-allocation bump directory arena (`arena.rs`), and scan coordinator (`scanner/`).
   - `crates/dscan-cli`: Zero-dependency terminal binary with neon ANSI formatting (`ui.rs`), CLI parser (`cli.rs`), and JSON exporter (`json.rs`).
   - `crates/dscan-jni`: Cross-platform dynamic library bridge (`libdscan.so`/`.dll`/`.dylib`) for Compose Multiplatform Desktop & Android.
   - `android/`: Compose Multiplatform project (`:shared`, `:desktopApp`, `:androidApp`).
3. Multi-platform target notes for Linux (`getdents64`, `statx`, `openat2`), Windows (`NtQueryDirectoryFileEx`, `FileIdBothDirectoryInfo`), and Android (NDK JNI).

---

## Test plan
- Run full workspace test suite: `cargo test --workspace` → all unit and integration tests pass across all crates.
- Run workspace Clippy gate: `cargo clippy --workspace --all-targets -- -D warnings` → zero warnings, exit 0.
- Verify Gradle wrapper integrity: `cd android && ./gradlew --version` → Gradle 8.10.2 executes.

## Done criteria
- [ ] `cargo check --workspace` exits 0.
- [ ] `cargo test --workspace` exits 0 with all crate tests passing.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` exits 0.
- [ ] `cargo fmt --check` exits 0.
- [ ] `android/gradle/wrapper/gradle-wrapper.properties` exists with Gradle 8.10.2.
- [ ] `android/gradlew` and `android/gradlew.bat` exist with executable permissions.
- [ ] `grep -rn "enableJetifier" android/` returns 0 matches.
- [ ] `grep -rn "isMinifyEnabled = true" android/app/build.gradle.kts` matches.
- [ ] `.github/workflows/ci.yml` uses `cargo clippy --workspace --all-targets -- -D warnings` and tests `--workspace`.
- [ ] `.github/workflows/release.yml` includes `publish-crates-io` and multi-ABI Android build.
- [ ] `CLAUDE.md` reflects multi-crate workspace and commands.
- [ ] `plans/README.md` indexes plans 001–037 with no broken links.
- [ ] `README.md` plan count updated to 37.

## STOP conditions
- `cargo check --workspace` fails due to unresolvable cyclic workspace dependency.
- Keystore env variable parsing in Kotlin DSL causes Gradle evaluation errors.

## Maintenance notes
- When adding a new workspace crate, add its manifest to `[workspace.members]` in root `Cargo.toml` and inherit common metadata via `*.workspace = true`.
- When releasing new versions, update `version = "x.y.z"` in `[workspace.package]` in root `Cargo.toml`; all member crates update simultaneously.

---

### Critical Files for Implementation
- `/home/ahoura/dscan/Cargo.toml`
- `/home/ahoura/dscan/android/app/build.gradle.kts`
- `/home/ahoura/dscan/.github/workflows/ci.yml`
- `/home/ahoura/dscan/.github/workflows/release.yml`
- `/home/ahoura/dscan/plans/README.md`
