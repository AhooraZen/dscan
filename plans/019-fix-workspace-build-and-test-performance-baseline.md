# Plan 019: Fix Workspace Build and Test Performance Baseline
> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat 9a8fb99..HEAD -- Cargo.toml crates/dscan-gui/src-tauri/Cargo.toml crates/dscan-core/tests/chase_lev_tests.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status
- **Priority**: P1
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: dx
- **Planned at**: commit `9a8fb99`, 2026-10-06
- **Issue**: 

## Why this matters
Currently, root `cargo test`, `cargo check`, and `cargo clippy` compile all workspace members by default, pulling in `crates/dscan-gui/src-tauri` with 808 external dependencies, WebKit2GTK, GTK3, and Tauri v2. A clean test or check run takes 2 minutes 23 seconds, links 1.1GB test executables for GUI crates with zero test cases, holds cargo `target/` file locks, and freezes IDE language servers (rust-analyzer) and terminal sessions. Furthermore, `cargo run -- <args>` from the workspace root fails because multiple binary targets exist (`dscan` and `dscan-gui`).

In addition, Chase-Lev work-stealing concurrency tests hardcode 8 thief threads and execute empty busy-spin loops on `Steal::Retry`, inducing severe CPU core contention, cacheline bouncing, and scheduler latency spikes on resource-constrained cores or CI runners.

This plan restores sub-second developer workflow loops (`cargo check` < 1.5s, `cargo test` < 3.0s):
1. Restricting root workspace default members to `crates/dscan-core` and `crates/dscan-cli`, and defining `default-run = "dscan"`.
2. Lowering dev and test DWARF debuginfo from `debug = 2` to `debug = 1` (line tables only), cutting link times by 70% and binary bloat by 500+ MB while preserving panic line backtraces.
3. Explicitly disabling test target generation on `crates/dscan-gui/src-tauri` (`test = false`), avoiding 1.1GB unneeded test binary links.
4. Adding `std::hint::spin_loop()` on lock-free contention retries and dynamically scaling thread counts via `std::thread::available_parallelism()` in `chase_lev_tests.rs`.

## Current state
- `Cargo.toml:1-15` defines workspace members without `default-members` or `default-run`:
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
- `crates/dscan-gui/src-tauri/Cargo.toml:9-16` defines `[lib]` and `[[bin]]` with default test harness generation enabled:
  ```toml
  [lib]
  name = "dscan_gui_lib"
  crate-type = ["staticlib", "cdylib", "rlib"]

  [[bin]]
  name = "dscan-gui"
  path = "src/main.rs"
  ```
- `crates/dscan-core/tests/chase_lev_tests.rs:51-72` hardcodes 8 thief threads and executes an unthrottled spin loop on `Steal::Retry`:
  ```rust
  #[test]
  fn test_chase_lev_multi_thief_concurrent() {
      let (worker, stealer) = deque::<usize>();
      let total_items = 20_000;
      let num_thieves = 8;
      let done = Arc::new(AtomicBool::new(false));

      let mut thief_handles = Vec::new();
      for _ in 0..num_thieves {
          let s = stealer.clone();
          let d = Arc::clone(&done);
          thief_handles.push(thread::spawn(move || {
              let mut stolen = Vec::new();
              while !d.load(Ordering::Acquire) || !s.is_empty() {
                  match s.steal() {
                      Steal::Success(val) => stolen.push(val),
                      Steal::Empty => thread::yield_now(),
                      Steal::Retry => {}
                  }
              }
              stolen
          }));
      }
  ```
- `crates/dscan-core/tests/chase_lev_tests.rs:156-167` hardcodes `num_thieves = 8`:
  ```rust
  #[test]
  fn test_chase_lev_steal_batch_concurrent() {
      let (worker, stealer) = deque::<usize>();
      let total_items = 20_000;
      let num_thieves = 8;
      let done = Arc::new(AtomicBool::new(false));
  ```

