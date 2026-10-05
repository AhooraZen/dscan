# Plan 018: dscan-gui: Ultra-Fast Native Desktop Disk Visualizer with Faithful WinDirStat Cushion Treemap Architecture via Tauri v2
> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat 03f38e1..HEAD -- Cargo.toml src/`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status
- **Priority**: P1
- **Effort**: L
- **Risk**: LOW
- **Depends on**: plans/013-extreme-performance-architecture.md, plans/014-cross-platform-windows-support.md, plans/016-god-speed-kernel-pipelining-and-zero-allocation.md
- **Category**: direction
- **Planned at**: commit `03f38e1`, 2026-10-05
- **Issue**: 

## Why this matters
`dscan` possesses the fastest directory traversal and disk space accounting engine on Linux and Windows, scanning up to 500,000+ files per second via raw kernel syscalls (`getdents64`, `statx`, `openat2`, `NtQueryDirectoryFile`) and zero-allocation lock-free arenas. However, disk usage analysis is fundamentally a visual task: human users struggle to identify rogue multi-gigabyte cache directories or stray disk-hogging video archives through text tables alone. Classic utilities like WinDirStat and SequoiaView solved this through the iconic 3-pane layout and Jarke J. van Wijk's quadratic cushion treemap, but WinDirStat is Windows-only, single-threaded, and takes minutes to scan modern terabyte NVMe drives.

This plan builds `dscan-gui`, an ultra-fast desktop GUI companion using Tauri v2 that pairs `dscan`'s zero-overhead engine with a faithful, high-fidelity WinDirStat visualization. By decoupling the codebase into a clean Cargo workspace (`dscan-core`, `dscan-cli`, and `dscan-gui`), the desktop visualizer achieves instant sub-second scans of entire SSDs while rendering 60fps WebGL/Canvas2D quadratic cushion treemaps. Crucially, Tauri IPC avoids flooding the webview with millions of raw events: it uses periodic atomic snapshots (100ms) pulling top active directories during scanning and virtualized hierarchical LOD rendering for display, guaranteeing zero DOM freezing even on filesystems containing over 1,000,000 files.

## Current state
`dscan` is currently structured as a monolithic single-crate repository in `Cargo.toml` and `src/`:
- `Cargo.toml:1-27` defines package `dscan` with `src/lib.rs` and `src/main.rs`.
- `src/lib.rs:1-13` exposes:
  ```rust
  pub mod arena;
  pub mod cli;
  pub mod format;
  pub mod scanner;
  pub mod simd;
  pub mod sys;
  pub mod ui;
  pub mod work_stealing;

  pub use cli::CliOptions;
  pub use format::format_bytes;
  pub use scanner::{ScanResult, print_report, run_scan};
  ```
- `src/arena.rs:95-103` implements `ArenaNode` and `DirArena`, a flat vector arena storing hierarchical tree nodes with pre-computed `total_bytes` rollups and string offsets:
  ```rust
  pub struct ArenaNode {
      pub parent_idx: u32,
      pub rel_depth: u16,
      pub flags: u16,
      pub direct_bytes: u64,
      pub total_bytes: u64,
      pub name_offset: u32,
      pub name_len: u16,
  }
  ```
- `src/scanner.rs:40-60` implements `GlobalState` managing work-stealing threads, atomic byte counters, and traversal completion signals.
- `src/ui.rs:1-250` provides ANSI terminal formatting (spinners, neon progress bars, terminal column widths).
- The repository currently has no GUI crate, no Tauri configuration, and no web frontend.

### Repo Conventions to Maintain
- **Zero-Regression Core Traversal**: `dscan-core` must retain 100% zero external dependencies (no `serde`, `clap`, `nix`, `rayon` in core). All raw kernel syscalls and memory alignment primitives must remain untouched.
- **Workspace Architecture**: `dscan-cli` continues to be a lean, fast-compiling terminal binary depending solely on `dscan-core`. `dscan-gui` (Tauri v2) is isolated in its own workspace crate.
- **Design Tokens & Accessibility**: Follow `ui-ux-pro-max` guidelines:
  * High-contrast Dark Mode (WCAG AA 4.5:1 minimum on all text).
  * Monospace data display: JetBrains Mono for numbers, file sizes, and paths; Inter for UI chrome.
  * No emojis as functional UI icons (use clean vector SVG icons).
  * Responsive 60fps interaction without UI stutter or garbage collection pauses.

## Commands you will need
| Purpose | Command | Expected on success |
|-----------|--------------------------|---------------------|
| Workspace Test | `cargo test --workspace` | exit 0, all tests pass |
| Workspace Check | `cargo check --workspace` | exit 0, no errors |
| Workspace Clippy | `cargo clippy --workspace --all-targets -- -D warnings` | exit 0, zero warnings |
| Format Check | `cargo fmt --check` | exit 0, clean formatting |
| Core Benchmark | `cargo test --test scanner_integration -- --nocapture` | exit 0, integration suites pass |
| Frontend Typecheck | `bun run typecheck` (or `pnpm typecheck` in GUI ui) | exit 0, zero TypeScript errors |
| Frontend Build | `bun run build` (or `pnpm build` in GUI ui) | exit 0, bundles static assets |

## Suggested executor toolkit
- **`ui-ux-pro-max`**: Design intelligence rules for desktop utilities, accessible palettes (`#0F172A` Slate Dark), layout splitters, and virtualized tree tables.
- **`rust-dev`**: Idiomatic Rust 2024, Cargo workspace layout, type-safe IPC serialization DTOs, and clippy compliance.
- **`typescript-dev`**: Strict TypeScript 5+ type models, zero `any`, canvas/WebGL rendering pipeline.

