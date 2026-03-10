use crate::core::{CalibPoint, DataPoint};
use crate::script::WorkspaceVar;
use eframe::egui::{Color32, TextureHandle, Vec2};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ── Serializable mirror types (no egui dependency) ─────────────────────

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SerializableGroup {
    pub name: String,
    pub color: [u8; 4],
}

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SerializableIdeState {
    pub is_open: bool,
    pub code: String,
    pub user_scripts: Vec<(String, String)>,
}

/// The on-disk representation of a project.
/// All fields have `#[serde(default)]` so that adding new fields in the future
/// is fully backwards-compatible: old files simply get the default value.
#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ProjectData {
    pub version: u32,
    pub calib_pts: Vec<CalibPoint>,
    pub data_pts: Vec<DataPoint>,
    pub groups: Vec<SerializableGroup>,
    pub active_group_idx: usize,
    pub x1_val: String,
    pub x2_val: String,
    pub y1_val: String,
    pub y2_val: String,
    pub log_x: bool,
    pub log_y: bool,
    pub ide: SerializableIdeState,
}

// ── Runtime types ──────────────────────────────────────────────────────

#[derive(Clone)]
pub struct PointGroup {
    pub name: String,
    pub color: Color32,
}

#[derive(Clone)]
pub struct HistorySnapshot {
    pub calib_pts: Vec<CalibPoint>,
    pub data_pts: Vec<DataPoint>,
    pub groups: Vec<PointGroup>,
    pub active_group_idx: usize,
}

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum AppMode {
    Select,
    AddCalib,
    AddData,
    Delete,
    Pan,
}

/// Actions that are deferred until unsaved-changes confirmation is resolved.
pub enum PendingAction {
    NewProject,
    LoadImage(
        std::path::PathBuf,
        eframe::egui::TextureHandle,
        eframe::egui::Vec2,
    ),
    LoadClipboardImage(
        eframe::egui::TextureHandle,
        eframe::egui::Vec2,
        Vec<u8>,
        u32,
        u32,
    ),
    OpenProject(crate::state::ProjectData, Vec<u8>, std::path::PathBuf),
    CloseApp,
}

pub struct AppState {
    pub mode: AppMode,

    // Image loading
    pub image_path: Option<PathBuf>,
    pub texture: Option<TextureHandle>,
    pub img_size: Vec2,
    pub raw_image_bytes: Option<Vec<u8>>,
    pub clipboard_rgba: Option<(Vec<u8>, u32, u32)>, // For lazily encoding pasted images
    pub pending_image: Option<(PathBuf, TextureHandle, Vec2)>,

    // Viewport transform (Panning & Zooming)
    pub pan: Vec2,
    pub zoom: f32,

    // Points & Groups
    pub calib_pts: Vec<CalibPoint>,
    pub data_pts: Vec<DataPoint>,
    pub groups: Vec<PointGroup>,
    pub active_group_idx: usize,

    // Calibration Settings
    pub x1_val: String,
    pub x2_val: String,
    pub y1_val: String,
    pub y2_val: String,
    pub log_x: bool,
    pub log_y: bool,

    // Interaction state
    pub dragging_calib_idx: Option<usize>,
    pub dragging_data_idx: Option<usize>,
    pub selected_calib_idx: Option<usize>,
    pub selected_data_indices: std::collections::HashSet<usize>,
    pub hovered_calib_idx: Option<usize>,
    pub hovered_data_idx: Option<usize>,

    // Transient Box Select state
    pub box_start: Option<eframe::egui::Pos2>,
    pub center_requested: bool,
    pub pending_clear_data: bool,

    // UI collapse state for group data lists
    pub collapsed_groups: std::collections::HashSet<usize>,

    // Pending action requiring unsaved-changes confirmation
    pub pending_action: Option<PendingAction>,

    // Show "no image in clipboard" modal
    pub show_clipboard_empty: bool,

    // Current project file path (for Ctrl+S save-in-place)
    pub project_path: Option<PathBuf>,

    // Show About dialog
    pub show_about: bool,

    // Project state tracking
    pub dirty: bool,

