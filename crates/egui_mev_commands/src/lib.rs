//! App commands for egui, generic over the app's command type.
//!
//! - [`Shortcut`]: shortcuts as text, typed, saved and displayed.
//! - [`Keymap`]: what runs when keys are pressed, and what tooltips and menus say.
//! - [`KeyOverrides`] and [`KeyRecorder`]: shortcuts the user chose.
//! - [`CommandQueue`]: commands waiting to run, kept in the [`egui::Context`], so
//!   there is no global and parallel test harnesses stay isolated.
//! - [`Menu`] and [`menu_bar`]: one menu table, drawn by egui or, with the
//!   `native_menu` feature on macOS, by `NativeMenuBar`.
//! - [`CommandPalette`]: a fuzzy search over actions that returns the one chosen.
//! - [`help_ui`]: the key bindings as an overlay or settings rows.

mod help;
mod keymap;
mod menu;
mod menu_bar;
#[cfg(all(feature = "native_menu", target_os = "macos"))]
mod native_menu;
mod overrides;
mod palette;
mod queue;
mod shortcut;

pub use help::{HelpRow, HelpSection, help_ui, shortcut_rows};
pub use keymap::{Binding, Keymap};
pub use menu::{Availability, Menu, MenuEntry, MenuItem, Sections, TextEditAction};
pub use menu_bar::menu_bar;
#[cfg(all(feature = "native_menu", target_os = "macos"))]
pub use native_menu::{
    MenuCommands, NativeMenuBar, NativeMenuOptions, accelerator, has_key_equivalent,
};
pub use overrides::{KeyOverrides, KeyRecorder, Recording, capture_key_press, normalize_modifiers};
pub use palette::{CommandPalette, PaletteItem, PaletteLabels, PaletteResponse, palette_items};
pub use queue::CommandQueue;
pub use shortcut::{ParseShortcutError, Shortcut};