## Scope
**In scope** (files to create/modify):
- `Cargo.toml` (convert to Cargo workspace root)
- `crates/dscan-core/Cargo.toml` (create: zero-dependency engine)
- `crates/dscan-core/src/*` (move from existing `src/` modules: `arena.rs`, `format.rs`, `scanner.rs`, `simd.rs`, `sys/`, `work_stealing.rs`)
- `crates/dscan-core/src/snapshot.rs` (create: thread-safe lightweight snapshot and tree export API)
- `crates/dscan-cli/Cargo.toml` (create: CLI binary crate)
- `crates/dscan-cli/src/main.rs`, `crates/dscan-cli/src/cli.rs`, `crates/dscan-cli/src/ui.rs`
- `crates/dscan-gui/Cargo.toml`, `crates/dscan-gui/src-tauri/Cargo.toml`, `crates/dscan-gui/src-tauri/src/main.rs`
- `crates/dscan-gui/src-tauri/src/commands.rs`, `crates/dscan-gui/src-tauri/src/state.rs`
- `crates/dscan-gui/src-tauri/tauri.conf.json`, `crates/dscan-gui/src-tauri/capabilities/default.json`
- `crates/dscan-gui/ui/*` (HTML5, Tailwind CSS, TypeScript, Canvas2D/WebGL cushion renderer)
- `tests/*` (update test imports to use `dscan_core`)
- `plans/README.md` (record Plan 018 status)

**Out of scope** (do NOT touch):
- `src/sys/linux.rs` and `src/sys/uring.rs` raw kernel syscall implementations: do not alter syscall numbers or C struct binary layouts.
- `src/simd.rs` vector instruction kernels: do not degrade SIMD acceleration.
- The command-line interface behavior of `dscan-cli`: flags (`--depth`, `--top`, `--threads`, `--exclude`) must remain 100% backward compatible.

## Git workflow
- Branch: `master` or feature branch `feat/dscan-gui-tauri-windirstat`
- Commit per logical step; message style: Conventional Commits (`feat(workspace): ...`, `feat(gui): ...`).
- Do NOT push or publish to remote registries unless instructed.

---

## Architecture & Visual Specification

### 1. Workspace Topology
```
dscan/
├── Cargo.toml                      # Workspace root [workspace]
├── crates/
│   ├── dscan-core/                 # Zero-dependency kernel scanner library
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs              # Re-exports engine & snapshot types
│   │       ├── arena.rs            # Flat DirArena & rollups
│   │       ├── snapshot.rs         # Lightweight atomic snapshot DTO generator
│   │       ├── scanner.rs          # Work-stealing traversal coordinator
│   │       ├── work_stealing.rs    # Chase-Lev deque
│   │       ├── simd.rs             # SIMD path & nul matchers
│   │       ├── format.rs           # Binary byte formatter
│   │       └── sys/                # Raw Linux/Windows kernel syscalls
│   ├── dscan-cli/                  # Existing CLI utility
│   │   ├── Cargo.toml              # Depends on dscan-core
│   │   └── src/
│   │       ├── main.rs
│   │       ├── cli.rs              # Zero-dependency argument parser
│   │       └── ui.rs               # Neon ANSI terminal formatter
│   └── dscan-gui/                  # Tauri v2 Desktop Visualizer
│       ├── Cargo.toml
│       ├── src-tauri/
│       │   ├── Cargo.toml          # Depends on tauri 2, serde, dscan-core
│       │   ├── tauri.conf.json     # Window dimensions (1280x800 min 1024x640)
│       │   ├── capabilities/       # Tauri v2 security capabilities
│       │   └── src/
│       │       ├── main.rs         # Tauri application entry
│       │       ├── commands.rs     # IPC command handlers
│       │       └── state.rs        # AppState holding Arc<Mutex<Option<ScanSession>>>
│       └── ui/                     # Web frontend (Vite + TS + Tailwind)
│           ├── package.json
│           ├── tsconfig.json
│           ├── vite.config.ts
│           ├── index.html
│           └── src/
│               ├── main.ts
│               ├── state/          # Reactive scan & selection store
│               ├── components/
│               │   ├── TitleBar.ts # Custom window chrome & drive selector
│               │   ├── DirectoryTree.ts # Top-Left virtualized tree with Pacman
│               │   ├── ExtensionLegend.ts # Top-Right extension breakdown table
│               │   ├── CushionTreemap.ts # Bottom quadratic cushion canvas
│               │   └── StatusBar.ts # Bottom metrics & drive capacity bar
│               ├── treemap/
│               │   ├── squarify.ts # Squarified treemap layout engine
│               │   └── cushion.ts  # Van Wijk quadratic cushion illumination shader
│               └── styles/
│                   └── main.css    # Dark Theme tokens, CSS Grid
└── tests/                          # Integration tests targeting dscan-core
```

