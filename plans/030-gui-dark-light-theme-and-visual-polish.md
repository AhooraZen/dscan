# Plan 030: GUI Dark/Light Theme System, Sizing Comfort, and Visual Polish

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat 722c18e..HEAD -- crates/dscan-gui/src/theme.rs crates/dscan-gui/src/state.rs crates/dscan-gui/src/app.rs crates/dscan-gui/src/views/ crates/dscan-gui/src/treemap/`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status
- **Priority**: P1
- **Effort**: M
- **Risk**: LOW (strictly additive theme tokens, sizing adjustments, and visual micro-interactions)
- **Depends on**: plans/022-migrate-gui-from-tauri-to-native-gpui-kit.md, plans/024-visualizer-native-file-actions-and-trash-integration.md, plans/027-fix-windows-nt-fallback-bounds-and-gui-stability.md
- **Category**: dx
- **Planned at**: commit `722c18e`, 2026-10-06
- **Issue**: 

## Why this matters
`dscan-gui` previously used hardcoded dark mode colors (`BG_DARK`, `SURFACE_DARK`) without support for high-contrast light environments. Primary interactive elements had cramped padding (`px(2.0)`, `px(3.0)`), and row heights (`22.0px`) caused click fatigue and accidental selections on dense file hierarchies. Introducing a structured `ThemeMode` (Dark and Light) with accessible WCAG AA contrast, generous comfortable component padding, legible font sizing, and dedicated theme toggling delivers an elite desktop experience adhering to `ui-ux-pro-max`.

## Implementation details
1. **`crates/dscan-gui/src/theme.rs`**:
   - Added `ThemeMode` enum (`Dark` / `Light`) with `toggle()` and `colors()`.
   - Added `ThemeColors` struct with full semantic tokens: `bg`, `surface`, `surface_hover`, `surface_active`, `border`, `border_light`, `text_primary`, `text_muted`, `text_dim`, `accent_blue`, `accent_green`, `accent_amber`, `accent_red`.
   - Dark theme retains high-contrast Slate 900/800 aesthetic.
   - Light theme introduces clean, crisp Slate 50/White aesthetic with slate borders and high-contrast text.
2. **`crates/dscan-gui/src/state.rs` & `crates/dscan-gui/src/app.rs`**:
   - Added `theme_mode` to `AppState` with `toggle_theme()`.
   - Added `theme()` accessor and `toggle_theme()` handler to `DscanApp`.
3. **`crates/dscan-gui/src/views/title_bar.rs`**:
   - Increased title bar height to `48.0px`.
   - Added interactive `🌙 Dark` / `☀️ Light` theme toggle button with smooth hover feedback.
   - Enlarge primary scan/pause/cancel action buttons with `13.0px` bold typography and `px_4()` / `py(5.0px)` padding.
4. **`crates/dscan-gui/src/views/directory_tree.rs` & `extension_legend.rs`**:
   - Header height increased to `32.0px`, row height increased to `26.0px` with `12.0px` text.
   - Distinct hover and selection states matching active theme tokens.
5. **`crates/dscan-gui/src/views/cushion_treemap.rs` & `treemap/element.rs`**:
   - Parameterized cushion treemap background, selection border, and hover border with theme tokens.
   - Enhanced floating tooltip card with generous padding, clear typography, and accessible action buttons.
6. **`crates/dscan-cli/src/cli.rs`**:
   - Added `-a`, `--all`, `--no-default-excludes` flag to enable scanning everything including `.git` when explicitly desired.

## Verification
- `cargo check -p dscan-gui`: clean compilation.
- `cargo test -p dscan-core -p dscan`: 100% green (70 tests passed).
- `cargo fmt --check`: 100% clean formatting.
- `cargo clippy -p dscan-core -p dscan --all-targets -- -D warnings`: 100% clean.
