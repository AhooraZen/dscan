# Plan 024: Visualizer Native File Actions, Explorer Reveal, and Safe Trash Integration
> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat 9a8fb99..HEAD -- crates/dscan-gui/src-tauri/Cargo.toml crates/dscan-gui/src-tauri/src/commands.rs crates/dscan-gui/src-tauri/src/state.rs crates/dscan-gui/src-tauri/src/lib.rs crates/dscan-gui/ui/src/state/scanStore.ts crates/dscan-gui/ui/src/state/scanController.ts crates/dscan-gui/ui/src/components/DirectoryTree.ts crates/dscan-gui/ui/src/components/CushionTreemap.ts`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status
- **Priority**: P2
- **Effort**: M
- **Risk**: LOW
- **Depends on**: plans/022-migrate-gui-from-tauri-to-native-gpui-kit.md
- **Category**: direction
- **Planned at**: commit `9a8fb99`, 2026-10-06
- **Issue**:

## Why this matters
Visual disk analysis is only half of the disk cleanup workflow: once users spot a rogue 45 GiB build directory, stale VM image, or forgotten archive in `dscan-gui`'s cushion treemap or directory tree, they need immediate actions to locate or remove it. Currently, users are forced to manually copy paths or navigate external file managers, breaking the fast feedback loop that makes disk visualizers useful.

Furthermore, naive deletion (`rm -rf` / `std::fs::remove_dir_all`) is unacceptable in a desktop utility due to catastrophic data loss risks (accidentally wiping root or system directories). This plan introduces native file actions:
1. **Accurate Reveal in File Manager**: Highlights the specific file or folder in Windows Explorer (`/select,`), macOS Finder (`open -R`), or Linux desktop file managers (via FreeDesktop D-Bus `org.freedesktop.FileManager1.ShowItems` with fallback to `xdg-open`).
2. **Safe Move to Trash / Recycle Bin**: Leverages the cross-platform `trash` crate to place files safely into the OS recycle bin rather than executing unrecoverable permanent deletes.
3. **Hardened Safety Boundaries**: Rigorous path guards strictly reject attempts to delete root (`/`, `C:\`), OS system directories (`/etc`, `/boot`, `/usr`, `C:\Windows`), or the user's home root directory.
4. **Live In-Memory Subtree Recalculation**: Prunes deleted nodes and bubbles size reductions up the ancestor chain in memory within 2ms, giving instant visual feedback without requiring an expensive full-drive re-scan.

## Current state
`dscan-gui` provides visualization via Tauri v2 and an HTML5/Canvas frontend, but file operations are either missing or naive:

- `crates/dscan-gui/src-tauri/src/commands.rs:130-161`:
  ```rust
  #[tauri::command]
  pub fn open_in_file_manager(path: String) -> Result<(), String> {
      let p = Path::new(&path);
      let _target_dir = if p.is_dir() {
          p
      } else {
          p.parent().unwrap_or(Path::new("/"))
      };

      #[cfg(target_os = "linux")]
      {
          Command::new("xdg-open")
              .arg(_target_dir)
              .spawn()
              .map_err(|e| format!("Failed to open file manager: {e}"))?;
      }
      #[cfg(target_os = "windows")]
      {
          Command::new("explorer")
              .arg(_target_dir)
              .spawn()
              .map_err(|e| format!("Failed to open explorer: {e}"))?;
      }
      #[cfg(target_os = "macos")]
      {
          Command::new("open")
              .arg(target_dir)
              .spawn()
              .map_err(|e| format!("Failed to open finder: {e}"))?;
      }

      Ok(())
  }
  ```
  *Defect*: `open_in_file_manager` merely opens the containing directory in `xdg-open` or `explorer.exe` without selecting or highlighting the file. On Windows, `explorer.exe /select,"<path>"` is required. On macOS, `open -R "<path>"` is required. On Linux, the FreeDesktop `org.freedesktop.FileManager1` D-Bus interface is required to highlight items in Dolphin, Nautilus, Nemo, and Thunar.

- `crates/dscan-gui/src-tauri/Cargo.toml:20-26`:
  ```toml
  [dependencies]
  dscan-core = { path = "../../dscan-core" }
  tauri = { version = "2", features = [] }
  serde = { version = "1.0", features = ["derive"] }
  serde_json = "1.0"
  parking_lot = "0.12"
  ```
  *Defect*: No trash abstraction exists in dependencies.

- `crates/dscan-gui/ui/src/state/scanStore.ts:46-54`:
  ```typescript
  rootPath: string = "";
  treeNodes: TreemapNode[] = [];
  nodeMap: Map<number, TreemapNode> = new Map();
  layoutItems: TreemapLayoutItem[] = [];

  selectedNodeId: number | null = null;
  hoveredNodeId: number | null = null;
  visualRootId: number = 0;
  breadcrumb: number[] = [0];
  ```
  *Defect*: No method exists to prune a deleted node, remove its subtree from `nodeMap`, subtract its byte count from ancestors, and trigger an in-memory re-render.

- `crates/dscan-gui/ui/src/components/DirectoryTree.ts:101-118`:
  Only handles left-click select and double-click drilldown. Right-click (`contextmenu`) and `Delete` key events are unhandled.

- `crates/dscan-gui/ui/src/components/CushionTreemap.ts:174-193`:
  Only handles left-click select and double-click zoom. Right-click (`contextmenu`) is unhandled.

### Repo Conventions to Maintain
- **Zero-Regression Core Traversal**: `crates/dscan-core` must remain strictly read-only and 100% free of external dependencies. Deletion and desktop integration logic belongs exclusively to `crates/dscan-gui`.
- **Safety Invariant**: Under no circumstances may code execute raw recursive deletion (`std::fs::remove_dir_all` or `rm -rf`). All deletions must pass through OS trash facilities (`trash` crate).
- **Design Tokens & Accessibility**: Follow `ui-ux-pro-max` guidelines:
  * Dark mode theme matching `#0F172A` (Slate Dark) and `#1E293B`.
  * Danger actions styled in `#EF4444` / Crimson with high-contrast confirmation.
  * No emojis used as icons; use clean, accessible vector SVGs.
  * Modal focus trapping and `Escape` keyboard dismissal.