    // History
    pub undo_stack: Vec<HistorySnapshot>,
    pub redo_stack: Vec<HistorySnapshot>,

    // IDE State
    pub ide: IdeState,
}

#[derive(Clone)]
pub struct IdeState {
    pub is_open: bool,
    pub code: String,
    pub output: String,
    pub workspace_vars: Vec<WorkspaceVar>,
    pub open_inspectors: std::collections::HashSet<String>,
    pub show_help: bool,
    pub user_scripts: Vec<(String, String)>, // (name, code)
    pub output_fraction: f32,
}

impl Default for IdeState {
    fn default() -> Self {
        Self {
            is_open: false,
            code: String::new(),
            output: String::new(),
            workspace_vars: Vec::new(),
            open_inspectors: std::collections::HashSet::new(),
            show_help: false,
            user_scripts: Vec::new(),
            output_fraction: 0.5,
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            mode: AppMode::Select,
            image_path: None,
            texture: None,
            img_size: Vec2::ZERO,
            raw_image_bytes: None,
            clipboard_rgba: None,
            pending_image: None,
            pan: Vec2::ZERO,
            zoom: 1.0,
            calib_pts: Vec::new(),
            data_pts: Vec::new(),
            groups: vec![PointGroup {
                name: "Group 1".to_string(),
                color: Color32::from_rgb(0xd7, 0x30, 0x27), // Palette 1
            }],
            active_group_idx: 0,
            x1_val: "0.0".to_string(),
            x2_val: "10.0".to_string(),
            y1_val: "0.0".to_string(),
            y2_val: "10.0".to_string(),
            log_x: false,
            log_y: false,
            dragging_calib_idx: None,
            dragging_data_idx: None,
            selected_calib_idx: None,
            selected_data_indices: std::collections::HashSet::new(),
            hovered_calib_idx: None,
            hovered_data_idx: None,
            box_start: None,
            center_requested: false,
            pending_clear_data: false,
            collapsed_groups: std::collections::HashSet::new(),
            pending_action: None,
            show_clipboard_empty: false,
            project_path: None,
            show_about: false,
            dirty: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            ide: IdeState::default(),
        }
    }
}

impl AppState {
    /// Returns the project display name (filename with extension or "untitled.prdx").
    pub fn project_name(&self) -> String {
        self.project_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "untitled.prdx".to_string())
    }

    /// Returns the window title: "PlotRedox — name" with "*" if dirty.
    pub fn window_title(&self) -> String {
        let name = self.project_name();
        if self.dirty {
            format!("PlotRedox — {}*", name)
        } else {
            format!("PlotRedox — {}", name)
        }
    }

    pub fn reset_extraction_data(&mut self) {
        // Clear all data points
        self.data_pts.clear();

        // Reset groups to default single group
        self.groups = vec![PointGroup {
            name: "Group 1".to_string(),
            color: Color32::from_rgb(0xd7, 0x30, 0x27),
        }];
        self.active_group_idx = 0;

        // Clear data interaction state
        self.selected_data_indices.clear();
        self.dragging_data_idx = None;
        self.hovered_data_idx = None;
        self.box_start = None;
        self.pending_clear_data = false;

        // Clear history
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    pub fn reset_calibration_data(&mut self) {
        self.calib_pts.clear();

        // Reset calibration axis values
        self.x1_val = "0.0".to_string();
        self.x2_val = "10.0".to_string();
        self.y1_val = "0.0".to_string();
        self.y2_val = "10.0".to_string();
        self.log_x = false;
        self.log_y = false;

        // Clear calib interaction state
        self.selected_calib_idx = None;
        self.dragging_calib_idx = None;
        self.hovered_calib_idx = None;
    }

    pub fn save_snapshot(&mut self) {
        let snapshot = HistorySnapshot {
            calib_pts: self.calib_pts.clone(),
            data_pts: self.data_pts.clone(),
            groups: self.groups.clone(),
            active_group_idx: self.active_group_idx,
        };
        self.undo_stack.push(snapshot);
        if self.undo_stack.len() > 50 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    pub fn update(&mut self, action: crate::action::Action) {
        crate::action_handler::handle(self, action);
    }
}
