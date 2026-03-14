use crate::action::Action;
use crate::state::{AppMode, AppState};
use eframe::egui;

pub fn draw_toolbar(
    state: &AppState,
    ui: &mut egui::Ui,
    canvas_rect: egui::Rect,
    actions: &mut Vec<Action>,
) {
    let window = egui::Window::new("CAD Toolbar")
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .anchor(egui::Align2::RIGHT_TOP, [-5.0, 25.0]);

    window.show(ui.ctx(), |ui| {
        ui.horizontal(|ui| {
            if ui
                .selectable_label(state.mode == AppMode::Select, "\u{2196} Select")
                .on_hover_text("Select & Drag")
                .clicked()
            {
                actions.push(Action::SetMode(AppMode::Select));
                actions.push(Action::ClearSelection);
            }
            if ui
                .selectable_label(state.mode == AppMode::AddData, "\u{2795} Add Data")
                .on_hover_text("Pick new points")
                .clicked()
            {
                actions.push(Action::SetMode(AppMode::AddData));
            }

            if ui
                .selectable_label(state.mode == AppMode::Delete, "\u{2796} Delete")
                .on_hover_text("Click points to delete them")
                .clicked()
            {
                actions.push(Action::SetMode(AppMode::Delete));
            }

            let magic_active = state.axis_mask.active
                && state.axis_mask.mask_mode == crate::state::MaskMode::AxisCalib;
            if ui
                .selectable_label(magic_active, "\u{1F4D0} Axis Brush")
                .on_hover_text("Auto-detect axes by painting a mask")
                .clicked()
            {
                actions.push(Action::MaskToggleForAxis);
            }

            let mask_active = state.data_mask.active
                && state.data_mask.mask_mode == crate::state::MaskMode::DataRecog;
            if ui
                .selectable_label(mask_active, "\u{1F5E0} Data Brush")
                .on_hover_text(
                    "Auto-extract data points using color recognition via a painted mask",
                )
                .clicked()
            {
                actions.push(Action::MaskToggle);
            }

            let is_space_pressed = ui.ctx().input(|i| i.key_down(egui::Key::Space));
            if ui
                .selectable_label(
                    state.mode == AppMode::Pan || is_space_pressed,
                    "\u{270B} Pan",
                )
                .on_hover_text("Left-click and drag to pan canvas (or hold Space)")
                .clicked()
            {
                actions.push(Action::SetMode(AppMode::Pan));
            }
            if ui
                .button("⛶ Center")
                .on_hover_text("Center canvas to fit window")
                .clicked()
            {
                actions.push(Action::CenterCanvas(canvas_rect));
            }
        });
    });
}
