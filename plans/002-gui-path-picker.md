# Plan 002: GUI Folder Picker & Interactive Path Input

> **Executor instructions**: Follow this plan step by step. Run every verification command and confirm expected result before moving to next step. If anything in "STOP conditions" occurs, stop and report — do not improvise.

## Status
- **Priority**: P1
- **Effort**: M
- **Risk**: LOW (self-contained GUI component and event routing; no core scanner alterations)
- **Category**: feature / ui-ux
- **Planned at**: 2026-10-07
- **Target Crate**: `crates/dscan-gui`
- **Depends on**: Existing GPUI architecture (`crates/dscan-gui`)

---

## 1. Context & Motivation

`dscan-gui` is a GPU-accelerated desktop visualizer built with GPUI (`gpui-kit` / Zed's UI engine).
Currently, the scan target path can only be set at startup by defaulting to the first detected drive mount point (e.g. `/` or `C:\`). Users cannot change the target directory interactively at runtime without restarting the application or relying on CLI arguments.

### Target User Experience
1. **Native Folder Picker Dialog**: Click a "Browse" / "📂" button in the title bar (or press `Ctrl+O`) to launch a native OS folder selection modal.
2. **Interactive Path Text Input**: Click into a text field in the title bar, type or paste (`Ctrl+V`) any path (e.g. `~/Downloads`, `/var/log`, `D:\Projects`), and press `Enter` (or click "Scan") to start scanning immediately.
3. **Synchronized State**:
   - Picking a folder in the native dialog updates both the state and the text input field.
   - Clicking a logical drive chip updates the text input field and triggers a scan.
   - Launching `dscan-gui <path>` populates the target path and text input field on startup.
4. **Resilient Fallback**: If the desktop portal/modal fails or is absent on minimal Linux installations, the direct text input works unconditionally.

---

## 2. Technical Framework Analysis: GPUI vs. RFD

### Option A: Built-in GPUI `cx.prompt_for_paths` (Recommended & Adopted)
GPUI (`gpui-pre` / `gpui-kit`) already embeds native modal dialogs in its platform layer:
- **API**: `cx.prompt_for_paths(PathPromptOptions { files: false, directories: true, multiple: false, prompt: Some(...) })`
- **Return Type**: `oneshot::Receiver<Result<Option<Vec<PathBuf>>>>`
- **Platform Implementations**:
  - **Linux (X11 & Wayland)**: `gpui-pre-linux` calls `ashpd::desktop::file_chooser::OpenFileRequest` with `directory(true)` and `modal(true)` via XDG Desktop Portal.
  - **Windows**: `gpui-pre-windows` calls native Win32 `IFileOpenDialog` with `FOS_PICKFOLDERS`.
  - **macOS**: `gpui-pre-mac` calls `NSOpenPanel`.
- **Dependencies**: **Zero extra crates**. `ashpd` and `windows-sys` COM interfaces are already compiled and linked inside `gpui-kit`.

### Option B: `rfd` (Rusty File Dialog crate)
- `rfd = "0.17"` uses `xdg-desktop-portal` by default on Linux, and `IFileOpenDialog` on Windows.
- On Linux, `rfd` without its optional `gtk3` feature fails identically to GPUI if `xdg-desktop-portal` is missing. Enabling `rfd/gtk3` pulls in C library headers (`libgtk-3-dev`) and breaks pure-Rust compilation.
- Adding `rfd` duplicates code already present in `gpui-kit`.

### Decision
Use **GPUI's native `prompt_for_paths`** for modal folder picking. Pair it with **`gpui-kit::component::input::{Input, InputState}`** as the universal text input and resilient fallback.

---

## 3. Architecture & Data Flow

```
┌────────────────────────────────────────────────────────────────────────┐
│                          Title Bar Component                           │
├───────────────┬─────────────────────────────────┬──────────────────────┤
│  [📂 Browse]  │ [📁 /home/ahoura/projects    ⨉] │ [ / (45%) ] [ /home] │
└───────┬───────┴────────────────┬────────────────┴──────────┬───────────┘
        │                        │                           │
        ▼                        ▼                           ▼
[Click or Ctrl+O]        [Type/Paste + Enter]        [Click Drive Chip]
        │                        │                           │
        │                        ├─► resolve_user_path()     │
        │                        │   (tilde expansion)       │
        │                        ├─► validate_dir()          │
        │                        │                           │
        ▼                        │                           │
cx.prompt_for_paths()            │                           │
        │                        │                           │
 (Native OS Dialog)              │                           │
        │                        │                           │
        ▼                        │                           │
Selected PathBuf                 │                           │
        │                        │                           │
        └──────────────┬─────────┴───────────────────────────┘
                       │
                       ▼
         Update `AppState::target_path`
         Update `InputState` value
         Sync `selected_drive_idx`
         Auto-scale threads for new target
         `DscanApp::start_scan()`
```

---

## 4. Key Files and Changes

| File | Changes |
|------|---------|
| `crates/dscan-gui/src/main.rs` | Parse CLI argument `std::env::args().nth(1)`; pass `initial_path` into `DscanApp::new`; route `OpenPath` (`Ctrl+O`) to folder picker. |
| `crates/dscan-gui/src/system.rs` | Add `resolve_user_path(s: &str) -> PathBuf` (with tilde `~` expansion on Unix) and `validate_scan_directory(path: &Path) -> Result<PathBuf, String>`. |
| `crates/dscan-gui/src/state.rs` | Add `initial_path: Option<PathBuf>` to `AppState::new`; add `path_error: Option<String>`; add `sync_selected_drive_with_target()`. |
| `crates/dscan-gui/src/app.rs` | Store `path_input: Entity<InputState>`, `window_handle: AnyWindowHandle`, and subscriptions; implement `open_folder_picker()`, `submit_path()`, and updated `select_drive()`. |
| `crates/dscan-gui/src/views/title_bar.rs` | Replace static target text with `[📂 Browse]` button and interactive `Input` field; style with active theme. |
| `crates/dscan-gui/src/views/status_bar.rs` | Display `path_error` when present (e.g. "Directory not found"). |

---

## 5. Step-by-Step Implementation Details

### Step 1: Path Resolution & Validation Helpers in `system.rs`

Users often paste `~/folder`, paths with trailing slashes, or paths surrounded by quotes. Add robust path normalization:

```rust
// In crates/dscan-gui/src/system.rs

/// Resolve user-entered path strings: trim whitespace, strip enclosing quotes,
/// and expand leading `~` on Unix to the user's home directory.
pub fn resolve_user_path(input: &str) -> PathBuf {
    let mut s = input.trim();
    // Strip surrounding matching quotes if user copied from terminal
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        if s.len() >= 2 {
            s = &s[1..s.len() - 1];
        }
    }
    let s = s.trim();

    #[cfg(unix)]
    if s == "~" {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home);
        }
    } else if let Some(stripped) = s.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(stripped);
        }
    }

    PathBuf::from(s)
}

