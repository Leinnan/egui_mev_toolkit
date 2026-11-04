//! Native window chrome that follows the theme. It only does anything on Windows.

use egui::Context;
use egui_mev_theme::{AccentShades, Palette, Theme, ThemeFamily, fluent};
use raw_window_handle::HasWindowHandle;

/// Native window chrome (rounded corners, caption colour, backdrop) kept in step
/// with the context's light or dark mode. Keep one per window.
///
/// A transparent window's backdrop is up to the OS, so this also settles the
/// theme: the Mica variant of the installed Fluent theme once Mica is showing,
/// and the opaque `fluent::theme(accent)` if a see-through palette would
/// otherwise sit on a window that has no Mica.
#[derive(Debug)]
pub struct NativeChrome {
    accent: AccentShades,
    /// Whether the window has a transparent swapchain for Mica to show through.
    transparent: bool,
    mica_showing: bool,
    #[cfg(windows)]
    inner: egui_mev_theme::win32::WindowChrome,
}

impl NativeChrome {
    /// Chrome for an ordinary opaque window: a solid backdrop and a caption in
    /// the Fluent colours.
    #[must_use]
    pub fn new(accent: AccentShades) -> Self {
        Self {
            accent,
            transparent: false,
            mica_showing: false,
            #[cfg(windows)]
            inner: egui_mev_theme::win32::WindowChrome::default(),
        }
    }

    /// Chrome for a window with a transparent swapchain and no redirection
    /// bitmap, which asks for the Mica backdrop. Only a window created for it
    /// can show Mica; see [`wants_mica`].
    #[must_use]
    pub fn transparent(accent: AccentShades) -> Self {
        Self {
            transparent: true,
            ..Self::new(accent)
        }
    }

    /// Bring `window` and the installed theme up to date. Call it every frame.
    /// It does nothing off Windows or while a family other than Fluent is
    /// installed, and retries next frame if `window` has no handle yet.
    pub fn sync(&mut self, ctx: &Context, window: &impl HasWindowHandle) {
        #[cfg(windows)]
        if ThemeFamily::of_ctx(ctx) == ThemeFamily::Fluent {
            use egui_mev_theme::win32::Backdrop;
            let requested = if self.transparent {
                Backdrop::Mica
            } else {
                Backdrop::Solid
            };
            let theme = ctx.theme();
            // The caption can't be see-through, whichever theme is installed.
            self.inner
                .sync(window, requested, theme, &self.opaque_palette(theme));
            if let Some(backdrop) = self.inner.backdrop() {
                self.settle(ctx, backdrop == Backdrop::Mica);
            }
        }
        #[cfg(not(windows))]
        let _ = (ctx, window);
    }

    /// Records what the OS gave the window and makes the theme agree with it.
    /// Themes the app installed that already agree are left alone, so custom
    /// metrics and a re-installed accent survive.
    #[cfg_attr(
        not(windows),
        allow(dead_code, reason = "only `sync` on Windows calls it")
    )]
    fn settle(&mut self, ctx: &Context, mica: bool) {
        self.mica_showing = mica;
        let installed = Theme::of(ctx);
        let replacement = match (mica, is_see_through(&installed)) {
            (true, false) => Some(mica_variant_of(&installed)),
            (false, true) => Some(fluent::theme(self.accent)),
            _ => None,
        };
        if let Some(replacement) = replacement {
            install_keeping_text_styles(replacement, ctx);
            ctx.request_repaint();
        }
    }

    /// The colour to clear the window with: `None` for an opaque window, which
    /// keeps the app's own, fully transparent while Mica shows through, otherwise
    /// the opaque colour of the window's base.
    #[must_use]
    pub fn clear_color(&self, ctx: &Context) -> Option<[f32; 4]> {
        if !self.transparent {
            return None;
        }
        let fluent = ThemeFamily::of_ctx(ctx) == ThemeFamily::Fluent;
        if fluent && self.mica_showing {
            return Some([0.0; 4]);
        }
        let palette = if fluent {
            self.opaque_palette(ctx.theme())
        } else {
            Palette::of_ctx(ctx)
        };
        Some(palette.sidebar_bg.to_normalized_gamma_f32())
    }

    fn opaque_palette(&self, mode: egui::Theme) -> Palette {
        match mode {
            egui::Theme::Dark => fluent::fluent_dark(self.accent),
            egui::Theme::Light => fluent::fluent_light(self.accent),
        }
    }
}

