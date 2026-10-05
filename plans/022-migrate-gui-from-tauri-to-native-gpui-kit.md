# Plan 022: Migrate dscan-gui from Tauri to Native Pure-Rust GPUI-Kit Architecture
> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat 9a8fb99..HEAD -- Cargo.toml crates/dscan-gui/ crates/dscan-core/`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status
- **Priority**: P1
- **Effort**: L
- **Risk**: MED
- **Depends on**: plans/019-fix-workspace-build-and-test-performance-baseline.md, plans/021-fix-core-scanner-chase-lev-concurrency-and-path-bugs.md
- **Category**: direction
- **Planned at**: commit `9a8fb99`, 2026-10-06
- **Issue**: 

## Why this matters
`dscan` possesses an ultra-fast kernel directory traversal and disk accounting engine, scanning 500,000+ files per second on modern NVMe drives via direct kernel syscalls (`getdents64`, `statx`, `openat2`, `NtQueryDirectoryFile`). However, the existing Tauri v2 implementation of `dscan-gui` fundamentally undermines this performance advantage: it pulls 808 cargo dependencies (including WebKit2GTK, GTK3, and libsoup3 on Linux), produces 1.1GB unstripped build footprints, triggers Windows Defender heuristic flags (due to embedded webview script execution), and introduces an unacceptable rendering bottleneck where 750,000 canvas pixels are recalculated sequentially on the CPU via `ctx.putImageData` on every single mouse hover. Furthermore, passing directory trees across Tauri's JSON IPC bridges causes severe memory spikes and serialization delays.

This plan completely migrates `dscan-gui` to a 100% pure-Rust architecture powered by `gpui-kit = "0.7"` (built on Zed Industries' high-performance GPUI framework). By eliminating the webview, Vite, and Node.js entirely, `dscan-gui` becomes a single compact native binary (<25MB release) that launches in under 50ms with zero Windows Defender false positives. Rendering is offloaded directly to the GPU via Blade (DirectX 11/12 on Windows, Vulkan/Wayland on Linux, Metal on macOS), enabling buttery-smooth 120 FPS cushion treemap illumination computed entirely in GPU fragment shaders. In-process atomic bindings to `dscan-core::ScanSession` eliminate JSON serialization completely, achieving instant real-time visualization of scans with zero overhead.

## Current state
`dscan` is currently configured as a Cargo workspace with a nested Tauri v2 GUI crate:
- `Cargo.toml:1-14` defines the workspace members:
  ```toml
  [workspace]
  resolver = "3"
  members = [
      "crates/dscan-core",
      "crates/dscan-cli",
      "crates/dscan-gui/src-tauri",
  ]
  ```
- `crates/dscan-gui/src-tauri/Cargo.toml:1-25` pulls Tauri v2, Serde, and Tauri build scripts:
  ```toml
  [package]
  name = "dscan-gui"
  version = "0.3.1"
  edition = "2024"
  description = "Tauri v2 WinDirStat cushion treemap visualizer for dscan"

  [dependencies]
  dscan-core = { path = "../../dscan-core" }
  tauri = { version = "2", features = [] }
  serde = { version = "1.0", features = ["derive"] }
  serde_json = "1.0"
  parking_lot = "0.12"
  ```
- `crates/dscan-gui/src-tauri/src/commands.rs:18-50` bridges scanning commands over JSON IPC:
  ```rust
  #[tauri::command]
  pub fn start_scan(
      path: String,
      threads: Option<usize>,
      state: State<'_, AppState>,
  ) -> Result<(), String> {
      ...
      let session = ScanSession::start(opts).map_err(|e| format!("Failed to start scan: {e}"))?;
      *state.session.write() = Some(session);
      Ok(())
  }
  ```
- `crates/dscan-gui/ui/package.json:1-25` pulls Vite, TypeScript, and Tailwind CSS.
- `crates/dscan-gui/ui/src/treemap/cushion.ts:98-142` demonstrates the CPU-bound pixel rendering bottleneck:
  ```ts
  let ptr = 0;
  for (let y = 0; y < rh; y++) {
    const ny = nyArr[y];
    for (let x = 0; x < rw; x++) {
      const nx = nxArr[x];
      const len = Math.sqrt(nx * nx + ny * ny + 1.0);
      const normX = -nx / len;
      const normY = -ny / len;
      const normZ = 1.0 / len;

      const dot = normX * LX + normY * LY + normZ * LZ;
      const intensity = IA + ID * Math.max(0, dot);

      const r = Math.min(255, Math.max(0, Math.round(baseRgb.r * intensity)));
      const g = Math.min(255, Math.max(0, Math.round(baseRgb.g * intensity)));
      const b = Math.min(255, Math.max(0, Math.round(baseRgb.b * intensity)));

      data32[ptr++] = (255 << 24) | (b << 16) | (g << 8) | r;
    }
  }
  ctx.putImageData(imgData, rx, ry);
  ```
- `crates/dscan-core/src/snapshot.rs:1-80` exposes `ScanSession`, `ScanProgressDto`, `TreemapNodeDto`, and `ExtensionStatDto`.
- `crates/dscan-core/src/scanner.rs:40-65` provides lock-free atomic counters for `total_bytes`, `total_files`, and `active_workers` within `GlobalState`.

### Repo Conventions to Maintain
- **Zero-Regression Core Traversal**: `dscan-core` must retain 100% zero external dependencies (no `serde`, `clap`, `nix`, `rayon` in core). All raw kernel syscalls and memory alignment primitives must remain untouched.
- **Native GUI Architecture**: `crates/dscan-gui` becomes a pure-Rust crate directly under `crates/dscan-gui/Cargo.toml` without nested `src-tauri` or web `ui/` directories.
- **Design Tokens & Accessibility**: Follow `ui-ux-pro-max` guidelines:
  * High-contrast Dark Mode (WCAG AA 4.5:1 minimum on all text).
  * Monospace data display: JetBrains Mono for numbers, file sizes, and paths; Inter for UI chrome.
  * No emojis as functional UI icons (use clean vector icons from `gpui-kit` / Lucide).
  * 120 FPS / 60 FPS hardware VSync interaction without UI stutter or garbage collection pauses.

## Commands you will need
| Purpose | Command | Expected on success |
|-----------|--------------------------|---------------------|
| Workspace Check | `cargo check --workspace` | exit 0, no errors |
| Workspace Test | `cargo test --workspace` | exit 0, all tests pass |
| GUI Crate Check | `cargo check -p dscan-gui` | exit 0, no errors |
| GUI Crate Test | `cargo test -p dscan-gui` | exit 0, all tests pass |
| Workspace Clippy | `cargo clippy --workspace --all-targets -- -D warnings` | exit 0, zero warnings |
| Format Check | `cargo fmt --check` | exit 0, clean formatting |
| Build Release Binary | `cargo build -p dscan-gui --release` | exit 0, standalone binary |

## Suggested executor toolkit
- **`gpui-kit`**: High-level desktop component framework (`0.7`) providing shadcn/ui-flavored `Resizable`, `DataTable`, `Tree`, `TitleBar`, `StatusBar`, `Button`, `Input`, and design tokens.
- **`gpui`**: Hardware-accelerated UI framework from Zed Industries (`0.2`), providing custom `Element` rendering, GPU window contexts, layout via Taffy, and event dispatch.
- **`rust-dev`**: Idiomatic Rust 2024 patterns, lock-free concurrency, zero-copy data modeling, and Clippy compliance.
- **`ui-ux-pro-max`**: Design intelligence for desktop utilities, accessible dark palettes (`#0F172A` Slate Dark), and virtualized hierarchy trees.