/// Validate that a path exists and is an accessible directory.
pub fn validate_scan_directory(path: &Path) -> Result<PathBuf, String> {
    if !path.exists() {
        return Err(format!("Directory does not exist: {}", path.display()));
    }
    if !path.is_dir() {
        return Err(format!("Path is not a directory: {}", path.display()));
    }
    // Canonicalize if possible, falling back to original path
    Ok(std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()))
}
```

Add unit tests in `system.rs`:
- Test tilde expansion.
- Test quoted string stripping.
- Test directory validation on existing temp dir vs non-existent path.

---

### Step 2: `AppState` Enhancements in `state.rs`

Update `AppState` to accept an optional initial path and manage error feedback:

```rust
// In crates/dscan-gui/src/state.rs

pub struct AppState {
    pub target_path: PathBuf,
    pub path_error: Option<String>,
    ...
}

impl AppState {
    pub fn new(initial_path: Option<PathBuf>) -> Self {
        let drives = detect_drives();
        let target_path = initial_path
            .and_then(|p| crate::system::validate_scan_directory(&p).ok())
            .or_else(|| drives.first().map(|d| d.mount_point.clone()))
            .unwrap_or_else(|| {
                if cfg!(windows) {
                    PathBuf::from("C:\\")
                } else {
                    PathBuf::from("/")
                }
            });

        let mut state = Self {
            target_path,
            path_error: None,
            drives,
            selected_drive_idx: 0,
            ...
        };
        state.sync_selected_drive_with_target();
        state
    }

    /// Match selected_drive_idx to target_path if it belongs to one of the detected drives
    pub fn sync_selected_drive_with_target(&mut self) {
        if let Some((idx, _)) = self.drives.iter().enumerate().find(|(_, d)| {
            self.target_path.starts_with(&d.mount_point)
        }) {
            self.selected_drive_idx = idx;
        }
    }

