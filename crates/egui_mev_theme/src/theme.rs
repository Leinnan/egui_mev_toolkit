use std::sync::Arc;

use egui::{Color32, Context, CornerRadius, FontId, Margin, Shadow, Stroke, Style, TextStyle};

use crate::{Metrics, Palette, presets};

/// How a theme turns its colours and sizes into an egui [`Style`].
pub type StyleFn = fn(&Style, egui::Theme, &Palette, &Metrics) -> Style;

/// A complete look for one egui context.
#[derive(Clone, Debug)]
pub struct Theme {
    /// The palette used in dark mode.
    pub dark: Palette,
    /// The palette used in light mode.
    pub light: Palette,
    /// Sizes shared by both modes.
    pub metrics: Metrics,
    /// Builds the egui style; [`standard_style`] unless a theme brings its own.
    pub style: StyleFn,
    /// Draw dark whatever the system asks for.
    pub dark_only: bool,
}

impl Default for Theme {
    fn default() -> Self {
        Self::new(presets::GRAPHITE_DARK, presets::GRAPHITE_LIGHT)
    }
}

impl Theme {
    /// A theme that follows the app's [`Appearance`].
    #[must_use]
    pub const fn new(dark: Palette, light: Palette) -> Self {
        Self {
            dark,
            light,
            metrics: Metrics::GRAPHITE,
            style: standard_style,
            dark_only: false,
        }
    }

    /// One dark palette, whatever the system asks for.
    #[must_use]
    pub const fn dark_only(palette: Palette) -> Self {
        Self {
            dark_only: true,
            ..Self::new(palette, palette)
        }
    }

    /// The same theme with other sizes.
    #[must_use]
    pub const fn with_metrics(mut self, metrics: Metrics) -> Self {
        self.metrics = metrics;
        self
    }

    /// The palette for a mode.
    #[must_use]
    pub const fn palette(&self, dark_mode: bool) -> Palette {
        if dark_mode { self.dark } else { self.light }
    }

    /// Install this theme on `ctx`. Safe to call again, for example to switch
    /// accent. Call it once per context, including extra viewports' contexts.
    pub fn install(self, ctx: &Context) {
        if self.dark_only {
            ctx.set_theme(egui::Theme::Dark);
            // Both slots, so an OS light-mode preference can't produce an
            // unstyled light UI.
            let style = (self.style)(
                &ctx.style_of(egui::Theme::Dark),
                egui::Theme::Dark,
                &self.dark,
                &self.metrics,
            );
            ctx.set_style_of(egui::Theme::Dark, style.clone());
            ctx.set_style_of(egui::Theme::Light, style);
        } else {
            for (mode, palette) in [
                (egui::Theme::Dark, &self.dark),
                (egui::Theme::Light, &self.light),
            ] {
                let style = (self.style)(&ctx.style_of(mode), mode, palette, &self.metrics);
                ctx.set_style_of(mode, style);
            }
        }
        ctx.data_mut(|data| data.insert_temp(theme_id(), Arc::new(self)));
    }

    /// The theme installed on `ctx`, or the default one when none has been.
    #[must_use]
    pub fn of(ctx: &Context) -> Arc<Self> {
        ctx.data(|data| data.get_temp(theme_id()))
            .unwrap_or_else(|| Arc::new(Self::default()))
    }
}

fn theme_id() -> egui::Id {
    egui::Id::unique("egui_mev_theme::theme")
}

/// Which mode to draw in, as a setting the user picks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Appearance {
    /// Follow the operating system.
    #[default]
    System,
    /// Always light.
    Light,
    /// Always dark.
    Dark,
}

impl Appearance {
    /// Every choice, in menu order.
    pub const ALL: [Self; 3] = [Self::System, Self::Light, Self::Dark];

    /// A label for a settings row.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::System => "Use system setting",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }

