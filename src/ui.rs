use eframe::egui;
use egui::{Color32, Pos2, Rect, Stroke, Vec2};
use rfd::FileDialog;
use std::fs::File;
use std::io::Write;

use crate::core::{recalculate_data, CalibPoint, DataPoint};
use crate::state::{AppMode, AppState};

pub fn draw_ui(state: &mut AppState, ctx: &egui::Context) {
    // Top Panel: Unified Toolbar
    egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.heading("PlotDigitizer");
            ui.add_space(20.0);

            if ui.button("📁 Load Image").clicked() {
                load_image(state, ctx);
            }
            if ui.button("💾 Export CSV").clicked() {
                export_csv(state);
            }
        });
        ui.add_space(8.0);
    });

    // Left Sidebar for Control Panels
    egui::SidePanel::left("left_panel")
        .resizable(true)
        .min_width(320.0)
        .max_width(350.0)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_space(10.0);

                // Set explicit horizontal filling for children so frames all snap to the same uniform width
                ui.set_min_width(ui.available_width());

                // Unified card style
                let frame = egui::Frame::group(ui.style())
                    .fill(Color32::from_gray(35))
                    .inner_margin(12.0)
                    .corner_radius(5.0);

                // Section 1: Calibration
                frame.show(ui, |ui| {
                    ui.strong("1. Axes Calibration");
                    ui.add_space(10.0);

                    ui.horizontal(|ui| {
                        let btn_text = if state.mode == AppMode::AddCalib {
                            "🛑 Stop Calib"
                        } else {
                            "📍 Place Calib Points"
                        };
                        let mut btn = egui::Button::new(btn_text);
                        if state.mode == AppMode::AddCalib {
                            btn = btn.fill(Color32::from_rgb(180, 50, 50));
                        }
                        if ui.add_sized([160.0, 30.0], btn).clicked() {
                            state.mode = if state.mode == AppMode::AddCalib {
                                AppMode::Idle
                            } else {
                                AppMode::AddCalib
                            };
                        }
                        ui.label(format!("Points: {}/4", state.calib_pts.len()));
                    });

                    if ui.button("Clear Calib").clicked() {
                        state.calib_pts.clear();
                        recalculate_data(
                            &state.calib_pts,
                            &mut state.data_pts,
                            &state.x1_val,
                            &state.x2_val,
                            &state.y1_val,
                            &state.y2_val,
                            state.log_x,
                            state.log_y,
                        );
                    }

                    ui.add_space(10.0);
                    egui::Grid::new("calib_grid")
                        .num_columns(2)
                        .spacing([10.0, 10.0])
                        .show(ui, |ui| {
                            ui.label("X1 (Min):");
                            ui.text_edit_singleline(&mut state.x1_val);
                            ui.end_row();
                            ui.label("X2 (Max):");
                            ui.text_edit_singleline(&mut state.x2_val);
                            ui.end_row();
                            ui.label("Y1 (Min):");
                            ui.text_edit_singleline(&mut state.y1_val);
                            ui.end_row();
                            ui.label("Y2 (Max):");
                            ui.text_edit_singleline(&mut state.y2_val);
                            ui.end_row();
                        });

                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.checkbox(&mut state.log_x, "Log X").changed() {
                            recalculate_data(
                                &state.calib_pts,
                                &mut state.data_pts,
                                &state.x1_val,
                                &state.x2_val,
                                &state.y1_val,
                                &state.y2_val,
                                state.log_x,
                                state.log_y,
                            );
                        }
                        if ui.checkbox(&mut state.log_y, "Log Y").changed() {
                            recalculate_data(
                                &state.calib_pts,
                                &mut state.data_pts,
                                &state.x1_val,
                                &state.x2_val,
                                &state.y1_val,
                                &state.y2_val,
                                state.log_x,
                                state.log_y,
                            );
                        }
                    });
                });

                ui.add_space(15.0);

                // Section 2: Extraction
                frame.show(ui, |ui| {
                    ui.strong("2. Data Extraction");
                    ui.add_space(10.0);

                    if state.calib_pts.len() < 4 {
                        ui.colored_label(Color32::RED, "⚠ Calibrate 4 axes points first");
                        ui.add_space(5.0);
                    }

                    ui.horizontal(|ui| {
                        let btn_text = if state.mode == AppMode::AddData {
                            "🛑 Stop Picking"
                        } else {
                            "🎯 Pick Data Points"
                        };
                        let mut btn = egui::Button::new(btn_text);
                        if state.mode == AppMode::AddData {
                            btn = btn.fill(Color32::from_rgb(50, 150, 50));
                        }

                        // Explicit constant width to look cleaner alongside button
                        let resp = ui.add_sized([160.0, 30.0], btn);
                        if resp.clicked() && state.calib_pts.len() == 4 {
                            state.mode = if state.mode == AppMode::AddData {
                                AppMode::Idle
                            } else {
                                AppMode::AddData
                            };
                        }
                        if state.calib_pts.len() < 4 {
                            resp.on_hover_text("Calibrate first");
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(format!("Count: {}", state.data_pts.len()));
                        });
                    });

                    let palette = [
                        Color32::from_rgb(0xe4, 0x1a, 0x1c), // Red
                        Color32::from_rgb(0x37, 0x7e, 0xb8), // Blue
                        Color32::from_rgb(0x4d, 0xaf, 0x4a), // Green
                        Color32::from_rgb(0x98, 0x4e, 0xa3), // Purple
                        Color32::from_rgb(0xff, 0x7f, 0x00), // Orange
                    ];

                    ui.horizontal(|ui| {
                        if ui.button("➕ Add Group").clicked() {
                            let new_idx = state.groups.len();
                            let col = palette[new_idx % palette.len()];
                            state.groups.push(crate::state::PointGroup {
                                name: format!("Group {}", new_idx + 1),
                                color: col,
                            });
                            state.active_group_idx = new_idx;
                        }
                        if ui.button("Clear All Data").clicked() {
                            state.data_pts.clear();
                        }
                    });

                    ui.add_space(10.0);
                    egui::ScrollArea::vertical()
                        .max_height(300.0)
                        .show(ui, |ui| {
                            let mut to_remove_group = None;
                            let mut to_remove_data = None;
                            let mut move_point = None;

                            let num_groups = state.groups.len();
                            for (g_idx, group) in state.groups.iter_mut().enumerate() {
                                let frame = egui::Frame::NONE
                                    .inner_margin(4.0)
                                    .corner_radius(4.0)
                                    .fill(if state.active_group_idx == g_idx {
                                        Color32::from_gray(50)
                                    } else {
                                        Color32::TRANSPARENT
                                    });

                                let (_inner_resp, payload_opt) =
                                    ui.dnd_drop_zone::<usize, _>(frame, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.radio_value(&mut state.active_group_idx, g_idx, "");
                                            ui.color_edit_button_srgba(&mut group.color);
                                            ui.add_sized(
                                                [100.0, 20.0],
                                                egui::TextEdit::singleline(&mut group.name),
                                            );

                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    if num_groups > 1 {
                                                        if ui.button("🗑").clicked() {
                                                            to_remove_group = Some(g_idx);
                                                        }
                                                    }
                                                    let count = state
                                                        .data_pts
                                                        .iter()
                                                        .filter(|p| p.group_id == g_idx)
                                                        .count();
                                                    ui.label(format!("({} pts)", count));
                                                },
                                            );
                                        });
                                    });

                                if let Some(payload) = payload_opt {
                                    move_point = Some((*payload, g_idx));
                                }

                                ui.add_space(4.0);

                                let mut has_points = false;
                                egui::Grid::new(format!("data_grid_{}", g_idx))
                                    .striped(true)
                                    .num_columns(4)
                                    .show(ui, |ui| {
                                        for (i, p) in state
                                            .data_pts
                                            .iter()
                                            .enumerate()
                                            .filter(|(_, p)| p.group_id == g_idx)
                                        {
                                            has_points = true;
                                            let is_selected = state.selected_data_idx == Some(i);
                                            let is_hovered = state.hovered_data_idx == Some(i);

                                            let pt_id = egui::Id::new("pt").with(i);
                                            ui.dnd_drag_source(pt_id, i, |ui| {
                                                ui.label(
                                                    egui::RichText::new("||").color(Color32::GRAY),
                                                );
                                            });

                                            let mut btn_text = egui::RichText::new("🗑");
                                            if is_selected {
                                                btn_text = btn_text
                                                    .color(Color32::from_rgb(0xEA, 0x43, 0x35));
                                            }
                                            if ui.button(btn_text).clicked() {
                                                to_remove_data = Some(i);
                                            }

                                            let mut text_x =
                                                egui::RichText::new(format!("{:.8}", p.lx));
                                            let mut text_y =
                                                egui::RichText::new(format!("{:.8}", p.ly));

                                            if is_selected {
                                                text_x = text_x.color(group.color).strong();
                                                text_y = text_y.color(group.color).strong();
                                            } else if is_hovered {
                                                let h_col = group.color.linear_multiply(0.8);
                                                text_x = text_x.color(h_col);
                                                text_y = text_y.color(h_col);
                                            }

                                            if ui.selectable_label(is_selected, text_x).clicked() {
                                                state.selected_data_idx = Some(i);
                                                state.selected_calib_idx = None;
                                            }
                                            if ui.selectable_label(is_selected, text_y).clicked() {
                                                state.selected_data_idx = Some(i);
                                                state.selected_calib_idx = None;
                                            }
                                            ui.end_row();
                                        }
                                    });

                                if !has_points {
                                    ui.label(
                                        egui::RichText::new(
                                            "   (Drag points here or click canvas to add)",
                                        )
                                        .color(Color32::DARK_GRAY)
                                        .small(),
                                    );
                                }

                                ui.add_space(8.0);
                            }

                            if let Some((pt_idx, new_g_idx)) = move_point {
                                if pt_idx < state.data_pts.len() {
                                    state.data_pts[pt_idx].group_id = new_g_idx;
                                }
                            }
                            if let Some(idx) = to_remove_data {
                                state.data_pts.remove(idx);
                            }
                            if let Some(idx) = to_remove_group {
                                state.groups.remove(idx);
                                // Cleanup deleted group_id and fallback active pointer
                                if state.active_group_idx == idx {
                                    state.active_group_idx = 0;
                                } else if state.active_group_idx > idx {
                                    state.active_group_idx -= 1;
                                }

                                // Re-assign orphaned points or delete them. We will re-assign to active group.
                                for p in &mut state.data_pts {
                                    if p.group_id == idx {
                                        p.group_id = state.active_group_idx;
                                    } else if p.group_id > idx {
                                        p.group_id -= 1;
                                    }
                                }
                            }
                        });
                });
            });
        });

    // Central Image Viewport Canvas
    egui::CentralPanel::default().show(ctx, |ui| {
        let (response, painter) =
            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());

        // Zoom/Pan
        if response.hovered() {
            let scroll = ctx.input(|i| i.raw_scroll_delta.y);
            if scroll != 0.0 {
                let zoom_delta = (scroll * 0.005).exp();
                if let Some(mouse_pos) = ctx.input(|i| i.pointer.hover_pos()) {
                    let rect_pos = response.rect.min;
                    let mouse_rel = mouse_pos - rect_pos - state.pan;
                    state.zoom *= zoom_delta;
                    let new_mouse_rel = mouse_rel * zoom_delta;
                    state.pan -= new_mouse_rel - mouse_rel;
                }
            }
            if response.dragged_by(egui::PointerButton::Middle)
                || response.dragged_by(egui::PointerButton::Secondary)
            {
                state.pan += response.drag_delta();
            }
        }

        // Draw Image
        if let Some(texture) = &state.texture {
            let rect =
                Rect::from_min_size(response.rect.min + state.pan, state.img_size * state.zoom);
            painter.image(
                texture.id(),
                rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        } else {
            painter.text(
                response.rect.center(),
                egui::Align2::CENTER_CENTER,
                "No Image Loaded.",
                egui::FontId::proportional(20.0),
                Color32::GRAY,
            );
        }

        // Coordinate transforms
        let rect_min = response.rect.min;
        let to_screen = |px: f32, py: f32, pan: Vec2, zoom: f32| -> Pos2 {
            rect_min + pan + Vec2::new(px * zoom, py * zoom)
        };
        let to_image = |pos: Pos2, pan: Vec2, zoom: f32| -> (f32, f32) {
            let pt = pos - rect_min - pan;
            (pt.x / zoom, pt.y / zoom)
        };

        let threshold = 15.0; // Px radius for clicking

        // Global Keyboard Nudging
        let mut moved = false;
        let mut nudge_x = 0.0;
        let mut nudge_y = 0.0;
        if response.hovered() || response.has_focus() {
            if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                nudge_y -= 1.0;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                nudge_y += 1.0;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
                nudge_x -= 1.0;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
                nudge_x += 1.0;
            }
        }

        if nudge_x != 0.0 || nudge_y != 0.0 {
            let img_nudge_x = nudge_x / state.zoom;
            let img_nudge_y = nudge_y / state.zoom;
            if let Some(idx) = state.selected_calib_idx {
                state.calib_pts[idx].px += img_nudge_x;
                state.calib_pts[idx].py += img_nudge_y;
                moved = true;
            } else if let Some(idx) = state.selected_data_idx {
                state.data_pts[idx].px += img_nudge_x;
                state.data_pts[idx].py += img_nudge_y;
                moved = true;
            }
            if moved {
                recalculate_data(
                    &state.calib_pts,
                    &mut state.data_pts,
                    &state.x1_val,
                    &state.x2_val,
                    &state.y1_val,
                    &state.y2_val,
                    state.log_x,
                    state.log_y,
                );
            }
        }

        // Handle Clicks
        let mouse_pos = ctx
            .input(|i| i.pointer.hover_pos())
            .or_else(|| ctx.input(|i| i.pointer.interact_pos()));
        let press_origin = ctx.input(|i| i.pointer.press_origin());

        if let Some(mouse_pos) = mouse_pos {
            let find_hit = |pos: Pos2| -> (Option<usize>, Option<usize>) {
                for (i, p) in state.calib_pts.iter().enumerate() {
                    if to_screen(p.px, p.py, state.pan, state.zoom).distance(pos) < threshold {
                        return (Some(i), None);
                    }
                }
                for (i, p) in state.data_pts.iter().enumerate() {
                    if to_screen(p.px, p.py, state.pan, state.zoom).distance(pos) < threshold {
                        return (None, Some(i));
                    }
                }
                (None, None)
            };

            let (hover_hit_calib, hover_hit_data) = find_hit(mouse_pos);
            let (press_hit_calib, press_hit_data) = if let Some(origin) = press_origin {
                find_hit(origin)
            } else {
                (hover_hit_calib, hover_hit_data)
            };

            state.hovered_calib_idx = hover_hit_calib;
            state.hovered_data_idx = hover_hit_data;

            if response.drag_started_by(egui::PointerButton::Primary) {
                if let Some(idx) = press_hit_calib {
                    state.dragging_calib_idx = Some(idx);
                    state.selected_calib_idx = Some(idx);
                    state.selected_data_idx = None;
                    response.request_focus();
                } else if let Some(idx) = press_hit_data {
                    state.dragging_data_idx = Some(idx);
                    state.selected_data_idx = Some(idx);
                    state.selected_calib_idx = None;
                    response.request_focus();
                } else {
                    state.selected_calib_idx = None;
                    state.selected_data_idx = None;
                }
            }

            if response.clicked_by(egui::PointerButton::Primary) {
                if let Some(idx) = press_hit_calib {
                    state.selected_calib_idx = Some(idx);
                    state.selected_data_idx = None;
                    response.request_focus();
                } else if let Some(idx) = press_hit_data {
                    state.selected_data_idx = Some(idx);
                    state.selected_calib_idx = None;
                    response.request_focus();
                } else if state.texture.is_some() {
                    let (img_x, img_y) = to_image(mouse_pos, state.pan, state.zoom);
                    if state.mode == AppMode::AddCalib && state.calib_pts.len() < 4 {
                        state.calib_pts.push(CalibPoint {
                            px: img_x,
                            py: img_y,
                        });
                        state.selected_calib_idx = Some(state.calib_pts.len() - 1);
                        state.selected_data_idx = None;
                        response.request_focus();

                        if state.calib_pts.len() == 4 {
                            state.mode = AppMode::Idle;
                        }
                        recalculate_data(
                            &state.calib_pts,
                            &mut state.data_pts,
                            &state.x1_val,
                            &state.x2_val,
                            &state.y1_val,
                            &state.y2_val,
                            state.log_x,
                            state.log_y,
                        );
                    } else if state.mode == AppMode::AddData {
                        state.data_pts.push(DataPoint {
                            px: img_x,
                            py: img_y,
                            lx: 0.0,
                            ly: 0.0,
                            group_id: state.active_group_idx,
                        });
                        state.selected_data_idx = Some(state.data_pts.len() - 1);
                        state.selected_calib_idx = None;
                        response.request_focus();
                        recalculate_data(
                            &state.calib_pts,
                            &mut state.data_pts,
                            &state.x1_val,
                            &state.x2_val,
                            &state.y1_val,
                            &state.y2_val,
                            state.log_x,
                            state.log_y,
                        );
                    } else {
                        state.selected_calib_idx = None;
                        state.selected_data_idx = None;
                    }
                } else {
                    state.selected_calib_idx = None;
                    state.selected_data_idx = None;
                }
            }

            if response.dragged_by(egui::PointerButton::Primary) {
                let (img_x, img_y) = to_image(mouse_pos, state.pan, state.zoom);
                if let Some(idx) = state.dragging_calib_idx {
                    state.calib_pts[idx].px = img_x;
                    state.calib_pts[idx].py = img_y;
                } else if let Some(idx) = state.dragging_data_idx {
                    state.data_pts[idx].px = img_x;
                    state.data_pts[idx].py = img_y;
                }
            }

            if response.drag_stopped() {
                if state.dragging_calib_idx.is_some() || state.dragging_data_idx.is_some() {
                    state.dragging_calib_idx = None;
                    state.dragging_data_idx = None;
                    recalculate_data(
                        &state.calib_pts,
                        &mut state.data_pts,
                        &state.x1_val,
                        &state.x2_val,
                        &state.y1_val,
                        &state.y2_val,
                        state.log_x,
                        state.log_y,
                    );
                }
            }
        } else {
            state.hovered_calib_idx = None;
            state.hovered_data_idx = None;
        }

        const GOOGLE_BLUE: Color32 = Color32::from_rgb(0x42, 0x85, 0xF4);
        const GOOGLE_GREEN: Color32 = Color32::from_rgb(0x34, 0xA8, 0x53);
        const GOOGLE_RED: Color32 = Color32::from_rgb(0xEA, 0x43, 0x35);

        let draw_point_target = |sp: Pos2, col: Color32, is_selected: bool, is_hovered: bool| {
            let (r_blk, r_wht, r_in) = if is_selected {
                (12.0, 9.0, 6.0)
            } else if is_hovered {
                (10.0, 8.0, 6.0)
            } else {
                (9.0, 7.0, 5.0)
            };

            painter.circle_filled(sp, r_blk, Color32::BLACK);
            painter.circle_filled(sp, r_wht, Color32::WHITE);
            painter.circle_filled(sp, r_in, col);
        };

        // Draw Points
        for (i, p) in state.data_pts.iter().enumerate() {
            let sp = to_screen(p.px, p.py, state.pan, state.zoom);
            let is_selected = state.selected_data_idx == Some(i);
            let is_hovered = state.hovered_data_idx == Some(i);

            let draw_color = state
                .groups
                .get(p.group_id)
                .map(|g| g.color)
                .unwrap_or(Color32::WHITE);
            draw_point_target(sp, draw_color, is_selected, is_hovered);
        }

        let calib_colors = [GOOGLE_BLUE, GOOGLE_BLUE, GOOGLE_GREEN, GOOGLE_GREEN];
        let calib_labels = ["X1", "X2", "Y1", "Y2"];
        for (i, p) in state.calib_pts.iter().enumerate() {
            let sp = to_screen(p.px, p.py, state.pan, state.zoom);
            let col = calib_colors[i];
            let is_selected = state.selected_calib_idx == Some(i);
            let is_hovered = state.hovered_calib_idx == Some(i);

            let cross_size = if is_selected { 14.0 } else { 10.0 };
            let cross_stroke = Stroke::new(if is_selected { 3.0 } else { 2.0 }, col);
            painter.line_segment(
                [
                    sp - Vec2::new(cross_size, cross_size),
                    sp + Vec2::new(cross_size, cross_size),
                ],
                cross_stroke,
            );
            painter.line_segment(
                [
                    sp - Vec2::new(cross_size, -cross_size),
                    sp + Vec2::new(cross_size, -cross_size),
                ],
                cross_stroke,
            );

            draw_point_target(sp, col, is_selected, is_hovered);

            let text_pos = sp + Vec2::new(10.0, -15.0);
            painter.text(
                text_pos + Vec2::new(1.0, 1.0),
                egui::Align2::LEFT_BOTTOM,
                calib_labels[i],
                egui::FontId::proportional(14.0),
                Color32::BLACK,
            );
            painter.text(
                text_pos,
                egui::Align2::LEFT_BOTTOM,
                calib_labels[i],
                egui::FontId::proportional(14.0),
                col,
            );
        }
    });
}