## Scope
**In scope** (files to create/modify/delete):
- `Cargo.toml` (update workspace members from `"crates/dscan-gui/src-tauri"` to `"crates/dscan-gui"`)
- `crates/dscan-gui/Cargo.toml` (create: pure-Rust crate configuration with `gpui-kit`, `gpui`, `sysinfo`, `open`)
- `crates/dscan-gui/src/main.rs` (create: native application entry and window lifecycle)
- `crates/dscan-gui/src/app.rs` (create: root `DscanApp` view, 3-pane layout, shortcut bindings)
- `crates/dscan-gui/src/state.rs` (create: reactive UI state model, zero-copy `ScanSession` polling)
- `crates/dscan-gui/src/theme.rs` (create: Slate 900 design tokens, typography, OKLCH extension palette)
- `crates/dscan-gui/src/system.rs` (create: cross-platform drive detection and file manager launcher)
- `crates/dscan-gui/src/views/mod.rs` (create: view module exports)
- `crates/dscan-gui/src/views/title_bar.rs` (create: custom titlebar with drive picker, scan controls)
- `crates/dscan-gui/src/views/directory_tree.rs` (create: virtualized directory tree table with Pacman indicator)
- `crates/dscan-gui/src/views/extension_legend.rs` (create: extension table with color chips and click-filtering)
- `crates/dscan-gui/src/views/cushion_treemap.rs` (create: treemap container, mouse tracking, tooltip card)
- `crates/dscan-gui/src/views/status_bar.rs` (create: bottom status bar with scan rates, elapsed time, capacity bar)
- `crates/dscan-gui/src/treemap/mod.rs` (create: treemap engine module exports)
- `crates/dscan-gui/src/treemap/squarify.rs` (create: pure-Rust squarified treemap layout algorithm)
- `crates/dscan-gui/src/treemap/cushion.rs` (create: Van Wijk quadratic cushion mathematical model)
- `crates/dscan-gui/src/treemap/element.rs` (create: custom GPUI `Element` for GPU cushion rendering)
- `crates/dscan-gui/src/treemap/shader.rs` (create: WGSL GPU shader definitions for Blade pipeline)
- `crates/dscan-gui/src-tauri/` (delete: remove obsolete Tauri v2 backend)
- `crates/dscan-gui/ui/` (delete: remove obsolete web frontend and node_modules)
- `plans/README.md` (record Plan 022 status)

**Out of scope** (do NOT touch):
- `crates/dscan-core/src/scanner.rs`, `sys/`, `simd.rs`, `arena.rs`, `work_stealing.rs`: the core kernel traversal engine must remain completely unchanged.
- `crates/dscan-cli/`: the command-line interface and its ANSI terminal formatting remain independent.
- Any change to CLI flag names or options (`--depth`, `--top`, `--threads`, `--exclude`).

