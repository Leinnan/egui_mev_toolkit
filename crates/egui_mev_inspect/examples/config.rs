//! A settings window: `cargo run -p egui_mev_inspect --example config --features configs`.

use bevy_reflect::{Reflect, std_traits::ReflectDefault};
use egui_mev_inspect::config::ConfigEditorApp;
use serde::{Deserialize, Serialize};

/// Settings for the application.
#[derive(Debug, Default, Clone, PartialEq, Reflect, Serialize, Deserialize)]
#[reflect(Default, Debug)]
pub struct Settings {
    /// Name of the user.
    pub name: String,
    /// Age of the user.
    pub age: u32,
}

struct SettingsWindow(ConfigEditorApp<Settings>);

impl eframe::App for SettingsWindow {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.0.show(ui);
    }
}

fn main() -> eframe::Result {
    let path = std::env::temp_dir().join("egui_mev_inspect_example_settings.toml");
    let editor = ConfigEditorApp::<Settings>::at_path(path);
    eframe::run_native(
        &editor.get_window_name(),
        eframe::NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(SettingsWindow(editor)))),
    )
}