## Commands you will need
| Purpose | Command | Expected on success |
|-----------|--------------------------|---------------------|
| GUI Backend Check | `cargo check -p dscan-gui` | exit 0, no errors |
| GUI Backend Test | `cargo test -p dscan-gui` | exit 0, all tests pass |
| GUI Backend Clippy | `cargo clippy -p dscan-gui -- -D warnings` | exit 0, zero warnings |
| Format Check | `cargo fmt --check` | exit 0, clean formatting |
| Frontend Typecheck | `cd crates/dscan-gui/ui && bun run typecheck` | exit 0, zero errors |
| Frontend Test | `cd crates/dscan-gui/ui && bun test` | exit 0, all tests pass |
| Frontend Build | `cd crates/dscan-gui/ui && bun run build` | exit 0, build succeeds |

## Suggested executor toolkit
- **`rust-dev`**: Idiomatic Rust 2024, type-safe path validation, error handling via `Result<T, E>`.
- **`typescript-dev`**: Strict TypeScript 5+ type models, non-null assertions avoided, immutable store updates.
- **`ui-ux-pro-max`**: Accessible modal dialog design, backdrop blur, focus trap, and keyboard event bindings.

## Scope
**In scope** (files to create/modify):
- `crates/dscan-gui/src-tauri/Cargo.toml` (add `trash = "5.2"`)
- `crates/dscan-gui/src-tauri/src/safety.rs` (create: path validation and protected root guards)
- `crates/dscan-gui/src-tauri/src/commands.rs` (update `reveal_in_file_manager`, add `trash_node`)
- `crates/dscan-gui/src-tauri/src/state.rs` (add `TrashResultResponse` DTO)
- `crates/dscan-gui/src-tauri/src/lib.rs` (register new commands and modules)
- `crates/dscan-gui/src-tauri/tests/safety_tests.rs` (create: safety verification suite)
- `crates/dscan-gui/ui/src/state/scanStore.ts` (add `pruneNodeAndRecalculate` method)
- `crates/dscan-gui/ui/src/state/scanController.ts` (add `revealInFileManager`, `trashNode`)
- `crates/dscan-gui/ui/src/components/ContextMenu.ts` (create: desktop right-click menu)
- `crates/dscan-gui/ui/src/components/ConfirmDeleteModal.ts` (create: accessible confirmation dialog)
- `crates/dscan-gui/ui/src/components/DirectoryTree.ts` (wire context menu and `Delete` key)
- `crates/dscan-gui/ui/src/components/CushionTreemap.ts` (wire right-click context menu)
- `crates/dscan-gui/ui/src/state/scanStore.test.ts` (create: in-memory pruning test)
- `crates/dscan-gui/ui/src/components/ConfirmDeleteModal.test.ts` (create: modal dialog test)