    pub fn set_path_error(&mut self, err: Option<String>) {
        self.path_error = err;
    }
}
```

---

### Step 3: `DscanApp` State & Event Routing in `app.rs`

Import GPUI's `Input` and `InputState` from `gpui-kit::component::input`:

```rust
// In crates/dscan-gui/src/app.rs
use gpui::{AnyWindowHandle, Subscription};
use gpui_kit::component::input::{Input, InputEvent, InputState};
```

Update `DscanApp` struct:
```rust
pub struct DscanApp {
    pub state: AppState,
    pub path_input: Entity<InputState>,
    pub window_handle: AnyWindowHandle,
    pub threads: usize,
    pub pacman_phase: usize,
    pub context_menu: Option<ContextMenuState>,
    pub pending_trash: Option<PendingTrash>,
    pub _subscriptions: Vec<Subscription>,
}
```

Update `DscanApp::new`:
```rust
impl DscanApp {
    pub fn new(initial_path: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let app_state = AppState::new(initial_path);
        let target_str = app_state.target_path.to_string_lossy().to_string();
        let threads = dscan_core::ScanOptions::auto_threads_for_path(&app_state.target_path);
        let window_handle = window.window_handle();

        let path_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Enter folder path...")
                .default_value(target_str)
        });

        let input_sub = cx.subscribe(&path_input, |this, input_state, event: &InputEvent, cx| {
            if let InputEvent::PressEnter { .. } = event {
                let text = input_state.read(cx).value().to_string();
                this.submit_path(&text, cx);
            }
        });

        Self {
            state: app_state,
            path_input,
            window_handle,
            threads,
            pacman_phase: 0,
            context_menu: None,
            pending_trash: None,
            _subscriptions: vec![input_sub],
        }
    }
```

Add path submission and drive selection methods in `DscanApp`:
```rust
    pub fn submit_path(&mut self, text: &str, cx: &mut Context<Self>) {
        let resolved = crate::system::resolve_user_path(text);
        match crate::system::validate_scan_directory(&resolved) {
            Ok(valid_dir) => {
                self.state.path_error = None;
                self.state.target_path = valid_dir.clone();
                self.state.sync_selected_drive_with_target();
                self.threads = dscan_core::ScanOptions::auto_threads_for_path(&valid_dir);
                self.start_scan(cx);
            }
            Err(err) => {
                self.state.set_path_error(Some(err));
                cx.notify();
            }
        }
    }

    pub fn select_drive(&mut self, idx: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.state.select_drive(idx);
        let target_str = self.state.target_path.to_string_lossy().to_string();
        self.path_input.update(cx, |input, cx| {
            input.set_value(target_str, window, cx);
        });
        self.state.path_error = None;
        self.threads = dscan_core::ScanOptions::auto_threads_for_path(&self.state.target_path);
        cx.notify();
    }
```

Implement `open_folder_picker`:
```rust
    pub fn open_folder_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Choose directory to scan".into()),
        });
        let window_handle = self.window_handle;

        cx.spawn(async move |this, mut cx| {
            match rx.await {
                Ok(Ok(Some(paths))) => {
                    if let Some(folder) = paths.into_iter().next() {
                        let _ = cx.update_window(window_handle, |_, window, cx| {
                            if let Some(this) = this.upgrade() {
                                this.update(cx, |this, cx| {
                                    let path_str = folder.to_string_lossy().to_string();
                                    this.path_input.update(cx, |input, cx| {
                                        input.set_value(path_str, window, cx);
                                    });
                                    this.state.path_error = None;
                                    this.state.target_path = folder.clone();
                                    this.state.sync_selected_drive_with_target();
                                    this.threads = dscan_core::ScanOptions::auto_threads_for_path(&folder);
                                    this.start_scan(cx);
                                });
                            }
                        });
                    }
                }
                Ok(Ok(None)) => {
                    // User canceled picker modal
                }
                Ok(Err(err)) => {
                    eprintln!("dscan-gui: folder picker failed: {err:?}");
                }
                Err(_) => {
                    // Channel dropped
                }
            }
        })
        .detach();
    }
```

In `render()`:
Wire action `OpenPath` to `this.open_folder_picker(window, cx);`.

---

### Step 4: Redesign Title Bar in `views/title_bar.rs`

Redesign the left side of the title bar to contain the branding, Browse button, and Input field:

```rust
// Left section in render_title_bar
div()
    .h_flex()
    .items_center()
    .gap(px(14.0))
    // 1. Branding
    .child(
        div()
            .h_flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .font_weight(FontWeight::BOLD)
                    .text_color(t.accent_green)
                    .text_size(px(18.0))
                    .child("⚡ dscan"),
            )
            .child(
                div()
                    .text_size(px(10.0))
                    .font_weight(FontWeight::MEDIUM)
                    .px(px(8.0))
                    .py(px(2.0))
                    .rounded_md()
                    .bg(t.surface_hover)
                    .text_color(t.text_dim)
                    .child(concat!("v", env!("CARGO_PKG_VERSION"))),
            ),
    )
    // 2. Browse Button
    .child(
        div()
            .id("btn-browse")
            .h_flex()
            .items_center()
            .gap(px(5.0))
            .px(px(10.0))
            .py(px(6.0))
            .rounded_lg()
            .bg(t.surface_hover)
            .hover(move |h| h.bg(t.border_light))
            .cursor_pointer()
            .text_size(px(12.0))
            .font_weight(FontWeight::MEDIUM)
            .text_color(t.text_primary)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.open_folder_picker(window, cx);
                }),
            )
            .child("📂 Browse"),
    )
    // 3. Path Input Field
    .child(
        div()
            .w(px(300.0))
            .h(px(32.0))
            .child(
                Input::new(&app.path_input)
                    .id("path-input")
                    .cleanable(true)
                    .text_size(px(12.0))
            ),
    )