### 2. UI/UX Pro Max Visual Hierarchy & Design Tokens

#### 3-Pane Faithful WinDirStat Desktop Layout
```
+----------------------------------------------------------------------------------------------------+
|  [>] dscan-gui   | Root: /home/ahoura   [Scan Drive...] [Pause] [Cancel]           [_] [^] [X]     |
+-----------------------------------------------------------------+----------------------------------+
|  Top-Left: Directory Tree View (55% width)                      |  Top-Right: File Extension List  |
|  Name               | % Subtree | Size      | Files  | Pacman   |  Ext    | Color | % Disk | Size    |
|  > home/            | █████████ |  412.4 GB | 842.1k |          |  .mp4   | [red] |  38.2% | 157.5GB |
|    > ahoura/        | ████████  |  398.1 GB | 812.0k | <( - - ) |  .iso   | [blu] |  21.4% |  88.2GB |
|      > .cache/      | ████      |   84.2 GB | 145.2k |          |  .tar   | [grn] |  12.1% |  49.9GB |
|      > Projects/    | ███       |   62.7 GB | 280.4k |          |  .rs    | [org] |   4.8% |  19.8GB |
|      > Downloads/   | ██        |   41.0 GB |   1.2k |          |  [..]   | [gry] |  23.5% |  97.0GB |
+-----------------------------------------------------------------+----------------------------------+
|  <--------------------------------- Resizable Splitter ----------------------------------------->  |
+----------------------------------------------------------------------------------------------------+
|  Bottom: High-Performance Interactive Cushion Treemap (100% width, flexible height)               |
|  +-------------------------------------------------------------+--------------------------------+  |
|  | [ home/ahoura/Videos/movie.mp4 ]                            | [.cache/cargo/registry]        |  |
|  | (Smooth quadratic illuminated 3D cushion blocks)            |                                |  |
|  | Nested bounding cushions reveal directory hierarchy         |                                |  |
|  |                                                             |                                |  |
|  +-------------------------------------------------------------+--------------------------------+  |
+----------------------------------------------------------------------------------------------------+
|  Status Bar: Scanning /home/ahoura/.cache/yarn | 482,104 files/s | Elapsed: 00:01.4 | [██████░░ 78%]|
+----------------------------------------------------------------------------------------------------+
```

#### Color Tokens (Accessible Slate Dark - WCAG AA Compliant)
- **Background**: `#0F172A` (Slate 900)
- **Panel Surfaces**: `#1E293B` (Slate 800)
- **Card / Row Hover**: `#334155` (Slate 700)
- **Selection Highlight**: `#1D4ED8` (Blue 700) with `#93C5FD` (Blue 300) focus ring
- **Borders & Dividers**: `#334155` (1px solid border between panes)
- **Primary Text**: `#F8FAFC` (Slate 50, contrast 13.5:1 against `#0F172A`)
- **Secondary / Muted Text**: `#94A3B8` (Slate 400, contrast 5.4:1 against `#0F172A`)
- **Metric Highlights**: `#38BDF8` (Sky 400), `#4ADE80` (Green 400), `#FACC15` (Yellow 400)
- **Extension Palette**: 16 perceptually distinct categorical colors generated in OKLCH color space for maximum contrast and zero adjacent clash.

#### Typography
- **UI Chrome & Labels**: `Inter, system-ui, -apple-system, sans-serif` (13px, weight 400/500/600).
- **Numbers, Byte Sizes, Paths, Metrics**: `JetBrains Mono, SF Mono, Menlo, monospace` (12px, tabular figures `font-variant-numeric: tabular-nums`).