**Out of scope** (do NOT touch):
- `crates/dscan-core/*`: Core traversal engine is strictly read-only and zero-dependency. Do not add deletion logic or external crates to `dscan-core`.
- `crates/dscan-cli/*`: Terminal CLI is non-interactive; interactive trash and GUI file actions are GUI-only.
- Permanent deletion bypassing trash (`rm -rf`, `std::fs::remove_file`): Destructive and unrecoverable; strictly excluded.

## Git workflow
- Branch: `feat/024-visualizer-native-file-actions-and-trash`
- Commit per step or per logical unit; message style: Conventional Commits (`feat(gui): ...`, `test(gui): ...`).
- Do NOT push or open a PR unless explicitly instructed by the operator.

---

## Steps

### Step 1: Implement Path Protection and Safety Boundary Guard
Create `crates/dscan-gui/src-tauri/src/safety.rs` to validate candidate paths before any file action is executed.

1. Create enum `SafetyError`:
   ```rust
   #[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
   pub enum SafetyError {
       EmptyPath,
       DoesNotExist(String),
       RootDirectoryForbidden(String),
       HomeDirectoryForbidden(String),
       ProtectedSystemPath(String),
       OutsideScanRoot { path: String, root: String },
       CanonicalizationFailed(String),
   }
   ```
