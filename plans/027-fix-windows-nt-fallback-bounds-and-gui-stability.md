# Plan 027: Fix Windows NT Fallback Bounds, Path Escaping, and GUI Stability

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat 0330385..HEAD -- crates/dscan-core/src/sys/windows.rs crates/dscan-gui/src/system.rs crates/dscan-core/src/snapshot.rs crates/dscan-gui/src/main.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status
- **Priority**: P2
- **Effort**: S
- **Risk**: LOW (platform-isolated bug fixes in Windows fallback parsing, file manager reveal, and treemap layout)
- **Depends on**: plans/020-fix-windows-nt-kernel-directory-traversal-and-crashes.md, plans/022-migrate-gui-from-tauri-to-native-gpui-kit.md
- **Category**: bug
- **Planned at**: commit `0330385`, 2026-10-06
- **Issue**: 

## Why this matters
Several edge cases in Windows fallback parsing, file explorer interaction, and desktop GUI layout can cause crashes or inaccurate rendering on Windows:

1. **Windows NT Fallback Record Truncation**:
   In `crates/dscan-core/src/sys/windows.rs:294`:
   ```rust
   let min_hdr = std::mem::size_of::<FileIdBothDirInfo>(); // 112 bytes
   while offset + min_hdr <= len as usize {
   ```
   `size_of::<FileIdBothDirInfo>()` includes 8 bytes of trailing struct alignment padding. A 1-character file or directory entry is 106 bytes in total. If a single-character entry ends within 6 bytes of the buffer boundary, `offset + min_hdr` exceeds `len`, prematurely aborting the loop and dropping that entry from the scan results.
2. **Broken Windows Explorer Reveal**:
   In `crates/dscan-gui/src/system.rs:98-102`:
   ```rust
   std::process::Command::new("explorer")
       .arg(format!("/select,\"{}\"", path.display()))
       .spawn()
   ```
   Rust's `Command` implementation on Windows automatically quotes arguments containing special characters or spaces. Including explicit literal quotes inside `format!("/select,\"{}\"")` causes Explorer to receive `explorer "/select,\"C:\path\""`, which Explorer fails to parse, either doing nothing or opening the default Documents folder instead of highlighting the selected file.
3. **Treemap Intermediate Ancestor Size Inconsistency**:
   In `crates/dscan-core/src/snapshot.rs:280-310`, `build_treemap_nodes` creates intermediate directory ancestors on-the-fly and initializes them with `total_bytes: *dir_bytes` of the first child that encounters them. If other sibling subdirectories are later added to that same parent, the parent's `total_bytes` is not updated. In step 3 (`Balance subtree sizes`), `nodes[dir_id].total_bytes` can end up smaller than `sum(children.total_bytes)`, causing squarified treemap cells to exceed parent bounding boxes and generating distorted cushion treemaps.
4. **Unhandled Window Creation Failure on Windows**:
   In `crates/dscan-gui/src/main.rs:38-40`, `open_window` ends with `.expect("failed to open dscan visualizer window")`. On systems without hardware Direct3D 12 / Vulkan support (e.g. Remote Desktop sessions, VMs, older GPUs), this produces an unhandled panic without user diagnostics.

## Current state
- `crates/dscan-core/src/sys/windows.rs:294`:
  ```rust
  let mut actual_bytes = 0usize;
  let mut offset = 0usize;
  let min_hdr = std::mem::size_of::<FileIdBothDirInfo>();
  let fn_offset = std::mem::offset_of!(FileIdBothDirInfo, file_name);

  while offset + min_hdr <= len as usize {
  ```
- `crates/dscan-gui/src/system.rs:96-102`:
  ```rust
  #[cfg(target_os = "windows")]
  {
      std::process::Command::new("explorer")
          .arg(format!("/select,\"{}\"", path.display()))
          .spawn()
          .map_err(|e| e.to_string())?;
  }
  ```

## Implementation steps

### Step 1: Fix `min_hdr` Buffer Condition in `crates/dscan-core/src/sys/windows.rs`
Update line 294 in `crates/dscan-core/src/sys/windows.rs`:
```rust
let fn_offset = std::mem::offset_of!(FileIdBothDirInfo, file_name);

while offset + fn_offset <= len as usize {
    let entry = unsafe {
        std::ptr::read_unaligned(
            (buffer as *const u8).add(offset) as *const FileIdBothDirInfo
        )
    };
    let entry_end = offset + fn_offset + (entry.file_name_length as usize);
    if entry_end > len as usize {
        break;
    }
```

### Step 2: Fix Explorer Argument Formatting in `crates/dscan-gui/src/system.rs`
Update line 99 in `crates/dscan-gui/src/system.rs`:
```rust
#[cfg(target_os = "windows")]
{
    std::process::Command::new("explorer")
        .arg(format!("/select,{}", path.display()))
        .spawn()
        .map_err(|e| e.to_string())?;
}
```

### Step 3: Ensure Strict Tree Balance in `crates/dscan-core/src/snapshot.rs`
In `build_treemap_nodes`:
1. Initialize intermediate ancestor nodes with `total_bytes: 0`.
2. After inserting all directories and top files, perform a bottom-up reverse pass across `nodes`:
   For any directory node, set `node.total_bytes = node.children_ids.iter().map(|&id| nodes[id as usize].total_bytes).sum()`.
   If the directory has a known `direct_bytes` or leftover space, insert an `[other files]` node to absorb the difference, ensuring `parent.total_bytes == sum(children.total_bytes)` strictly.

### Step 4: Verification
Run tests to verify that Windows NtQueryDirectory fallback and treemap building tests pass:
```bash
cargo test -p dscan-core --test snapshot_tests
cargo check --workspace
```

## STOP conditions
1. If modifying `build_treemap_nodes` causes `test_scan_session_synthetic_tree` to fail, verify node ID mapping.
2. Confirm that `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass cleanly.
