# Plan 023: CLI Machine-Readable JSON Output and Extension Breakdown
> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat 9a8fb99..HEAD -- crates/dscan-cli/src/cli.rs crates/dscan-cli/src/main.rs crates/dscan-cli/src/ui.rs crates/dscan-cli/src/lib.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status
- **Priority**: P2
- **Effort**: S
- **Risk**: LOW
- **Depends on**: plans/019-fix-workspace-build-and-test-performance-baseline.md
- **Category**: direction
- **Planned at**: commit `9a8fb99`, 2026-10-06
- **Issue**: 

## Why this matters
`dscan` currently outputs scan results exclusively as human-targeted ANSI terminal text containing colors, Unicode box-drawing characters, and progress spinners. It cannot be consumed by automation scripts, piped into `jq`, ingested by CI/CD disk monitoring jobs, or used by headless server monitoring agents. Furthermore, while the core scanning engine (`dscan-core`) already implements high-performance thread-local file extension tracking (`collect_ext_stats`), the CLI hardcodes `collect_ext_stats = false` in `crates/dscan-cli/src/cli.rs:169` and provides neither a flag to enable it nor a UI table to display extension space distribution.

Adding `--json` provides machine-readable output with zero extra dependencies, and adding `--ext` exposes the existing extension breakdown in both ANSI terminal reports and structured JSON exports.

## Current state
- `crates/dscan-cli/src/cli.rs`:
  - `CliOptions` (lines 4–12): Contains flags for target path, excludes, top limit, max depth, threads, symlinks, and cross-filesystem boundaries. Missing `json` and `ext` flags.
  - `to_scan_options` (lines 160–171): Hardcodes `collect_ext_stats: false`.
  - Argument parsing loop (lines 50–130): Implements naive index increments without validating whether argument values exist or if next tokens are flags (e.g. `--top --threads 4` swallows `--threads` into `--top`), and allows unbounded `--threads` values (`.max(1)` without an upper ceiling).
- `crates/dscan-cli/src/main.rs`:
  - Unconditionally calls `print_header` (line 17) and renders spinner lines to stdout (lines 23–26), contaminating stdout and preventing piped stream consumers.
  - Unconditionally invokes `ui::render_report` (line 31).
- `crates/dscan-cli/src/ui.rs`:
  - `render_report` (lines 216–313): Formats ANSI summary cards, top directories, and top files, but lacks an extension breakdown table.
- `crates/dscan-core/src/scanner.rs`:
  - `ScanOptions` (lines 27–36): Has `pub collect_ext_stats: bool`.
  - `ScanResult` (lines 388–399): Holds `pub elapsed: Duration`, `pub total_bytes: u64`, `pub total_files: u64`, `pub top_dirs: Vec<(PathBuf, u64)>`, `pub top_files: Vec<(u64, PathBuf)>`, `pub extension_stats: std::collections::HashMap<String, (u64, u64)>`.
- `crates/dscan-core/src/snapshot.rs`:
  - `pub fn build_extension_breakdown(ext_stats: &HashMap<String, (u64, u64)>, grand_total: u64, limit: usize) -> Vec<ExtensionStatDto>` (lines 185–229): Already aggregates, sorts by bytes descending, computes percentage, and groups into `[other]`.

### Repo conventions
- **Zero external dependencies in CLI**: `dscan` and `dscan-core` maintain zero third-party dependencies (`Cargo.toml` in `crates/dscan-cli` depends only on `dscan-core`). No `serde` or `serde_json` allowed in CLI. JSON serialization must use standard library formatting and string escaping.
- **Strict linting**: `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` must remain 100% clean.

## Commands you will need
| Purpose | Command | Expected on success |
|-----------|--------------------------|---------------------|
| Check build | `cargo check --all-targets` | exit 0 |
| Run CLI unit tests | `cargo test -p dscan` | exit 0, all pass |
| Run all workspace tests | `cargo test` | exit 0, all pass |
| Check clippy | `cargo clippy -p dscan --all-targets -- -D warnings` | exit 0, no warnings |
| Check formatting | `cargo fmt --check` | exit 0, formatted |
| Verify JSON output | `cargo run -p dscan -- . --json \| jq .` | valid JSON object |