2. Implement `validate_trash_target(raw_path: &str, active_scan_root: Option<&str>) -> Result<std::path::PathBuf, SafetyError>`.
   - Rejects empty strings.
   - Converts to `PathBuf` and calls `std::fs::canonicalize(p)`. If target does not exist, return `SafetyError::DoesNotExist`.
   - **Root checks**:
     * Unix: Reject `/`.
     * Windows: Reject drive roots like `C:\`, `D:\`, or UNC roots `\\server\share`.
   - **User Home check**:
     * Query `$HOME` or `%USERPROFILE%`.
     * If canonicalized path equals home directory, return `SafetyError::HomeDirectoryForbidden`.
   - **System Directory checks**:
     * Unix: Check if canonicalized path equals or is a direct parent of:
       `["/bin", "/boot", "/dev", "/etc", "/lib", "/lib32", "/lib64", "/proc", "/root", "/run", "/sbin", "/sys", "/usr", "/var"]`.
     * Windows: Check if canonicalized path equals or is within:
       `["C:\\Windows", "C:\\Windows\\System32", "C:\\Program Files", "C:\\Program Files (x86)", "C:\\ProgramData", "C:\\Users"]`.
   - **Scan Root boundary check**:
     * If `active_scan_root` is provided, canonicalize `active_scan_root`.
     * Ensure `canonical_path.starts_with(&canonical_scan_root)`. If not, return `SafetyError::OutsideScanRoot`.

**Verify**:
`cargo check -p dscan-gui` → exit 0

---

### Step 2: Implement Native Reveal in File Manager
Update `crates/dscan-gui/src-tauri/src/commands.rs` to replace naive `open_in_file_manager` with true native selection across Windows, macOS, and Linux:

```rust
pub fn reveal_in_file_manager_impl(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("Path does not exist: {}", path.display()));
    }

    #[cfg(target_os = "windows")]
    {
        // Windows Explorer: /select,"<path>" highlights the file or folder
        let path_str = path.to_string_lossy().replace('/', "\\");
        let arg = format!("/select,{}", path_str);
        Command::new("explorer")
            .arg(arg)
            .spawn()
            .map_err(|e| format!("Failed to spawn explorer: {e}"))?;
    }

    #[cfg(target_os = "macos")]
    {
        // macOS Finder: open -R <path> reveals the item
        Command::new("open")
            .arg("-R")
            .arg(path)
            .spawn()
            .map_err(|e| format!("Failed to spawn open -R: {e}"))?;
    }

    #[cfg(target_os = "linux")]
    {
        // 1. Primary: FreeDesktop org.freedesktop.FileManager1 D-Bus interface
        let uri = format!("file://{}", path.canonicalize().unwrap_or_else(|_| path.to_path_buf()).display());
        let dbus_status = Command::new("dbus-send")
            .args([
                "--session",
                "--dest=org.freedesktop.FileManager1",
                "--type=method_call",
                "/org/freedesktop/FileManager1",
                "org.freedesktop.FileManager1.ShowItems",
                &format!("array:string:{uri}"),
                "string:",
            ])
            .status();

        let dbus_succeeded = dbus_status.map(|s| s.success()).unwrap_or(false);

        // 2. Fallback: If D-Bus method failed or dbus-send missing, use xdg-open on container dir
        if !dbus_succeeded {
            let target_dir = if path.is_dir() {
                path
            } else {
                path.parent().unwrap_or(Path::new("/"))
            };
            Command::new("xdg-open")
                .arg(target_dir)
                .spawn()
                .map_err(|e| format!("Failed to spawn xdg-open: {e}"))?;
        }
    }

    Ok(())
}
```

Expose this as `#[tauri::command] pub fn reveal_in_file_manager(path: String) -> Result<(), String>`.

**Verify**:
`cargo check -p dscan-gui` → exit 0

---

