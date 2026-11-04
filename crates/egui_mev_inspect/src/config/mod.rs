//! A configuration editor for your application, loading and saving one file.
//!
//! [`ConfigEditorApp`] only draws into a [`egui::Ui`]. The app owns the window:
//! see the `config` example for a settings window on eframe.

use std::path::PathBuf;

pub mod serialization;
use bevy_reflect::{CreateTypeData, GetTypeRegistration, Reflect, TypePath};
use egui::Frame;
use egui_mev_core::atomic::{Recovered, load_or_recover};

use crate::config::serialization::ConfigsSerializerExt;

/// Configuration editor for your application.
pub struct ConfigEditorApp<T: bevy_reflect::Reflect + Clone + Default + PartialEq> {
    config: T,
    old_config: T,
    path: Option<PathBuf>,
    registry: bevy_reflect::TypeRegistry,
    undoer: egui::util::undoer::Undoer<T>,
    window_name: Option<String>,
    set_aside: Option<PathBuf>,
    save_error: Option<String>,
}

impl<
    T: bevy_reflect::Reflect
        + PartialEq
        + Clone
        + Default
        + serde::de::DeserializeOwned
        + serde::Serialize,
> ConfigEditorApp<T>
{
    /// Creates a new configuration editor for the given path.
    ///
    /// A missing or unparsable file gives the default config. An unparsable file
    /// is moved aside so the next save can't destroy it: see [`Self::set_aside`].
    pub fn at_path_with_registry(path: PathBuf, registry: bevy_reflect::TypeRegistry) -> Self {
        let recovered =
            load_or_recover(&path, serialization::parse_config::<T>).unwrap_or(Recovered {
                value: T::default(),
                set_aside: None,
            });
        let (config, set_aside) = (recovered.value, recovered.set_aside);
        let old_config = config.clone();
        let undoer = egui::util::undoer::Undoer::default();
        Self {
            config,
            old_config,
            undoer,
            path: Some(path),
            registry,
            window_name: None,
            set_aside,
            save_error: None,
        }
    }
    /// Creates a new configuration editor for the given path.
    pub fn at_path(path: PathBuf) -> Self {
        Self::at_path_with_registry(path, crate::create_registry())
    }

    /// Where an unparsable config file was moved when this editor opened it, so
    /// the app can tell the user.
    #[must_use]
    pub fn set_aside(&self) -> Option<&std::path::Path> {
        self.set_aside.as_deref()
    }

    /// Sets the window name for the configuration editor.
    pub fn with_window_name(mut self, name: impl ToString) -> Self {
        self.window_name = Some(name.to_string());
        self
    }

    /// Checks if the configuration has been modified.
    pub fn is_modified(&self) -> bool {
        self.config != self.old_config
    }

    /// Returns a reference to the inner configuration value.
    pub fn config(&self) -> &T {
        &self.config
    }

    /// Returns a mutable reference to the inner configuration value.
    pub fn config_mut(&mut self) -> &mut T {
        &mut self.config
    }

    /// Saves the configuration to the path specified in the constructor.
    pub fn save_to_file(&mut self) -> Result<(), std::io::Error> {
        let config = &self.config;
        let path = self.path.as_ref().ok_or(std::io::Error::other("No path"))?;

        self.registry.try_save_config(config, path)?;
        self.old_config = config.clone();
        Ok(())
    }

    /// Returns the window name of the configuration editor.
    pub fn get_window_name(&self) -> String {
        self.window_name
            .clone()
            .unwrap_or_else(|| "Configuration editor".to_string())
    }

    /// One frame of the whole editor: records undo history, then draws the
    /// toolbar and the fields.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        self.update_undo(ui.ctx());
        self.show_top_panel(ui);
        egui::CentralPanel::default().show(ui, |ui| {
            self.show_editor(ui);
        });
    }

    pub fn show_editor(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            crate::show(
                &self.registry,
                ui,
                ui.make_persistent_id("egui_mev_inspect_config"),
                self.config.as_partial_reflect_mut(),
                None,
            );
        });
    }

    pub fn undo_redo_save_horizontal(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add_enabled_ui(self.is_modified(), |ui| {
                if ui.button("Save").clicked() {
                    self.save_error = self.save_to_file().err().map(|error| error.to_string());
                }
            });
            ui.add_space(10.0);
            let has_undo = self.undoer.has_undo(&self.config);
            let has_redo = self.undoer.has_redo(&self.config);
            ui.add_enabled_ui(has_undo, |ui| {
                if ui.button("Undo").clicked()
                    && let Some(data) = self.undoer.undo(&self.config)
                {
                    self.config = data.clone();
                }
            });
            ui.add_enabled_ui(has_redo, |ui| {
                if ui.button("Redo").clicked()
                    && let Some(data) = self.undoer.redo(&self.config)
                {
                    self.config = data.clone();
                }
            });
            if let Some(error) = &self.save_error {
                ui.colored_label(
                    egui_mev_theme::Palette::of(ui).critical,
                    format!("Could not save: {error}"),
                );
            }
        });
    }

    pub fn update_undo(&mut self, ctx: &egui::Context) {
        self.undoer
            .feed_state(ctx.input(|input| input.time), &self.config);
    }

    /// Returns whether an older configuration state can be restored.
    #[must_use]
    pub fn has_undo(&self) -> bool {
        self.undoer.has_undo(&self.config)
    }

    /// Returns whether a newer configuration state can be restored.
    #[must_use]
    pub fn has_redo(&self) -> bool {
        self.undoer.has_redo(&self.config)
    }

    /// Restores the previous configuration state.
    ///
    /// Returns `true` when a state was restored.
    pub fn undo(&mut self) -> bool {
        let Some(data) = self.undoer.undo(&self.config) else {
            return false;
        };
        self.config = data.clone();
        true
    }

    /// Restores the next configuration state.
    ///
    /// Returns `true` when a state was restored.
    pub fn redo(&mut self) -> bool {
        let Some(data) = self.undoer.redo(&self.config) else {
            return false;
        };
        self.config = data.clone();
        true
    }

    pub fn show_top_panel(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("egui_mev_inspect_config_toolbar")
            .frame(Frame::canvas(ui.style()).inner_margin(8))
            .show(ui, |ui| {
                self.undo_redo_save_horizontal(ui);
            });
    }
}

