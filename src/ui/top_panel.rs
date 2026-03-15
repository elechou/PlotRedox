use crate::action::Action;
use crate::i18n::t;
use crate::icons;
use crate::state::AppState;
use eframe::egui;

pub fn draw(state: &mut AppState, ctx: &egui::Context, actions: &mut Vec<Action>) {
    let lang = state.lang;
    egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            // ── Menu Bar ──────────────────────────────────────────
            ui.menu_button(t(lang, "file"), |ui| {
                if ui.button(t(lang, "new_project")).clicked() {
                    actions.push(Action::NewProject);
                    ui.close();
                }
                ui.separator();
                if ui
                    .add(egui::Button::new(t(lang, "open_project")).shortcut_text(""))
                    .clicked()
                {
                    actions.push(Action::OpenProject);
                    ui.close();
                }
                ui.separator();
                if ui
                    .add(egui::Button::new(t(lang, "save_project")).shortcut_text("Ctrl+S"))
                    .clicked()
                {
                    actions.push(Action::SaveProject);
                    ui.close();
                }
                if ui
                    .add(egui::Button::new(t(lang, "save_project_as")).shortcut_text(""))
                    .clicked()
                {
                    actions.push(Action::SaveProjectAs);
                    ui.close();
                }
            });

            ui.menu_button(t(lang, "edit"), |ui| {
                if ui.button(t(lang, "load_image_ellipsis")).clicked() {
                    crate::ui::panel::load_image(state, ctx, actions);
                    ui.close();
                }
                if ui.button(t(lang, "paste_image")).clicked() {
                    paste_clipboard_image(state, ctx, actions);
                    ui.close();
                }
                ui.separator();
                if ui.button(t(lang, "export_csv")).clicked() {
                    actions.push(Action::RequestExportCsv);
                    ui.close();
                }
            });

            ui.menu_button(t(lang, "about"), |ui| {
                if ui.button(t(lang, "about_plotredox")).clicked() {
                    state.show_about = true;
                    ui.close();
                }
            });

            ui.add_space(176.0);
            ui.add(
                egui::Label::new(egui::RichText::new("|").color(ui.visuals().weak_text_color()))
                    .selectable(false),
            );
            ui.add_space(8.0);

            if ui
                .button(t(lang, "load_image"))
                .on_hover_text(t(lang, "hover_load_image"))
                .clicked()
            {
                crate::ui::panel::load_image(state, ctx, actions);
            }
            if ui
                .button(t(lang, "paste_image"))
                .on_hover_text(t(lang, "hover_paste_image"))
                .clicked()
            {
                paste_clipboard_image(state, ctx, actions);
            }

            if ui
                .button(t(lang, "save_project"))
                .on_hover_text(t(lang, "save_project"))
                .clicked()
            {
                actions.push(Action::SaveProject);
            }

            // Right-aligned Tools
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .selectable_label(state.ide.is_open, format!("{} {}", icons::CODE, t(lang, "script_ide")))
                    .clicked()
                {
                    actions.push(Action::ToggleIDE);
                }

                // ── Quick-Access Toolbar Buttons ──────────────────────
                let is_dark = ctx.style().visuals.dark_mode;
                let icon = if is_dark { icons::SUN } else { icons::MOON };
                if ui.button(icon).on_hover_text(t(lang, "toggle_theme")).clicked() {
                    if is_dark {
                        ctx.set_visuals(egui::Visuals::light());
                    } else {
                        ctx.set_visuals(egui::Visuals::dark());
                    }
                }

                // Language toggle button
                if ui.button(icons::LANGUAGE).on_hover_text(t(lang, "toggle_lang")).clicked() {
                    actions.push(Action::ToggleLang);
                }
            });
        });
    });
}

/// Helper: paste image from clipboard and push LoadImage actions directly
pub fn paste_clipboard_image(
    state: &mut crate::state::AppState,
    ctx: &egui::Context,
    actions: &mut Vec<Action>,
) {
    use clipboard_rs::{common::RustImage, Clipboard, ClipboardContext};
    let mut found = false;
    if let Ok(ctx_cb) = ClipboardContext::new() {
        if ctx_cb.has(clipboard_rs::ContentFormat::Image) {
            if let Ok(image) = ctx_cb.get_image() {
                let (w, h) = image.get_size();
                let size = [w as usize, h as usize];
                let rgba = image
                    .to_rgba8()
                    .expect("Failed to convert clipboard image to RGBA");
                let bytes = rgba.into_raw();

                let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &bytes);
                let handle = ctx.load_texture("clipboard_image", color_image, Default::default());
                let img_size = eframe::egui::Vec2::new(size[0] as f32, size[1] as f32);

                if state.dirty {
                    state.pending_action = Some(crate::state::PendingAction::LoadClipboardImage(
                        handle, img_size, bytes, w as u32, h as u32,
                    ));
                } else {
                    actions.push(Action::LoadClipboardImage(
                        handle, img_size, bytes, w as u32, h as u32,
                    ));
                    actions.push(Action::RequestCenter);
                }
                found = true;
            }
        }
    }
    if !found {
        state.show_clipboard_empty = true;
    }
}