### Step 3: Add `trash` Crate and Backend Move-to-Trash Command
1. In `crates/dscan-gui/src-tauri/Cargo.toml`, add `trash = "5.2"` under `[dependencies]`.
2. In `crates/dscan-gui/src-tauri/src/state.rs`, define response DTO:
   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct TrashResultResponse {
       pub success: bool,
       pub node_id: u32,
       pub path: String,
       pub reclaimed_bytes: u64,
       pub is_dir: bool,
   }
   ```
3. In `crates/dscan-gui/src-tauri/src/commands.rs`, implement `trash_node`:
   ```rust
   #[tauri::command]
   pub fn trash_node(
       node_id: u32,
       path: String,
       state: State<'_, AppState>,
   ) -> Result<TrashResultResponse, String> {
       let current_scan_root = state.current_path.read().clone();
       let scan_root_opt = if current_scan_root.is_empty() {
           None
       } else {
           Some(current_scan_root.as_str())
       };

       // 1. Enforce safety invariants
       let validated_path = crate::safety::validate_trash_target(&path, scan_root_opt)
           .map_err(|e| format!("Safety violation: {e:?}"))?;

       // 2. Query metadata before trashing
       let meta = std::fs::symlink_metadata(&validated_path)
           .map_err(|e| format!("Failed to read metadata: {e}"))?;
       let is_dir = meta.is_dir();
       
       // Calculate reclaimed bytes (for file, use file size; for dir, read from AppState or metadata)
       let reclaimed_bytes = if is_dir {
           // Compute directory size or fallback to stat
           crate::safety::compute_directory_size_quick(&validated_path).unwrap_or(0)
       } else {
           meta.len()
       };

       // 3. Move to system Trash / Recycle Bin
       trash::delete(&validated_path)
           .map_err(|e| format!("Failed to move to trash: {e}"))?;

       Ok(TrashResultResponse {
           success: true,
           node_id,
           path,
           reclaimed_bytes,
           is_dir,
       })
   }
   ```
4. Register `safety` module in `crates/dscan-gui/src-tauri/src/lib.rs`, and add `reveal_in_file_manager` and `trash_node` to `tauri::generate_handler![...]`.

**Verify**:
`cargo check -p dscan-gui` → exit 0

---

### Step 4: Add Backend Safety Unit Tests
Create `crates/dscan-gui/src-tauri/tests/safety_tests.rs`:
1. `test_validate_rejects_root`: verifies `/` (or `C:\`) is rejected with `SafetyError::RootDirectoryForbidden`.
2. `test_validate_rejects_system_dirs`: verifies `/etc`, `/usr`, `/boot` return `SafetyError::ProtectedSystemPath`.
3. `test_validate_rejects_home_dir`: verifies `$HOME` itself returns `SafetyError::HomeDirectoryForbidden`.
4. `test_validate_allows_safe_temp_file`: creates a tempfile in `std::env::temp_dir()`, confirms validation succeeds.
5. `test_validate_enforces_scan_root`: verifies a path outside `active_scan_root` returns `SafetyError::OutsideScanRoot`.

**Verify**:
`cargo test -p dscan-gui --test safety_tests` → exit 0, all 5 tests pass.

---

### Step 5: Implement In-Memory Subtree Pruning in `ScanStore`
In `crates/dscan-gui/ui/src/state/scanStore.ts`, implement `pruneNodeAndRecalculate(nodeId: number, backendReclaimedBytes?: number)`:

```typescript
pruneNodeAndRecalculate(nodeId: number, backendReclaimedBytes?: number): { reclaimedBytes: number; deletedFiles: number } | null {
  const target = this.nodeMap.get(nodeId);
  if (!target) return null;

  const reclaimedBytes = backendReclaimedBytes ?? target.total_bytes;
  let deletedFiles = 0;
  const deletedIds = new Set<number>();

  // 1. Collect all descendant node IDs
  const collectDescendants = (id: number) => {
    deletedIds.add(id);
    const curr = this.nodeMap.get(id);
    if (!curr) return;
    if (!curr.is_dir) {
      deletedFiles += 1;
      // Decrement extension stats
      const ext = curr.extension || "";
      const stat = this.extensionStats.find((s) => s.extension === ext);
      if (stat) {
        stat.totalBytes = Math.max(0, stat.totalBytes - curr.total_bytes);
        stat.fileCount = Math.max(0, stat.fileCount - 1);
      }
    }
    for (const childId of curr.children_ids) {
      collectDescendants(childId);
    }
  };

  collectDescendants(nodeId);

  // 2. Remove from parent's children_ids
  if (target.parent_id !== target.id) {
    const parent = this.nodeMap.get(target.parent_id);
    if (parent) {
      parent.children_ids = parent.children_ids.filter((id) => id !== nodeId);
    }
  }

  // 3. Roll up size subtraction up the ancestor chain
  let ancestorId: number | null = target.parent_id;
  while (ancestorId !== null) {
    const ancestor = this.nodeMap.get(ancestorId);
    if (!ancestor) break;
    ancestor.total_bytes = Math.max(0, ancestor.total_bytes - reclaimedBytes);
    if (ancestor.id === 0 || ancestor.parent_id === ancestor.id) break;
    ancestorId = ancestor.parent_id;
  }

  // 4. Remove pruned nodes from nodeMap and treeNodes
  for (const id of deletedIds) {
    this.nodeMap.delete(id);
  }
  this.treeNodes = this.treeNodes.filter((n) => !deletedIds.has(n.id));

  // 5. Adjust global progress counters
  this.progress.totalBytes = Math.max(0, this.progress.totalBytes - reclaimedBytes);
  this.progress.totalFiles = Math.max(0, this.progress.totalFiles - deletedFiles);

  // 6. Selection & Breadcrumb recovery
  if (this.selectedNodeId !== null && deletedIds.has(this.selectedNodeId)) {
    this.selectedNodeId = this.nodeMap.has(target.parent_id) ? target.parent_id : 0;
  }
  if (deletedIds.has(this.visualRootId)) {
    this.visualRootId = 0;
    this.breadcrumb = [0];
  } else {
    this.breadcrumb = this.breadcrumb.filter((id) => !deletedIds.has(id));
  }

  // 7. Rebalance extension percentages
  const grandTotal = this.progress.totalBytes;
  for (const stat of this.extensionStats) {
    stat.percentageOfTotal = grandTotal > 0 ? (stat.totalBytes / grandTotal) * 100 : 0;
  }
  this.extensionStats = this.extensionStats.filter((s) => s.totalBytes > 0);

  // 8. Notify all UI listeners (Tree, Treemap, Status, Legend)
  this.notify();

  return { reclaimedBytes, deletedFiles };
}
```

Add a helper method `getNodeFullPath(nodeId: number): string` to resolve full filesystem paths by walking up the `parent_id` hierarchy to `rootPath`.

**Verify**:
`cd crates/dscan-gui/ui && bun run typecheck` → exit 0

---

### Step 6: Create Confirmation Dialog and Context Menu Components
1. **`crates/dscan-gui/ui/src/components/ConfirmDeleteModal.ts`**:
   - Accessible modal overlaid on `#app` with `z-50 backdrop-blur-sm bg-black/60`.
   - Shows:
     * Header: "Move to Trash" (with SVG warning icon, no emojis).
     * Monospace path display (truncated with full path tooltip).
     * Reclaimed disk space formatted (e.g. `4.82 GiB`).
     * Note: "This item will be moved to the system Trash / Recycle Bin."
     * Buttons: Cancel (secondary, focused by default) and "Move to Trash" (`bg-accent-red hover:bg-accent-red/90 text-white`).
     * Keyboard bindings: `Escape` dismisses; `Enter` activates focused button.
