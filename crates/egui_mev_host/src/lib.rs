//! A raw winit and wgpu window for egui, for what eframe can't do: opening a
//! window inside an event loop the app already runs, and hosting Mica on Windows,
//! which needs a window without a redirection bitmap.
//!
//! - [`run`] opens one window and runs until it closes.
//! - [`HostedWindow`] is the same window for an app with its own event loop.
//!
//! Either way the app is a [`HostApp`] and [`HostOptions`] describes the window.
//! Apps that don't need either of those are better off with eframe.
//!
//! Fonts come from [`egui_mev_theme::fonts`]. Where the system has none of them,
//! text needs [`egui_mev_theme::FontOptions::bundled_ui`] or egui's `default_fonts`.

mod run;
mod window;

use std::fmt;

use egui_mev_theme::{FontOptions, Theme};

pub use run::run;
pub use window::HostedWindow;

/// What a window shows.
pub trait HostApp {
    /// Draw one frame. `ui` covers the whole window and has no background of its
    /// own, so use panels, which also lets Mica show through their chrome.
    fn ui(&mut self, ui: &mut egui::Ui);
}

/// How to create a window.
#[derive(Clone, Debug)]
pub struct HostOptions {
    /// The window's title.
    pub title: String,
    /// The starting size of the content, in points.
    pub inner_size: [f32; 2],
    /// The smallest the content can get, in points.
    pub min_inner_size: [f32; 2],
    /// The window icon, where the OS shows one.
    pub icon: Option<egui::IconData>,
    /// The look. A Fluent theme gets Mica on Windows where the OS has it, and the
    /// host installs the theme's Mica variant by itself.
    pub theme: Theme,
    /// What the app adds to the platform's fonts.
    pub fonts: FontOptions,
}

impl HostOptions {
    /// A window with this title and the OS's own look.
    #[must_use]
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            inner_size: [960.0, 720.0],
            min_inner_size: [420.0, 320.0],
            icon: None,
            theme: Theme::system(),
            fonts: FontOptions::default(),
        }
    }
}

/// Why a window couldn't be created or run.
#[derive(Debug)]
pub enum HostError {
    /// The OS wouldn't create the window.
    WindowCreate(winit::error::OsError),
    /// The event loop failed to start or run.
    EventLoop(winit::error::EventLoopError),
    /// Neither the hardware nor the fallback adapter could render.
    WgpuInit(String),
    /// wgpu couldn't render to the window.
    Wgpu(egui_wgpu::WgpuError),
}

impl fmt::Display for HostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WindowCreate(error) => write!(f, "failed to create window: {error}"),
            Self::EventLoop(error) => write!(f, "event loop failed: {error}"),
            Self::WgpuInit(message) => write!(f, "wgpu initialization failed: {message}"),
            Self::Wgpu(error) => write!(f, "wgpu error: {error}"),
        }
    }
}

impl std::error::Error for HostError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::WindowCreate(error) => Some(error),
            Self::EventLoop(error) => Some(error),
            Self::WgpuInit(_) => None,
            Self::Wgpu(error) => Some(error),
        }
    }
}

impl From<winit::error::OsError> for HostError {
    fn from(error: winit::error::OsError) -> Self {
        Self::WindowCreate(error)
    }
}

impl From<winit::error::EventLoopError> for HostError {
    fn from(error: winit::error::EventLoopError) -> Self {
        Self::EventLoop(error)
    }
}

impl From<egui_wgpu::WgpuError> for HostError {
    fn from(error: egui_wgpu::WgpuError) -> Self {
        Self::Wgpu(error)
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error as _;

    use super::{HostError, HostOptions};

    #[test]
    fn options_default_to_the_systems_look() {
        let options = HostOptions::new("Title");
        assert_eq!(options.title, "Title");
        assert!(options.icon.is_none());
        assert_eq!(
            options.theme.family,
            egui_mev_theme::ThemeFamily::platform_default()
        );
        assert!(options.min_inner_size[0] <= options.inner_size[0]);
        assert!(options.min_inner_size[1] <= options.inner_size[1]);
    }

    #[test]
    fn errors_say_what_failed() {
        let error = HostError::WgpuInit("no adapter".to_owned());
        assert_eq!(error.to_string(), "wgpu initialization failed: no adapter");
        assert!(error.source().is_none());
    }
}