/// Whether a theme expects a backdrop to show through its chrome.
fn is_see_through(theme: &Theme) -> bool {
    theme.dark.toolbar_bg.a() < u8::MAX || theme.light.toolbar_bg.a() < u8::MAX
}

/// `theme` over Mica, keeping everything else about it. For `fluent::theme(accent)`
/// this is `fluent::mica_theme(accent)`.
fn mica_variant_of(theme: &Theme) -> Theme {
    Theme {
        dark: fluent::mica_variant(&theme.dark, egui::Theme::Dark),
        light: fluent::mica_variant(&theme.light, egui::Theme::Light),
        ..theme.clone()
    }
}

/// Installing rebuilds the styles, which would lose text styles set after the
/// theme went in, like the semibold heading of `Theme::install_with_fonts`.
fn install_keeping_text_styles(theme: Theme, ctx: &Context) {
    let keep = |mode| ctx.style_of(mode).text_styles.clone();
    let (dark, light) = (keep(egui::Theme::Dark), keep(egui::Theme::Light));
    theme.install(ctx);
    ctx.style_mut_of(egui::Theme::Dark, |style| style.text_styles = dark);
    ctx.style_mut_of(egui::Theme::Light, |style| style.text_styles = light);
}

/// Whether a window for `family` should be created transparent to host Mica: only
/// Fluent on Windows 10 and later. Windows 7 and 8 have no DirectX 12, so their
/// windows stay opaque and render through another backend. Mica needs a window
/// without a redirection bitmap, which eframe can't create.
#[must_use]
pub fn wants_mica(family: ThemeFamily) -> bool {
    #[cfg(windows)]
    {
        family == ThemeFamily::Fluent && egui_mev_theme::win32::supports_transparent_swapchain()
    }
    #[cfg(not(windows))]
    {
        let _ = family;
        false
    }
}

/// wgpu setup for a transparent window that hosts Mica: DirectX 12 with a
/// `DirectComposition` swapchain, the only Windows presentation path with
/// per-pixel transparency.
///
/// The window must also be created without a redirection bitmap
/// (`WS_EX_NOREDIRECTIONBITMAP`), or the GDI surface hides Mica. eframe doesn't
/// expose that flag, so this is for raw winit hosts.
#[cfg(all(feature = "mica", windows))]
#[must_use]
pub fn mica_wgpu_setup() -> egui_wgpu::WgpuSetupCreateNew {
    let mut setup = egui_wgpu::WgpuSetupCreateNew::without_display_handle();
    setup.instance_descriptor.backends = wgpu::Backends::DX12;
    setup
        .instance_descriptor
        .backend_options
        .dx12
        .presentation_system = wgpu::Dx12SwapchainKind::DxgiFromVisual;
    setup
}

/// Wraps an [`eframe::App`] so its native window chrome follows the light or dark
/// mode (see [`NativeChrome`]). The chrome is always solid, because eframe can't
/// create the window Mica needs.
///
/// `App::on_exit` doesn't reach the wrapped app: its signature depends on whether
/// eframe has `glow`, which this crate can't see. Clean up in `Drop` instead.
#[cfg(all(feature = "eframe", not(target_arch = "wasm32")))]
pub struct ChromeApp<A> {
    app: A,
    chrome: NativeChrome,
}

#[cfg(all(feature = "eframe", not(target_arch = "wasm32")))]
impl<A> ChromeApp<A> {
    /// Wraps `app`. `accent` is the one the app's Fluent theme uses.
    #[must_use]
    pub fn new(app: A, accent: AccentShades) -> Self {
        Self {
            app,
            chrome: NativeChrome::new(accent),
        }
    }
}

#[cfg(all(feature = "eframe", not(target_arch = "wasm32")))]
impl<A: eframe::App> eframe::App for ChromeApp<A> {
    fn logic(&mut self, ctx: &Context, frame: &mut eframe::Frame) {
        self.chrome.sync(ctx, frame);
        self.app.logic(ctx, frame);
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.app.ui(ui, frame);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.app.save(storage);
    }

