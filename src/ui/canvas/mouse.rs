use eframe::egui;
use egui::{Color32, Pos2, Rect};

use crate::action::Action;
use crate::state::{AppMode, AppState};

/// Handle all mouse interactions on the canvas: clicks, drags, box-select,
/// hover detection, context menus, and zoom/pan.
pub fn handle_mouse(
    state: &AppState,
    ctx: &egui::Context,
    response: &egui::Response,
    actions: &mut Vec<Action>,
    to_screen: &dyn Fn(f32, f32) -> Pos2,
) {
    let threshold = 15.0; // Px radius for clicking

    // --- Zoom / Pan via scroll & drag ---
    if response.hovered() {
        let scroll = ctx.input(|i| i.raw_scroll_delta.y);
        let mut new_zoom = state.zoom;
        let mut new_pan = state.pan;

        if scroll != 0.0 {
            let zoom_delta = (scroll * 0.005).exp();
            if let Some(mouse_pos) = ctx.input(|i| i.pointer.hover_pos()) {
                let rect_pos = response.rect.min;
                let mouse_rel = mouse_pos - rect_pos - state.pan;
                new_zoom *= zoom_delta;
                let new_mouse_rel = mouse_rel * zoom_delta;
                new_pan -= new_mouse_rel - mouse_rel;
            }
        }

        let is_space_pressed = ctx.input(|i| i.key_down(egui::Key::Space));
        let mut is_panning = response.dragged_by(egui::PointerButton::Middle)
            || response.dragged_by(egui::PointerButton::Secondary);

        if (state.mode == AppMode::Pan || is_space_pressed)
            && response.dragged_by(egui::PointerButton::Primary)
        {
            is_panning = true;
        }

        // In mask mode, right-click/middle-click still pans
        if matches!(state.mode, AppMode::AxisMask | AppMode::DataMask) {
            // Pan via right/middle is already handled above
        }

        if is_panning {
            new_pan += response.drag_delta();
        }

        if new_zoom != state.zoom || new_pan != state.pan {
            actions.push(Action::SetPanZoom {
                pan: new_pan,
                zoom: new_zoom,
            });
        }
    }

    // --- Mask mode: painting ---
    if matches!(state.mode, AppMode::AxisMask | AppMode::DataMask) {
        handle_mask_mouse(state, ctx, response, actions);
        return; // Don't process normal click/drag logic in mask mode
    }

    // --- Hit testing ---
    let mouse_pos = ctx
        .input(|i| i.pointer.hover_pos())
        .or_else(|| ctx.input(|i| i.pointer.interact_pos()));
    let press_origin = ctx.input(|i| i.pointer.press_origin());

    if let Some(mouse_pos) = mouse_pos {
        let find_hit = |pos: Pos2| -> (Option<usize>, Option<usize>) {
            for (i, p) in state.calib_pts.iter().enumerate() {
                if to_screen(p.px, p.py).distance(pos) < threshold {
                    return (Some(i), None);
                }
            }
            for (i, p) in state.data_pts.iter().enumerate() {
                if to_screen(p.px, p.py).distance(pos) < threshold {
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

        // Update hover state
        if state.hovered_calib_idx != hover_hit_calib {
            actions.push(Action::SetHoveredCalib(hover_hit_calib));
        }
        if state.hovered_data_idx != hover_hit_data {
            actions.push(Action::SetHoveredData(hover_hit_data));
        }

        // Compute effective mode (Space → Pan, Alt → Delete)
        let is_alt_pressed = ctx.input(|i| i.modifiers.alt);
        let is_space_held = ctx.input(|i| i.key_down(egui::Key::Space));
        let effective_mode = if is_space_held {
            AppMode::Pan
        } else if is_alt_pressed
            && (state.mode == AppMode::Select || state.mode == AppMode::AddData)
        {
            AppMode::Delete
        } else {
            state.mode
        };

        if effective_mode == AppMode::Delete {
            ctx.set_cursor_icon(egui::CursorIcon::Crosshair);
        }

        // --- Drag start ---
        if response.drag_started_by(egui::PointerButton::Primary) {
            if let Some(idx) = press_hit_calib {
                actions.push(Action::SetDraggingPoint {
                    is_calib: true,
                    idx: Some(idx),
                });
                actions.push(Action::SelectCalibPoint(idx));
            } else if let Some(idx) = press_hit_data {
                actions.push(Action::SetDraggingPoint {
                    is_calib: false,
                    idx: Some(idx),
                });
                let is_multi =
                    ctx.input(|i| i.modifiers.shift || i.modifiers.ctrl || i.modifiers.command);
                if !state.selected_data_indices.contains(&idx) {
                    actions.push(Action::SelectPoints(vec![idx], is_multi));
                }
            } else if state.mode == AppMode::Select {
                if let Some(pos) = press_origin {
                    actions.push(Action::SetBoxStart(Some(pos)));
                }
            }
        }

        // --- Primary click ---
        if response.clicked_by(egui::PointerButton::Primary) {
            if effective_mode == AppMode::Delete {
                if let Some(idx) = press_hit_data {
                    actions.push(Action::RemoveDataPoint(idx));
                }
            } else if effective_mode == AppMode::Select {
                if let Some(idx) = press_hit_calib {
                    actions.push(Action::SelectCalibPoint(idx));
                } else if let Some(idx) = press_hit_data {
                    let is_multi =
                        ctx.input(|i| i.modifiers.shift || i.modifiers.command || i.modifiers.ctrl);
                    actions.push(Action::SelectPoints(vec![idx], is_multi));
                } else {
                    actions.push(Action::ClearSelection);
                }
            } else if state.mode == AppMode::Pan {
                // Do nothing on empty click
            } else if state.texture.is_some() {
                let rect_min = response.rect.min;
                let img_pt = mouse_pos - rect_min - state.pan;
                let img_x = img_pt.x / state.zoom;
                let img_y = img_pt.y / state.zoom;

                if state.mode == AppMode::AddCalib && state.calib_pts.len() < 4 {
                    actions.push(Action::AddCalibPoint { img_x, img_y });
                } else if state.mode == AppMode::AddData {
                    if let Some(idx) = press_hit_data {
                        let is_multi = ctx.input(|i| {
                            i.modifiers.shift || i.modifiers.command || i.modifiers.ctrl
                        });
                        actions.push(Action::SelectPoints(vec![idx], is_multi));
                    } else {
                        actions.push(Action::AddDataPoint { img_x, img_y });
                    }
                }
            }
        }

        // --- Right-click: ensure point is selected for context menu ---
        if response.clicked_by(egui::PointerButton::Secondary) {
            if let Some(idx) = press_hit_data {
                if !state.selected_data_indices.contains(&idx) {
                    actions.push(Action::SelectPoints(vec![idx], false));
                }
            }
        }

        // --- Context menu ---
        response.context_menu(|ui| {
            ui.set_min_width(120.0);
            ui.set_max_width(120.0);
            if !state.selected_data_indices.is_empty() {
                let num_selected = state.selected_data_indices.len();
                ui.label(format!("Options ({} selected)", num_selected));
                ui.separator();

                ui.menu_button("Move to Group", |ui| {
                    for (g_idx, group) in state.groups.iter().enumerate() {
                        let mut btn_text = egui::RichText::new(&group.name);
                        btn_text = btn_text.color(group.color);
                        if ui.button(btn_text).clicked() {
                            actions.push(Action::MovePointsToGroup {
                                indices: state.selected_data_indices.iter().copied().collect(),
                                new_group_id: g_idx,
                            });
                            ui.close();
                        }
                    }
                });

                if ui
                    .button(egui::RichText::new("Delete").color(Color32::RED))
                    .clicked()
                {
                    actions.push(Action::DeleteSelectedPoints);
                    ui.close();
                }
            } else {
                ui.label(
                    egui::RichText::new("Select points \n to see options.").color(Color32::GRAY),
                );
            }
        });

        // --- Drag movement ---
        if response.dragged_by(egui::PointerButton::Primary)
            && state.mode != AppMode::Pan
            && !ctx.input(|i| i.key_down(egui::Key::Space))
        {
            let drag_delta = response.drag_delta() / state.zoom;
            actions.push(Action::MoveSelected {
                dx: drag_delta.x,
                dy: drag_delta.y,
            });
            actions.push(Action::RecalculateData);
        }

        // --- Drag stop ---
        if response.drag_stopped() {
            if state.box_start.is_some() {
                if let Some(start_pos) = state.box_start {
                    let end_pos = mouse_pos;
                    let box_rect = Rect::from_two_pos(start_pos, end_pos);

                    let is_multi =
                        ctx.input(|i| i.modifiers.shift || i.modifiers.command || i.modifiers.ctrl);

                    let mut selected = Vec::new();
                    for (i, p) in state.data_pts.iter().enumerate() {
                        let sp = to_screen(p.px, p.py);
                        if box_rect.contains(sp) {
                            selected.push(i);
                        }
                    }

                    if !selected.is_empty() || !is_multi {
                        actions.push(Action::SelectPoints(selected, is_multi));
                    }
                }
                actions.push(Action::SetBoxStart(None));
            }

            if state.dragging_calib_idx.is_some() || state.dragging_data_idx.is_some() {
                actions.push(Action::StopDragging);
            }
        }
    } else {
        // Mouse left the canvas area
        if state.hovered_calib_idx.is_some() {
            actions.push(Action::SetHoveredCalib(None));
        }
        if state.hovered_data_idx.is_some() {
            actions.push(Action::SetHoveredData(None));
        }
        if response.drag_stopped() && state.box_start.is_some() {
            actions.push(Action::SetBoxStart(None));
        }
    }
}

// ────────────────────────────────────────────────────────────────
//  Mask-mode mouse handling
// ────────────────────────────────────────────────────────────────

fn handle_mask_mouse(
    state: &AppState,
    ctx: &egui::Context,
    response: &egui::Response,
    actions: &mut Vec<Action>,
) {
    // Only paint with left mouse button
    if response.drag_started_by(egui::PointerButton::Primary) {
        actions.push(Action::MaskPaintStart);
        // Paint at the starting position
        if let Some(mouse_pos) = ctx.input(|i| i.pointer.interact_pos()) {
            let rect_min = response.rect.min;
            let img_pt = mouse_pos - rect_min - state.pan;
            let img_x = img_pt.x / state.zoom;
            let img_y = img_pt.y / state.zoom;
            actions.push(Action::MaskPaintStroke { x: img_x, y: img_y });
        }
    }

    if response.dragged_by(egui::PointerButton::Primary) {
        if let Some(mouse_pos) = ctx.input(|i| i.pointer.hover_pos()) {
            let rect_min = response.rect.min;
            let img_pt = mouse_pos - rect_min - state.pan;
            let img_x = img_pt.x / state.zoom;
            let img_y = img_pt.y / state.zoom;
            actions.push(Action::MaskPaintStroke { x: img_x, y: img_y });
        }
    }

    if response.drag_stopped_by(egui::PointerButton::Primary) {
        actions.push(Action::MaskPaintEnd);
    }
}