fn load_image(state: &mut AppState, ctx: &egui::Context) {
    if let Some(path) = FileDialog::new()
        .add_filter("image", &["png", "jpg", "jpeg"])
        .pick_file()
    {
        if let Ok(img) = image::open(&path) {
            let img = img.to_rgba8();
            let size = [img.width() as _, img.height() as _];
            let pixels = img.as_flat_samples();
            let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
            state.texture = Some(ctx.load_texture("main_image", color_image, Default::default()));
            state.img_size = Vec2::new(size[0] as f32, size[1] as f32);
            state.image_path = Some(path);
            state.pan = Vec2::ZERO;
            state.zoom = 1.0;
            state.calib_pts.clear();
            state.data_pts.clear();
        }
    }
}

fn export_csv(state: &AppState) {
    if let Some(path) = FileDialog::new()
        .set_file_name("extracted_data.csv")
        .add_filter("csv", &["csv"])
        .save_file()
    {
        if let Ok(mut file) = File::create(path) {
            let _ = writeln!(file, "Group,X,Y");
            for p in &state.data_pts {
                let group_name = state
                    .groups
                    .get(p.group_id)
                    .map(|g| g.name.as_str())
                    .unwrap_or("Unknown");
                let _ = writeln!(file, "\"{}\",{:.8},{:.8}", group_name, p.lx, p.ly);
            }
        }
    }
}