    /// Switch `ctx` to this mode.
    pub fn apply(self, ctx: &Context) {
        ctx.set_theme(match self {
            Self::System => egui::ThemePreference::System,
            Self::Light => egui::ThemePreference::Light,
            Self::Dark => egui::ThemePreference::Dark,
        });
    }
}

/// The compact desktop style behind the graphite and slate presets.
///
/// `override_text_color` stays unset on purpose: egui would bake it into every
/// galley, hint text included, so hints would read as bright as typed text.
/// Each widget state gets `text` explicitly and hints use `weak_text_color`.
#[must_use]
pub fn standard_style(base: &Style, mode: egui::Theme, p: &Palette, m: &Metrics) -> Style {
    let mut style = base.clone();
    type_scale(&mut style, m);

    let spacing = &mut style.spacing;
    spacing.item_spacing = egui::vec2(8.0, 6.0);
    spacing.button_padding = egui::vec2(8.0, 3.0);
    spacing.interact_size.y = m.interact_height;
    spacing.indent = 14.0;
    spacing.window_margin = Margin::same(12);
    spacing.menu_margin = Margin::same(5);

    let v = &mut style.visuals;
    *v = match mode {
        egui::Theme::Dark => egui::Visuals::dark(),
        egui::Theme::Light => egui::Visuals::light(),
    };
    v.override_text_color = None;
    v.weak_text_color = Some(p.hint_text);
    v.hyperlink_color = p.accent_secondary;
    v.panel_fill = p.panel_bg;
    v.window_fill = p.window_bg;
    v.faint_bg_color = p.section_bg;
    v.extreme_bg_color = p.extreme_bg;
    v.code_bg_color = p.section_bg;
    v.warn_fg_color = p.caution;
    v.error_fg_color = p.critical;
    v.selection.bg_fill = p.selection_bg;
    v.selection.stroke = Stroke::new(1.0, p.accent);
    v.slider_trailing_fill = true;
    v.window_stroke = Stroke::new(1.0, p.border);
    v.window_corner_radius = CornerRadius::same(m.overlay_radius);
    v.menu_corner_radius = CornerRadius::same(m.card_radius);

    let shadow_alpha = if mode == egui::Theme::Dark { 140 } else { 60 };
    v.window_shadow = Shadow {
        offset: [0, 12],
        blur: 40,
        spread: 0,
        color: Color32::from_black_alpha(shadow_alpha),
    };
    v.popup_shadow = Shadow {
        offset: [0, 6],
        blur: 18,
        spread: 0,
        color: Color32::from_black_alpha(shadow_alpha),
    };

    let control = CornerRadius::same(m.control_radius);
    let w = &mut v.widgets;
    for state in [
        &mut w.noninteractive,
        &mut w.inactive,
        &mut w.hovered,
        &mut w.active,
        &mut w.open,
    ] {
        state.fg_stroke.color = p.text;
        state.corner_radius = control;
    }
    w.noninteractive.corner_radius = CornerRadius::same(m.card_radius);
    w.noninteractive.bg_fill = p.card_bg;
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.border_subtle);
    // `bg_fill` paints slider rails and checkbox wells, `weak_bg_fill` paints
    // button faces: keep rails lighter so they read against grouped boxes.
    w.inactive.bg_fill = p.control_track;
    w.inactive.weak_bg_fill = p.card_bg;
    w.inactive.bg_stroke = Stroke::new(1.0, p.border);
    w.hovered.bg_fill = p.card_bg_hover;
    w.hovered.weak_bg_fill = p.card_bg_hover;
    w.hovered.bg_stroke = Stroke::new(1.0, p.border);
    w.active.bg_fill = p.accent;
    w.active.weak_bg_fill = p.accent;
    w.active.bg_stroke = Stroke::new(1.0, p.accent);
    w.open.weak_bg_fill = p.card_bg_hover;
    style
}

