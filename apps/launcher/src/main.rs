use std::{
    fs,
    path::{Path, PathBuf},
};

use eframe::egui;
use serde::{Deserialize, Serialize};

const APP_NAME: &str = "Skate Threecomp";

#[derive(Default, Serialize, Deserialize)]
struct Settings {
    source_path: Option<PathBuf>,
}

struct Launcher {
    settings: Settings,
    settings_path: PathBuf,
    save_file_path: PathBuf,
    message: Option<String>,
}

impl Launcher {
    fn new() -> Self {
        let project_dirs = directories::ProjectDirs::from("com", "Threecomp", APP_NAME);
        let config_dir = project_dirs
            .as_ref()
            .map(|dirs| dirs.config_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        let data_dir = project_dirs
            .as_ref()
            .map(|dirs| dirs.data_local_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        let settings_path = config_dir.join("settings.json");
        let settings = fs::read(&settings_path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        let save_file_path = data_dir.join("saves").join("skater.p");

        Self {
            settings,
            settings_path,
            save_file_path,
            message: None,
        }
    }

    fn choose_source(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Xbox 360 game files", &["iso", "xex"])
            .add_filter("Disc images", &["iso"])
            .add_filter("Xbox executables", &["xex"])
            .pick_file()
        {
            self.settings.source_path = Some(path);
            self.persist_settings();
            self.message =
                Some("Source selected. The game runtime is planned for a later phase.".into());
        }
    }

    fn persist_settings(&mut self) {
        if let Some(parent) = self.settings_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(bytes) = serde_json::to_vec_pretty(&self.settings) {
            let _ = fs::write(&self.settings_path, bytes);
        }
    }

    fn source_name(&self) -> String {
        self.settings
            .source_path
            .as_ref()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "No game source selected".into())
    }

    fn source_exists(&self) -> bool {
        self.settings
            .source_path
            .as_ref()
            .is_some_and(|path| path.is_file())
    }

    fn open_save_location(&mut self) {
        let folder = self.save_file_path.parent().unwrap_or(Path::new("."));
        if let Err(error) = fs::create_dir_all(folder)
            .and_then(|_| open::that(folder).map_err(std::io::Error::other))
        {
            self.message = Some(format!("Could not open save folder: {error}"));
        }
    }
}

impl eframe::App for Launcher {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_visuals(egui::Visuals::dark());
        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(egui::Color32::from_rgb(14, 18, 20)).inner_margin(egui::Margin::same(30)))
            .show(ctx, |ui| {
                ui.set_max_width(760.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("THREECOMP").size(13.0).strong().color(egui::Color32::from_rgb(184, 242, 86)));
                    ui.add_space(10.0);
                    ui.label(egui::RichText::new("PHASE 01  /  LAUNCHER FOUNDATION").size(11.0).color(egui::Color32::from_rgb(143, 157, 159)));
                });
                ui.add_space(18.0);
                ui.label(egui::RichText::new("SKATE THREECOMP").size(36.0).strong().color(egui::Color32::WHITE));
                ui.label(egui::RichText::new("A clean-room native PC port project").size(16.0).color(egui::Color32::from_rgb(168, 182, 183)));
                ui.add_space(28.0);

                card(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new("GAME SOURCE").size(12.0).strong().color(egui::Color32::from_rgb(184, 242, 86)));
                            ui.add_space(5.0);
                            ui.label(egui::RichText::new(self.source_name()).size(17.0).strong());
                            if let Some(path) = &self.settings.source_path {
                                ui.label(egui::RichText::new(path.display().to_string()).size(12.0).color(egui::Color32::from_rgb(143, 157, 159)));
                            } else {
                                ui.label(egui::RichText::new("Choose your own Xbox 360 .iso or .xex file").size(12.0).color(egui::Color32::from_rgb(143, 157, 159)));
                            }
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Choose file").clicked() { self.choose_source(); }
                        });
                    });
                    ui.add_space(15.0);
                    let state = if self.source_exists() { "SOURCE FOUND" } else { "SOURCE NEEDED" };
                    let color = if self.source_exists() { egui::Color32::from_rgb(184, 242, 86) } else { egui::Color32::from_rgb(244, 190, 95) };
                    ui.label(egui::RichText::new(state).size(11.0).strong().color(color));
                });

                ui.add_space(14.0);
                card(ui, |ui| {
                    ui.label(egui::RichText::new("YOUR SAVE FILE").size(12.0).strong().color(egui::Color32::from_rgb(184, 242, 86)));
                    ui.add_space(7.0);
                    ui.label(egui::RichText::new(self.save_file_path.display().to_string()).size(13.0).monospace());
                    ui.add_space(10.0);
                    if ui.button("Open save folder").clicked() { self.open_save_location(); }
                });

                ui.add_space(22.0);
                ui.horizontal(|ui| {
                    let play = egui::Button::new(egui::RichText::new("PLAY  ·  NOT AVAILABLE YET").strong().color(egui::Color32::from_rgb(88, 98, 94)))
                        .fill(egui::Color32::from_rgb(38, 47, 43));
                    ui.add_enabled(false, play);
                    ui.add_space(12.0);
                    ui.label(egui::RichText::new("The native game runtime is not implemented in this phase.").size(12.0).color(egui::Color32::from_rgb(143, 157, 159)));
                });
                if let Some(message) = &self.message {
                    ui.add_space(10.0);
                    ui.label(egui::RichText::new(message).size(12.0).color(egui::Color32::from_rgb(184, 242, 86)));
                }
                ui.add_space(26.0);
                ui.separator();
                ui.label(egui::RichText::new("Bring your own legally obtained game files. No game code or assets are distributed here.").size(11.0).color(egui::Color32::from_rgb(116, 132, 133)));
            });
    }
}

fn card(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::default()
        .fill(egui::Color32::from_rgb(24, 31, 33))
        .stroke(egui::Stroke::new(
            1.0_f32,
            egui::Color32::from_rgb(43, 55, 56),
        ))
        .corner_radius(egui::CornerRadius::same(9))
        .inner_margin(egui::Margin::same(18))
        .show(ui, add_contents);
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([820.0, 660.0])
            .with_min_inner_size([680.0, 560.0])
            .with_title(APP_NAME),
        ..Default::default()
    };
    eframe::run_native(
        APP_NAME,
        options,
        Box::new(|_cc| Ok(Box::new(Launcher::new()))),
    )
}
