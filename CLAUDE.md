# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

PlotRedox is a native desktop plot digitizer application (v0.2.3) written in Rust using egui. It extracts data coordinates from plot images through manual point clicking, automated mask-based recognition, and exports to CSV. It includes a built-in Rhai scripting IDE for post-processing. Supports English and Chinese (Simplified) UI.

## Build Commands

- **Build & run (debug):** `cargo run`
- **Build & run (release, optimized):** `cargo run --release`
- **Build only:** `cargo build --release`
- **Run tests:** `cargo test`
- **Run specific test:** `cargo test <test_name>` (e.g., `cargo test recognition::axis::tests`)
- **Check without building:** `cargo check`
- **Lint:** `cargo clippy`

## Architecture

**Action-based state machine pattern:** UI emits `Action` variants → main loop dispatches to `action_handler::handle()` → state mutations on `AppState`. This drives undo/redo (stack-based snapshots) and keeps UI rendering pure.

### Key modules

- **`main.rs`** — eframe app lifecycle, font loading (including CJK subset), project save/load, async mask/grid-removal result polling
- **`action.rs`** / **`action_handler.rs`** — Action enum (60+ variants) and dispatcher. All state changes flow through here.
- **`state.rs`** — `AppState` (runtime) and `ProjectData` (serializable). Core data types: `CalibPoint`, `DataPoint`, `PointGroup`, `MaskState`, `GridRemovalState`
- **`core.rs`** — Calibration math: maps image pixel coordinates to logical plot coordinates (linear & logarithmic)
- **`project.rs`** — .prdx file format (ZIP containing `data.json` + `image.png`)
- **`i18n.rs`** — Internationalization: `t(lang, key)` returns translated strings. Supports `Lang::En` and `Lang::Zh`
- **`icons.rs`** — Nerd Font icon constants

### UI layer (`src/ui/`)

- `mod.rs` — orchestrates all panels, handles global shortcuts (Ctrl+S, Ctrl+Z, drag-drop)
- `top_panel.rs` — menu bar (File, Edit, IDE menus)
- `canvas/` — image viewport with pan/zoom, point rendering, mouse/keyboard interaction
- `panel.rs` — left sidebar (calibration points, groups, data table)
- `toolbar.rs` — canvas mode selection (Select/AddData/Delete/AxisMask/DataMask/Grid/Pan/Center)
- `sub_toolbar.rs` — context-sensitive toolbar for mask tools (brush/eraser, size slider, visibility) and recognition results (axis detection, data color groups, grid removal strength)
- `modals.rs` — dialog windows (unsaved changes, clear data, about, clipboard error)

### IDE (`src/ide/`)

Built-in script editor with multiple panels:
- `mod.rs` — IDE layout orchestrator
- `editor.rs` — code editor with syntax highlighting
- `workspace.rs` — post-execution variable inspector
- `inspector.rs` — floating table windows for arrays/maps
- `help.rs` — interactive scripting reference (loads from `docs/scripting_help.md` or `scripting_help_zh.md`)
- `presets.rs` — script template menu + import/export

### Recognition engine (`src/recognition/`)

Mask-based automatic detection running on background threads via mpsc channels:
- `axis/` — axis line detection (directional line fitting), tick mark extraction, morphological preprocessing
- `data/` — color clustering, curve/scatter detection, arc-length point sampling
- `mask/` — mask painting UI, GPU-cached overlay rendering, hover contour highlights
- `grid_removal.rs` — spatial median-profile grid line removal with adjustable strength
- `geometry.rs`, `pixels.rs`, `spatial.rs` — shared helpers

### Scripting (`src/script/`)

Embedded Rhai scripting engine with math helpers (linreg, polyfit, lstsq, trig, stats). Scripts in `example_scripts/` are auto-discovered by `build.rs` and compiled into the binary.

### Build system

- **`build.rs`** — orchestrates build tasks: script embedding, font subsetting, Windows icon compilation
- **`build_font_subset.rs`** — extracts CJK characters from `src/i18n.rs` and `docs/scripting_help_zh.md`, subsets Sarasa UI SC font to reduce binary size