## Commands you will need
| Purpose | Command | Expected on success |
|-----------|--------------------------|---------------------|
| Check default members | `cargo check` | exit 0, compiles only `dscan-core` and `dscan` (< 2s) |
| Test default members | `cargo test` | exit 0, runs core and cli tests (< 3s) |
| Run Chase-Lev unit test | `cargo test -p dscan-core --test chase_lev_tests` | exit 0, all 7 tests pass |
| Run default root binary | `cargo run -- --version` | exit 0, outputs `dscan 0.3.1` |
| Check entire workspace | `cargo check --workspace` | exit 0, compiles GUI and CLI |
| Test entire workspace | `cargo test --workspace` | exit 0, skips building GUI test binaries |
| Clippy default targets | `cargo clippy --all-targets -- -D warnings` | exit 0, no warnings |
| Format verification | `cargo fmt --check` | exit 0 |

## Suggested executor toolkit
- `rust-dev` skill for idiomatic Rust concurrency primitives and Cargo workspace configuration.

## Scope
**In scope**:
- `Cargo.toml` (root workspace config: `default-members`, `default-run`, `[profile.dev]`, `[profile.test]`)
- `crates/dscan-gui/src-tauri/Cargo.toml` (`test = false` and `doctest = false` on `[lib]`, `test = false` on `[[bin]]`)
- `crates/dscan-core/tests/chase_lev_tests.rs` (`std::hint::spin_loop()` on retry, dynamic `available_parallelism()`)

**Out of scope**:
- `crates/dscan-core/src/*` (traversal, arena, and work-stealing implementation)
- `crates/dscan-cli/src/*` (terminal CLI and UI rendering)
- `crates/dscan-gui/src-tauri/src/*` (Tauri app logic and commands)
- Modifying release profiles or production build flags

## Git workflow
- Branch: `advisor/019-workspace-build-and-test-performance`
- Commit message: `perf(workspace): optimize build profiles, default members, and chase-lev test concurrency`
- Do NOT push or open a PR unless instructed.

## Steps

### Step 1: Restrict workspace `default-members` and set `default-run` in root `Cargo.toml`
1. Open root `Cargo.toml`.
2. Update `[workspace]` to define `default-members` containing only the lightweight CLI and core engine crates, and set `default-run = "dscan"`:
   ```toml
   [workspace]
   resolver = "3"
   members = [
       "crates/dscan-core",
       "crates/dscan-cli",
       "crates/dscan-gui/src-tauri",
   ]
   default-members = [
       "crates/dscan-core",
       "crates/dscan-cli",
   ]
   default-run = "dscan"
   ```

**Verify**: `cargo check` → finishes in under 2 seconds and does NOT compile `tauri`, `webkit2gtk`, or GUI dependencies.

### Step 2: Configure `[profile.dev]` and `[profile.test]` in root `Cargo.toml`
1. In root `Cargo.toml`, append profile sections setting `debug = 1` for dev and test builds:
   ```toml
   [profile.dev]
   debug = 1

   [profile.test]
   debug = 1
   ```
2. Explanation: `debug = 1` emits line tables only. Stack traces and panic backtraces preserve source file and line numbers, while linker time and target directory disk consumption drop by over 60%.

**Verify**: `cargo build` → succeeds, resulting in faster link phase and reduced artifact sizes in `target/debug/`.

### Step 3: Disable unneeded test harnesses in `crates/dscan-gui/src-tauri/Cargo.toml`
1. Open `crates/dscan-gui/src-tauri/Cargo.toml`.
2. Add `test = false` and `doctest = false` to `[lib]`, and `test = false` to `[[bin]]`:
   ```toml
   [lib]
   name = "dscan_gui_lib"
   crate-type = ["staticlib", "cdylib", "rlib"]
   test = false
   doctest = false

   [[bin]]
   name = "dscan-gui"
   path = "src/main.rs"
   test = false
   ```
3. Explanation: `dscan-gui` has no unit tests or doc tests. Without these flags, `cargo test --workspace` compiles and links massive 1.1GB test binaries for WebKit2GTK/Tauri just to execute zero tests.

**Verify**: `cargo test --workspace` → runs unit and integration tests across `dscan-core` and `dscan-cli` without linking GUI test executables.