## Git workflow
- Branch: `feat/gui-gpui-kit-migration`
- Commit per step; message style: Conventional Commits (`feat(gui): ...`, `refactor(gui): ...`, `clean(gui): ...`).
- Do NOT push or open a PR unless instructed by the operator.

---

## Architecture & Visual Specification

### 1. Workspace Topology
```
dscan/
├── Cargo.toml                          # Workspace root: members = ["crates/dscan-core", "crates/dscan-cli", "crates/dscan-gui"]
├── crates/
│   ├── dscan-core/                     # Zero-dependency kernel scanner library (unchanged)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── arena.rs
│   │       ├── snapshot.rs             # ScanSession, ScanProgressDto, TreemapNodeDto
│   │       ├── scanner.rs
│   │       └── sys/
│   ├── dscan-cli/                      # Lean terminal CLI binary (unchanged)
│   │   ├── Cargo.toml
│   │   └── src/
│   └── dscan-gui/                      # 100% Pure-Rust Native GPUI-Kit Visualizer
│       ├── Cargo.toml                  # Depends on dscan-core, gpui-kit = "0.7", gpui = "0.2"
│       └── src/
│           ├── main.rs                 # GPUI Application bootstrap, window options, theme init
│           ├── app.rs                  # Root DscanApp view: 3-pane resizable layout
│           ├── state.rs                # In-process reactive state, zero-copy ScanSession binding
│           ├── theme.rs                # Slate Dark tokens, typography, 16 OKLCH extension colors
│           ├── system.rs               # Drive enumeration & open-in-file-manager
│           ├── views/
│           │   ├── mod.rs
│           │   ├── title_bar.rs        # Custom window titlebar with drive picker & controls
│           │   ├── directory_tree.rs   # Top-Left virtualized tree table with Pacman indicator
│           │   ├── extension_legend.rs # Top-Right extension list with color chips & filter
│           │   ├── cushion_treemap.rs  # Bottom interactive cushion treemap container
│           │   └── status_bar.rs       # Bottom status bar with live speed & capacity gauge
│           └── treemap/
│               ├── mod.rs
│               ├── squarify.rs         # Pure-Rust squarified treemap layout engine
│               ├── cushion.rs          # Van Wijk quadratic surface mathematics
│               ├── element.rs          # Custom GPUI Element (request_layout, prepaint, paint)
│               └── shader.rs           # GPU fragment shader (WGSL / Blade pipeline)
```

### 2. WinDirStat 3-Pane Layout with GPUI-Kit Components
```
+----------------------------------------------------------------------------------------------------+
|  [>] dscan   | Target: [/home/ahoura       ] [v Drives]  [Scan] [Pause] [Cancel]       [_] [^] [X] |
+-----------------------------------------------------------------+----------------------------------+
|  Top-Left: Directory Tree View (Resizable: ~55% width)          |  Top-Right: Extension Legend     |
|  Name               | % Subtree | Size      | Files  | Pacman   |  Ext    | Chip  | % Disk | Size    |
|  > home/            | █████████ |  412.4 GB | 842.1k |          |  .mp4   | [red] |  38.2% | 157.5GB |
|    > ahoura/        | ████████  |  398.1 GB | 812.0k | <( - - ) |  .iso   | [blu] |  21.4% |  88.2GB |
|      > .cache/      | ████      |   84.2 GB | 145.2k |          |  .tar   | [grn] |  12.1% |  49.9GB |
|      > Projects/    | ███       |   62.7 GB | 280.4k |          |  .rs    | [org] |   4.8% |  19.8GB |
|      > Downloads/   | ██        |   41.0 GB |   1.2k |          |  [..]   | [gry] |  23.5% |  97.0GB |
+-----------------------------------------------------------------+----------------------------------+
|  <================================== Resizable Splitter =========================================>  |
+----------------------------------------------------------------------------------------------------+
|  Bottom: Native GPU Cushion Treemap (100% width, flexible height, 120 FPS Direct GPU Rendering)     |
|  +-------------------------------------------------------------+--------------------------------+  |
|  | [ home/ahoura/Videos/movie.mp4 ] (Hovered: 4.2 GB, 1.0% disk)| [.cache/cargo/registry]        |  |
|  | (Direct GPU Quadratic Cushion Shading computed in fragment   |                                |  |
|  |  shader; zero CPU putImageData; smooth interactive lighting) |                                |  |
|  |                                                             |                                |  |
|  +-------------------------------------------------------------+--------------------------------+  |
+----------------------------------------------------------------------------------------------------+
|  Status: Scanning /home/ahoura/.cache/yarn | 512,480 files/s | Elapsed: 00:01.2 | [██████░░ 78%]   |
+----------------------------------------------------------------------------------------------------+
```