2. **`crates/dscan-gui/ui/src/components/ContextMenu.ts`**:
   - Positioned at `(clientX, clientY)` with boundary clamping to viewport dimensions.
   - Menu items:
     * **Reveal in File Manager** (`scanController.revealInFileManager(path)`).
     * **Copy Path** (`navigator.clipboard.writeText(path)`).
     * Divider.
     * **Move to Trash...** (opens `ConfirmDeleteModal`).
   - Closes on click-outside, scroll, or window resize.
3. Update `crates/dscan-gui/ui/src/state/scanController.ts`:
   - Add `revealInFileManager(path: string)`.
   - Add `trashNode(nodeId: number, path: string)`. Calls `safeInvoke("trash_node", { nodeId, path })` and on success invokes `scanStore.pruneNodeAndRecalculate(nodeId, result.reclaimed_bytes)`.

**Verify**:
`cd crates/dscan-gui/ui && bun run typecheck` → exit 0

---

### Step 7: Wire Context Menu & Keyboard Actions in Tree and Treemap
1. In `crates/dscan-gui/ui/src/components/DirectoryTree.ts`:
   - Attach `contextmenu` listener on `this.content`:
     * Prevent default browser menu.
     * Detect closest `[data-node-id]`.
     * Select the node and display `ContextMenu`.
   - In `handleKeyDown(e: KeyboardEvent)`:
     * Add `Delete` key handler: if a node is selected, prompt `ConfirmDeleteModal`.
2. In `crates/dscan-gui/ui/src/components/CushionTreemap.ts`:
   - Attach `contextmenu` listener on `this.canvas`:
     * Prevent default browser menu.
     * Find item at `(x, y)` via `this.findItemAt(x, y)`.
     * Select the node and display `ContextMenu`.
