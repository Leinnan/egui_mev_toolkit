//! An egui app shell: the parts every app writes around its own screens.
//!
//! Nothing here knows an app. IDs, names and env var names are passed in, and
//! colours come from [`egui_mev_theme::Palette`].
//!
//! - Always: [`title_bar`], [`handle_window_drag`], [`sidebar_auto_collapse`],
//!   [`startup`], [`paths`] and [`logs`].
//! - `toml` or `json`: `settings`, a value kept in a file.
//! - `eframe`: `native`, a window icon and native-option presets.
//! - `chrome`: `NativeChrome` and `wants_mica`, and with `eframe`, `ChromeApp`.
//! - `mica`: on Windows, `mica_wgpu_setup`, without eframe.
//! - `wgpu`: `gpu`, per-OS backends and a present-mode override, plus `chrome`
//!   and `mica`.
//! - `dialogs`: `dialogs`, native dialogs with a test double.

mod chrome;
#[cfg(feature = "chrome")]
mod chrome_native;
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

pub use chrome::{
    TOOLBAR_HEIGHT, TRAFFIC_LIGHT_INSET, TitleBarOptions, handle_window_drag,
    sidebar_auto_collapse, title_bar,
};
#[cfg(all(feature = "chrome", feature = "eframe", not(target_arch = "wasm32")))]
pub use chrome_native::ChromeApp;
#[cfg(all(feature = "mica", windows))]
pub use chrome_native::mica_wgpu_setup;
#[cfg(feature = "chrome")]
pub use chrome_native::{NativeChrome, wants_mica};
pub use logs::{LogBuffer, LogCapture, LogModalOptions, log_modal, log_view};
pub use paths::{AppId, AppPaths};
#[cfg(any(feature = "toml", feature = "json"))]
pub use settings::{Format, Settings};