```

In the drive chips:
Pass `window` to `this.select_drive(idx, window, cx)`.

In the scan button:
When clicking `▶ Scan`:
Read current text from `app.path_input`, validate and submit, starting the scan.

---

### Step 5: Status Bar Feedback in `views/status_bar.rs`

When `app.state.path_error` is `Some(ref err)`:
Display a prominent error badge in the status bar with `t.accent_red` and the error description so the user gets instant feedback if a path does not exist.

---

### Step 6: CLI Argument Support in `main.rs`

Update `main.rs`:
```rust
fn main() {
    let initial_path = std::env::args().nth(1).map(std::path::PathBuf::from);

    gpui_kit::application().run(move |cx: &mut App| {
        gpui_kit::init(cx);

        cx.bind_keys([
            KeyBinding::new("ctrl-o", OpenPath, None),
            KeyBinding::new("space", TogglePause, None),
            KeyBinding::new("escape", CancelScan, None),
        ]);

        let bounds = WindowBounds::centered(size(px(1440.0), px(920.0)), cx);

        let window_options = WindowOptions {
            window_bounds: Some(bounds),
            titlebar: Some(gpui::TitlebarOptions {
                title: Some("dscan".into()),
                appears_transparent: false,
                traffic_light_position: None,
            }),
            ..Default::default()
        };

        if let Err(e) = open_window(window_options, cx, |window, cx| {
            cx.new(|cx| DscanApp::new(initial_path.clone(), window, cx))
        }) {
            eprintln!("dscan-gui error: failed to initialize window: {e:?}");
            std::process::exit(1);
        }
    });
}
```

---

## 6. Edge Cases & Safety Invariants

1. **Non-Existent or Inaccessible Directory**:
   - `validate_scan_directory` rejects non-existent paths with a clear error string.
   - The scanner is never started with an invalid path.
2. **File Instead of Directory**:
   - If the user selects or types a regular file (e.g. `/path/to/file.txt`), `validate_scan_directory` detects `!path.is_dir()` and reports "Path is not a directory".
3. **Tilde Expansion on Unix**:
   - `~/...` is expanded using `$HOME`. On Windows, standard drive letters (`C:\`) and UNC paths (`\\server\share`) are preserved verbatim.
4. **Active Scan Cancellation**:
   - If a scan is already running when a new path is selected via dialog, input Enter, or drive chip, `this.start_scan()` cancels the in-flight scan and restarts cleanly on the new target.
5. **No Desktop Portal Daemon**:
   - If `ashpd` fails on minimal Linux (e.g. headless, i3 without portal), `open_folder_picker` logs the error without panicking, and the user can still type/paste directly into the text input.
6. **Thread Scaling on Drive Switch**:
   - Switching between an NVMe drive and an external USB HDD re-runs `auto_threads_for_path`, preventing I/O starvation or thrashing.

---

## 7. Verification Steps

### Automated Verification
```bash
# 1. Check compilation across workspace
cargo check --workspace

# 2. Check GUI specifically
cargo check -p dscan-gui

# 3. Unit tests for path expansion and normalization
cargo test -p dscan-gui

# 4. Clippy and formatting checks
cargo clippy -p dscan-gui --all-targets -- -D warnings
cargo fmt --check
```

### Manual Verification Scenarios
1. **Initial Launch**: Run `cargo run -p dscan-gui`. Observe title bar shows `[📂 Browse]` button and text input pre-filled with the primary drive path.
2. **CLI Path**: Run `cargo run -p dscan-gui -- src`. Verify input box displays the resolved `src` path and begins scanning `src`.
3. **Folder Picker**: Click "📂 Browse" (or press `Ctrl+O`). Choose a folder in the OS dialog. Confirm the path input updates and the scan restarts on the chosen directory.
4. **Manual Typing**: Click input field, clear text, type `~`, press `Enter`. Verify it expands to home directory and scans.
5. **Invalid Path**: Type `/nonexistent/path/xyz`, press `Enter`. Confirm status bar displays error and application does not crash.
6. **Drive Switching**: Click a drive chip in the title bar. Confirm input field updates to the drive mount point.
