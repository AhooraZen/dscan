# Plan 012: Fix Cargo Fmt Drift and Clippy Warnings
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
- **Priority**: P3
- **Effort**: S
- **Risk**: LOW
- **Depends on**: plans/001-test-harness-and-verification-baseline.md
- **Category**: dx
- **Planned at**: commit `initial`, 2026-10-05
- **Issue**: 

## Why this matters
`CLAUDE.md:23-24` defines two mandatory quality gates: `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check`. Currently, both fail out-of-the-box: `cargo fmt --check` fails across 5 files, and `cargo clippy` emits 5 warnings (3 collapsible `if` blocks and 2 `unnecessary_sort_by` warnings). Resolving these warnings restores clean build gate execution for continuous integration.

## Current state
`cargo fmt --check` reports formatting differences in:
- `src/cli.rs:68`
- `src/lib.rs:6`
- `src/scanner.rs:12`
- `src/sys.rs:41`
- `src/ui.rs:1`

`cargo clippy --all-targets` reports 5 warnings in `src/scanner.rs`:
1. `src/scanner.rs:151`: `warning: this if statement can be collapsed`
2. `src/scanner.rs:163`: `warning: this if statement can be collapsed`
3. `src/scanner.rs:197`: `warning: this if statement can be collapsed`
4. `src/scanner.rs:393`: `warning: consider using sort_by_key` (`sorted_dirs.sort_by(|a, b| b.1.cmp(&a.1))`)
5. `src/scanner.rs:399`: `warning: consider using sort_by_key` (`files_vec.sort_by(|a, b| b.0.cmp(&a.0))`)

## Commands you will need
| Purpose | Command | Expected on success |
|-----------|--------------------------|---------------------|
| Run clippy | `cargo clippy --all-targets -- -D warnings` | exit 0, 0 warnings |
| Check formatting | `cargo fmt --check` | exit 0, no diff |
| Auto-format | `cargo fmt` | exit 0 |

## Scope
**In scope**:
- `src/scanner.rs` (address collapsible `if` and `sort_by_key`)
- All files touched by `cargo fmt` (`src/cli.rs`, `src/lib.rs`, `src/scanner.rs`, `src/sys.rs`, `src/ui.rs`)

**Out of scope**:
- Changing runtime behavior or CLI options

## Git workflow
- Branch: `advisor/012-clippy-fmt`
- Commit message: `style: resolve clippy warnings and apply cargo fmt`

## Steps

### Step 1: Address Clippy warnings in `src/scanner.rs`
1. Collapse nested `if` statements at line 163 and line 197:
   ```rust
   } else if let Some(Reverse((min_sz, _))) = local_top_files.peek() && sz > *min_sz {
       local_top_files.pop();
       local_top_files.push(Reverse((sz, item_path)));
   }
   ```
2. Replace manual `sort_by` with `sort_by_key` at lines 393 and 399:
   ```rust
   sorted_dirs.sort_by_key(|a| std::cmp::Reverse(a.1));
   files_vec.sort_by_key(|a| std::cmp::Reverse(a.0));
   ```

**Verify**: `cargo clippy --all-targets -- -D warnings` → passes with 0 warnings.

### Step 2: Run `cargo fmt`
Run `cargo fmt` across the repository.

**Verify**: `cargo fmt --check` → exits 0 with no diff.

### Step 3: Run full verification gate
Run:
```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```
All three commands must exit 0.

## Test plan
- Verify all quality gates pass cleanly without suppressing any lints.
- Verification: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`

## Done criteria
- [ ] `cargo fmt --check` exits 0
- [ ] `cargo clippy --all-targets -- -D warnings` exits 0 with zero warnings
- [ ] `cargo test` exits 0
- [ ] `plans/README.md` status row for 012 updated to DONE

## STOP conditions
- If resolving a clippy warning alters sorting order or changes program behavior.

## Maintenance notes
- Maintain strict zero-warning policy on all future PRs.
