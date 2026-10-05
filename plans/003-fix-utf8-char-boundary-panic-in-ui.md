# Plan 003: Fix Non-ASCII UTF-8 Slice Panic in UI Spinner
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
In `src/ui.rs:90`, path truncation computes a raw byte slice index:
`&current_path[current_path.len() - (max_path_len - 3)..]`
If the path contains non-ASCII multi-byte UTF-8 characters (e.g. Persian, Arabic, Cyrillic, Chinese, Japanese, or emoji) and the truncation boundary falls in the middle of a multi-byte sequence, Rust panics with `byte index ... is not a char boundary`. Because `Cargo.toml` sets `panic = "abort"` in release mode, scanning any directory containing non-ASCII filenames causes an immediate hard process crash.

## Current state
In `src/ui.rs:88-94`:
```rust
        let max_path_len = term_width.saturating_sub(48).max(10);
        let truncated = if current_path.len() > max_path_len {
            format!("...{}", &current_path[current_path.len() - (max_path_len - 3)..])
        } else {
            current_path.to_string()
        };
```

## Commands you will need
| Purpose | Command | Expected on success |
|-----------|--------------------------|---------------------|
| Build | `cargo build` | exit 0 |
| Run tests | `cargo test ui::tests` | exit 0, all pass |

## Scope
**In scope**:
- `src/ui.rs` (implement safe string truncation and add unit tests)

**Out of scope**:
- Terminal width detection, spinner character array, `src/scanner.rs`

## Git workflow
- Branch: `advisor/003-fix-utf8-panic`
- Commit message: `fix(ui): safely truncate path strings on UTF-8 character boundaries`

## Steps

### Step 1: Extract and implement safe UTF-8 truncation helper in `src/ui.rs`
Define a helper function:
```rust
pub fn truncate_path_tail(path: &str, max_len: usize) -> String {
    if path.len() <= max_len || max_len < 4 {
        return path.to_string();
    }
    let target_bytes = max_len - 3;
    let start_idx = path.len().saturating_sub(target_bytes);
    
    // Find nearest valid UTF-8 char boundary >= start_idx
    let safe_start = (start_idx..=path.len())
        .find(|&i| path.is_char_boundary(i))
        .unwrap_or(path.len());
        
    format!("...{}", &path[safe_start..])
}
```
Use this helper inside `render_spinner_line`:
```rust
let max_path_len = term_width.saturating_sub(48).max(10);
let truncated = truncate_path_tail(current_path, max_path_len);
```

### Step 2: Add unit tests in `src/ui.rs`
Add `#[cfg(test)] mod tests` in `src/ui.rs`:
Test:
1. ASCII path under limit: `truncate_path_tail("short/path", 20)` -> `"short/path"`
2. ASCII path over limit: `truncate_path_tail("a/very/long/path/to/file.txt", 15)` -> `"...h/to/file.txt"`
3. Multi-byte Persian/Arabic characters: `truncate_path_tail("پروژه‌های_من/دایرکتوری/تست_فایل.txt", 20)` — must NOT panic, must return valid UTF-8 string with `...` prefix.
4. CJK characters: `truncate_path_tail("文件夹/子目录/超长大文件测试.tar.gz", 16)` — must NOT panic.
5. Multi-byte emoji: `truncate_path_tail("photos/🚀_launch/🌌_nebula.jpg", 18)` — must NOT panic.

**Verify**: `cargo test ui::tests` → all pass.

## Test plan
- Unit tests covering ASCII, multi-byte UTF-8, and emoji path truncation without panics.
- Verification: `cargo test`

## Done criteria
- [ ] No raw byte slicing of `current_path` without character boundary validation
- [ ] Unit tests for `truncate_path_tail` pass with non-ASCII and emoji strings
- [ ] `cargo test` exits 0
- [ ] `plans/README.md` status row for 003 updated to DONE

## STOP conditions
- If character boundary search produces an empty or invalid UTF-8 string.

## Maintenance notes
- Keep terminal column alignment intact: multi-byte characters may have wide display widths (e.g. CJK 2 columns), but ensuring valid UTF-8 byte boundaries prevents process aborts.
