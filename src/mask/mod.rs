pub mod results_panel;

use eframe::egui;
use egui::{Color32, Pos2, Stroke};

use crate::action::Action;
use crate::state::{AppState, MaskTool};

// ────────────────────────────────────────────────────────────────
//  Mask Sub-Toolbar (anchored at canvas top-left)
// ────────────────────────────────────────────────────────────────

pub fn draw_mask_toolbar(state: &AppState, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
    let mask = if state.axis_mask.active {
        &state.axis_mask
    } else if state.data_mask.active {
        &state.data_mask
    } else {
        return;
    };

    let window = egui::Window::new("Mask Sub-Toolbar")
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .anchor(egui::Align2::LEFT_TOP, [325.0, 25.0]);

    window.show(ui.ctx(), |ui| {
        ui.horizontal(|ui| {
            if ui
                .selectable_label(mask.tool == MaskTool::Pen, "\u{270F} Pen")
                .on_hover_text("Paint mask")
                .clicked()
            {
                actions.push(Action::MaskSetTool(MaskTool::Pen));
            }
            if ui
                .selectable_label(mask.tool == MaskTool::Eraser, "\u{1F4D7} Eraser")
                .on_hover_text("Erase mask")
                .clicked()
            {
                actions.push(Action::MaskSetTool(MaskTool::Eraser));
            }

            // Compact brush size slider (no label, narrow)
            let mut size = mask.brush_size;
            let slider = egui::Slider::new(&mut size, 2.0..=80.0)
                .step_by(1.0)
                .show_value(false);
            if ui
                .add_sized([40.0, 18.0], slider)
                .on_hover_text(format!("Brush size: {:.0} px", mask.brush_size))
                .changed()
            {
                actions.push(Action::MaskSetBrushSize(size));
            }

            // Icon-only visibility toggle
            let vis_icon = if mask.visible {
                "\u{1F441}"
            } else {
                "\u{1F6AB}"
            };
            if ui
                .selectable_label(!mask.visible, vis_icon)
                .on_hover_text(if mask.visible {
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
    let mask = if state.axis_mask.active {
        &state.axis_mask
    } else if state.data_mask.active {
        &state.data_mask
    } else {
        return;
    };
    if !mask.visible || mask.buffer.is_empty() {
        return;
    }

    let is_erasing = mask.painting && mask.tool != crate::state::MaskTool::Pen;

    // Draw cached texture (skip during eraser strokes — texture is stale)
    if !is_erasing {
        if let Some(tex) = &mask.mask_texture {
            let p0 = to_screen(0.0, 0.0);
            let p1 = to_screen(mask.width as f32, mask.height as f32);
            painter.image(
                tex.id(),
                egui::Rect::from_min_max(p0, p1),
                egui::Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
    }

    if mask.painting && !mask.stroke_snapshot.is_empty() {
        if !is_erasing {
            // Pen: incremental — only render NEW pixels added in this stroke
            draw_mask_rects_diff(
                &mask.buffer,
                &mask.stroke_snapshot,
                mask.width as usize,
                mask.height as usize,
                painter,
                to_screen,
            );
        } else {
            // Eraser: texture is stale, full rect render of current buffer
            draw_mask_rects_full(
                &mask.buffer,
                mask.width as usize,
                mask.height as usize,
                painter,
                to_screen,
            );
        }
    } else if mask.mask_texture.is_none() {
        // No texture yet — full rect fallback
        draw_mask_rects_full(
            &mask.buffer,
            mask.width as usize,
            mask.height as usize,
            painter,
            to_screen,
        );
    }
}

/// Draw rects for ALL masked pixels in the buffer.
fn draw_mask_rects_full(
    buffer: &[bool],
    w: usize,
    h: usize,
    painter: &egui::Painter,
    to_screen: &dyn Fn(f32, f32) -> Pos2,
) {
    let color = Color32::from_rgba_unmultiplied(220, 80, 40, 90);
    for y in 0..h {
        let row = y * w;
        let mut x = 0;
        while x < w {
            if buffer[row + x] {
                let run_start = x;
                while x < w && buffer[row + x] {
                    x += 1;
                }
                let p0 = to_screen(run_start as f32, y as f32);
                let p1 = to_screen(x as f32, (y + 1) as f32);
                painter.rect_filled(egui::Rect::from_min_max(p0, p1), 0.0, color);
            } else {
                x += 1;
            }
        }
    }
}

/// Draw rects only for pixels that are ON in `buffer` but OFF in `snapshot` (new this stroke).
fn draw_mask_rects_diff(
    buffer: &[bool],
    snapshot: &[bool],
    w: usize,
    h: usize,
    painter: &egui::Painter,
    to_screen: &dyn Fn(f32, f32) -> Pos2,
) {
    let color = Color32::from_rgba_unmultiplied(220, 80, 40, 90);
    for y in 0..h {
        let row = y * w;
        let mut x = 0;
        while x < w {
            let i = row + x;
            if buffer[i] && !snapshot[i] {
                let run_start = x;
                x += 1;
                while x < w {
                    let j = row + x;
                    if buffer[j] && !snapshot[j] {
                        x += 1;
                    } else {
                        break;
                    }
                }
                let p0 = to_screen(run_start as f32, y as f32);
                let p1 = to_screen(x as f32, (y + 1) as f32);
                painter.rect_filled(egui::Rect::from_min_max(p0, p1), 0.0, color);
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
    let rgba = state.decoded_rgba.as_deref();
    let mask = if state.axis_mask.active {
        &state.axis_mask
    } else if state.data_mask.active {
        &state.data_mask
    } else {
        return;
    };

    // Axis highlight
    if let Some(ref axis_hl) = mask.highlight_axis {
        if let Some(ref result) = mask.axis_result {
            let pixels_ref = match axis_hl {
                crate::state::AxisHighlight::X => &result.x_axis_pixels,
                crate::state::AxisHighlight::Y => &result.y_axis_pixels,
            };

            draw_pixel_set_real_color(painter, pixels_ref, to_screen, rgba.map(|v| &**v), mask.width);

            // Draw ❌ crosses at tick positions
            let ticks = match axis_hl {
                crate::state::AxisHighlight::X => &result.x_ticks,
                crate::state::AxisHighlight::Y => &result.y_ticks,
            };
            let endpoints = match axis_hl {
                crate::state::AxisHighlight::X => &result.x_axis,
                crate::state::AxisHighlight::Y => &result.y_axis,
            };

            for tick in ticks {
                let pos = to_screen(tick.0, tick.1);
                let is_endpoint = endpoints.map_or(false, |(start, end)| {
                    ((tick.0 - start.0).abs() < 1.0 && (tick.1 - start.1).abs() < 1.0)
                        || ((tick.0 - end.0).abs() < 1.0 && (tick.1 - end.1).abs() < 1.0)
                });
                let cross_size = if is_endpoint { 8.0 } else { 5.0 };
                let cross_width = if is_endpoint { 2.0 } else { 1.5 };
                let cross_color = match axis_hl {
                    crate::state::AxisHighlight::X => Color32::from_rgb(0x42, 0x85, 0xF4),
                    crate::state::AxisHighlight::Y => Color32::from_rgb(0x34, 0xA8, 0x53),
                };

                // White outline
                painter.line_segment(
                    [
                        Pos2::new(pos.x - cross_size, pos.y - cross_size),
                        Pos2::new(pos.x + cross_size, pos.y + cross_size),
                    ],
                    Stroke::new(cross_width + 1.5, Color32::WHITE),
                );
                painter.line_segment(
                    [
                        Pos2::new(pos.x + cross_size, pos.y - cross_size),
                        Pos2::new(pos.x - cross_size, pos.y + cross_size),
                    ],
                    Stroke::new(cross_width + 1.5, Color32::WHITE),
                );
                // Colored cross
                painter.line_segment(
                    [
                        Pos2::new(pos.x - cross_size, pos.y - cross_size),
                        Pos2::new(pos.x + cross_size, pos.y + cross_size),
                    ],
                    Stroke::new(cross_width, cross_color),
                );
                painter.line_segment(
                    [
                        Pos2::new(pos.x + cross_size, pos.y - cross_size),
                        Pos2::new(pos.x - cross_size, pos.y + cross_size),
                    ],
                    Stroke::new(cross_width, cross_color),
                );
            }
        }
    }

    // Data highlight
    if let Some(idx) = mask.highlight_data_idx {
        if let Some(ref result) = mask.data_result {
            if let Some(group) = result.groups.get(idx) {
                draw_pixel_set_real_color(
                    painter,
                    &group.pixel_coords,
                    to_screen,
                    rgba.map(|v| &**v),
                    mask.width,
                );

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

/// Draw highlighted pixels using their actual color from the image,
/// with outer glow for visibility.
fn draw_pixel_set_real_color(
    painter: &egui::Painter,
    pixels: &[(u32, u32)],
    to_screen: &dyn Fn(f32, f32) -> Pos2,
    rgba: Option<&[u8]>,
    img_width: u32,
) {
    if pixels.is_empty() {
        return;
    }

    let pixel_set: std::collections::HashSet<(u32, u32)> = pixels.iter().copied().collect();

    // ── Outer glow via boundary edge strokes (wide → narrow, faint → bright) ──
    // Each layer = (stroke width, alpha)
    // let glow_layers: &[(f32, u8)] = &[(5.0, 100), (3.0, 100), (1.5, 100)];
    let glow_layers: &[(f32, u8)] = &[(5.0, 225)];

    let w_us = img_width as usize;

    // Collect boundary edges and draw glow layers (widest/faintest first)
    for &(width, alpha) in glow_layers {
        let glow = Color32::from_rgba_unmultiplied(255, 255, 255, alpha);
        let stroke = Stroke::new(width, glow);
        for &(px, py) in pixels {
            // Top edge
            if !pixel_set.contains(&(px, py.wrapping_sub(1))) {
                let a = to_screen(px as f32, py as f32);
                let b = to_screen((px + 1) as f32, py as f32);
                painter.line_segment([a, b], stroke);
            }
            // Bottom edge
            if !pixel_set.contains(&(px, py + 1)) {
                let a = to_screen(px as f32, (py + 1) as f32);
                let b = to_screen((px + 1) as f32, (py + 1) as f32);
                painter.line_segment([a, b], stroke);
            }
            // Left edge
            if !pixel_set.contains(&(px.wrapping_sub(1), py)) {
                let a = to_screen(px as f32, py as f32);
                let b = to_screen(px as f32, (py + 1) as f32);
                painter.line_segment([a, b], stroke);
            }
            // Right edge
            if !pixel_set.contains(&(px + 1, py)) {
                let a = to_screen((px + 1) as f32, py as f32);
                let b = to_screen((px + 1) as f32, (py + 1) as f32);
                painter.line_segment([a, b], stroke);
            }
        }
    }

    // ── Fill pixels with their actual color ──
    for &(px, py) in pixels {
        let color = if let Some(rgba_data) = rgba {
            let off = ((py as usize) * w_us + (px as usize)) * 4;
            if off + 2 < rgba_data.len() {
                Color32::from_rgb(rgba_data[off], rgba_data[off + 1], rgba_data[off + 2])
            } else {
                Color32::from_rgb(200, 100, 50)
            }
        } else {
            Color32::from_rgb(200, 100, 50)
        };

        let p0 = to_screen(px as f32, py as f32);
        let p1 = to_screen((px + 1) as f32, (py + 1) as f32);
        painter.rect_filled(egui::Rect::from_min_max(p0, p1), 0.0, color);
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
    let mask = if state.axis_mask.active {
        &state.axis_mask
    } else if state.data_mask.active {
        &state.data_mask
    } else {
        return;
    };

    if let Some(mouse_pos) = ctx.input(|i| i.pointer.hover_pos()) {
        if response.rect.contains(mouse_pos) {
            let screen_radius = mask.brush_size * zoom;
            let cursor_color = match mask.tool {
                MaskTool::Pen => Color32::from_rgba_unmultiplied(220, 80, 40, 160),
                MaskTool::Eraser => Color32::from_rgba_unmultiplied(100, 180, 255, 160),
            };
            painter.circle_stroke(mouse_pos, screen_radius, Stroke::new(1.5, cursor_color));
            // Small center dot
            painter.circle_filled(mouse_pos, 2.0, cursor_color);
        }
    }
}