---

## Steps

### Step 1: Cargo Workspace Restructuring
Decouple the monolithic repository into three workspace crates without changing any scanning or syscall logic.

1. Create directory structure:
   ```bash
   mkdir -p crates/dscan-core/src crates/dscan-cli/src crates/dscan-gui/src-tauri/src
   ```
2. Move core modules from `src/` to `crates/dscan-core/src/`:
   - `src/arena.rs` -> `crates/dscan-core/src/arena.rs`
   - `src/format.rs` -> `crates/dscan-core/src/format.rs`
   - `src/scanner.rs` -> `crates/dscan-core/src/scanner.rs`
   - `src/simd.rs` -> `crates/dscan-core/src/simd.rs`
   - `src/sys/` -> `crates/dscan-core/src/sys/`
   - `src/work_stealing.rs` -> `crates/dscan-core/src/work_stealing.rs`
3. Create `crates/dscan-core/src/lib.rs` exporting the core primitives:
   ```rust
   pub mod arena;
   pub mod format;
   pub mod scanner;
   pub mod simd;
   pub mod sys;
   pub mod work_stealing;

   pub use arena::{ArenaNode, DirArena, LocalTopFiles, TopFileCandidate};
   pub use format::format_bytes;
   pub use scanner::{ScanConfig, ScanResult, run_scan};
   ```
4. Move CLI-specific code from `src/` to `crates/dscan-cli/src/`:
   - `src/cli.rs` -> `crates/dscan-cli/src/cli.rs`
   - `src/ui.rs` -> `crates/dscan-cli/src/ui.rs`
   - `src/main.rs` -> `crates/dscan-cli/src/main.rs`
5. Configure `crates/dscan-core/Cargo.toml`:
   ```toml
   [package]
   name = "dscan-core"
   version = "0.2.0"
   edition = "2024"
   authors = ["AhooraZen <ahoora935137@gmail.com>"]
   description = "High-throughput kernel directory traversal and disk space accounting engine"
   license = "MIT OR Apache-2.0"

   [lib]
   name = "dscan_core"
   path = "src/lib.rs"
   ```
6. Configure `crates/dscan-cli/Cargo.toml`:
   ```toml
   [package]
   name = "dscan-cli"
   version = "0.2.0"
   edition = "2024"
   authors = ["AhooraZen <ahoora935137@gmail.com>"]
   description = "Fast, zero-dependency multi-threaded disk usage analyzer CLI"
   license = "MIT OR Apache-2.0"

   [[bin]]
   name = "dscan"
   path = "src/main.rs"

   [dependencies]
   dscan-core = { path = "../dscan-core" }
   ```
7. Replace root `Cargo.toml` with:
   ```toml
   [workspace]
   resolver = "3"
   members = [
       "crates/dscan-core",
       "crates/dscan-cli",
       "crates/dscan-gui/src-tauri",
   ]

   [profile.release]
   opt-level = 3
   lto = "fat"
   codegen-units = 1
   panic = "abort"
   strip = true
   ```
8. Update `tests/*.rs` test harnesses to reference `dscan_core::*`.

**Verify**:
```bash
cargo test -p dscan-core && cargo check --workspace
```
→ Exit code 0, all 31 core unit tests and integration tests pass cleanly under the new workspace layout.

---

### Step 2: Headless Scanning Engine & Snapshot Export API in `dscan-core`
The GUI cannot wait for a scan to finish before showing data, nor can it process millions of unbuffered events across Tauri IPC. `dscan-core` must expose a thread-safe, non-blocking snapshot API.

1. Implement `crates/dscan-core/src/snapshot.rs`:
   - `ScanStatusSnapshot`:
     ```rust
     #[derive(Debug, Clone)]
     pub struct ScanProgressDto {
         pub total_bytes: u64,
         pub total_files: u64,
         pub active_workers: usize,
         pub current_path: String,
         pub elapsed_millis: u64,
         pub files_per_sec: f64,
         pub bytes_per_sec: f64,
         pub is_complete: bool,
     }
     ```
   - `TreemapNodeDto`:
     ```rust
     #[derive(Debug, Clone)]
     pub struct TreemapNodeDto {
         pub id: u32,
         pub parent_id: u32,
         pub name: String,
         pub total_bytes: u64,
         pub direct_bytes: u64,
         pub rel_depth: u16,
         pub is_dir: bool,
         pub extension: String,
         pub children_ids: Vec<u32>,
     }
     ```
   - `ExtensionStatDto`:
     ```rust
     #[derive(Debug, Clone)]
     pub struct ExtensionStatDto {
         pub extension: String,
         pub total_bytes: u64,
         pub file_count: u64,
         pub percentage_of_total: f64,
     }
     ```