## Suggested executor toolkit
- `jq` CLI tool (installed at `/home/ahoura/.local/bin/jq` or in `PATH`) to validate generated JSON schemas.
- `cargo test -p dscan -- --nocapture` to inspect test output details.

## Scope
**In scope**:
- `crates/dscan-cli/src/cli.rs` (add `--json` and `--ext`, harden parser against flag swallowing, clamp threads to `1..=64`, add CLI tests)
- `crates/dscan-cli/src/json.rs` (new file: zero-dependency JSON serializer for `ScanResult` and string escaping with unit tests)
- `crates/dscan-cli/src/lib.rs` (export `pub mod json;`)
- `crates/dscan-cli/src/ui.rs` (add optional top 10 extensions breakdown table in `render_report`)
- `crates/dscan-cli/src/main.rs` (branch on `options.json`: suppress banner/spinner, output clean JSON to stdout, exit code on error)

**Out of scope**:
- `crates/dscan-core/src/*` (traversal engine, syscalls, work-stealing, and snapshot DTOs are already implemented and working; do not touch).
- `crates/dscan-gui/*` (GUI desktop app uses Tauri/serde; do not modify).
- Adding `serde`, `serde_json`, or any other crate to `crates/dscan-cli/Cargo.toml`.

## Git workflow
- Branch: `advisor/023-cli-json-output-and-ext-stats`
- Commit per logical step; message style: conventional commits (e.g. `feat(cli): add --json output flag and zero-dependency serializer`, `feat(cli): add --ext flag and extension breakdown report`, `fix(cli): clamp threads to 64 and guard parser against flag swallowing`).
- Do NOT push or open a PR unless instructed by the operator.

---

## Steps

### Step 1: Add `--json` and `--ext` flags to `CliOptions` in `crates/dscan-cli/src/cli.rs`

1. Update `CliOptions` struct:
   ```rust
   #[derive(Debug, Clone)]
   pub struct CliOptions {
       pub target_path: String,
       pub excludes: Vec<String>,
       pub top_limit: usize,
       pub max_depth: usize,
       pub threads: usize,
       pub follow_symlinks: bool,
       pub cross_filesystems: bool,
       pub json: bool,
       pub ext: bool,
   }
   ```

2. In `to_scan_options(&self)`:
   Enable extension stats when either `self.ext` or `self.json` is requested:
   ```rust
   pub fn to_scan_options(&self) -> dscan_core::ScanOptions {
       dscan_core::ScanOptions {
           target_path: self.target_path.clone(),
           excludes: self.excludes.clone(),
           top_limit: self.top_limit,
           max_depth: self.max_depth,
           threads: self.threads,
           follow_symlinks: self.follow_symlinks,
           cross_filesystems: self.cross_filesystems,
           collect_ext_stats: self.ext || self.json,
       }
   }
   ```

3. Update help output (`-h` / `--help`) in `cli.rs`:
   Add descriptions for the new flags:
   ```rust
   println!("  --json                Output scan results as machine-readable JSON");
   println!("  --ext, --extensions   Display breakdown of top file extensions");
   ```

**Verify**: `cargo check -p dscan` → exits with compiler errors in `main.rs` due to missing struct fields (expected until Step 4/5).

---

### Step 2: Fix argument parser edge cases and add flags in `crates/dscan-cli/src/cli.rs`

1. Address flag swallowing and integer validation:
   Define a local parsing helper to safely consume an argument value only when it does not start with `-`:
   ```rust
   fn parse_val<'a>(args: &'a [String], i: &mut usize) -> Option<&'a str> {
       if *i + 1 < args.len() && !args[*i + 1].starts_with('-') {
           *i += 1;
           Some(&args[*i])
       } else {
           None
       }
   }
   ```