### 3. GPU Cushion Treemap Rendering Pipeline
```
[ScanSession Arena Nodes]
         │
         ▼  (Zero-Copy In-Process Slice)
[squarify::compute_treemap_layout] ──► Computes Rectangles [x1, y1, x2, y2] & Depth Chains
         │
         ▼
[CushionTreemapElement::paint] ────► Passes Geometry & Surface Bounds to GPUI Window Context
         │
         ▼
[Blade GPU Pipeline (WGSL)] ──────► Translated to DirectX 11 (HLSL) / Vulkan / Metal
         │
         ▼
[Fragment Shader Evaluation]
   For every pixel (x, y):
     1. Evaluate quadratic cushion surface derivatives:
          nx = sum( -4 * h_k * (2*x - (x1 + x2)) / (x2 - x1)^2 )
          ny = sum( -4 * h_k * (2*y - (y1 + y2)) / (y2 - y1)^2 )
     2. Compute normalized surface normal:
          N = normalize( -nx, -ny, 1.0 )
     3. Apply directional illumination:
          I = Ia + Id * max(0.0, dot(N, L))
     4. Final Color = BaseColor * I
   Output: Buttery-smooth 120 FPS rendering; zero CPU pixel loops!
```

### 4. Zero-Copy In-Process Synchronization Architecture
```
┌─────────────────────────────────────────────────────────────────────────────┐
│ Old Tauri Architecture (Slow, Heavy, Fragile)                               │
│ [Rust Engine] ──► Serde JSON ──► Named Pipe/IPC ──► V8 JSON Parse ──► JS    │
│   (50,000 nodes = 15MB JSON string serialization + GC freeze on every tick) │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ New Native GPUI Architecture (Instant, Zero-Copy, Pure Rust)                │
│ [dscan-core::ScanSession]                                                   │
│   │   Atomic Counters (total_bytes, total_files, active_workers)             │
│   │   Flat DirArena & TopFiles min-heap in shared memory                     │
│   │                                                                         │
│   ▼  Direct Arc<ScanSession> Pointer Dereference                            │
│ [DscanState (GPUI Model)]                                                    │
│   │   Timer poll (50ms): cx.notify() on atomic changes                       │
│   │   Treemap fetch: Direct slice iteration, zero JSON, zero string copies   │
│   │                                                                         │
│   ▼  Hardware GPU Submission                                                │
│ [GPUI Window Context (DirectX 11 / Vulkan / Metal)] ──► 120 FPS VSync      │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 5. Color Tokens & Design Specs (Slate Dark)
- **Background Root**: `#0F172A` (Slate 900)
- **Surfaces & Cards**: `#1E293B` (Slate 800)
- **Border Dividers**: `#334155` (Slate 700, 1px solid)
- **Hover Rows / Controls**: `#334155` (Slate 700)
- **Selection Highlight**: `#1D4ED8` (Blue 700) with `#38BDF8` (Sky 400) border
- **Primary Text**: `#F8FAFC` (Slate 50, contrast 13.5:1)
- **Muted Text**: `#94A3B8` (Slate 400, contrast 5.4:1)
- **Metric Highlight**: `#38BDF8` (Sky 400), `#4ADE80` (Green 400), `#FACC15` (Yellow 400)
- **Extension Palette (16 Distinct Colors)**:
  1. `.mp4, .mkv, .avi`: `#EF4444` (Red 500)
  2. `.iso, .img, .vdi`: `#3B82F6` (Blue 500)
  3. `.tar, .gz, .zip, .zst`: `#10B981` (Emerald 500)
  4. `.rs, .c, .cpp, .go`: `#F97316` (Orange 500)
  5. `.png, .jpg, .webp`: `#EC4899` (Pink 500)
  6. `.pdf, .doc, .txt`: `#8B5CF6` (Violet 500)
  7. `.dll, .so, .exe`: `#06B6D4` (Cyan 500)
  8. `.sqlite, .db, .bin`: `#EAB308` (Yellow 500)
  9. `.js, .ts, .json`: `#14B8A6` (Teal 500)
  10. `.mp3, .wav, .flac`: `#F43F5E` (Rose 500)
  11. `.lock, .toml, .yaml`: `#A855F7` (Purple 500)
  12. `.py, .pyc, .whl`: `#6366F1` (Indigo 500)
  13. `.deb, .rpm, .apk`: `#84CC16` (Lime 500)
  14. `.log, .out`: `#64748B` (Slate 500)
  15. `.tmp, .cache`: `#78716C` (Stone 500)
  16. `Other / Unknown`: `#94A3B8` (Slate 400)

---

## Steps

### Step 1: Workspace Cargo Configuration & Crate Scaffolding
Reconfigure the Cargo workspace to establish `crates/dscan-gui` as a pure-Rust crate and remove the nested `src-tauri` workspace member.

1. Update the root `Cargo.toml`:
   ```toml
   [workspace]
   resolver = "3"
   members = [
       "crates/dscan-core",
       "crates/dscan-cli",
       "crates/dscan-gui",
   ]

   [profile.release]
   opt-level = 3
   lto = "fat"
   codegen-units = 1
   panic = "abort"
   strip = true
   ```
2. Create `crates/dscan-gui/Cargo.toml`:
   ```toml
   [package]
   name = "dscan-gui"
   version = "0.3.1"
   edition = "2024"
   authors = ["AhooraZen <ahoora935137@gmail.com>"]
   description = "Pure-Rust native GPUI WinDirStat cushion treemap visualizer for dscan"
   license = "MIT OR Apache-2.0"

   [[bin]]
   name = "dscan-gui"
   path = "src/main.rs"

   [dependencies]
   dscan-core = { path = "../dscan-core" }
   gpui = "0.2"
   gpui-kit = "0.7"
   parking_lot = "0.12"
   sysinfo = "0.33"
   open = "5.3"

   [target.'cfg(target_os = "windows")'.dependencies]
   windows-sys = { version = "0.59", features = ["Win32_Storage_FileSystem", "Win32_System_SystemInformation"] }
   ```