2. Implement `ScanSession` in `dscan_core::scanner`:
   - Wraps `GlobalState` inside an `Arc`.
   - Adds an `AtomicBool` for `cancel_requested` and `pause_requested`.
   - Exposes `pub fn poll_progress(&self) -> ScanProgressDto`.
   - Exposes `pub fn get_hierarchical_view(&self, max_depth: u16, max_nodes: usize) -> Vec<TreemapNodeDto>`:
     Traverses `DirArena` in memory and returns a bounded, pre-aggregated tree DTO (default max 5,000 nodes) for instant JSON serialization without webview lag.
   - Exposes `pub fn get_extension_breakdown(&self, limit: usize) -> Vec<ExtensionStatDto>`:
     Aggregates extensions across scanned files, sorted descending by size.

**Verify**:
```bash
cargo test -p dscan-core --lib snapshot
```
→ Exit code 0, snapshot unit test confirms `poll_progress()` and `get_hierarchical_view()` run contention-free while worker threads scan synthetic trees.

---

### Step 3: Tauri v2 Native Host Setup in `crates/dscan-gui`
Set up the official Tauri v2 backend with custom commands, capability declarations, and state management.

1. Configure `crates/dscan-gui/src-tauri/Cargo.toml`:
   ```toml
   [package]
   name = "dscan-gui"
   version = "0.2.0"
   edition = "2024"

   [lib]
   name = "dscan_gui_lib"
   crate-type = ["staticlib", "cdylib", "rlib"]

   [build-dependencies]
   tauri-build = { version = "2", features = [] }

   [dependencies]
   dscan-core = { path = "../../dscan-core" }
   tauri = { version = "2", features = [] }
   serde = { version = "1.0", features = ["derive"] }
   serde_json = "1.0"
   parking_lot = "0.12"
   ```
2. Create `crates/dscan-gui/src-tauri/tauri.conf.json`:
   - App identifier: `com.parchlinux.dscan`
   - Window: title `"dscan - Disk Space Analyzer"`, width `1280`, height `800`, minWidth `1024`, minHeight `640`.
   - Frontend dist directory: `../ui/dist`.
3. Create `crates/dscan-gui/src-tauri/capabilities/default.json` declaring Tauri v2 window, dialog, and process permissions.
4. Implement `crates/dscan-gui/src-tauri/src/state.rs`:
   - Holds `Arc<RwLock<Option<ActiveScanSession>>>`.
5. Implement `crates/dscan-gui/src-tauri/src/commands.rs`:
   - `start_scan(path: String, threads: Option<usize>) -> Result<(), String>`
   - `poll_progress() -> Result<ScanProgressDto, String>`
   - `get_treemap_data(node_id: Option<u32>, depth: Option<u16>) -> Result<Vec<TreemapNodeDto>, String>`
   - `get_extension_legend() -> Result<Vec<ExtensionStatDto>, String>`
   - `open_in_file_manager(path: String) -> Result<(), String>`
   - `cancel_scan() -> Result<(), String>`
6. Implement `crates/dscan-gui/src-tauri/src/main.rs` wiring Tauri commands and state into `tauri::Builder`.

**Verify**:
```bash
cargo check -p dscan-gui
```
→ Exit code 0, Tauri v2 backend compiles and passes clippy with zero warnings.

---

### Step 4: Frontend Scaffolding, Theme Tokens & Resizable Layout
Initialize the lightweight, zero-bloat webview frontend using Vite + TypeScript + Tailwind CSS.

1. Initialize `crates/dscan-gui/ui/`:
   - Setup `package.json` with `@tauri-apps/api: ^2.0`, `tailwindcss: ^3.4`, `lucide: ^0.400`, `typescript: ^5.4`, `vite: ^5.2`.
   - Setup `vite.config.ts` configured for Tauri (clearScreen: false, server port 1420).
   - Setup `tailwind.config.js` with Slate dark palette tokens:
     * Surface: `#0F172A`, `#1E293B`, `#334155`
     * Accent: `#38BDF8`, `#3B82F6`
     * Monospace: `JetBrains Mono, monospace`
2. Implement 3-Pane Resizable Layout (`index.html` and `src/components/Layout.ts`):
   - Flex column container matching 100vh / 100vw.
   - Top Header Bar: Root drive input, "Scan" button, "Pause/Resume" button, "Cancel" button, drive selector dropdown.
   - Middle Split Area:
     * Left Pane (55% default width): Directory Tree View container.
     * Horizontal Splitter Bar (draggable, 4px wide with 8px hover hit area).
     * Right Pane (45% default width): File Extension Legend container.
   - Vertical Splitter Bar (draggable, 4px high).
   - Bottom Pane (45% default height): Cushion Treemap Canvas container.
   - Bottom Status Bar (28px height, fixed): Scan rate, current file path, drive capacity bar.