/// ConfigBuilder is a builder for creating a Config.
pub struct ConfigBuilder {
    registry: bevy_reflect::TypeRegistry,
    window_name: String,
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigBuilder {
    /// Creates a new `ConfigBuilder`.
    pub fn new() -> Self {
        Self {
            registry: crate::create_registry(),
            window_name: "Settings".to_string(),
        }
    }

    /// Sets the window name for the configuration editor.
    pub fn with_window_name(mut self, name: impl ToString) -> Self {
        self.window_name = name.to_string();
        self
    }

    /// Registers a type data for the given type.
    pub fn with_data_type<T, D>(mut self) -> Self
    where
        T: Reflect + TypePath,
        D: CreateTypeData<T>,
    {
        self.registry.register_type_data::<T, D>();
        self
    }

    /// Registers given type in the registry.
    pub fn with_type<T: GetTypeRegistration>(mut self) -> Self {
        self.registry.register::<T>();
        self
    }

    /// Builds a `ConfigEditorApp` with the given path and registry.
    pub fn build<
        T: bevy_reflect::Reflect
            + Clone
            + Default
            + PartialEq
            + serde::de::DeserializeOwned
            + serde::Serialize,
    >(
        self,
        path: PathBuf,
    ) -> ConfigEditorApp<T> {
        ConfigEditorApp::at_path_with_registry(path, self.registry)
            .with_window_name(self.window_name)
    }
}