3. Create the initial directory structure:
   ```bash
   mkdir -p crates/dscan-gui/src/views crates/dscan-gui/src/treemap
   ```

**Verify**: `cargo check -p dscan-gui` → exit 0 (after creating a stub `src/main.rs` returning `fn main() {}`).

---

### Step 2: Pure-Rust Squarified Cushion Treemap Engine
Implement the pure-Rust squarified layout algorithm (Bruls, Huizing, van Wijk) and quadratic cushion surface calculation without any external dependencies.

1. Create `crates/dscan-gui/src/treemap/cushion.rs`:
   - Define constants:
     ```rust
     pub const H0: f32 = 0.5;
     pub const F: f32 = 0.65;
     pub const IA: f32 = 0.25;
     pub const ID: f32 = 0.75;
     pub const LX: f32 = -0.40824829;
     pub const LY: f32 = -0.40824829;
     pub const LZ: f32 = 0.81649658;
     ```
   - Implement `CushionSurface`:
     ```rust
     #[derive(Debug, Clone, Copy, PartialEq)]
     pub struct CushionSurface {
         pub x1: f32,
         pub y1: f32,
         pub x2: f32,
         pub y2: f32,
         pub depth: u16,
     }
     ```
   - Implement quadratic normal evaluation function `compute_cushion_derivatives(cushions: &[CushionSurface], x: f32, y: f32) -> (f32, f32)`.
   - Implement intensity calculation function `compute_cushion_intensity(nx: f32, ny: f32) -> f32`.

2. Create `crates/dscan-gui/src/treemap/squarify.rs`:
   - Implement `TreemapRect { pub x: f32, pub y: f32, pub w: f32, pub h: f32 }`.
   - Implement `TreemapItem`:
     ```rust
     #[derive(Debug, Clone)]
     pub struct TreemapItem {
         pub node_id: u32,
         pub parent_id: u32,
         pub name: String,
         pub size: u64,
         pub is_dir: bool,
         pub extension: String,
         pub rect: TreemapRect,
         pub depth: u16,
         pub cushions: Vec<CushionSurface>,
     }
     ```
   - Implement the squarify partitioning algorithm that minimizes aspect ratios:
     `pub fn squarify_layout(nodes: &[dscan_core::TreemapNodeDto], bounds: TreemapRect) -> Vec<TreemapItem>`
   - Implement recursive cushion inheritance: each child inherits the ancestor's `CushionSurface` chain with depth multiplier $h_k = H_0 \cdot F^k$.

3. Create `crates/dscan-gui/src/treemap/mod.rs`:
   ```rust
   pub mod cushion;
   pub mod element;
   pub mod shader;
   pub mod squarify;

   pub use cushion::*;
   pub use element::*;
   pub use squarify::*;
   ```

**Verify**: `cargo test -p dscan-gui --lib treemap` → exit 0, squarify aspect ratios $\ge 1.0$ and normal bounds validated.

---

### Step 3: GPUI Custom Element & GPU Cushion Shader
Implement a custom GPUI `Element` that renders the treemap directly to the GPU scene via GPUI's drawing primitives and hardware shader passes.

1. Create `crates/dscan-gui/src/treemap/shader.rs`:
   - Define WGSL fragment shader `CUSHION_WGSL` implementing the per-pixel Van Wijk illumination:
     ```wgsl
     struct Uniforms {
         light_dir: vec3<f32>,
         ambient: f32,
         diffuse: f32,
         viewport: vec2<f32>,
     };

     @group(0) @binding(0) var<uniform> u: Uniforms;

     struct FragmentInput {
         @location(0) uv: vec2<f32>,
         @location(1) @interpolate(flat) base_color: vec4<f32>,
         @location(2) @interpolate(flat) cushion_bounds: vec4<f32>, // x1, y1, x2, y2
         @location(3) @interpolate(flat) depth_factor: f32,
     };

     @fragment
     fn fs_main(in: FragmentInput) -> @location(0) vec4<f32> {
         let w = in.cushion_bounds.z - in.cushion_bounds.x;
         let h = in.cushion_bounds.w - in.cushion_bounds.y;
         let px = in.uv.x;
         let py = in.uv.y;

         let nx = -4.0 * in.depth_factor * (2.0 * px - (in.cushion_bounds.x + in.cushion_bounds.z)) / (w * w);
         let ny = -4.0 * in.depth_factor * (2.0 * py - (in.cushion_bounds.y + in.cushion_bounds.w)) / (h * h);
         let len = sqrt(nx * nx + ny * ny + 1.0);
         let normal = vec3<f32>(-nx / len, -ny / len, 1.0 / len);

         let n_dot_l = max(0.0, dot(normal, u.light_dir));
         let intensity = u.ambient + u.diffuse * n_dot_l;

         return vec4<f32>(in.base_color.rgb * intensity, 1.0);
     }
     ```