    fn auto_save_interval(&self) -> std::time::Duration {
        self.app.auto_save_interval()
    }

    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        self.app.clear_color(visuals)
    }

    fn persist_egui_memory(&self) -> bool {
        self.app.persist_egui_memory()
    }

    fn raw_input_hook(&mut self, ctx: &Context, raw_input: &mut egui::RawInput) {
        self.app.raw_input_hook(ctx, raw_input);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use egui::{Color32, Context, FontFamily, FontId, TextStyle};
    use egui_mev_theme::{AccentShades, Metrics, Palette, Theme, ThemeFamily, fluent};
    use raw_window_handle::{HandleError, HasWindowHandle, WindowHandle};

    use super::{NativeChrome, mica_variant_of, wants_mica};

    const ACCENT: AccentShades = AccentShades::WINDOWS_DEFAULT;
    const CLEAR: [f32; 4] = [0.0; 4];

    /// A window whose handle isn't available.
    struct NoHandle;

    impl HasWindowHandle for NoHandle {
        fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
            Err(HandleError::Unavailable)
        }
    }

    fn context(theme: Theme, mode: egui::Theme) -> Context {
        let ctx = Context::default();
        theme.install(&ctx);
        ctx.set_theme(mode);
        ctx
    }

    fn opaque_base(mode: egui::Theme) -> [f32; 4] {
        match mode {
            egui::Theme::Dark => fluent::fluent_dark(ACCENT).sidebar_bg,
            egui::Theme::Light => fluent::fluent_light(ACCENT).sidebar_bg,
        }
        .to_normalized_gamma_f32()
    }

    #[test]
    fn an_opaque_window_keeps_the_apps_clear_colour() {
        let ctx = context(fluent::theme(ACCENT), egui::Theme::Dark);
        let mut chrome = NativeChrome::new(ACCENT);
        assert_eq!(chrome.clear_color(&ctx), None);
        chrome.settle(&ctx, false);
        assert_eq!(chrome.clear_color(&ctx), None);
    }

    #[test]
    fn a_transparent_window_clears_opaque_until_mica_is_accepted() {
        for mode in [egui::Theme::Dark, egui::Theme::Light] {
            // Even with the Mica palette already in, whose base is see-through.
            let ctx = context(fluent::mica_theme(ACCENT), mode);
            let chrome = NativeChrome::transparent(ACCENT);
            assert_eq!(chrome.clear_color(&ctx), Some(opaque_base(mode)));
        }
    }

    #[test]
    fn accepted_mica_clears_transparent_and_installs_the_mica_theme() {
        for mode in [egui::Theme::Dark, egui::Theme::Light] {
            let ctx = context(fluent::theme(ACCENT), mode);
            let mut chrome = NativeChrome::transparent(ACCENT);
            chrome.settle(&ctx, true);
            assert_eq!(chrome.clear_color(&ctx), Some(CLEAR));
            let expected = fluent::mica_theme(ACCENT);
            let installed = Theme::of(&ctx);
            assert_eq!(installed.dark, expected.dark);
            assert_eq!(installed.light, expected.light);
            assert_eq!(Palette::of_ctx(&ctx).toolbar_bg, Color32::TRANSPARENT);
        }
    }

    #[test]
    fn rejected_mica_installs_the_opaque_theme_and_clears_opaque() {
        for mode in [egui::Theme::Dark, egui::Theme::Light] {
            let ctx = context(fluent::mica_theme(ACCENT), mode);
            let mut chrome = NativeChrome::transparent(ACCENT);
            chrome.settle(&ctx, false);
            let expected = fluent::theme(ACCENT);
            let installed = Theme::of(&ctx);
            assert_eq!(installed.dark, expected.dark);
            assert_eq!(installed.light, expected.light);
            assert_eq!(chrome.clear_color(&ctx), Some(opaque_base(mode)));
        }
    }

    #[test]
    fn an_opaque_window_never_keeps_a_see_through_palette() {
        let ctx = context(fluent::mica_theme(ACCENT), egui::Theme::Dark);
        let mut chrome = NativeChrome::new(ACCENT);
        chrome.settle(&ctx, false);
        assert_eq!(Palette::of_ctx(&ctx), fluent::fluent_dark(ACCENT));
    }

    #[test]
    fn a_theme_that_already_agrees_is_not_touched() {
        let ctx = context(fluent::theme(ACCENT), egui::Theme::Dark);
        let before = Theme::of(&ctx);
        let mut chrome = NativeChrome::new(ACCENT);
        chrome.settle(&ctx, false);
        assert!(Arc::ptr_eq(&before, &Theme::of(&ctx)));

        let mut chrome = NativeChrome::transparent(ACCENT);
        chrome.settle(&ctx, true);
        let settled = Theme::of(&ctx);
        chrome.settle(&ctx, true);
        assert!(Arc::ptr_eq(&settled, &Theme::of(&ctx)));
    }

    #[test]
    fn mica_keeps_what_the_app_chose_about_its_theme() {
        let custom_accent = AccentShades::from_base(Color32::from_rgb(0xC0, 0x30, 0x60));
        let metrics = Metrics {
            card_radius: 3,
            ..Metrics::FLUENT
        };
        let ctx = context(
            fluent::theme(custom_accent).with_metrics(metrics),
            egui::Theme::Dark,
        );
        let mut chrome = NativeChrome::transparent(ACCENT);
        chrome.settle(&ctx, true);
        assert_eq!(Metrics::of_ctx(&ctx), metrics);
        assert_eq!(
            Palette::of_ctx(&ctx).accent,
            fluent::fluent_dark(custom_accent).accent
        );
        assert_eq!(ThemeFamily::of_ctx(&ctx), ThemeFamily::Fluent);
    }

    /// `Theme::install_with_fonts` sets the semibold heading after installing.
    #[test]
    fn swapping_the_theme_keeps_text_styles() {
        let ctx = context(fluent::theme(ACCENT), egui::Theme::Dark);
        let heading = FontId::new(33.0, FontFamily::Monospace);
        ctx.all_styles_mut(|style| {
            style
                .text_styles
                .insert(TextStyle::Heading, heading.clone());
        });
        NativeChrome::transparent(ACCENT).settle(&ctx, true);
        for mode in [egui::Theme::Dark, egui::Theme::Light] {
            assert_eq!(
                ctx.style_of(mode).text_styles[&TextStyle::Heading],
                heading,
                "{mode:?}"
            );
        }
    }

    #[test]
    fn the_derived_mica_theme_matches_fluents_own() {
        let derived = mica_variant_of(&fluent::theme(ACCENT));
        let own = fluent::mica_theme(ACCENT);
        assert_eq!(derived.dark, own.dark);
        assert_eq!(derived.light, own.light);
        assert_eq!(derived.metrics, own.metrics);
        assert_eq!(derived.family, own.family);
    }

    #[test]
    fn the_caption_palette_is_opaque_whichever_theme_is_installed() {
        let chrome = NativeChrome::transparent(ACCENT);
        for mode in [egui::Theme::Dark, egui::Theme::Light] {
            let palette = chrome.opaque_palette(mode);
            assert_eq!(palette.toolbar_bg.a(), u8::MAX);
            assert_eq!(palette.text.a(), u8::MAX);
        }
    }

    #[test]
    fn sync_does_nothing_under_graphite() {
        let ctx = context(
            Theme::graphite(AccentShades::MACOS_DEFAULT),
            egui::Theme::Dark,
        );
        let before = Theme::of(&ctx);
        let mut chrome = NativeChrome::transparent(ACCENT);
        chrome.sync(&ctx, &NoHandle);
        assert!(Arc::ptr_eq(&before, &Theme::of(&ctx)));
        assert!(!chrome.mica_showing);
    }

    #[cfg(not(windows))]
    #[test]
    fn sync_does_nothing_off_windows() {
        let ctx = context(fluent::mica_theme(ACCENT), egui::Theme::Dark);
        let before = Theme::of(&ctx);
        let mut chrome = NativeChrome::new(ACCENT);
        chrome.sync(&ctx, &NoHandle);
        assert!(Arc::ptr_eq(&before, &Theme::of(&ctx)));
        assert!(!chrome.mica_showing);
    }

    /// A handle that isn't ready yet settles nothing, so the next frame retries.
    #[test]
    fn sync_without_a_handle_leaves_the_theme_alone() {
        let ctx = context(fluent::theme(ACCENT), egui::Theme::Dark);
        let before = Theme::of(&ctx);
        let mut chrome = NativeChrome::transparent(ACCENT);
        chrome.sync(&ctx, &NoHandle);
        assert!(Arc::ptr_eq(&before, &Theme::of(&ctx)));
        assert_eq!(
            chrome.clear_color(&ctx),
            Some(opaque_base(egui::Theme::Dark))
        );
    }

    #[test]
    fn switching_a_mica_window_to_another_family_clears_opaque() {
        let ctx = context(fluent::theme(ACCENT), egui::Theme::Dark);
        let mut chrome = NativeChrome::transparent(ACCENT);
        chrome.settle(&ctx, true);

        let graphite = Theme::graphite(AccentShades::MACOS_DEFAULT);
        let base = graphite.dark.sidebar_bg.to_normalized_gamma_f32();
        graphite.install(&ctx);
        assert_eq!(chrome.clear_color(&ctx), Some(base));
    }

    #[test]
    fn wants_mica_is_false_for_graphite() {
        assert!(!wants_mica(ThemeFamily::Graphite));
    }

    #[cfg(not(windows))]
    #[test]
    fn wants_mica_is_false_off_windows() {
        assert!(!wants_mica(ThemeFamily::Fluent));
    }

    #[cfg(all(feature = "eframe", not(target_arch = "wasm32")))]
    mod chrome_app {
        use std::cell::Cell;
        use std::rc::Rc;
        use std::time::Duration;

        use eframe::App as _;

        use super::ACCENT;
        use crate::ChromeApp;

        #[derive(Default)]
        struct Calls {
            logic: Cell<u32>,
            ui: Cell<u32>,
            saved: Cell<u32>,
            hooked: Cell<u32>,
        }

        struct Inner(Rc<Calls>);

        impl eframe::App for Inner {
            fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
                self.0.logic.set(self.0.logic.get() + 1);
            }
            fn ui(&mut self, _ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
                self.0.ui.set(self.0.ui.get() + 1);
            }
            fn save(&mut self, _storage: &mut dyn eframe::Storage) {
                self.0.saved.set(self.0.saved.get() + 1);
            }
            fn auto_save_interval(&self) -> Duration {
                Duration::from_secs(7)
            }
            fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
                [0.25; 4]
            }
            fn persist_egui_memory(&self) -> bool {
                false
            }
            fn raw_input_hook(&mut self, _ctx: &egui::Context, _raw: &mut egui::RawInput) {
                self.0.hooked.set(self.0.hooked.get() + 1);
            }
        }

        struct NoStorage;

        impl eframe::Storage for NoStorage {
            fn get_string(&self, _key: &str) -> Option<String> {
                None
            }
            fn set_string(&mut self, _key: &str, _value: String) {}
            fn remove_string(&mut self, _key: &str) {}
            fn flush(&mut self) {}
        }

        /// Dropping any of these would lose an app's settings or its frames.
        #[test]
        fn the_wrapped_app_still_gets_everything_eframe_sends() {
            let calls = Rc::new(Calls::default());
            let mut app = ChromeApp::new(Inner(Rc::clone(&calls)), ACCENT);
            let mut frame = eframe::Frame::_new_kittest();
            let ctx = egui::Context::default();

            app.raw_input_hook(&ctx, &mut egui::RawInput::default());
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                app.logic(ui.ctx(), &mut frame);
                app.ui(ui, &mut frame);
            });
            output.textures_delta.clear();
            app.save(&mut NoStorage);

            assert_eq!(calls.hooked.get(), 1);
            assert!(calls.logic.get() >= 1);
            assert_eq!(calls.logic.get(), calls.ui.get());
            assert_eq!(calls.saved.get(), 1);
            assert_eq!(app.auto_save_interval(), Duration::from_secs(7));
            assert_eq!(app.clear_color(&egui::Visuals::dark()), [0.25; 4]);
            assert!(!app.persist_egui_memory());
        }
    }
}
