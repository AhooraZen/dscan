# Plan 011: Add .gitignore for Build Artifacts
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
- **Depends on**: none
- **Category**: dx
- **Planned at**: commit `initial`, 2026-10-05
- **Issue**: 

## Why this matters
The repository lacks a `.gitignore` file. Running `cargo build` or `cargo test` creates a `target/` directory containing gigabytes of intermediate build artifacts and compiled binaries. `git status` reports `target/` as untracked, creating repository clutter and risking accidental commit of multi-gigabyte build artifacts.

## Current state
`git status` in the repository root lists:
```
Untracked files:
  (use "git add <file>..." to include in what will be committed)
	...
	target/
```
No `.gitignore` file exists at `/home/ahoura/dscan/.gitignore`.

## Commands you will need
| Purpose | Command | Expected on success |
|-----------|--------------------------|---------------------|
| Check git status | `git status --short` | `target/` no longer appears |

## Scope
**In scope**:
- `.gitignore` (create in repository root)

**Out of scope**:
- Any source file in `src/` or `Cargo.toml`

## Git workflow
- Branch: `advisor/011-add-gitignore`
- Commit message: `chore: add .gitignore for Cargo build artifacts`

## Steps

### Step 1: Create `.gitignore` in repository root
Write `.gitignore` at `/home/ahoura/dscan/.gitignore`:
```gitignore
/target
**/*.rs.bk
*.swp
*.bak
.DS_Store
```

### Step 2: Verify `git status`
Run `git status --short`.
Ensure `target/` is ignored and does not appear in untracked files.

**Verify**: `git status --short | grep target` → returns exit 1 (no output).

## Test plan
- Verify `git status` does not list `target/`.
- Verification: `git status`

## Done criteria
- [ ] `.gitignore` exists at repository root
- [ ] `target/` is ignored by git
- [ ] `plans/README.md` status row for 011 updated to DONE

## STOP conditions
- If `.gitignore` ignores tracked source files or documentation.

## Maintenance notes
- Cargo standard convention is `/target` at workspace root.