2. Create `crates/dscan-gui/src/treemap/element.rs`:
   - Implement `CushionTreemapElement`:
     ```rust
     pub struct CushionTreemapElement {
         items: Arc<Vec<TreemapItem>>,
         selected_id: Option<u32>,
         hovered_id: Option<u32>,
         color_lookup: Arc<HashMap<String, Hsla>>,
         on_click: Option<Box<dyn Fn(u32, &mut Window, &mut App) + 'static>>,
         on_hover: Option<Box<dyn Fn(Option<u32>, &mut Window, &mut App) + 'static>>,
     }
     ```
   - Implement GPUI's `Element` trait:
     * `request_layout`: Requests full layout bounds from Taffy (`style.flex_grow = 1.0`).
     * `prepaint`: Registers mouse hitboxes for hover and click events.
     * `paint`: Draws the treemap items onto the window context. In primary mode, quads with dynamic corner shading and border indicators (`window.paint_quad`) are submitted in batched GPU calls; selection and hover rings use `#38BDF8` (Sky 400) and `#F8FAFC` (Slate 50).
   - Implement binary search / quadtree spatial index for $O(\log N)$ mouse coordinate hit-testing.

**Verify**: `cargo check -p dscan-gui` → exit 0, `Element` trait properly implemented for `CushionTreemapElement`.

---

### Step 4: Reactive In-Process State Management
Implement the reactive data model that interfaces directly with `dscan-core::ScanSession` without Serde or IPC serialization.

1. Create `crates/dscan-gui/src/theme.rs`:
   - Define color constants using GPUI `Hsla` and hex helpers:
     ```rust
     use gpui::Hsla;

     pub const BG_ROOT: Hsla = gpui::rgb(0x0F172A);
     pub const BG_SURFACE: Hsla = gpui::rgb(0x1E293B);
     pub const BG_HOVER: Hsla = gpui::rgb(0x334155);
     pub const BORDER_COLOR: Hsla = gpui::rgb(0x334155);
     pub const TEXT_PRIMARY: Hsla = gpui::rgb(0xF8FAFC);
     pub const TEXT_MUTED: Hsla = gpui::rgb(0x94A3B8);
     pub const ACCENT_BLUE: Hsla = gpui::rgb(0x38BDF8);
     pub const ACCENT_GREEN: Hsla = gpui::rgb(0x4ADE80);
     pub const ACCENT_YELLOW: Hsla = gpui::rgb(0xFACC15);
     ```
   - Implement `extension_color(ext: &str) -> Hsla` returning distinct OKLCH-derived categorical colors for file extensions.

2. Create `crates/dscan-gui/src/state.rs`:
   - Define `DscanModel`:
     ```rust
     pub struct DscanModel {
         pub session: Option<Arc<dscan_core::ScanSession>>,
         pub current_path: String,
         pub is_scanning: bool,
         pub is_paused: bool,
         pub progress: dscan_core::ScanProgressDto,
         pub treemap_items: Arc<Vec<TreemapItem>>,
         pub extension_stats: Vec<dscan_core::ExtensionStatDto>,
         pub selected_node_id: Option<u32>,
         pub hovered_node_id: Option<u32>,
         pub highlighted_extension: Option<String>,
         pub drives: Vec<DriveInfo>,
         pub selected_drive: Option<String>,
     }
     ```
   - Implement state transition methods:
     * `start_scan(&mut self, path: String, cx: &mut Context<Self>)`: Creates `dscan_core::ScanOptions`, invokes `dscan_core::ScanSession::start(opts)`, resets state, and starts a 50ms periodic timer via `cx.on_interval`.
     * `poll_tick(&mut self, cx: &mut Context<Self>)`: Directly calls `session.poll_progress()`. On scan completion, invokes `session.get_treemap_data(6)` and `session.get_extension_stats()`, runs `squarify_layout`, and calls `cx.notify()`. Zero serialization!
     * `pause_scan(&mut self, cx: &mut Context<Self>)`: Toggles `session.pause()` / `resume()`.
     * `cancel_scan(&mut self, cx: &mut Context<Self>)`: Calls `session.cancel()`.
     * `select_node(&mut self, node_id: Option<u32>, cx: &mut Context<Self>)`.
     * `filter_by_extension(&mut self, ext: Option<String>, cx: &mut Context<Self>)`.

**Verify**: `cargo check -p dscan-gui` → exit 0, reactive model compiles cleanly.

---

### Step 5: WinDirStat 3-Pane UI Layout with GPUI-Kit Components
Construct the 3-pane desktop layout using `gpui-kit`'s shadcn-flavored components (`Resizable`, `DataTable`, `Tree`, `TitleBar`, `StatusBar`).

1. Create `crates/dscan-gui/src/views/title_bar.rs`:
   - Implement `TitleBarView`:
     * Displays target path input box.
     * Drive dropdown selector showing mounted drives with free/total capacity.
     * Action buttons: "Scan", "Pause", "Cancel" with active states and hotkey hints (`Enter`, `Space`, `Esc`).

2. Create `crates/dscan-gui/src/views/directory_tree.rs`:
   - Implement `DirectoryTreeView`:
     * Top-left virtualized tree table showing hierarchical directories.
     * Columns: Name, Subtree Size % (mini progress bar), Size (formatted via `dscan_core::format_bytes`), File Count, and animated Pacman icon during scan.
     * Synchronized selection with `DscanModel::selected_node_id`.

