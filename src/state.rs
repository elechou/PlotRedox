use crate::core::{CalibPoint, DataPoint};
use eframe::egui::{Color32, TextureHandle, Vec2};
use std::path::PathBuf;

#[derive(Clone)]
pub struct PointGroup {
    pub name: String,
    pub color: Color32,
}

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum AppMode {
    Select,
    AddCalib,
    AddData,
    Delete,
    Pan,
}

pub struct AppState {
    pub mode: AppMode,

    // Image loading
    pub image_path: Option<PathBuf>,
    pub texture: Option<TextureHandle>,
    pub img_size: Vec2,

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
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            mode: AppMode::Select,
            image_path: None,
            texture: None,
            img_size: Vec2::ZERO,
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
        }
    }
}

impl AppState {
    pub fn update(&mut self, action: crate::action::Action) {
        use crate::action::Action;
        match action {
            Action::MovePointsToGroup {
                indices,
                new_group_id,
            } => {
                let mut moved_pts = Vec::new();
                for &pt_idx in &indices {
                    if pt_idx < self.data_pts.len() {
                        let mut pt = self.data_pts[pt_idx].clone();
                        pt.group_id = new_group_id;
                        moved_pts.push(pt);
                    }
                }

                let mut to_remove = indices.clone();
                to_remove.sort_unstable_by(|a, b| b.cmp(a));
                for &idx in &to_remove {
                    if idx < self.data_pts.len() {
                        self.data_pts.remove(idx);
                    }
                }

                self.data_pts.extend(moved_pts);

                self.selected_data_indices.clear();
                let new_len = self.data_pts.len();
                for i in 0..indices.len() {
                    self.selected_data_indices
                        .insert(new_len - indices.len() + i);
                }
            }
            Action::DeleteSelectedPoints => {
                let mut to_remove: Vec<usize> =
                    self.selected_data_indices.iter().copied().collect();
                to_remove.sort_unstable_by(|a, b| b.cmp(a));
                for idx in to_remove {
                    if idx < self.data_pts.len() {
                        self.data_pts.remove(idx);
                    }
                }
                self.selected_data_indices.clear();
            }
            Action::RemoveDataPoint(idx) => {
                if idx < self.data_pts.len() {
                    self.data_pts.remove(idx);
                    self.selected_data_indices.remove(&idx);
                }
            }
            Action::DeleteGroup(idx) => {
                if idx < self.groups.len() {
                    self.groups.remove(idx);
                    if self.active_group_idx == idx {
                        self.active_group_idx = 0;
                    } else if self.active_group_idx > idx {
                        self.active_group_idx -= 1;
                    }

                    self.data_pts.retain(|p| p.group_id != idx);

                    for p in &mut self.data_pts {
                        if p.group_id > idx {
                            p.group_id -= 1;
                        }
                    }
                    self.selected_data_indices.clear();
                }
            }
            Action::UpdateGroupName(idx, name) => {
                if let Some(g) = self.groups.get_mut(idx) {
                    g.name = name;
                }
            }
            Action::UpdateGroupColor(idx, color) => {
                if let Some(g) = self.groups.get_mut(idx) {
                    g.color = color;
                }
            }
            Action::SetActiveGroup(idx) => {
                self.active_group_idx = idx;
            }
            Action::AddCalibPoint { img_x, img_y } => {
                if self.calib_pts.len() < 4 {
                    self.calib_pts.push(CalibPoint {
                        px: img_x,
                        py: img_y,
                    });
                    self.selected_calib_idx = Some(self.calib_pts.len() - 1);
                    self.selected_data_indices.clear();

                    if self.calib_pts.len() == 4 {
                        self.mode = AppMode::AddData;
                    }
                    crate::core::recalculate_data(
                        &self.calib_pts,
                        &mut self.data_pts,
                        &self.x1_val,
                        &self.x2_val,
                        &self.y1_val,
                        &self.y2_val,
                        self.log_x,
                        self.log_y,
                    );
                }
            }
            Action::AddDataPoint { img_x, img_y } => {
                self.data_pts.push(DataPoint {
                    px: img_x,
                    py: img_y,
                    lx: 0.0,
                    ly: 0.0,
                    group_id: self.active_group_idx,
                });
                self.selected_data_indices.clear();
                self.selected_data_indices.insert(self.data_pts.len() - 1);
                self.selected_calib_idx = None;
                crate::core::recalculate_data(
                    &self.calib_pts,
                    &mut self.data_pts,
                    &self.x1_val,
                    &self.x2_val,
                    &self.y1_val,
                    &self.y2_val,
                    self.log_x,
                    self.log_y,
                );
            }
            Action::MoveSelected { dx, dy } => {
                if let Some(idx) = self.selected_calib_idx {
                    if let Some(p) = self.calib_pts.get_mut(idx) {
                        p.px += dx;
                        p.py += dy;
                    }
                } else {
                    for &idx in &self.selected_data_indices {
                        if let Some(p) = self.data_pts.get_mut(idx) {
                            p.px += dx;
                            p.py += dy;
                        }
                    }
                }
            }
            Action::RecalculateData => {
                crate::core::recalculate_data(
                    &self.calib_pts,
                    &mut self.data_pts,
                    &self.x1_val,
                    &self.x2_val,
                    &self.y1_val,
                    &self.y2_val,
                    self.log_x,
                    self.log_y,
                );
            }
            Action::SelectPoints(indices, is_multi) => {
                let should_toggle = is_multi && indices.len() == 1;
                if !is_multi {
                    self.selected_data_indices.clear();
                }
                for idx in indices {
                    if should_toggle && self.selected_data_indices.contains(&idx) {
                        self.selected_data_indices.remove(&idx);
                    } else {
                        self.selected_data_indices.insert(idx);
                    }
                }
            }
            Action::SetDraggingPoint { is_calib, idx } => {
                if is_calib {
                    self.dragging_calib_idx = idx;
                } else {
                    self.dragging_data_idx = idx;
                }
            }
            Action::StopDragging => {
                self.dragging_calib_idx = None;
                self.dragging_data_idx = None;
                crate::core::recalculate_data(
                    &self.calib_pts,
                    &mut self.data_pts,
                    &self.x1_val,
                    &self.x2_val,
                    &self.y1_val,
                    &self.y2_val,
                    self.log_x,
                    self.log_y,
                );
            }
            Action::SetHoveredCalib(idx) => {
                self.hovered_calib_idx = idx;
            }
            Action::SetHoveredData(idx) => {
                self.hovered_data_idx = idx;
            }
            Action::SetBoxStart(pos) => {
                self.box_start = pos;
            }
            Action::ClearData => {
                self.data_pts.clear();
            }
            Action::ClearCalib => {
                self.calib_pts.clear();
                crate::core::recalculate_data(
                    &self.calib_pts,
                    &mut self.data_pts,
                    &self.x1_val,
                    &self.x2_val,
                    &self.y1_val,
                    &self.y2_val,
                    self.log_x,
                    self.log_y,
                );
            }
            Action::UpdateCalibAxis(axis, val) => {
                match axis.as_str() {
                    "x1" => self.x1_val = val,
                    "x2" => self.x2_val = val,
                    "y1" => self.y1_val = val,
                    "y2" => self.y2_val = val,
                    _ => {}
                }
                crate::core::recalculate_data(
                    &self.calib_pts,
                    &mut self.data_pts,
                    &self.x1_val,
                    &self.x2_val,
                    &self.y1_val,
                    &self.y2_val,
                    self.log_x,
                    self.log_y,
                );
            }
            Action::UpdateLogScale(log_x, log_y) => {
                self.log_x = log_x;
                self.log_y = log_y;
                crate::core::recalculate_data(
                    &self.calib_pts,
                    &mut self.data_pts,
                    &self.x1_val,
                    &self.x2_val,
                    &self.y1_val,
                    &self.y2_val,
                    self.log_x,
                    self.log_y,
                );
            }
            Action::AddGroup => {
                let new_idx = self.groups.len();
                let palette = [
                    Color32::from_rgb(0xe4, 0x1a, 0x1c),
                    Color32::from_rgb(0x37, 0x7e, 0xb8),
                    Color32::from_rgb(0x4d, 0xaf, 0x4a),
                    Color32::from_rgb(0x98, 0x4e, 0xa3),
                    Color32::from_rgb(0xff, 0x7f, 0x00),
                ];
                let col = palette[new_idx % palette.len()];
                self.groups.push(crate::state::PointGroup {
                    name: format!("Group {}", new_idx + 1),
                    color: col,
                });
                self.active_group_idx = new_idx;
            }
            Action::SetMode(mode) => {
                self.mode = mode;
            }
            Action::LoadImage(path, tex, size) => {
                // Keep image state as-is, waiting to be processed by a context
                self.image_path = Some(path);
                self.texture = Some(tex);
                self.img_size = size;
                self.pan = Vec2::ZERO;
                self.zoom = 1.0;
                self.calib_pts.clear();
                self.data_pts.clear();
            }
            Action::ClearSelection => {
                self.selected_calib_idx = None;
                self.selected_data_indices.clear();
                self.dragging_calib_idx = None;
                self.dragging_data_idx = None;
            }
            Action::RequestCenter => {
                self.center_requested = true;
            }
            Action::CenterCanvas(canvas_rect) => {
                self.center_requested = false;
                if self.img_size.x > 0.0 && self.img_size.y > 0.0 {
                    let scale_x = canvas_rect.width() / self.img_size.x;
                    let scale_y = canvas_rect.height() / self.img_size.y;
                    self.zoom = scale_x.min(scale_y) * 0.95; // 5% padding
                    let scaled_size = self.img_size * self.zoom;
                    self.pan = (canvas_rect.size() - scaled_size) / 2.0;
                } else {
                    self.pan = eframe::egui::Vec2::ZERO;
                    self.zoom = 1.0;
                }
            }
            Action::SetPanZoom { pan, zoom } => {
                self.pan = pan;
                self.zoom = zoom;
            }
            Action::RequestExportCsv => {
                crate::ui::panel::export_csv(self);
            }
        }
    }
}