/// Text styles from the metrics' type ramp, in the families every font setup has.
pub fn type_scale(style: &mut Style, m: &Metrics) {
    style.text_styles = [
        (TextStyle::Small, FontId::proportional(m.caption)),
        (TextStyle::Body, FontId::proportional(m.body)),
        (TextStyle::Button, FontId::proportional(m.body)),
        (TextStyle::Heading, FontId::proportional(m.title)),
        (TextStyle::Monospace, FontId::monospace(m.body)),
    ]
    .into();
}

#[cfg(test)]
mod tests {
    use egui::{Color32, Context};

    use super::{Appearance, Theme};
    use crate::{Metrics, Palette, presets, run_frame};

    #[test]
    fn an_uninstalled_context_falls_back_to_graphite() {
        let ctx = Context::default();
        assert_eq!(Metrics::of_ctx(&ctx), Metrics::GRAPHITE);
        ctx.set_theme(egui::Theme::Dark);
        assert_eq!(Palette::of_ctx(&ctx), presets::GRAPHITE_DARK);
    }

    #[test]
    fn each_context_keeps_its_own_theme() {
        let (a, b) = (Context::default(), Context::default());
        Theme::dark_only(presets::SLATE_DARK).install(&a);
        Theme::dark_only(presets::GRAPHITE_DARK).install(&b);
        assert_eq!(Palette::of_ctx(&a), presets::SLATE_DARK);
        assert_eq!(Palette::of_ctx(&b), presets::GRAPHITE_DARK);
    }

    #[test]
    fn the_palette_follows_the_mode_widgets_draw_in() {
        let ctx = Context::default();
        Theme::default().install(&ctx);
        ctx.set_theme(egui::Theme::Light);
        run_frame(&ctx, |ui| {
            assert_eq!(Palette::of(ui), presets::GRAPHITE_LIGHT);
            assert_eq!(ui.visuals().panel_fill, presets::GRAPHITE_LIGHT.panel_bg);
        });
        ctx.set_theme(egui::Theme::Dark);
        run_frame(&ctx, |ui| {
            assert_eq!(Palette::of(ui), presets::GRAPHITE_DARK);
            assert_eq!(ui.visuals().panel_fill, presets::GRAPHITE_DARK.panel_bg);
        });
    }

    #[test]
    fn dark_only_stays_dark_when_the_os_asks_for_light() {
        let ctx = Context::default();
        Theme::dark_only(presets::SLATE_DARK).install(&ctx);
        // Both slots are styled, so a later switch can't reveal an unstyled look.
        assert!(ctx.style_of(egui::Theme::Light).visuals.dark_mode);
        assert_eq!(
            ctx.style_of(egui::Theme::Light).visuals.panel_fill,
            presets::SLATE_DARK.panel_bg
        );
        assert_eq!(Palette::of_ctx(&ctx), presets::SLATE_DARK);
    }

    #[test]
    fn hint_text_stays_dimmer_than_typed_text() {
        let ctx = Context::default();
        Theme::default().install(&ctx);
        let visuals = &ctx.style_of(egui::Theme::Dark).visuals;
        assert_eq!(visuals.override_text_color, None);
        assert_eq!(
            visuals.weak_text_color,
            Some(presets::GRAPHITE_DARK.hint_text)
        );
        assert_ne!(
            visuals.widgets.inactive.fg_stroke.color,
            Color32::TRANSPARENT
        );
    }

    #[test]
    fn reinstalling_replaces_the_theme() {
        let ctx = Context::default();
        Theme::dark_only(presets::SLATE_DARK).install(&ctx);
        Theme::dark_only(presets::GRAPHITE_DARK).install(&ctx);
        assert_eq!(Palette::of_ctx(&ctx), presets::GRAPHITE_DARK);
    }

    #[test]
    fn appearance_switches_the_mode() {
        let ctx = Context::default();
        Appearance::Light.apply(&ctx);
        run_frame(&ctx, |_| {});
        assert_eq!(ctx.theme(), egui::Theme::Light);
        Appearance::Dark.apply(&ctx);
        run_frame(&ctx, |_| {});
        assert_eq!(ctx.theme(), egui::Theme::Dark);
    }
}
