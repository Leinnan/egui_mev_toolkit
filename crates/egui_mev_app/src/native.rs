//! Presets for [`eframe::NativeOptions`]. eframe comes with no renderer from
//! this crate: the app enables glow or wgpu on its own eframe dependency, or
//! this crate's `wgpu` feature. eframe doesn't build without one.

use egui::{IconData, ViewportBuilder};

/// Decode a PNG file into a window icon.
pub fn window_icon(png: &[u8]) -> Result<IconData, Box<dyn std::error::Error + Send + Sync>> {
    Ok(eframe::icon_data::from_png_bytes(png)?)
}

/// A window with a title, a starting size and a minimum size.
#[must_use]
pub fn window(
    title: impl Into<String>,
    inner_size: [f32; 2],
    min_inner_size: [f32; 2],
) -> ViewportBuilder {
    ViewportBuilder::default()
        .with_title(title)
        .with_inner_size(inner_size)
        .with_min_inner_size(min_inner_size)
}

/// macOS: extend content under a transparent title bar, so the app's toolbar and
/// the traffic-light buttons share one bar. Pair it with
/// [`crate::handle_window_drag`] on the toolbar. Other systems are unaffected.
#[must_use]
pub fn unified_titlebar(viewport: ViewportBuilder) -> ViewportBuilder {
    viewport
        .with_fullsize_content_view(cfg!(target_os = "macos"))
        .with_titlebar_shown(!cfg!(target_os = "macos"))
        .with_title_shown(!cfg!(target_os = "macos"))
}

/// Native options for `viewport`, with everything else at eframe's defaults.
#[must_use]
pub fn native_options(viewport: ViewportBuilder) -> eframe::NativeOptions {
    eframe::NativeOptions {
        viewport,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::{native_options, unified_titlebar, window, window_icon};

    /// A one-pixel RGBA PNG.
    const PIXEL_PNG: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0xd, 0xa, 0x1a, 0xa, 0x0, 0x0, 0x0, 0xd, 0x49, 0x48, 0x44, 0x52,
        0x0, 0x0, 0x0, 0x1, 0x0, 0x0, 0x0, 0x1, 0x8, 0x6, 0x0, 0x0, 0x0, 0x1f, 0x15, 0xc4, 0x89,
        0x0, 0x0, 0x0, 0xd, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x10, 0x54, 0x32, 0xfe, 0xf,
        0x0, 0x2, 0x14, 0x1, 0x66, 0x4, 0xe0, 0xde, 0x7f, 0x0, 0x0, 0x0, 0x0, 0x49, 0x45, 0x4e,
        0x44, 0xae, 0x42, 0x60, 0x82,
    ];

    #[test]
    fn window_icon_decodes_a_png() {
        let icon = window_icon(PIXEL_PNG).expect("should decode");
        assert_eq!((icon.width, icon.height), (1, 1));
        assert_eq!(icon.rgba, [0x11, 0x22, 0x33, 0xff]);
    }

    #[test]
    fn window_icon_reports_bad_bytes() {
        assert!(window_icon(b"not a png").is_err());
    }

    #[test]
    fn window_sets_title_and_sizes() {
        let viewport = window("Title", [800.0, 600.0], [400.0, 300.0]);
        assert_eq!(viewport.title.as_deref(), Some("Title"));
        assert_eq!(viewport.inner_size, Some(egui::vec2(800.0, 600.0)));
        assert_eq!(viewport.min_inner_size, Some(egui::vec2(400.0, 300.0)));
    }

    #[test]
    fn unified_titlebar_only_changes_macos() {
        let viewport = unified_titlebar(window("Title", [800.0, 600.0], [400.0, 300.0]));
        let macos = cfg!(target_os = "macos");
        assert_eq!(viewport.fullsize_content_view, Some(macos));
        assert_eq!(viewport.titlebar_shown, Some(!macos));
        assert_eq!(viewport.title_shown, Some(!macos));
    }

    #[test]
    fn native_options_keeps_the_viewport() {
        let options = native_options(window("Title", [800.0, 600.0], [400.0, 300.0]));
        assert_eq!(options.viewport.title.as_deref(), Some("Title"));
    }
}
