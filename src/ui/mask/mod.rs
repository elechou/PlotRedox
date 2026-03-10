pub mod analysis;
pub mod results_panel;

use eframe::egui;
use egui::{Color32, Pos2, Stroke};

use crate::action::Action;
use crate::state::{AppState, MaskTool};

// ────────────────────────────────────────────────────────────────
//  Mask Sub-Toolbar (anchored at canvas top-left)
// ────────────────────────────────────────────────────────────────

pub fn draw_mask_toolbar(state: &AppState, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
    if !state.mask.active {
        return;
    }

    let window = egui::Window::new("Mask Sub-Toolbar")
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .anchor(egui::Align2::LEFT_TOP, [325.0, 25.0]);

    window.show(ui.ctx(), |ui| {
        ui.horizontal(|ui| {
            if ui
                .selectable_label(state.mask.tool == MaskTool::Pen, "\u{270F} Pen")
                .on_hover_text("Paint mask")
                .clicked()
            {
                actions.push(Action::MaskSetTool(MaskTool::Pen));
            }
            if ui
                .selectable_label(state.mask.tool == MaskTool::Eraser, "\u{1F4D7} Eraser")
                .on_hover_text("Erase mask")
                .clicked()
            {
                actions.push(Action::MaskSetTool(MaskTool::Eraser));
            }

            // Compact brush size slider (no label, narrow)
            let mut size = state.mask.brush_size;
            let slider = egui::Slider::new(&mut size, 2.0..=80.0)
                .step_by(1.0)
                .show_value(false);
            if ui
                .add_sized([40.0, 18.0], slider)
                .on_hover_text(format!("Brush size: {:.0} px", state.mask.brush_size))
                .changed()
            {
                actions.push(Action::MaskSetBrushSize(size));
            }

            // Icon-only visibility toggle
            let vis_icon = if state.mask.visible {
                "\u{1F441}"
            } else {
                "\u{1F6AB}"
            };
            if ui
                .selectable_label(!state.mask.visible, vis_icon)
                .on_hover_text(if state.mask.visible {
                    "Hide mask overlay"
                } else {
                    "Show mask overlay"
                })
                .clicked()
            {
                actions.push(Action::MaskToggleVisibility);
            }

            // Icon-only clear button
            if ui.button("🗑").on_hover_text("Clear all mask").clicked() {
                actions.push(Action::MaskClear);
            }
        });
    });
}

// ────────────────────────────────────────────────────────────────
//  Mask Overlay Rendering
// ────────────────────────────────────────────────────────────────

pub fn draw_mask_overlay(
    state: &AppState,
    painter: &egui::Painter,
    to_screen: &dyn Fn(f32, f32) -> Pos2,
    _zoom: f32,
) {
    if !state.mask.active && !state.mask.visible {
        return;
    }
    if !state.mask.visible {
        return;
    }
    if state.mask.buffer.is_empty() {
        return;
    }

    let w = state.mask.width as usize;
    let h = state.mask.height as usize;

    // Semi-transparent red-orange overlay color (multiply-like effect)
    let overlay_color = Color32::from_rgba_unmultiplied(220, 80, 40, 90);

    // Scan the mask and draw filled rectangles for contiguous runs of masked pixels.
    // This is much more efficient than drawing one rect per pixel.
    for y in 0..h {
        let row_start = y * w;
        let mut x = 0;
        while x < w {
            if state.mask.buffer[row_start + x] {
                // Find the end of this run
                let run_start = x;
                while x < w && state.mask.buffer[row_start + x] {
                    x += 1;
                }
                let run_end = x;

                // Draw a single rectangle for the entire run
                let p0 = to_screen(run_start as f32, y as f32);
                let p1 = to_screen(run_end as f32, (y + 1) as f32);
                let rect = egui::Rect::from_min_max(p0, p1);
                painter.rect_filled(rect, 0.0, overlay_color);
            } else {
                x += 1;
            }
        }
    }
}

// ────────────────────────────────────────────────────────────────
//  Mask Highlight Rendering (for axis/data hover)
// ────────────────────────────────────────────────────────────────

pub fn draw_mask_highlights(
    state: &AppState,
    painter: &egui::Painter,
    to_screen: &dyn Fn(f32, f32) -> Pos2,
    _zoom: f32,
) {
    // Axis highlight
    if let Some(ref axis_hl) = state.mask.highlight_axis {
        if let Some(ref result) = state.mask.axis_result {
            let pixels = match axis_hl {
                crate::state::AxisHighlight::X => &result.x_axis_pixels,
                crate::state::AxisHighlight::Y => &result.y_axis_pixels,
            };
            let hl_color = match axis_hl {
                crate::state::AxisHighlight::X => {
                    Color32::from_rgba_unmultiplied(66, 133, 244, 180)
                }
                crate::state::AxisHighlight::Y => Color32::from_rgba_unmultiplied(52, 168, 83, 180),
            };

            // Draw highlighted pixels in runs for performance
            draw_pixel_set_highlight(painter, pixels, to_screen, hl_color);
        }
    }

    // Data highlight
    if let Some(idx) = state.mask.highlight_data_idx {
        if let Some(ref result) = state.mask.data_result {
            if let Some(group) = result.groups.get(idx) {
                let hl_color = Color32::from_rgba_unmultiplied(
                    group.color[0],
                    group.color[1],
                    group.color[2],
                    180,
                );
                draw_pixel_set_highlight(painter, &group.pixel_coords, to_screen, hl_color);

                // Draw sampled points
                for &(px, py) in &group.sampled_points {
                    let sp = to_screen(px, py);
                    painter.circle_filled(sp, 4.0, Color32::WHITE);
                    painter.circle_filled(
                        sp,
                        3.0,
                        Color32::from_rgb(group.color[0], group.color[1], group.color[2]),
                    );
                }
            }
        }
    }
}

fn draw_pixel_set_highlight(
    painter: &egui::Painter,
    pixels: &[(u32, u32)],
    to_screen: &dyn Fn(f32, f32) -> Pos2,
    color: Color32,
) {
    // Sort by y then x for run-length compression
    // Since pixels may come pre-sorted or not, we batch by row
    if pixels.is_empty() {
        return;
    }

    for &(px, py) in pixels {
        let p0 = to_screen(px as f32, py as f32);
        let p1 = to_screen((px + 1) as f32, (py + 1) as f32);
        let rect = egui::Rect::from_min_max(p0, p1);
        painter.rect_filled(rect, 0.0, color);
    }
}

// ────────────────────────────────────────────────────────────────
//  Mask Brush Cursor
// ────────────────────────────────────────────────────────────────

pub fn draw_mask_cursor(
    state: &AppState,
    painter: &egui::Painter,
    ctx: &egui::Context,
    response: &egui::Response,
    zoom: f32,
) {
    if !state.mask.active {
        return;
    }

    if let Some(mouse_pos) = ctx.input(|i| i.pointer.hover_pos()) {
        if response.rect.contains(mouse_pos) {
            let screen_radius = state.mask.brush_size * zoom;
            let cursor_color = match state.mask.tool {
                MaskTool::Pen => Color32::from_rgba_unmultiplied(220, 80, 40, 160),
                MaskTool::Eraser => Color32::from_rgba_unmultiplied(100, 180, 255, 160),
            };
            painter.circle_stroke(mouse_pos, screen_radius, Stroke::new(1.5, cursor_color));
            // Small center dot
            painter.circle_filled(mouse_pos, 2.0, cursor_color);
        }
    }
}