2. Parse flags safely:
   - `--exclude`:
     ```rust
     "--exclude" => {
         if let Some(val) = parse_val(args, &mut i) {
             custom_excludes.push(val.to_string());
         }
         i += 1;
     }
     ```
   - `--top`:
     ```rust
     "--top" => {
         if let Some(val) = parse_val(args, &mut i) {
             if let Ok(n) = val.parse::<usize>() {
                 top_limit = n.max(1);
             }
         }
         i += 1;
     }
     ```
   - `--depth`:
     ```rust
     "--depth" => {
         if let Some(val) = parse_val(args, &mut i) {
             if let Ok(d) = val.parse::<usize>() {
                 max_depth = if d == 0 { usize::MAX } else { d };
             }
         }
         i += 1;
     }
     ```
   - `--threads` / `-j`: Clamp worker threads between `1` and `64`:
     ```rust
     "--threads" | "-j" => {
         if let Some(val) = parse_val(args, &mut i) {
             if let Ok(n) = val.parse::<usize>() {
                 threads = n.clamp(1, 64);
             }
         }
         i += 1;
     }
     ```
   - `--json`:
     ```rust
     "--json" => {
         json = true;
         i += 1;
     }
     ```
   - `--ext` / `--extensions`:
     ```rust
     "--ext" | "--extensions" => {
         ext = true;
         i += 1;
     }
     ```

3. Add unit tests in `cli.rs`:
   - `test_cli_json_and_ext_flags`: verify `--json` and `--ext` set corresponding booleans to `true`.
   - `test_cli_thread_clamping`: verify `--threads 0` or negative clamps to `1`, and `--threads 128` clamps to `64`.
   - `test_cli_flag_swallowing_guard`: verify `dscan --top --threads 4` does not treat `--threads` as `--top`'s argument, keeping `--top` at 25 and setting `threads` to 4.

**Verify**: `cargo test -p dscan -- cli::tests` → all pass.

---

### Step 3: Implement zero-dependency JSON serializer in `crates/dscan-cli/src/json.rs`

1. Create `crates/dscan-cli/src/json.rs`.
2. Implement RFC 8259 compliant string escaping:
   ```rust
   use std::fmt::Write;
   use dscan_core::ScanResult;
   use dscan_core::snapshot::build_extension_breakdown;

   pub fn escape_json_str(s: &str, out: &mut String) {
       out.reserve(s.len());
       for c in s.chars() {
           match c {
               '"' => out.push_str("\\\""),
               '\\' => out.push_str("\\\\"),
               '\x08' => out.push_str("\\b"),
               '\x0C' => out.push_str("\\f"),
               '\n' => out.push_str("\\n"),
               '\r' => out.push_str("\\r"),
               '\t' => out.push_str("\\t"),
               c if (c as u32) < 0x20 => {
                   let _ = write!(out, "\\u{:04x}", c as u32);
               }
               c => out.push(c),
           }
       }
   }
   ```

3. Implement `serialize_scan_result`:
   Output schema matching:
   ```json
   {
     "target_path": ".",
     "total_bytes": 1024000,
     "total_files": 128,
     "duration_ms": 142,
     "top_dirs": [
       { "path": "target", "bytes": 512000 }
     ],
     "top_files": [
       { "path": "target/lib.a", "bytes": 256000 }
     ],
     "extension_stats": [
       { "extension": ".rs", "total_bytes": 128000, "file_count": 32, "percentage": 12.50 }
     ]
   }
   ```
   Implementation:
   ```rust
   pub fn serialize_scan_result(target_path: &str, result: &ScanResult) -> String {
       let mut out = String::with_capacity(4096);
       out.push_str("{\n  \"target_path\": \"");
       escape_json_str(target_path, &mut out);
       let _ = write!(
           out,
           "\",\n  \"total_bytes\": {},\n  \"total_files\": {},\n  \"duration_ms\": {},\n  \"top_dirs\": [",
           result.total_bytes,
           result.total_files,
           result.elapsed.as_millis()
       );

       for (i, (path, bytes)) in result.top_dirs.iter().enumerate() {
           if i > 0 {
               out.push(',');
           }
           out.push_str("\n    {\n      \"path\": \"");
           escape_json_str(&path.display().to_string(), &mut out);
           let _ = write!(out, "\",\n      \"bytes\": {}\n    }}", bytes);
       }
       if !result.top_dirs.is_empty() {
           out.push_str("\n  ");
       }
       out.push_str("],\n  \"top_files\": [");

       for (i, (bytes, path)) in result.top_files.iter().enumerate() {
           if i > 0 {
               out.push(',');
           }
           out.push_str("\n    {\n      \"path\": \"");
           escape_json_str(&path.display().to_string(), &mut out);
           let _ = write!(out, "\",\n      \"bytes\": {}\n    }}", bytes);
       }
       if !result.top_files.is_empty() {
           out.push_str("\n  ");
       }
       out.push_str("],\n  \"extension_stats\": [");

       // Collect all extensions without truncation for JSON export (limit = 0)
       let ext_list = build_extension_breakdown(&result.extension_stats, result.total_bytes, 0);
       for (i, item) in ext_list.iter().enumerate() {
           if i > 0 {
               out.push(',');
           }
           out.push_str("\n    {\n      \"extension\": \"");
           escape_json_str(&item.extension, &mut out);
           let _ = write!(
               out,
               "\",\n      \"total_bytes\": {},\n      \"file_count\": {},\n      \"percentage\": {:.2}\n    }}",
               item.total_bytes,
               item.file_count,
               item.percentage_of_total
           );
       }
       if !ext_list.is_empty() {
           out.push_str("\n  ");
       }
       out.push_str("]\n}\n");

       out
   }
   ```

