# Plan 001: GUI Treemap Right-Click Context Menu and Safe File Actions

> **Executor instructions**: Follow this plan step by step. Run every verification command and confirm expected result before moving to next step. If anything in "STOP conditions" occurs, stop and report — do not improvise.

## Status
- **Priority**: P1
- **Effort**: S
- **Risk**: LOW (self-contained GUI interaction layer and event handlers)
- **Category**: feature / ui-ux
- **Planned at**: 2026-10-07
- **Target Crate**: `crates/dscan-gui`

---

## 1. Context & Motivation

`dscan` features a high-performance GPUI-based desktop visualizer (`crates/dscan-gui`) rendering interactive cushion treemaps of disk allocations. Users exploring large directories or disk hogs require fast contextual file management directly from the visualizer tiles:

1. **Open in file manager**: Reveal and focus the item in the native desktop file manager (Explorer, Dolphin/Nautilus, Finder).
2. **Copy path**: Copy absolute filesystem path to system clipboard.
3. **Delete to trash**: Move safe files/folders to the OS recycle bin with safety guards and confirmation dialog.

### Key Existing Building Blocks
- `crates/dscan-gui/src/system.rs`:
  - `reveal_in_file_manager(path: &Path) -> Result<(), String>`: Native platform file manager launcher (`explorer /select`, `open -R`, or `open::that`).
  - `is_safe_to_trash(path: &Path, scan_root: &Path) -> Result<(), &'static str>`: Guard rejecting root (`/`, `C:\`), scan root directory, user home directory, and OS system directories (`/etc`, `/bin`, `/usr`, `C:\Windows`, etc.).
  - `move_to_trash(path: &Path, scan_root: &Path) -> Result<(), String>`: Safe trashing via `trash` crate with guard validation.
- `crates/dscan-gui/src/state.rs`:
  - `hit_test(px, py)`: Translates window coordinates to treemap node IDs.
  - `node_full_path(id)`: Walks ancestor hierarchy to reconstruct absolute filesystem paths.
  - `prune_node_and_bubble_size(id)`: Removes deleted nodes, bubbles reclaimed bytes up to ancestors, re-indexes contiguous node IDs, updates extension breakdowns, and rebuilds treemap layout.
- `crates/dscan-gui/src/treemap/element.rs`:
  - `CushionTreemapElement`: Custom GPUI `Element` rendering shaded cushions and hover/selection outlines.
- `crates/dscan-gui/src/views/cushion_treemap.rs`:
  - Enclosing pane handling mouse interactions (`on_mouse_move`, `on_mouse_down`).

---

## 2. Gaps in Live Codebase

1. **Missing "Copy Path" Action**: Context menu does not expose a "Copy Path" button; `DscanApp` lacks a `copy_node_path()` method.
2. **Clipboard Integration**: Must utilize GPUI's built-in platform clipboard API (`cx.write_to_clipboard(gpui::ClipboardItem::new_string(path))`) with zero new external crates.
3. **Selection Desync on Right-Click**: Right-clicking in `cushion_treemap.rs` currently opens the menu without updating `selected_node_id`, leaving the targeted tile unhighlighted.
4. **Context Menu Geometry**: Current context menu popup height (`84.0px`) assumes only 2 items. With 3 actions plus a visual separator, height must expand to `~130.0px` with proper viewport edge clamping.
5. **Safety Pre-Check for Delete Action**:
   - In context menu: If `is_safe_to_trash()` returns `Err`, "Delete to Trash" should be visibly disabled/muted (`cursor_not_allowed`, dimmed color) rather than appearing clickable.
   - In `request_trash_node()`: Must run `is_safe_to_trash()` upfront and populate `error_msg` if invalid.
   - In `trash-confirm-modal`: If `error_msg` is present, the primary "Move to Trash" button must be disabled or replaced with "Dismiss", preventing accidental deletion attempts.
6. **Dismissal & Escape Interception**: Pressing `Escape` currently executes `CancelScan` unconditionally. It should first dismiss any active confirmation modal or context menu.

---

## 3. Architecture & Data Flow

```
User Right-Clicks Treemap Tile
          │
          ▼
`views/cushion_treemap.rs` (on_mouse_down Right)
          │
          ├─► state.hit_test(px, py) -> Some(node_id)
          ├─► state.selected_node_id = Some(node_id)  [highlight tile]
          └─► app.open_context_menu(node_id, x, y, cx)
                    │
                    ▼
          `DscanApp::render()` -> renders `#app-context-menu`
                    │
   ┌────────────────┼────────────────────────┐
   ▼                ▼                        ▼
Action 1:        Action 2:               Action 3:
Open in FM       Copy Path               Delete to Trash
   │                │                        │
   ├─► system::     ├─► cx.write_to_         ├─► check system::is_safe_to_trash
   │   reveal_in_   │   clipboard(           │   └─ If unsafe: disabled in menu
   │   file_manager │   ClipboardItem)       │
   └─► close_menu   └─► close_menu           └─► app.request_trash_node()
                                                     │
                                                     ▼
                                            `#trash-confirm-modal`
                                                     │
                                            [Cancel] ┴ [Confirm]
                                               │           │
                                               │           ▼
                                               │      system::move_to_trash()
                                               │           │
                                               │      state.prune_node_and_bubble_size()
                                               │           │
                                            cancel_trash  rebuild_layout()
```

---

## 4. Implementation Steps

### Step 1: Add Clipboard & Path Action in `crates/dscan-gui/src/app.rs`

1. Ensure `gpui::ClipboardItem` is imported in `crates/dscan-gui/src/app.rs`.
2. Add `copy_node_path` method to `DscanApp`:
```rust
pub fn copy_node_path(&mut self, node_id: u32, cx: &mut Context<Self>) {
    let path = self.state.node_full_path(node_id);
    let path_str = path.to_string_lossy().to_string();
    cx.write_to_clipboard(gpui::ClipboardItem::new_string(path_str));
    self.close_context_menu(cx);
}
```

### Step 2: Synchronize Selection in `open_context_menu`

Update `open_context_menu` in `crates/dscan-gui/src/app.rs`:
```rust
pub fn open_context_menu(
    &mut self,
    node_id: u32,
    pos_x: f32,
    pos_y: f32,
    cx: &mut Context<Self>,
) {
    self.state.selected_node_id = Some(node_id);
    self.context_menu = Some(ContextMenuState {
        node_id,
        pos_x,
        pos_y,
    });
    cx.notify();
}
```

### Step 3: Enforce Upfront Safety Validation in `request_trash_node`

Update `request_trash_node` in `crates/dscan-gui/src/app.rs`:
```rust
pub fn request_trash_node(&mut self, node_id: u32, cx: &mut Context<Self>) {
    self.context_menu = None;
    if let Some(node) = self.state.find_node(node_id) {
        let path = self.state.node_full_path(node_id);
        let error_msg = crate::system::is_safe_to_trash(&path, &self.state.target_path)
            .err()
            .map(|e| e.to_string());
        self.pending_trash = Some(PendingTrash {
            node_id,
            name: node.name.clone(),
            total_bytes: node.total_bytes,
            path,
            error_msg,
        });
        cx.notify();
    }
}
```

### Step 4: Wire Right-Click Selection & Dismissal in `crates/dscan-gui/src/views/cushion_treemap.rs`

In `render_cushion_treemap`:
```rust
.on_mouse_down(
    MouseButton::Right,
    cx.listener(|this, event: &MouseDownEvent, _window, cx| {
        let px_x: f32 = event.position.x.into();
        let px_y: f32 = event.position.y.into();
        if let Some(id) = this.state.hit_test(px_x, px_y) {
            this.open_context_menu(id, px_x, px_y, cx);
        } else {
            this.close_context_menu(cx);
        }
    }),
)
```

Also add "📋 Copy Path" button to the treemap floating hover card (`when_some(active_node, ...)` in `cushion_treemap.rs`) for complete UI consistency alongside existing "📂 Reveal" and "🗑 Trash" buttons.

### Step 5: Render 3-Action Context Menu with Safety Guards in `crates/dscan-gui/src/app.rs`

In `DscanApp::render`:
1. Calculate dimensions:
   - `menu_w = 230.0`
   - `menu_h = 132.0`
   - Clamping:
     ```rust
     let x = menu.pos_x.min(avail_w - menu_w - 8.0).max(8.0);
     let y = menu.pos_y.min(total_h - menu_h - 8.0).max(8.0);
     ```
2. Check safety of target path:
   ```rust
   let path = self.state.node_full_path(node_id);
   let is_safe = crate::system::is_safe_to_trash(&path, &self.state.target_path).is_ok();
   ```
3. Context menu items:
   - **Item 1: Open in File Manager** (`#ctx-reveal-btn`):
     - Icon: `📂`, Text: "Open in File Manager"
     - Click: `this.reveal_node(node_id, cx)`
   - **Item 2: Copy Path** (`#ctx-copy-btn`):
     - Icon: `📋`, Text: "Copy Path"
     - Click: `this.copy_node_path(node_id, cx)`
   - **Divider**:
     - `div().h(px(1.0)).bg(t.border).my(px(4.0)).mx(px(8.0))`
   - **Item 3: Delete to Trash** (`#ctx-trash-btn`):
     - Icon: `🗑`, Text: if `is_safe` { "Move to Trash" } else { "Move to Trash (Protected)" }
     - If `is_safe`: text color `t.accent_red`, hover `t.surface_hover`, click calls `this.request_trash_node(node_id, cx)`.
     - If not `is_safe`: text color `t.text_dim`, `cursor_not_allowed`, no click handler.

### Step 6: Confirmation Dialog Modal Guarding

In `trash-confirm-modal`:
1. If `err_opt.is_some()`:
   - Show red error banner explaining why action is blocked.
   - Omit or disable the red "Move to Trash" button, showing only "Cancel" / "Close".
2. Allow clicking backdrop (`#trash-confirm-modal-backdrop`) to call `this.cancel_trash(cx)`.

### Step 7: Update `Escape` Action Priority in `crates/dscan-gui/src/app.rs`

In root `on_action(CancelScan)` listener:
```rust
.on_action(cx.listener(|this, _: &CancelScan, _window, cx| {
    if this.pending_trash.is_some() {
        this.cancel_trash(cx);
    } else if this.context_menu.is_some() {
        this.close_context_menu(cx);
    } else if this.state.is_scanning {
        this.cancel_scan(cx);
    }
}))
```

---

## 5. Verification Commands

1. **Compilation Check**:
   ```bash
   cargo check -p dscan-gui
   ```
2. **Clippy & Formatting**:
   ```bash
   cargo clippy -p dscan-gui --all-targets -- -D warnings
   cargo fmt --check
   ```
3. **Automated Unit Tests**:
   ```bash
   cargo test -p dscan-gui
   cargo test -p dscan-core
   ```
4. **Manual Functional Scenarios**:
   - Run GUI: `cargo run -p dscan-gui`
   - Scan local directory or drive.
   - Right-click file node in cushion treemap:
     - Verify blue selection outline immediately wraps the right-clicked tile.
     - Verify menu appears at cursor and stays fully within window bounds.
     - Click "Copy Path": paste in terminal/editor; verify exact absolute path matches.
     - Click "Open in File Manager": verify system file manager opens and reveals parent directory.
     - Click "Move to Trash": verify confirmation modal appears with file name, path, and human-readable size.
     - In modal, click "Cancel": verify modal closes without modifying filesystem.
     - On a test file, confirm "Move to Trash": verify file moves to trash and treemap instantly redraws with reclaimed space.
     - Right-click scan root directory: verify "Move to Trash" option is disabled and marked protected.
     - Press `Escape`: verify context menu or modal closes cleanly.

---

## 6. STOP Conditions

Stop and report if:
1. `gpui::ClipboardItem` is unavailable or differs in signature in `gpui-pre-0.3.8`.
2. Treemap hit testing fails to resolve coordinates under high-DPI scaling.
3. System trash provider throws unexpected panic on read-only mount points.