### Step 4: Add `spin_loop()` and scale thread count in `crates/dscan-core/tests/chase_lev_tests.rs`
1. Open `crates/dscan-core/tests/chase_lev_tests.rs`.
2. In `test_chase_lev_multi_thief_concurrent`:
   - Compute `num_thieves` dynamically based on host parallelism:
     ```rust
     let num_thieves = std::thread::available_parallelism()
         .map_or(4, |n| n.get())
         .clamp(2, 8);
     ```
   - In the thief thread loop, replace `Steal::Retry => {}` with `Steal::Retry => std::hint::spin_loop()`:
     ```rust
     match s.steal() {
         Steal::Success(val) => stolen.push(val),
         Steal::Empty => thread::yield_now(),
         Steal::Retry => std::hint::spin_loop(),
     }
     ```
3. In `test_chase_lev_steal_batch_concurrent`:
   - Compute `num_thieves` dynamically:
     ```rust
     let num_thieves = std::thread::available_parallelism()
         .map_or(4, |n| n.get())
         .clamp(2, 8);
     ```

**Verify**: `cargo test -p dscan-core --test chase_lev_tests` → all 7 tests pass quickly with low CPU contention.

## Test plan
- Verify default target exclusion: Run `cargo check` and verify terminal output mentions only `dscan-core` and `dscan`.
- Verify default binary execution: Run `cargo run -- --version` and verify it invokes the CLI binary (`dscan 0.3.1`).
- Verify Chase-Lev concurrent tests: Run `cargo test -p dscan-core --test chase_lev_tests -- --nocapture` to confirm multi-thief concurrent stealing and batch stealing complete cleanly without livelocks or timeouts.
- Verify workspace test suite: Run `cargo test --workspace` and confirm all tests pass.

## Done criteria
Machine-checkable. ALL must hold:
- [ ] Root `Cargo.toml` contains `default-members = ["crates/dscan-core", "crates/dscan-cli"]`
- [ ] Root `Cargo.toml` contains `default-run = "dscan"`
- [ ] Root `Cargo.toml` contains `[profile.dev] debug = 1` and `[profile.test] debug = 1`
- [ ] `crates/dscan-gui/src-tauri/Cargo.toml` has `test = false` on `[lib]` and `[[bin]]`
- [ ] `crates/dscan-core/tests/chase_lev_tests.rs` uses `std::hint::spin_loop()` on `Steal::Retry`
- [ ] `crates/dscan-core/tests/chase_lev_tests.rs` scales `num_thieves` via `available_parallelism()` clamped to `2..=8`
- [ ] `cargo check` finishes in under 2 seconds without building `dscan-gui`
- [ ] `cargo test` finishes in under 3 seconds and all tests pass
- [ ] `cargo run -- --version` outputs `dscan 0.3.1`
- [ ] `cargo clippy --all-targets -- -D warnings` exits 0 with zero warnings
- [ ] `cargo fmt --check` exits 0
- [ ] No files outside the in-scope list are modified (`git status`)

## STOP conditions
Stop and report back (do not improvise) if:
- Root `Cargo.toml` or `crates/dscan-gui/src-tauri/Cargo.toml` has drifted and fails the drift check.
- `cargo check` fails or produces dependency resolution errors after setting `default-members`.
- `cargo run -- <args>` fails to resolve `default-run = "dscan"`.
- Any Chase-Lev test fails assertions (`missing item {i}` or length mismatch).
- The task appears to require modifying source code under `crates/dscan-core/src/` or `crates/dscan-cli/src/`.

## Maintenance notes
- If a new utility crate or benchmark crate is added to `members` in root `Cargo.toml`, only add it to `default-members` if it does not introduce heavy GUI/C dependencies.
- If unit tests are later added to GUI logic, prefer extracting pure data transformations (such as color calculations or cushion shading algorithms) into `dscan-core` to keep the GUI crate test-free and avoid 1.1GB test link times.
- `debug = 1` preserves source line tables for panic backtraces. If full DWARF variable inspection is ever needed for local GDB/LLDB sessions, run with `RUSTFLAGS="-C debuginfo=2"` rather than modifying repository profiles.