3. State Management (`src/state/scanStore.ts`):
   - Reactive store tracking:
     * `isScanning: boolean`
     * `progress: ScanProgressDto`
     * `treeNodes: Map<number, TreemapNodeDto>`
     * `selectedNodeId: number | null`
     * `hoveredNodeId: number | null`
     * `extensionStats: ExtensionStatDto[]`
     * `colorMap: Map<string, string>` (extension -> hex color)

**Verify**:
```bash
cd crates/dscan-gui/ui && bun run build
```
→ Exit code 0, `dist/index.html` and bundled assets generated cleanly.

---

### Step 5: Directory Tree View with Classic Pacman Scanning Animation
Implement the Top-Left directory tree view with high information density, virtualized scrolling, and WinDirStat's iconic Pacman animation.

1. Implement `src/components/DirectoryTree.ts`:
   - Virtualized Row Renderer: Only renders visible DOM rows ($\approx 30-40$ rows based on container height), completely eliminating DOM churn.
   - Columns:
     * **Name**: Folder icon (SVG), folder name, caret for expand/collapse.
     * **Percentage**: Visual progress bar (`#38BDF8`) showing ratio relative to parent directory size.
     * **Size**: Formatted binary byte string (e.g. `142.5 GiB`) in tabular JetBrains Mono.
     * **Items**: Total file count formatted with commas (e.g. `842,109`).
     * **Pacman Indicator**: Dedicated column displaying the scanning animation when a directory is actively being read by worker threads.
2. Implement Canvas/SVG Pacman Animation (`src/components/Pacman.ts`):
   - Recreate classic WinDirStat Pacman:
     * Yellow circular wedge (`#FACC15`) with eye.
     * Mouth opens and closes at 8-12Hz between $0^\circ$ and $45^\circ$ using `requestAnimationFrame`.
     * Food dots (`#94A3B8`) move from right to left into Pacman's mouth as files are read.
     * Rendered on a compact $24 \times 16$ px canvas per active scanning row.
3. Keyboard & Mouse Interaction:
   - Click to select node: broadcasts `selectedNodeId` to Treemap and Breadcrumb.
   - Arrow keys: `ArrowUp`/`ArrowDown` navigates selection; `ArrowRight` expands; `ArrowLeft` collapses.
   - Double-click: drills down into folder (makes it new visual root).
   - Right-click context menu: "Open in File Manager", "Copy Path", "Delete to Trash".

**Verify**:
```bash
bun test src/components/DirectoryTree.test.ts
```
→ Exit code 0, virtualization tests verify that scrolling through 100,000 tree nodes renders exactly 35 DOM elements and retains 60fps frame budget (<16ms).

---

### Step 6: File Extension Breakdown Legend
Implement the Top-Right file extension summary table, giving users immediate insight into what file types consume disk space.

1. Implement `src/components/ExtensionLegend.ts`:
   - Columns:
     * **Extension**: Extension name (e.g. `.mp4`, `.tar.gz`, `.iso`, `[no ext]`).
     * **Color**: Solid color swatch matching the exact cushion block color in the Treemap below.
     * **Bytes**: Total space occupied, formatted in JetBrains Mono.
     * **% Disk**: Percentage of total scanned space with mini bar chart.
     * **Files**: Total count of files with this extension.
2. High-Contrast Categorical Palette Generator (`src/styles/palette.ts`):
   - Computes 16 perceptually uniform, maximally distinct colors using OKLCH color space:
     * Top 1: Coral Red (`oklch(0.65 0.22 25)`)
     * Top 2: Sky Cyan (`oklch(0.70 0.18 210)`)
     * Top 3: Vivid Emerald (`oklch(0.68 0.20 145)`)
     * Top 4: Golden Amber (`oklch(0.75 0.19 80)`)
     * Top 5: Electric Purple (`oklch(0.65 0.23 300)`)
     * Top 6-15: Evenly distributed hue angles with $L=0.68, C=0.18$.
     * Overflow (>16): Muted Slate (`#64748B`).
3. Interactivity:
   - Hovering an extension row highlights all corresponding cushion tiles in the Treemap with a bright pulsating border.
   - Clicking an extension filters the Treemap to dim all other file types, allowing instant visual isolation of huge video files or archives.

**Verify**:
```bash
bun test src/styles/palette.test.ts
```
→ Exit code 0, tests verify all generated colors achieve at least WCAG 3:1 non-text contrast against panel background `#1E293B` and no two top-10 colors have Delta-E < 15.

---

