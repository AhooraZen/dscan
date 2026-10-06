# Plan 031: Fix Path Normalization Splitting and GUI Treemap Relative Root Dropping

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat 909f3e0..HEAD -- crates/dscan-core/src/scanner.rs crates/dscan-core/src/snapshot.rs crates/dscan-gui/src/state.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status
- **Priority**: P1
- **Effort**: S
- **Risk**: LOW (pure normalization and prefix-stripping correctness fixes)
- **Depends on**: plans/028-device-aware-adaptive-rotational-thread-scaling.md
- **Category**: bug
- **Planned at**: commit `909f3e0`, 2026-10-06

## Root Cause Analysis

### 1. CLI Directory Size Splitting (`./Downloads` vs `Downloads`)
When scanning relative paths like `.`:
- Worker 0 starts with task `PathBuf::from(".")`, creating an arena root named `b"."`. Directories scanned by Worker 0 are reconstructed with the prefix `./`, e.g. `./Downloads` and `./.config`.
- Subdirectories stolen by other workers (e.g. Worker 1) are pushed as bare relative names without `.`, e.g. `Downloads` or `.config`. Worker 1 creates an arena root named `b"Downloads"`.
- When all results are aggregated in `execute_workers_and_rollup`, the hash map treats `./Downloads` and `Downloads` as two completely separate directories. The directory's total size is fragmented across two entries (e.g. 6.48 GiB in `Downloads` and 3.92 GiB in `./Downloads`, summing to 10.40 GiB). Depending on thread scheduling and work-stealing order, the split varies between runs, making directory sizes appear nondeterministic.
- **Fix**: Canonicalize / normalize all directory and file paths via `normalize_scan_path(path: &Path) -> PathBuf`. By stripping redundant `CurDir` (`.`) components, both `./Downloads` and `Downloads` normalize to `Downloads`, merging them deterministically.

### 2. GUI Blank / Only Working on `/efi`
- In `crates/dscan-core/src/snapshot.rs`:
  ```rust
  let is_complete = self.state.done.load(Ordering::Relaxed);
  ```
  `self.state.done` is set to `true` as soon as workers finish directory scanning, while the coordinator thread is still executing rollup and constructing `ScanResult`. GUI frames polling at 60 FPS saw `is_complete == true` while `self.result` was still `None`, returned a dummy single-element root, marked `self.is_complete = true`, and stopped polling permanently.
- In `crates/dscan-core/src/snapshot.rs:build_treemap_nodes`:
  ```rust
  let Ok(rel) = dir_path.strip_prefix(root_path) else { continue; };
  ```
  When scanning relative targets (`.` or bare folder names), `Path::new("Downloads").strip_prefix(Path::new("."))` in Rust fails with `StripPrefixError` because `"Downloads"` does not have a leading `CurDir` component. As a result, 100% of directories and top files were dropped on relative scans, producing an empty visualizer. It only worked on absolute paths like `/efi` or `/boot/efi` where `strip_prefix` matched.
- **Fix**:
  1. In `ScanSession::poll_progress()`, define `is_complete = self.result.lock().unwrap().is_some()`.
  2. In `build_treemap_nodes`, handle relative roots cleanly: if `root_path == Path::new(".")` or if `dir_path.strip_prefix(root_path)` fails but `dir_path` is already relative, use `dir_path` directly.

## Implementation Steps

### Step 1: Implement `normalize_scan_path` in `crates/dscan-core/src/scanner.rs`
```rust
pub fn normalize_scan_path(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::CurDir => continue,
            _ => out.push(c),
        }
    }
    if out.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        out
    }
}
```
Use `normalize_scan_path` when collecting `all_dirs` and `files_vec` in `execute_workers_and_rollup`.

### Step 2: Fix `ScanSession::poll_progress` and `is_complete` in `crates/dscan-core/src/snapshot.rs`
Ensure `is_complete` in `ScanProgressDto` and `ScanSession::is_complete()` strictly checks `self.result.lock().unwrap().is_some()`.

### Step 3: Fix `build_treemap_nodes` Relative Root Handling in `crates/dscan-core/src/snapshot.rs`
For both `top_dirs` and `top_files`:
```rust
let rel_opt = if root_path == Path::new(".") || root_path.as_os_str().is_empty() {
    Some(dir_path.as_path())
} else if let Ok(rel) = dir_path.strip_prefix(root_path) {
    Some(rel)
} else if dir_path.is_relative() {
    Some(dir_path.as_path())
} else {
    None
};
let Some(rel) = rel_opt else { continue; };
```

### Step 4: Add Unit and Integration Tests
- Add test in `crates/dscan-core/tests/scanner_integration.rs` verifying that scanning `.` yields no duplicate `./` and bare directory names.
- Add test in `crates/dscan-core/src/snapshot.rs` verifying that `build_treemap_nodes` with `root = "."` successfully builds multi-level hierarchy without dropping child nodes.

### Step 5: Verification & Publish
- `cargo test -p dscan-core -p dscan` (100% green).
- `cargo check -p dscan-gui`.
- Bump version to `0.4.3` in all 3 Cargo.toml files.
- Publish to crates.io and push tag `v0.4.3`.
