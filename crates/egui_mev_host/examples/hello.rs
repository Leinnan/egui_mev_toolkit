//! A small window: `cargo run -p egui_mev_host --example hello`.

use egui_mev_app::{TitleBarOptions, title_bar};
use egui_mev_host::{HostApp, HostError, HostOptions, run};
use egui_mev_theme::ThemeFamily;
use egui_mev_widgets::toggle_switch;

struct Hello {
    on: bool,
}

impl HostApp for Hello {
    fn ui(&mut self, ui: &mut egui::Ui) {
        title_bar(ui, &TitleBarOptions::new("Hello"), |_| {});
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("Hello from egui_mev_host");
            ui.horizontal(|ui| {
                toggle_switch(ui, &mut self.on, "A switch");
                ui.label(if self.on { "On" } else { "Off" });
            });
            ui.label(format!("Family: {:?}", ThemeFamily::of(ui)));
        });
    }
}

fn main() -> Result<(), HostError> {
    let options = HostOptions {
        inner_size: [480.0, 320.0],
        ..HostOptions::new("Hello")
    };
    run(Hello { on: true }, options)
}