### Step 7: Quadratic Cushion Treemap Engine (van Wijk Illumination Model)
Implement the centerpiece of WinDirStat: the quadratic cushion treemap on HTML5 Canvas and WebGL 2D.

1. Implement Squarified Treemap Layout (`src/treemap/squarify.ts`):
   - Implements Bruls, Huizing, van Wijk (2000) squarifying algorithm:
     * Recursively partitions a bounding rectangle $R = [x_1, y_1, w, h]$ into rows/columns such that node aspect ratios $\max(w/h, h/w)$ approach 1.0 (avoiding long, thin unreadable needles).
     * Bounded recursion depth: aggregates subtrees smaller than 2px into parent cushion tiles, preventing sub-pixel rasterization waste.
2. Implement Cushion Treemap Mathematical Illumination Model (`src/treemap/cushion.ts`):
   - For each rectangle $R = [x_1, x_2] \times [y_1, y_2]$ at tree depth $k$:
     * Additive quadratic surface:
       $$z_k(x, y) = -4 h_k \left[ \frac{(x - x_1)(x - x_2)}{(x_2 - x_1)^2} + \frac{(y - y_1)(y - y_2)}{(y_2 - y_1)^2} \right]$$
     * Ridge height decay per level: $h_k = h_0 \cdot f^k$, where $h_0 = 0.5$ and $f = 0.65$.
     * Partial derivatives:
       $$\frac{\partial z_k}{\partial x} = -4 h_k \frac{2x - (x_1 + x_2)}{(x_2 - x_1)^2}, \quad \frac{\partial z_k}{\partial y} = -4 h_k \frac{2y - (y_1 + y_2)}{(y_2 - y_1)^2}$$
     * Accumulate total normal components across all ancestors:
       $$n_x(x) = \sum_{k} \frac{\partial z_k}{\partial x}, \quad n_y(y) = \sum_{k} \frac{\partial z_k}{\partial y}$$
     * Surface normal vector:
       $$\mathbf{N} = \frac{(-n_x, -n_y, 1)}{\sqrt{n_x^2 + n_y^2 + 1}}$$
     * Directional light vector pointing from top-left:
       $$\mathbf{L} = \frac{(-0.5, -0.5, 1.0)}{\sqrt{0.5^2 + 0.5^2 + 1.0^2}} = (-0.408, -0.408, 0.816)$$
     * Diffuse illumination with ambient component $I_a = 0.25, I_d = 0.75$:
       $$I = I_a + I_d \max(0, \mathbf{N} \cdot \mathbf{L})$$
     * Pixel Color:
       $$\mathbf{C}_{final} = \mathbf{C}_{ext} \cdot I$$
3. Renderer Implementations:
   - **Canvas2D Fallback**: Uses `Uint32Array` on `ctx.createImageData()` with pre-computed 1D scanline gradients for fast CPU rendering.
   - **WebGL 2D Shader Engine**: Renders rectangles into a lightweight vertex buffer and executes the cushion shading in a GLSL fragment shader, achieving 60-144fps at native 4K display resolutions!
4. Treemap Interaction:
   - **Hover**: High-precision quadtree / spatial index resolves mouse $(x, y)$ to leaf node in $O(\log N)$ time.
   - **Tooltip**: Renders floating dark card displaying:
     * Full file path
     * File size (bytes + formatted GiB)
     * % of parent directory
     * % of entire drive
     * Extension
   - **Left Click**: Selects file and scrolls Directory Tree to reveal and highlight corresponding row.
   - **Double Click**: Zooms Treemap into selected folder as new visual root (with animated breadcrumb navigation to zoom back out).

**Verify**:
```bash
bun test src/treemap/cushion.test.ts
```
→ Exit code 0, mathematical tests verify normal vector at cushion center is $(0, 0, 1)$, illumination at center matches $I_a + I_d \cdot L_z$, and boundary values transition smoothly without NaN/overflow.

---

### Step 8: Bottom Status Bar & Drive Usage Metrics
Implement the bottom status bar displaying scan throughput and system disk metrics.

1. Implement `src/components/StatusBar.ts`:
   - Left section: Scanning state pill:
     * "Scanning" (pulsing blue dot `#38BDF8`)
     * "Ready" (solid green dot `#4ADE80`)
     * "Paused" (yellow dot `#FACC15`)
   - Path section: Current scanning path, using smart tail truncation with ellipsis (`.../user/Downloads/archive.iso`).
   - Center metrics section:
     * Scanning rate: `files_per_sec` (e.g. `412,800 files/s`) and `bytes_per_sec` (e.g. `1.8 GB/s`).
     * Elapsed time: `mm:ss.s` stopwatch timer.
     * Active worker threads count: `32 threads`.
   - Right section: Drive Capacity Gauge:
     * Compact horizontal stacked progress bar:
       - Scanned Area (`#38BDF8`)
       - Other Used Space (`#64748B`)
       - Free Space (`#1E293B`)
     * Text: `412 GB / 953 GB (43% used)`.

