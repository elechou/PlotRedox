use eframe::egui;

use crate::action::Action;
use crate::state::{AppMode, AppState};

/// Handle all canvas-scoped keyboard shortcuts.
///
/// Layer 3 in the keyboard hierarchy:
///   Layer 1 — text_focused: all keys consumed by editor widgets, we skip entirely.
///   Layer 2 — global shortcuts (Ctrl+Z) handled in ui/mod.rs. (Ctrl+V is intentionally disabled)
///   Layer 3 — canvas shortcuts handled here.
pub fn handle_keyboard(
    state: &AppState,
    ctx: &egui::Context,
    _response: &egui::Response,
    actions: &mut Vec<Action>,
) {
    // When a TextEdit / DragValue / etc. has focus, skip all canvas shortcuts
    let text_focused = ctx.wants_keyboard_input();
    if text_focused {
        return;
    }

    // --- Escape: reset to Select mode ---
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        actions.push(Action::SetMode(AppMode::Select));
        actions.push(Action::ClearSelection);
    }

    // --- Delete / Backspace: delete selected points ---
    if ctx.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)) {
        actions.push(Action::DeleteSelectedPoints);
    }

    // --- Arrow keys: nudge selected points ---
    // Use input_mut + consume_key so egui's internal focus-navigation system
    // does NOT eat the arrow keys (it would cycle between panel/canvas/IDE).
    // No hover/focus guard — if no text widget is focused (checked above),
    // arrow keys always nudge selected canvas points.
    let mut nudge_x = 0.0f32;
    let mut nudge_y = 0.0f32;

    ctx.input_mut(|i| {
        if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
            nudge_y -= 1.0;
        }
        if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
            nudge_y += 1.0;
        }
        if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft) {
            nudge_x -= 1.0;
        }
        if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight) {
            nudge_x += 1.0;
        }
    });

    if nudge_x != 0.0 || nudge_y != 0.0 {
        let img_nudge_x = nudge_x / state.zoom;
        let img_nudge_y = nudge_y / state.zoom;
        actions.push(Action::NudgeSelected {
            dx: img_nudge_x,
            dy: img_nudge_y,
        });
        actions.push(Action::RecalculateData);
    }
}
