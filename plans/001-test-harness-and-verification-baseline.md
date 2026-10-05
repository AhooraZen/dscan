# Plan 001: Establish Test Harness and Verification Baseline
> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: Compare the "Current state" excerpts against
> the live code before proceeding; on a mismatch, treat it as a STOP condition.

## Status
- **Priority**: P1
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: tests
- **Planned at**: commit `initial`, 2026-10-05
- **Issue**: 

## Why this matters
Currently, `cargo test` executes 0 tests across `src/lib.rs` and `src/main.rs`. There is zero automated verification for pure formatting functions, argument parsing, exclusion logic, or disk traversal. Any future bug fix, optimization, or refactor risks breaking core functionality with zero warning. Establishing a fast, self-contained unit and tempdir integration test suite creates a reliable safety net for all subsequent improvements.

## Current state
- `Cargo.toml:1-27` defines `[lib]` and `[[bin]]` with no test dependencies.
- `src/format.rs:3-20` contains `format_bytes` with zero test coverage.
- `src/cli.rs:13-96` contains `CliOptions::parse` which reads directly from `std::env::args()`, preventing direct in-process unit testing with custom arguments.
- `src/scanner.rs:62-72` contains `is_excluded_dir` with zero test coverage.
- `src/scanner.rs:303-413` contains `run_scan`, which has no end-to-end integration tests on synthetic directory hierarchies.

Exemplar test convention: Rust standard `#[cfg(test)] mod tests` with `use super::*;` and standard `assert_eq!`. No external testing crates required.

## Commands you will need
| Purpose | Command | Expected on success |
|-----------|--------------------------|---------------------|
| Build | `cargo build` | exit 0 |
| Run all tests | `cargo test` | exit 0, all passed |
| Run single test | `cargo test test_format_bytes` | exit 0, passed |
| Run integration test | `cargo test --test scanner_integration` | exit 0, passed |

## Suggested executor toolkit
- `rust-dev` skill for idiomatic Rust test design.

## Scope
**In scope**:
- `src/format.rs` (add unit tests module)
- `src/cli.rs` (extract `parse_from<I>(args: I)` and add unit tests)
- `src/scanner.rs` (add unit tests for exclusion logic)
- `tests/scanner_integration.rs` (new integration test file using `std::env::temp_dir()`)

**Out of scope**:
- Modifying traversal algorithm, work-stealing queue, or UI spinner rendering.
- Adding third-party testing crates (e.g. `tempfile`, `pretty_assertions`). Use Rust standard library only.

## Git workflow
- Branch: `advisor/001-test-baseline`
- Commit message: `test: establish test suite and verification baseline`

## Steps

### Step 1: Add unit tests for `format_bytes` in `src/format.rs`
Add `#[cfg(test)] mod tests` at the bottom of `src/format.rs`:
Cover:
- Zero bytes: `format_bytes(0)` -> `"0 B"`
- Boundary under 1 KiB: `format_bytes(1023)` -> `"1023 B"`
- Exact 1 KiB: `format_bytes(1024)` -> `"1.00 KiB"`
- Fractions: `format_bytes(1536)` -> `"1.50 KiB"`
- Megabytes, Gigabytes, Terabytes: `1024 * 1024`, `1024 * 1024 * 1024`, `1024 * 1024 * 1024 * 1024`
- Petabytes and large numbers: `u64::MAX`

**Verify**: `cargo test test_format_bytes` → `test format::tests::test_format_bytes ... ok`

### Step 2: Decouple `CliOptions` argument parsing and add unit tests in `src/cli.rs`
1. In `src/cli.rs`, extract the parsing logic from `parse()` into a helper:
   ```rust
   pub fn parse() -> Option<Self> {
       let args: Vec<String> = std::env::args().collect();
       Self::parse_from_args(&args)
   }

   pub fn parse_from_args(args: &[String]) -> Option<Self> {
       // Existing parsing loop logic operating on args slice
   }
   ```
2. Add `#[cfg(test)] mod tests` in `src/cli.rs`:
   - Default arguments: `["dscan"]` -> target `"."`, top 25, depth 3, threads 32
   - Target path provided: `["dscan", "/var/log"]` -> target `"/var/log"`
   - `--top 10`: top 10
   - `--depth 5`: max_depth 5
   - `--threads 8` and `-j 4`: threads 8 and 4
   - `--exclude pattern`: appends to excludes

**Verify**: `cargo test cli::tests` → all tests pass.

### Step 3: Add unit tests for `is_excluded_dir` in `src/scanner.rs`
Add `#[cfg(test)] mod tests` in `src/scanner.rs`:
- Exact directory name matches: `name_bytes == b".git"`, `excludes = [b".git".to_vec()]` -> `true`
- Non-matching names -> `false`
- Empty exclude list -> `false`

**Verify**: `cargo test scanner::tests` → all tests pass.

### Step 4: Create tempdir integration test in `tests/scanner_integration.rs`
Create `tests/scanner_integration.rs`:
1. Creates a unique temporary directory under `std::env::temp_dir()`.
2. Creates known structure:
   - `test_dir/file1.txt` (1024 bytes)
   - `test_dir/subdir/file2.txt` (2048 bytes)
   - `test_dir/.git/ignored.txt` (4096 bytes)
3. Invokes `dscan::run_scan(&options)` targeting `test_dir`.
4. Asserts:
   - `result.total_files >= 2`
   - `.git` contents are excluded
   - `result.top_files` contains `file2.txt` with size >= 2048
5. Cleans up temporary directory on completion (or in RAII drop helper).

**Verify**: `cargo test --test scanner_integration` → test passes.

## Test plan
- Run `cargo test`: Ensure all unit and integration tests run and pass without root privileges.
- Verification command: `cargo test`

## Done criteria
- [ ] `cargo test` exits 0 with at least 5 passing unit and integration tests
- [ ] `cargo check` exits 0
- [ ] No external dependencies added to `Cargo.toml`
- [ ] `plans/README.md` status row for 001 updated to DONE

## STOP conditions
- If synthetic test creation fails due to filesystem permission errors in `std::env::temp_dir()`.
- If modifying `CliOptions::parse` breaks the public signature or binary entry in `src/main.rs`.

## Maintenance notes
- Integration tests must always clean up temporary files to avoid leaking disk space during CI runs.