3. Create `crates/dscan-gui/src/views/extension_legend.rs`:
   - Implement `ExtensionLegendView`:
     * Top-right table listing discovered file extensions sorted by total bytes.
     * Columns: Extension (`.mp4`), Color Chip (`[■]`), % of Disk, Total Size, File Count.
     * Clicking a row sets `DscanModel::highlighted_extension`, dimming non-matching cushions in the treemap view.

4. Create `crates/dscan-gui/src/views/cushion_treemap.rs`:
   - Implement `CushionTreemapView`:
     * Houses `CushionTreemapElement`.
     * Renders floating hover tooltip card displaying file name, absolute path, size, and % of disk.
     * Double-click handler triggering system file manager opening.

5. Create `crates/dscan-gui/src/views/status_bar.rs`:
   - Implement `StatusBarView`:
     * Bottom status strip showing: scanning path, traversal rate (e.g. `512,480 files/s`), elapsed time, total files, and disk capacity gauge.

6. Create `crates/dscan-gui/src/views/mod.rs`:
   ```rust
   pub mod cushion_treemap;
   pub mod directory_tree;
   pub mod extension_legend;
   pub mod status_bar;
   pub mod title_bar;

   pub use cushion_treemap::*;
   pub use directory_tree::*;
   pub use extension_legend::*;
   pub use status_bar::*;
   pub use title_bar::*;
   ```

7. Create `crates/dscan-gui/src/app.rs`:
   - Implement `DscanApp` as the root GPUI view:
     * Combines `TitleBarView` at top.
     * Uses `gpui_kit::component::resizable::Resizable` to create:
       a. Horizontal splitter dividing `DirectoryTreeView` (left 55%) and `ExtensionLegendView` (right 45%).
       b. Vertical splitter dividing the Top Pane and `CushionTreemapView` (bottom).
     * Connects `StatusBarView` at bottom.
     * Binds keyboard shortcuts (`Ctrl+O` for folder picker, `Space` for pause/resume, `Escape` for cancel, `F5` for rescan).

**Verify**: `cargo check -p dscan-gui` → exit 0, view hierarchy and resizable panels compile without error.

---

### Step 6: Native System Integrations
Implement cross-platform drive enumeration and file manager launching using native OS APIs and standard Rust libraries.

1. Create `crates/dscan-gui/src/system.rs`:
   - Implement `DriveInfo`:
     ```rust
     #[derive(Debug, Clone, PartialEq)]
     pub struct DriveInfo {
         pub mount_point: String,
         pub name: String,
         pub total_bytes: u64,
         pub free_bytes: u64,
     }
     ```
   - Implement `get_system_drives() -> Vec<DriveInfo>`:
     * On Linux: parse `/proc/mounts` or use `sysinfo::Disks`. Filter out pseudo-filesystems (`sysfs`, `proc`, `tmpfs`, `devpts`).
     * On Windows: invoke `GetLogicalDriveStringsW` and `GetDiskFreeSpaceExW` via `windows-sys`.
   - Implement `open_in_file_manager(path: &str)`:
     * Uses the `open::that(path)` crate to launch `xdg-open` / dolphin / nautilus on Linux, `explorer.exe` on Windows, and `Finder` on macOS.

**Verify**: `cargo check -p dscan-gui` → exit 0, system drive detection and file manager integrations compile on target OS.

---

### Step 7: Application Entry Point & Native Window Lifecycle
Implement `crates/dscan-gui/src/main.rs` initializing GPUI, configuring the native window, and launching the event loop.

1. Write `crates/dscan-gui/src/main.rs`:
   ```rust
   mod app;
   mod state;
   mod system;
   mod theme;
   mod treemap;
   mod views;

   use app::DscanApp;
   use gpui::*;
   use state::DscanModel;

   fn main() {
       Application::new().run(|cx: &mut App| {
           // Configure window options
           let bounds = WindowBounds::Windowed(Bounds::centered(
               None,
               size(px(1280.0), px(800.0)),
               cx,
           ));

           let window_options = WindowOptions {
               window_bounds: Some(bounds),
               titlebar: Some(TitlebarOptions {
                   title: Some("dscan — Fast Disk Space Analyzer".into()),
                   appears_transparent: true,
                   traffic_light_position: Some(point(px(9.0), px(9.0))),
               }),
               window_min_size: Some(size(px(960.0), px(600.0))),
               ..Default::default()
           };

           cx.open_window(window_options, |cx| {
               let model = cx.new_model(|_cx| DscanModel::new());
               cx.new_view(|cx| DscanApp::new(model, cx))
           })
           .expect("failed to open dscan-gui window");
       });
   }
   ```

2. Implement `DscanModel::new()` in `crates/dscan-gui/src/state.rs` initializing default drive list and idle status.

**Verify**: `cargo build -p dscan-gui` → exit 0, native binary produced in `target/debug/dscan-gui`.

---

### Step 8: Retirement of Tauri v2 & Web Frontend Artifacts
Delete the obsolete Tauri v2 configuration, C build scripts, and web frontend directories.

1. Delete the Tauri backend crate:
   ```bash
   rm -rf crates/dscan-gui/src-tauri
   ```
2. Delete the web UI directory:
   ```bash
   rm -rf crates/dscan-gui/ui
   ```
3. Verify git status shows only clean additions in `crates/dscan-gui/` and clean removals of `src-tauri` and `ui`.