4. Register module in `crates/dscan-cli/src/lib.rs`:
   ```rust
   pub mod cli;
   pub mod json;
   pub mod ui;

   pub use cli::CliOptions;
   ```

5. Add unit tests in `crates/dscan-cli/src/json.rs`:
   - `test_escape_json_special_characters`: verifies quotes, backslashes, newlines, and control chars are escaped.
   - `test_serialize_empty_scan_result`: verifies empty lists produce valid JSON array brackets `[]`.
   - `test_serialize_scan_result_fields`: validates JSON structure and numeric fields.

**Verify**: `cargo test -p dscan -- json::tests` → all pass.

---

### Step 4: Add extension breakdown table to terminal UI in `crates/dscan-cli/src/ui.rs`

1. Update `render_report` function signature:
   ```rust
   pub fn render_report(result: &ScanResult, show_extensions: bool)
   ```
2. In `render_report`, if `show_extensions` is true, render the top 10 extensions:
   ```rust
   if show_extensions {
       println!("{C_BOLD}{C_YELLOW}📊 Top File Extensions By Size:{C_RESET}");
       println!("{C_DIM}{divider}{C_RESET}");

       let ext_list = dscan_core::snapshot::build_extension_breakdown(&result.extension_stats, result.total_bytes, 10);
       if ext_list.is_empty() {
           println!("  {C_DIM}(no extension data collected){C_RESET}");
       } else {
           for item in &ext_list {
               let bar = make_bar(item.percentage_of_total, bar_width);
               println!(
                   "  {C_BOLD}{C_GREEN}{:>10}{C_RESET} {} {C_BOLD}{C_CYAN}{:<10}{C_RESET} {C_YELLOW}{:>5.1}%{C_RESET} {C_DIM}({:>6} files){C_RESET}",
                   format_bytes(item.total_bytes),
                   bar,
                   item.extension,
                   item.percentage_of_total,
                   format_count(item.file_count)
               );
           }
       }
       println!();
   }
   ```

**Verify**: `cargo check -p dscan` → exits 0.

---

### Step 5: Wire `--json` branching in `crates/dscan-cli/src/main.rs`

1. Update `crates/dscan-cli/src/main.rs`:
   - Only call `print_header` if `!options.json`.
   - If `options.json` is `true`, set `progress_cb = None` so no spinner frames or escape codes are printed to stdout.
   - On `Ok(result)`:
     - If `options.json`: print serialized JSON directly to stdout:
       ```rust
       println!("{}", dscan_cli::json::serialize_scan_result(&options.target_path, &result));
       ```
     - Else: call `ui::clear_spinner_line()` and `ui::render_report(&result, options.ext)`.
   - On `Err(e)`:
     - Clear spinner line if in terminal mode.
     - Print error message to `eprintln!`.
     - Exit with code `1` (`std::process::exit(1);`).