3. In `crates/dscan-gui/ui/src/main.ts`:
   - Ensure the modal and context menu singleton DOM nodes are attached to `#app`.

**Verify**:
`cd crates/dscan-gui/ui && bun run build` → exit 0, bundle generated cleanly.

---

### Step 8: Write Frontend Unit Tests and Final Verification
1. Create `crates/dscan-gui/ui/src/state/scanStore.test.ts`:
   - Sets up synthetic tree nodes (root `0`, parent `1`, child `2`).
   - Calls `scanStore.pruneNodeAndRecalculate(2)`.
   - Asserts:
     * Node `2` removed from `nodeMap`.
     * Node `1`'s `total_bytes` reduced by child size.
     * Root `0`'s `total_bytes` reduced by child size.
     * `progress.totalBytes` updated accurately.
     * `selectedNodeId` reset safely to parent if child was selected.
2. Create `crates/dscan-gui/ui/src/components/ConfirmDeleteModal.test.ts`:
   - Verifies dialog mounts to DOM, renders formatted size and path, and resolves cancel and confirm callbacks.

**Verify**:
`cd crates/dscan-gui/ui && bun test` → exit 0, all tests pass.

---

## Test plan
1. **Safety Boundaries (Rust)**:
   - `cargo test -p dscan-gui --test safety_tests`
   - Covers: root rejection, system path rejection, home directory rejection, traversal outside scan root.
2. **In-Memory Pruning (TypeScript)**:
   - `cd crates/dscan-gui/ui && bun test`
   - Covers: ancestor rollup subtraction, child pruning, extension stat recalculation, selection preservation.
3. **Workspace Check & Lint**:
   - `cargo check -p dscan-gui`
   - `cargo clippy -p dscan-gui -- -D warnings`
   - `cargo fmt --check`
   - `cd crates/dscan-gui/ui && bun run typecheck`
   - `cd crates/dscan-gui/ui && bun run build`

## Done criteria
- [ ] `cargo check -p dscan-gui` exits 0.
- [ ] `cargo test -p dscan-gui` exits 0 with all safety tests passing.
- [ ] `cargo clippy -p dscan-gui -- -D warnings` exits 0 with zero warnings.
- [ ] `cd crates/dscan-gui/ui && bun run typecheck` exits 0 with zero TypeScript errors.
- [ ] `cd crates/dscan-gui/ui && bun test` exits 0 with all frontend tests passing.
- [ ] `cd crates/dscan-gui/ui && bun run build` exits 0 and bundles static assets.
- [ ] Context menu and `ConfirmDeleteModal` are accessible without emoji characters.
- [ ] No files in `crates/dscan-core/` or `crates/dscan-cli/` are modified.
- [ ] Status row for Plan 024 updated in `plans/README.md`.

## STOP conditions
Stop and report back (do not improvise) if:
- `trash` crate fails to link or compile on Linux target without system C development headers.
- The executor attempts to implement permanent deletion (`rm -rf` / `remove_dir_all`) as a fallback when `trash` is unavailable. (Permanent deletion is strictly forbidden; report immediately).
- Live tree data structure in `scanStore.ts` does not contain `parent_id` relationships, preventing upward ancestor rollup.
- Operating system permissions block reading Trash paths on non-standard distributions (e.g. NixOS / containerized flatpaks).

## Maintenance notes
- **Future GPUI Migration Portability (Plan 022)**: The safety checks in `safety.rs` and the cross-platform file manager reveal commands in `commands.rs` are designed as pure Rust functions independent of Tauri. When migrating to native GPUI/Rust UI, these modules can be directly imported without rewriting.
- **Undo / Trash Restoration**: Moving to Trash preserves OS-native undo capability (e.g. `Ctrl+Z` in Windows Explorer or opening the Trash folder on Linux/macOS to restore). Future iterations may add a temporary in-app "Undo" toast by tracking trashed item records if supported by the OS trash backend.
