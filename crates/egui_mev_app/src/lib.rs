//! An egui app shell: the parts every app writes around its own screens.
//!
//! Nothing here knows an app. IDs, names and env var names are passed in, and
//! colours come from [`egui_mev_theme::Palette`].
//!
//! - Always: [`handle_window_drag`], [`sidebar_auto_collapse`], [`startup`],
//!   [`paths`] and [`logs`].
//! - `toml` or `json`: `settings`, a value kept in a file.
//! - `eframe`: `native`, a window icon and native-option presets.
//! - `wgpu`: `gpu`, per-OS backends and a present-mode override.
//! - `dialogs`: `dialogs`, native dialogs with a test double.

mod chrome;
#[cfg(all(feature = "dialogs", not(target_arch = "wasm32")))]
pub mod dialogs;
#[cfg(all(feature = "wgpu", not(target_arch = "wasm32")))]
pub mod gpu;
pub mod logs;
#[cfg(all(feature = "eframe", not(target_arch = "wasm32")))]
pub mod native;
pub mod paths;
#[cfg(any(feature = "toml", feature = "json"))]
pub mod settings;
pub mod startup;

pub use chrome::{handle_window_drag, sidebar_auto_collapse};
pub use logs::{LogBuffer, LogCapture, LogModalOptions, log_modal, log_view};
pub use paths::{AppId, AppPaths};
#[cfg(any(feature = "toml", feature = "json"))]
pub use settings::{Format, Settings};