Full `main.rs` structure:
```rust
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use dscan_cli::cli::CliOptions;
use dscan_cli::json;
use dscan_cli::ui::{self, print_header};
use dscan_core::run_scan_with_progress;

fn main() {
    #[cfg(windows)]
    dscan_core::sys::enable_virtual_terminal_processing();

    let options = match CliOptions::parse() {
        Some(opts) => opts,
        None => return,
    };

    if !options.json {
        print_header(&options.target_path, options.threads, &options.excludes);
    }

    let progress_cb = if options.json {
        None
    } else {
        let spinners = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let idx = AtomicUsize::new(0);
        Some(Arc::new(move |bytes, files, workers, path: &str| {
            let i = idx.fetch_add(1, Ordering::Relaxed) % spinners.len();
            ui::render_spinner_line(spinners[i], bytes, files, workers, path);
        }))
    };

    let scan_opts = options.to_scan_options();

    match run_scan_with_progress(&scan_opts, progress_cb) {
        Ok(result) => {
            if options.json {
                print!("{}", json::serialize_scan_result(&options.target_path, &result));
            } else {
                ui::clear_spinner_line();
                ui::render_report(&result, options.ext);
            }
        }
        Err(e) => {
            if !options.json {
                ui::clear_spinner_line();
            }
            eprintln!("\x1b[1m\x1b[31m❌ Error scanning path:\x1b[0m {}", e);
            std::process::exit(1);
        }
    }
}
```

**Verify**:
- `cargo run -p dscan -- . --json | jq .` → parses valid JSON.
- `cargo run -p dscan -- . --ext --top 5` → displays extension breakdown table in terminal report.

---

## Test plan
1. **Unit tests in `crates/dscan-cli/src/cli.rs`**:
   - `test_cli_defaults`: confirms `json = false`, `ext = false`.
   - `test_cli_json_and_ext_flags`: confirms `--json` and `--ext` are parsed.
   - `test_cli_flag_swallowing_guard`: passes `--top --threads 4` and verifies `--top` uses default and `--threads` is 4.
   - `test_cli_thread_clamping`: verifies threads are clamped into `1..=64`.
2. **Unit tests in `crates/dscan-cli/src/json.rs`**:
   - `test_escape_json_special_characters`: verifies quotes, slashes, tabs, newlines, and unicode control points.
   - `test_serialize_empty_scan_result`: verifies serialization of empty directories/files lists produces valid JSON syntax without trailing commas.
   - `test_serialize_scan_result_full`: verifies all fields (`target_path`, `total_bytes`, `total_files`, `duration_ms`, `top_dirs`, `top_files`, `extension_stats`) are present and valid.
3. **End-to-End Piping Verification**:
   - Validate with `jq`: `cargo run -p dscan -- . --json | jq -e '.total_bytes >= 0 and .total_files >= 0' > /dev/null` exits 0.
   - Inspect top extensions in JSON: `cargo run -p dscan -- . --json | jq '.extension_stats[0]'`.
   - Run terminal UI with extension breakdown: `cargo run -p dscan -- . --ext --top 5`.

## Done criteria
- [ ] `cargo check --all-targets` exits 0.
- [ ] `cargo test -p dscan` exits 0 with all unit tests passing.
- [ ] `cargo clippy -p dscan --all-targets -- -D warnings` exits 0 without warnings.
- [ ] `cargo fmt --check` exits 0.
- [ ] `cargo run -p dscan -- . --json | jq .` outputs valid JSON without any ANSI escape codes or banner text.
- [ ] `cargo run -p dscan -- . --ext` outputs terminal report with `Top File Extensions By Size` table.
- [ ] `dscan --threads 128` clamps thread count to 64.
- [ ] `plans/README.md` status row updated to DONE.

## STOP conditions
- If `dscan_core::ScanResult` does not export `extension_stats` or `elapsed`.
- If JSON output fails `jq` parsing due to unescaped characters in paths.
- If implementing JSON serialization requires adding any external crate to `crates/dscan-cli/Cargo.toml`.

## Maintenance notes
- The minimal JSON serializer deliberately stays in `dscan-cli` rather than pulling `serde` into `dscan-core` or `dscan-cli`.
- If additional fields are added to `ScanResult` in the future (e.g. symlink counts or mount boundary stats), update `json::serialize_scan_result` accordingly.
- Keep the upper thread clamp at 64 to avoid OS thread exhaustion on systems with high core counts unless an explicit thread pool architecture is introduced.
