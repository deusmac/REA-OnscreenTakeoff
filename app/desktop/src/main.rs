// Hide the console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use rea_core::units::{format_feet_inches, parse_packed_imperial};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("REA Takeoff")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([900.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "REA Takeoff",
        options,
        Box::new(|_cc| Ok(Box::<TakeoffApp>::default())),
    )
}

#[derive(Default)]
struct TakeoffApp {
    /// Scratch box to try packed feet-inch entry until real dialogs exist.
    entry: String,
}

impl eframe::App for TakeoffApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        egui::Panel::top("menu").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    ui.add_enabled(false, egui::Button::new("New project (M2)"));
                    ui.add_enabled(false, egui::Button::new("Open plan PDF (M1)"));
                    ui.separator();
                    if ui.button("Exit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });
        });

        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("No project open");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("REA Takeoff {}", rea_core::VERSION));
                });
            });
        });

        egui::Panel::left("conditions")
            .default_size(260.0)
            .show(ui, |ui| {
                ui.heading("Conditions");
                ui.separator();
                ui.label("No conditions yet. Linear, area and count conditions arrive in M3.");
            });

        egui::CentralPanel::default().show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() * 0.3);
                ui.heading("Open a plan PDF to begin");
                ui.label("The plan viewer arrives in milestone M1.");
                ui.add_space(24.0);
                ui.label("Try feet-inch entry (last two digits are inches):");
                ui.add(egui::TextEdit::singleline(&mut self.entry).desired_width(120.0));
                let result = match parse_packed_imperial(&self.entry) {
                    Ok(inches) => format!("= {} ({inches} in)", format_feet_inches(inches, 2)),
                    Err(e) => e.to_string(),
                };
                ui.label(result);
            });
        });
    }
}
