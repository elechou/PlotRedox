# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

PlotRedox is a native desktop plot digitizer application written in Rust using egui. It extracts data coordinates from plot images through manual point clicking, automated mask-based recognition, and exports to CSV. It includes a built-in Rhai scripting IDE for post-processing.

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

- **`main.rs`** — eframe app lifecycle, project save/load, async mask result polling
- **`action.rs`** / **`action_handler.rs`** — Action enum (60+ variants) and dispatcher. All state changes flow through here.
- **`state.rs`** — `AppState` (runtime) and `ProjectData` (serializable). Core data types: `CalibPoint`, `DataPoint`, `PointGroup`, `MaskState`
- **`core.rs`** — Calibration math: maps image pixel coordinates to logical plot coordinates
- **`project.rs`** — .prdx file format (ZIP containing `data.json` + `image.png`)

### UI layer (`src/ui/`)

- `mod.rs` — orchestrates all panels, handles global shortcuts (Ctrl+S, Ctrl+Z, drag-drop)
- `canvas/` — image viewport with pan/zoom, point rendering, mouse/keyboard interaction
- `panel.rs` — left sidebar (calibration points, groups, data table)
- `toolbar.rs` — canvas mode selection (Select/AddData/Pan/Delete/AxisMask/DataMask)
- `modals.rs` — dialog windows

### Recognition engine (`src/recognition/`)

Mask-based automatic detection running on background threads via mpsc channels:
- `axis/` — axis line detection (Hough transform), tick mark extraction, morphological preprocessing
- `data/` — color clustering, curve/scatter detection, point sampling
- `mask/` — mask painting UI and detection result display

### Scripting (`src/script/`)

Embedded Rhai scripting engine with math helpers (linreg, polyfit, lstsq). Scripts in `example_scripts/` are auto-discovered by `build.rs` and compiled into the binary.

### Build script (`build.rs`)

Auto-discovers `.rhai` files in `example_scripts/`, generates `embedded_scripts.rs` at compile time, and compiles Windows icon resources.
