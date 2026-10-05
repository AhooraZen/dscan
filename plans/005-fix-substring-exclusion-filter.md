# Plan 005: Fix Substring Exclusion Path False Positives
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
- **Depends on**: plans/001-test-harness-and-verification-baseline.md
- **Category**: bug
- **Planned at**: commit `initial`, 2026-10-05
- **Issue**: 

## Why this matters
In `src/scanner.rs:67`, `is_excluded_dir` executes an unanchored substring search using `.windows(ex.len()).any(|w| w == ex.as_slice())`. Because default system exclusions are `"/proc"`, `"/sys"`, `"/dev"`, `"/run"`, and `"/tmp"`, any normal user directory whose path happens to contain these substrings (such as `/home/user/development`, `/home/user/rust_runner`, `/home/user/system_backup`, or `/home/user/tmp_files`) is falsely excluded from scans and completely omitted from size statistics.

## Current state
In `src/scanner.rs:62-72`:
```rust
fn is_excluded_dir(path_bytes: &[u8], name_bytes: &[u8], excludes: &[Vec<u8>]) -> bool {
    for ex in excludes {
        if ex.is_empty() {
            continue;
        }
        if name_bytes == ex.as_slice() {
            return true;
        }
        if path_bytes.windows(ex.len()).any(|w| w == ex.as_slice()) {
            return true;
        }
    }
    false
}
```

## Solution
Match exclusions based on path semantics rather than raw string substrings:
1. **Absolute path exclusions** (patterns starting with `/` like `/proc`, `/sys`, `/dev`):
   Match if `path_bytes == ex` OR (`path_bytes.starts_with(ex)` AND `path_bytes[ex.len()] == b'/'`).
   Example: `/dev` matches `/dev` and `/dev/snd`, but does NOT match `/home/user/development`.
2. **Relative / name exclusions** (patterns without leading `/` like `.git`, `node_modules`):
   Match if `name_bytes == ex` OR any directory component in `path_bytes` equals `ex`.
3. Guard against empty patterns `ex.is_empty()`.

## Commands you will need
| Purpose | Command | Expected on success |
|-----------|--------------------------|---------------------|
| Build | `cargo build` | exit 0 |
| Run tests | `cargo test scanner::tests` | exit 0, all pass |

## Scope
**In scope**:
- `src/scanner.rs` (`is_excluded_dir` and unit tests)

**Out of scope**:
- `src/cli.rs`, `src/ui.rs`, `src/sys.rs`

## Git workflow
- Branch: `advisor/005-fix-exclusion-matching`
- Commit message: `fix(scanner): enforce path boundary checks in directory exclusion filter`

## Steps

### Step 1: Update `is_excluded_dir` in `src/scanner.rs`
Replace `is_excluded_dir` with path-boundary-aware matching:
```rust
pub fn is_excluded_dir(path_bytes: &[u8], name_bytes: &[u8], excludes: &[Vec<u8>]) -> bool {
    for ex in excludes {
        if ex.is_empty() {
            continue;
        }
        
        let ex_slice = ex.as_slice();
        
        // Exact name match (e.g. ".git", "target")
        if name_bytes == ex_slice {
            return true;
        }

        // Absolute root prefix match (e.g. "/proc", "/sys", "/dev")
        if ex_slice.starts_with(b"/") {
            if path_bytes == ex_slice {
                return true;
            }
            if path_bytes.starts_with(ex_slice) && path_bytes.get(ex_slice.len()) == Some(&b'/') {
                return true;
            }
        } else {
            // Relative folder match across components: match "/name/" or "/name" at end
            if path_bytes.split(|&b| b == b'/').any(|segment| segment == ex_slice) {
                return true;
            }
        }
    }
    false
}
```

### Step 2: Add comprehensive unit tests in `src/scanner.rs`
In `#[cfg(test)] mod tests` in `src/scanner.rs`:
Test:
1. Exact name exclude: `name = ".git"`, `ex = ".git"` -> `true`
2. Component exclude: `path = "/home/user/project/.git/objects"`, `ex = ".git"` -> `true`
3. Root prefix exclude: `path = "/dev/shm"`, `ex = "/dev"` -> `true`
4. Root prefix exact: `path = "/dev"`, `ex = "/dev"` -> `true`
5. Substring non-match: `path = "/home/user/development/code"`, `ex = "/dev"` -> MUST BE `false`
6. Substring non-match: `path = "/home/user/runner/job"`, `ex = "/run"` -> MUST BE `false`
7. Substring non-match: `path = "/home/user/system_monitor"`, `ex = "/sys"` -> MUST BE `false`
8. Empty exclude list and empty pattern -> `false`

**Verify**: `cargo test scanner::tests` → all pass.

## Test plan
- Run unit tests with positive and negative path boundary cases.
- Verification: `cargo test`

## Done criteria
- [ ] `/home/user/development` is NOT excluded by `/dev`
- [ ] `/dev` and `/dev/snd` ARE excluded by `/dev`
- [ ] `.git` inside any path is excluded
- [ ] `cargo test` exits 0
- [ ] `plans/README.md` status row for 005 updated to DONE

## STOP conditions
- If default exclusions fail to block `/proc`, `/sys`, or `.git`.

## Maintenance notes
- User-supplied patterns with wildcards can be added as a future enhancement; maintain standard prefix and segment equality as baseline.
