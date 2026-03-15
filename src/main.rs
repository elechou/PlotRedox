#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub mod action;
mod action_handler;
mod core;
pub mod icons;
mod ide;
mod project;
mod recognition;
mod script;
mod state;
mod ui;

use eframe::egui;
use state::AppState;

struct PlotRedoxApp {
    state: AppState,
}

impl Default for PlotRedoxApp {
    fn default() -> Self {
        Self {
            state: AppState::default(),
        }
    }
}

impl PlotRedoxApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Load Nerd Font symbols as fallback font
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "nerd_symbols".to_owned(),
            std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
                "../assets/SymbolsNerdFontMono-Regular.ttf"
            ))),
        );
        // Add as fallback to both Proportional and Monospace families
        fonts
            .families
            .get_mut(&egui::FontFamily::Proportional)
            .unwrap()
            .push("nerd_symbols".to_owned());
        fonts
            .families
            .get_mut(&egui::FontFamily::Monospace)
            .unwrap()
            .push("nerd_symbols".to_owned());
        cc.egui_ctx.set_fonts(fonts);

        // Customize look
        let mut style = (*cc.egui_ctx.style()).clone();
        style.visuals = egui::Visuals::light();
        cc.egui_ctx.set_style(style);

        Self::default()
    }
}

impl eframe::App for PlotRedoxApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Update window title to reflect project name + dirty state
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(self.state.window_title()));

        // Intercept close event
        if ctx.input(|i| i.viewport().close_requested()) {
            if self.state.dirty {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.state.pending_action = Some(state::PendingAction::CloseApp);
            }
        }

        let mut actions = Vec::new();

        // Drain any async mask results
        if let Some(rx) = &self.state.mask_rx {
            while let Ok(action) = rx.try_recv() {
                actions.push(action);
            }
        }

        ui::draw_ui(&mut self.state, ctx, &mut actions);
        for action in actions {
            match action {
                crate::action::Action::SaveProject => {
                    if let Some(path) = self.state.project_path.clone() {
                        if path.exists() {
                            project::save_project_to_path(&self.state, &path);
                            self.state.dirty = false;
                        } else {
                            // File was moved or deleted, fall back to Save As
                            if let Some(new_path) = project::save_project_as(&self.state) {
                                self.state.project_path = Some(new_path);
                                self.state.dirty = false;
                            }
                        }
                    } else {
                        if let Some(path) = project::save_project_as(&self.state) {
                            self.state.project_path = Some(path);
                            self.state.dirty = false;
                        }
                    }
                }
                crate::action::Action::SaveProjectAs => {
                    if let Some(path) = project::save_project_as(&self.state) {
                        self.state.project_path = Some(path);
                        self.state.dirty = false;
                    }
                }
                crate::action::Action::OpenProject => {
                    if let Some((data, img_bytes, path)) = project::open_project() {
                        if self.state.dirty {
                            self.state.pending_action =
                                Some(state::PendingAction::OpenProject(data, img_bytes, path));
                        } else {
                            let proj_path = path.clone();
                            project::apply_project(&mut self.state, data, &img_bytes, path, ctx);
                            self.state.project_path = Some(proj_path);
                            self.state.dirty = false;
                        }
                    }
                }
                crate::action::Action::NewProject => {
                    if self.state.dirty {
                        self.state.pending_action = Some(state::PendingAction::NewProject);
                    } else {
                        self.state = AppState::default();
                    }
                }
                other => {
                    self.state.update(other);
                }
            }
        }
    }
}

fn load_icon() -> Option<egui::IconData> {
    let icon_bytes = include_bytes!("../assets/icon_exports/icon-iOS-Default-512x512@1x.png");
    let image = image::load_from_memory(icon_bytes).ok()?;
    let image = image.to_rgba8();
    let (width, height) = image.dimensions();
    Some(egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    })
}

fn main() -> eframe::Result<()> {
    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1200.0, 800.0]),
        ..Default::default()
    };

    if let Some(icon) = load_icon() {
        options.viewport = options.viewport.with_icon(icon);
    }

    eframe::run_native(
        "PlotRedox",
        options,
        Box::new(|cc| Ok(Box::new(PlotRedoxApp::new(cc)))),
    )
}