**Verify**:
```bash
bun test src/components/StatusBar.test.ts
```
→ Exit code 0, formatting and truncation tests pass for ASCII, UTF-8, and CJK path strings.

---

### Step 9: Zero-DOM-Freeze IPC Integration & Throttling
Connect Tauri backend commands with frontend stores, guaranteeing fluid 60fps interaction during intense 500,000 files/s scans.

1. IPC Polling Loop (`src/state/scanController.ts`):
   - Rather than backend pushing an event for every file or directory, the frontend runs a `requestAnimationFrame`-aligned 100ms polling timer calling `invoke("poll_progress")`.
   - If scan is actively progressing:
     * Updates status bar metrics.
     * Fetches top active directories to animate Pacman in the tree view.
     * Defers full Treemap re-layout until scan completes or user clicks "Pause".
   - When scan completes:
     * Fetches hierarchical view (`get_treemap_data(max_depth=5, max_nodes=5000)`).
     * Builds squarified cushion treemap.
     * Fetches extension breakdown table.
2. Cancellation & Memory Cleanliness:
   - "Cancel" button triggers `invoke("cancel_scan")`, which sets atomic flag on worker threads; workers drain local queues and exit within 5ms.
   - Memory footprint stays under 120MB RSS even when indexing 1,000,000 files.

**Verify**:
```bash
cargo test -p dscan-gui --test ipc_stress_tests
```
→ Exit code 0, synthetic scan test verifies 100 consecutive `poll_progress()` calls return within <1ms latency without blocking worker traversal threads.

---

## Test plan
1. **Core Workspace Integration**:
   - `cargo test --workspace` validates that moving modules into `dscan-core` and `dscan-cli` caused zero functional regressions.
2. **Snapshot & DTO Serialization**:
   - Unit test `test_snapshot_dto_serialization` verifies JSON serialization of `TreemapNodeDto` and `ExtensionStatDto` produces exact numeric values matching `DirArena` rollups.
3. **Cushion Shading Surface Validation**:
   - Unit test `test_cushion_normals_boundary`:
     Asserts $\mathbf{N} \cdot [0, 0, 1] > 0$ for all $(x, y) \in R$, ensuring all cushion normal vectors point toward the viewer.
4. **Squarify Aspect Ratio Invariant**:
   - Unit test `test_squarified_aspect_ratios`:
     Ensures no rectangle in the generated treemap has an aspect ratio $> 4.0$ when inputs are distributed randomly.
5. **High-Load Performance Benchmark**:
   - Run a scan on a 500,000-file directory tree:
     * Confirm webview UI maintains 60fps.
     * Confirm memory usage does not exceed 150MB RSS.
     * Confirm Pacman animation runs smoothly during scanning.

## Done criteria
Machine-checkable. ALL must hold:
- [ ] `cargo check --workspace` exits 0 with zero warnings.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` exits 0.
- [ ] `cargo test --workspace` exits 0; all existing and new tests pass.
- [ ] `cd crates/dscan-gui/ui && bun run typecheck` exits 0 with zero TypeScript errors.
- [ ] `cd crates/dscan-gui/ui && bun run build` exits 0, producing static webview assets.
- [ ] `dscan-cli` binary builds and executes identically to previous version (`cargo run -p dscan-cli -- --help`).
- [ ] `plans/README.md` status row updated with Plan 018 entry.

## STOP conditions
Stop and report back (do not improvise) if:
- Code at `src/` has uncommitted changes that drift from `03f38e1`.
- A platform-specific Linux syscall in `crates/dscan-core/src/sys/` fails compilation on non-Linux environments without proper `#[cfg(...)]` gating.
- Tauri v2 IPC transfer latency for 5,000 treemap nodes exceeds 25ms on localhost.
- The cushion shading formula causes negative light values or color clipping in WebGL fragment shaders.

## Maintenance notes
- **Desktop Packaging**: Building production `.deb`, `.rpm`, `.AppImage`, and `.exe` bundles is supported natively by Tauri v2 via `cargo tauri build`.
- **Theme Extensibility**: The Slate Dark theme is token-driven in `tailwind.config.js` and `palette.ts`; future PRs can add Light or High-Contrast OLED themes without touching rendering logic.
- **Remote Filesystem Scanning**: If scanning network mounts (NFS/SMB), `dscan-core`'s filesystem boundary guard (`dev == root_dev`) will prevent crossing mount points unless explicitly passed `--cross-device`.