**Verify**:
```bash
cargo check --workspace
cargo test --workspace
```
→ exit 0, workspace resolves cleanly with zero remaining references to `src-tauri` or web assets.

---

### Step 9: Integration Testing, Clippy & Formatting Quality Gate
Create unit and headless integration tests verifying state transitions, squarify layouts, and cushion normal bounds. Ensure zero Clippy warnings across the workspace.

1. Create `crates/dscan-gui/tests/gui_integration.rs`:
   - Test `test_squarify_aspect_ratios`: asserts all generated rectangles have aspect ratio $\ge 1.0$ and sum of areas matches root area.
   - Test `test_cushion_normals`: asserts normal vector $N$ has unit length $\approx 1.0$ and intensity $I \in [0.25, 1.00]$.
   - Test `test_scan_lifecycle_transitions`: creates a mock `ScanSession`, triggers `poll_tick`, verifies progress values update without panic.
2. Run format check and auto-format:
   ```bash
   cargo fmt --check
   ```
3. Run Clippy across all targets:
   ```bash
   cargo clippy --workspace --all-targets -- -D warnings
   ```
4. Run full test suite:
   ```bash
   cargo test --workspace
   ```

**Verify**: All commands exit 0 with zero warnings and 100% test pass.

---

## Test plan
- **Unit Tests (`crates/dscan-gui/src/treemap/squarify.rs`)**:
  * `test_squarify_empty_input`: empty node list returns empty item vector without panic.
  * `test_squarify_single_node`: single node occupies entire bounding box.
  * `test_squarify_aspect_ratio_bound`: verify aspect ratio $\max(w/h, h/w)$ remains strictly bounded across diverse file size distributions.
  * `test_squarify_area_conservation`: verify $\sum (\text{rect.w} \times \text{rect.h}) == \text{bounds.w} \times \text{bounds.h}$ within floating-point epsilon.
- **Unit Tests (`crates/dscan-gui/src/treemap/cushion.rs`)**:
  * `test_normal_normalization`: for arbitrary $x, y$ coordinates within bounds, verify $N_x^2 + N_y^2 + N_z^2 = 1.0 \pm 10^{-5}$.
  * `test_intensity_bounds`: verify intensity $I \ge I_a$ (0.25) and $I \le I_a + I_d$ (1.00).
  * `test_ancestor_depth_decay`: verify ancestor cushion influence diminishes exponentially according to $F^k$.
- **Integration Tests (`crates/dscan-gui/tests/gui_integration.rs`)**:
  * `test_scan_progress_polling`: verify atomic progress counters (`total_bytes`, `total_files`) correctly update `DscanModel` state on each interval.
  * `test_extension_filter_toggle`: clicking an extension sets `highlighted_extension` and filters treemap rendering.
- **Verification Commands**:
  * `cargo check --workspace` → exit 0
  * `cargo test --workspace` → exit 0, all tests pass
  * `cargo clippy --workspace --all-targets -- -D warnings` → exit 0
  * `cargo fmt --check` → exit 0

## Done criteria
- [ ] `crates/dscan-gui/src-tauri` and `crates/dscan-gui/ui` directories are completely removed.
- [ ] Root `Cargo.toml` workspace members list `"crates/dscan-gui"` with zero Tauri or web dependencies.
- [ ] `crates/dscan-gui/Cargo.toml` depends only on `dscan-core`, `gpui-kit = "0.7"`, `gpui = "0.2"`, `parking_lot`, `sysinfo`, `open`.
- [ ] `cargo check --workspace` exits 0 with zero errors.
- [ ] `cargo test --workspace` exits 0 with all unit and integration tests passing.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` exits 0 with zero warnings.
- [ ] `cargo fmt --check` exits 0 with clean formatting.
- [ ] `cargo build -p dscan-gui --release` produces a single standalone native binary (<30MB).
- [ ] `grep -rn "putImageData" crates/dscan-gui/` returns zero matches.
- [ ] `grep -rn "tauri" crates/dscan-gui/` returns zero matches.
- [ ] `plans/README.md` status row for Plan 022 is recorded.

## STOP conditions
Stop and report back (do not improvise) if:
- Any file in `crates/dscan-core/` is modified or proposed to be modified (core engine must remain 100% untouched).
- `gpui` or `gpui-kit` fails to compile due to an unresolvable platform graphics driver or toolchain incompatibility.
- A proposed fix requires introducing webview, Electron, Node.js, npm, or WebKit dependencies.
- Any step fails verification twice after a reasonable fix attempt.

## Maintenance notes
- **GPU Shader Compatibility**: The WGSL shader compiles via Blade to HLSL for DirectX 11/12 on Windows, SPIR-V/Vulkan on Linux, and MSL on macOS. Future changes to cushion illumination parameters should be adjusted in `cushion.rs` and `shader.rs` in tandem.
- **Wayland & HiDPI**: GPUI natively handles Wayland fractional scaling protocols and Windows DPI scaling. When testing new window chrome or splitter components, verify layout bounds under $1.25\times$, $1.5\times$, and $2.0\times$ scale factors.
- **Review Scrutiny**: Reviewers should verify that `CushionTreemapElement::paint` performs zero heap allocations during mouse hover or window resize events to maintain 120 FPS frame timing.
