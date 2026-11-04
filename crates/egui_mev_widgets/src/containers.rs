//! Cards, section headers, banners and other things that hold content.

use egui::{
    Align, Align2, AsIdSalt, CollapsingHeader, Color32, CornerRadius, FontId, Frame, Label, Margin,
    Response, RichText, Role, Sense, Stroke, Ui, Vec2, WidgetInfo,
};
use egui_mev_theme::{Metrics, Palette, ThemeFamily, fonts};

use crate::marks::{self, Chevron};

/// Height of an inspector section header row (px).
const HEADER_HEIGHT: f32 = 24.0;

/// Space below an inspector section's body (px).
const SECTION_BOTTOM: f32 = 8.0;

/// Height a closed inspector section takes (px).
pub const SECTION_SPACE: f32 = HEADER_HEIGHT + SECTION_BOTTOM;

/// The standard card frame: card fill, subtle border, rounded corners and
/// inner padding.
pub fn card_frame(ui: &Ui) -> Frame {
    let p = Palette::of(ui);
    let m = Metrics::of(ui);
    Frame::new()
        .fill(p.card_bg)
        .stroke(Stroke::new(1.0, p.card_stroke))
        .corner_radius(CornerRadius::same(m.card_radius))
        .inner_margin(Margin::same(m.card_padding))
}

/// A card with an optional section title above its body. A leading gap keeps
/// stacked cards evenly spaced; the frame is stretched to the parent width so
/// short bodies still read as full-width sections.
pub fn card<R>(ui: &mut Ui, title: Option<&str>, add_body: impl FnOnce(&mut Ui) -> R) -> R {
    ui.add_space(Metrics::of(ui).card_gap);
    card_frame(ui)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            if let Some(title) = title {
                section_header(ui, title);
            }
            add_body(ui)
        })
        .inner
}

/// A group title: small and uppercase like Finder's "Favorites" (Graphite), or a
/// `WinUI` `BodyStrong` header in its original case (Fluent).
pub fn section_header(ui: &mut Ui, text: &str) {
    let label = match ThemeFamily::of(ui) {
        ThemeFamily::Graphite => RichText::new(text.to_uppercase())
            .size(10.5)
            .strong()
            .extra_letter_spacing(0.5),
        ThemeFamily::Fluent => RichText::new(text).font(fonts::semibold(ui.ctx(), 14.0)),
    };
    ui.label(label.color(Palette::of(ui).section_label));
    ui.add_space(4.0);
}

/// The title of a settings group, above its card.
pub fn group_title(ui: &mut Ui, title: &str) {
    let p = Palette::of(ui);
    let text = match ThemeFamily::of(ui) {
        ThemeFamily::Graphite => RichText::new(title).strong().size(13.0).color(p.text_weak),
        ThemeFamily::Fluent => RichText::new(title)
            .font(fonts::semibold(ui.ctx(), 14.0))
            .color(p.text),
    };
    ui.label(text);
}

/// A page title with its supporting description.
pub fn page_heading(ui: &mut Ui, title: &str, description: &str) {
    let p = Palette::of(ui);
    let m = Metrics::of(ui);
    let title = match ThemeFamily::of(ui) {
        ThemeFamily::Graphite => RichText::new(title).strong().size(m.title),
        ThemeFamily::Fluent => RichText::new(title).font(fonts::semibold(ui.ctx(), m.title)),
    };
    ui.label(title.color(p.text));
    ui.add_space(2.0);
    ui.label(
        RichText::new(description)
            .size(m.caption)
            .color(p.text_weak),
    );
    ui.add_space(14.0);
}

/// Collapsible inspector section: an uppercase title with a disclosure chevron.
/// The open state is kept in egui memory under `title`. With `reveal` the
/// section opens and scrolls into view.
pub fn inspector_section(ui: &mut Ui, title: &str, reveal: bool, body: impl FnOnce(&mut Ui)) {
    inspector_section_opening(ui, title, reveal, true, body);
}

/// [`inspector_section`] that starts closed unless `open_by_default`.
pub fn inspector_section_opening(
    ui: &mut Ui,
    title: &str,
    reveal: bool,
    open_by_default: bool,
    body: impl FnOnce(&mut Ui),
) {
    let p = Palette::of(ui);
    let m = Metrics::of(ui);
    let id = ui.scope_id().with(("inspector_section", title));
    let mut open = ui.data_mut(|d| *d.get_persisted_mut_or(id, open_by_default));
    if reveal && !open {
        open = true;
        ui.data_mut(|d| d.insert_persisted(id, open));
    }
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), HEADER_HEIGHT),
        Sense::click(),
    );
    response.widget_info(|| WidgetInfo::selected(Role::Button, true, open, title));
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(m.control_radius), p.card_bg_hover);
    }
    ui.painter().text(
        rect.left_center() + Vec2::new(4.0, 0.0),
        Align2::LEFT_CENTER,
        title.to_uppercase(),
        fonts::semibold(ui.ctx(), m.caption),
        p.section_label,
    );
    marks::chevron(
        ui.painter(),
        rect.right_center() - Vec2::new(10.0, 0.0),
        8.0,
        if open { Chevron::Down } else { Chevron::Right },
        p.text_weak,
    );
    if reveal {
        response.scroll_to_me(Some(Align::TOP));
    }
    if response.clicked() {
        open = !open;
        ui.data_mut(|d| d.insert_persisted(id, open));
    }
    if open {
        ui.add_space(2.0);
        body(ui);
    }
    ui.add_space(SECTION_BOTTOM);
}

/// A collapsible section; returns the body's result when open.
pub fn collapsible<R>(
    ui: &mut Ui,
    id: impl AsIdSalt,
    title: &str,
    default_open: bool,
    add_body: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    let color = Palette::of(ui).text_label;
    CollapsingHeader::new(RichText::new(title).color(color))
        .id_salt(id)
        .default_open(default_open)
        .show(ui, add_body)
        .body_returned
}

/// A status pill: a filled status dot plus a muted label inside a rounded
/// translucent pill. Returns the pill's response, for tooltips.
pub fn pill(ui: &mut Ui, label: &str, dot: Color32) -> Response {
    let p = Palette::of(ui);
    let caption = Metrics::of(ui).caption;
    Frame::new()
        .fill(p.border_subtle)
        .corner_radius(CornerRadius::same(9))
        .inner_margin(Margin::symmetric(8, 2))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                ui.painter().circle_filled(rect.center(), 4.0, dot);
                ui.label(RichText::new(label).size(caption).color(p.text_weak));
            });
        })
        .response
}

/// Severity of a [`banner`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BannerKind {
    /// Neutral information.
    Info,
    /// Success / healthy state.
    Success,
    /// Degraded but non-fatal state.
    Warning,
    /// Failure.
    Error,
}

impl BannerKind {
    /// `(fill, foreground)` for this severity.
    #[must_use]
    pub const fn colors(self, p: &Palette) -> (Color32, Color32) {
        match self {
            Self::Info => (p.info_bg, p.accent),
            Self::Success => (p.success_bg, p.success),
            Self::Warning => (p.caution_bg, p.caution),
            Self::Error => (p.critical_bg, p.critical),
        }
    }
}

/// A full-width banner with a severity icon: tinted with its severity's colour
/// (Graphite), or an opaque `WinUI` `InfoBar` with a neutral stroke (Fluent).
pub fn banner(ui: &mut Ui, kind: BannerKind, text: &str) -> Response {
    let p = Palette::of(ui);
    let m = Metrics::of(ui);
    let (fill, foreground) = kind.colors(&p);
    let frame = match ThemeFamily::of(ui) {
        ThemeFamily::Graphite => Frame::new()
            .fill(foreground.gamma_multiply(0.14))
            .stroke(Stroke::new(1.0, foreground.gamma_multiply(0.55)))
            .inner_margin(Margin::symmetric(10, 8)),
        ThemeFamily::Fluent => Frame::new()
            .fill(fill)
            .stroke(Stroke::new(1.0, p.card_stroke))
            .inner_margin(Margin::symmetric(14, 12)),
    };
    frame
        .corner_radius(CornerRadius::same(m.card_radius))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let font = FontId::proportional(m.body);
                // As tall as a line of text, so the icon lines up with the first one.
                let line = ui.fonts_mut(|fonts| fonts.row_height(&font));
                let (slot, _) = ui.allocate_exact_size(Vec2::new(14.0, line), Sense::hover());
                let icon = egui::Rect::from_center_size(slot.center(), Vec2::splat(14.0));
                marks::severity(ui.painter(), icon, kind, foreground);
                ui.add(Label::new(RichText::new(text).font(font).color(p.text)).wrap());
            });
        })
        .response
}

/// A centred empty-state placeholder: a bold muted title and an optional hint.
pub fn empty_state(ui: &mut Ui, title: &str, hint: &str) {
    let p = Palette::of(ui);
    ui.vertical_centered(|ui| {
        ui.add_space(16.0);
        ui.label(RichText::new(title).strong().color(p.text_weak));
        if !hint.is_empty() {
            ui.label(RichText::new(hint).small().color(p.hint_text));
        }
        ui.add_space(16.0);
    });
}

#[cfg(test)]
mod tests {
    use egui_kittest::{Harness, kittest::Queryable as _};
    use egui_mev_theme::presets;

    use super::*;

    #[test]
    fn banner_foregrounds_are_distinct() {
        let kinds = [
            BannerKind::Info,
            BannerKind::Success,
            BannerKind::Warning,
            BannerKind::Error,
        ];
        for palette in [
            presets::GRAPHITE_DARK,
            presets::GRAPHITE_LIGHT,
            presets::SLATE_DARK,
        ] {
            for (index, a) in kinds.iter().enumerate() {
                for b in kinds.iter().skip(index + 1) {
                    assert_ne!(a.colors(&palette).1, b.colors(&palette).1);
                }
            }
        }
    }

    #[test]
    fn a_card_reads_the_palette_of_the_mode_it_draws_in() {
        for (mode, palette) in [
            (egui::Theme::Dark, presets::GRAPHITE_DARK),
            (egui::Theme::Light, presets::GRAPHITE_LIGHT),
        ] {
            let ctx = crate::test_support::themed(mode);
            crate::test_support::frame_at(&ctx, 0.0, Vec::new(), |ui| {
                let frame = card_frame(ui);
                assert_eq!(frame.fill, palette.card_bg);
                assert_eq!(frame.stroke.color, palette.card_stroke);
            });
        }
    }

    #[test]
    fn a_section_header_keeps_its_case_only_under_fluent() {
        for (family, shown, hidden) in [
            (ThemeFamily::Graphite, "FAVORITES", "Favorites"),
            (ThemeFamily::Fluent, "Favorites", "FAVORITES"),
        ] {
            let mut harness = Harness::builder()
                .with_size([300.0, 80.0])
                .with_context_setup(move |ctx| {
                    egui_mev_test::install(ctx, crate::test_support::family_theme(family));
                })
                .build_ui(|ui| section_header(ui, "Favorites"));
            harness.run();
            assert!(harness.query_by_label(shown).is_some(), "{family:?}");
            assert!(harness.query_by_label(hidden).is_none(), "{family:?}");
        }
    }

    #[test]
    fn a_banner_tints_on_graphite_and_is_opaque_on_fluent() {
        for kind in [
            BannerKind::Info,
            BannerKind::Success,
            BannerKind::Warning,
            BannerKind::Error,
        ] {
            let ctx = crate::test_support::themed_family(ThemeFamily::Graphite, egui::Theme::Dark);
            let (_, foreground) = kind.colors(&egui_mev_theme::Theme::of(&ctx).dark);
            let output = crate::test_support::frame_output(&ctx, 0.0, Vec::new(), |ui| {
                banner(ui, kind, "Note");
            });
            let colors = crate::test_support::painted_colors(&output);
            assert!(
                colors.contains(&foreground.gamma_multiply(0.14)),
                "{kind:?}"
            );
            assert!(
                colors.contains(&foreground.gamma_multiply(0.55)),
                "{kind:?}"
            );

            let ctx = crate::test_support::themed_family(ThemeFamily::Fluent, egui::Theme::Dark);
            let p = egui_mev_theme::Theme::of(&ctx).dark;
            let (fill, _) = kind.colors(&p);
            let output = crate::test_support::frame_output(&ctx, 0.0, Vec::new(), |ui| {
                banner(ui, kind, "Note");
            });
            let colors = crate::test_support::painted_colors(&output);
            assert!(colors.contains(&fill), "{kind:?}");
            assert!(colors.contains(&p.card_stroke), "{kind:?}");
        }
    }

    #[test]
    fn a_long_banner_wraps_inside_its_width() {
        let ctx = crate::test_support::themed_family(ThemeFamily::Fluent, egui::Theme::Dark);
        crate::test_support::frame_at(&ctx, 0.0, Vec::new(), |ui| {
            ui.set_max_width(200.0);
            let one_line = banner(ui, BannerKind::Info, "Short").rect;
            let wrapped = banner(ui, BannerKind::Info, &"A long sentence. ".repeat(8)).rect;
            assert!(wrapped.height() > one_line.height());
            assert!(wrapped.width() <= 200.0 + 1.0);
        });
    }

    #[test]
    fn titles_follow_the_family() {
        for family in [ThemeFamily::Graphite, ThemeFamily::Fluent] {
            let mut harness = Harness::builder()
                .with_size([400.0, 200.0])
                .with_context_setup(move |ctx| {
                    egui_mev_test::install(ctx, crate::test_support::family_theme(family));
                })
                .build_ui(|ui| {
                    page_heading(ui, "Network", "How this app reaches the server.");
                    group_title(ui, "Proxy");
                });
            harness.run();
            for label in ["Network", "How this app reaches the server.", "Proxy"] {
                assert!(
                    harness.query_by_label(label).is_some(),
                    "{family:?} {label}"
                );
            }
        }
    }

    #[test]
    fn card_frames_read_their_sizes_from_the_metrics() {
        let ctx = egui::Context::default();
        egui_mev_theme::Theme::dark_only(presets::GRAPHITE_DARK)
            .with_metrics(Metrics {
                card_radius: 3,
                card_padding: 21,
                ..Metrics::GRAPHITE
            })
            .install(&ctx);
        crate::test_support::frame_at(&ctx, 0.0, Vec::new(), |ui| {
            let frame = card_frame(ui);
            assert_eq!(frame.corner_radius, CornerRadius::same(3));
            assert_eq!(frame.inner_margin, Margin::same(21));
        });
    }

    #[test]
    fn an_inspector_section_toggles_by_its_title() {
        let mut harness = Harness::builder().with_size([300.0, 200.0]).build_ui(|ui| {
            inspector_section(ui, "History", false, |ui| {
                ui.label("Body");
            });
        });
        harness.run();
        assert!(harness.query_by_label("Body").is_some());
        harness.get_by_label("History").click();
        harness.run();
        assert!(harness.query_by_label("Body").is_none());
    }

    #[test]
    fn reveal_opens_a_section_that_starts_closed() {
        let mut harness = Harness::builder().with_size([300.0, 200.0]).build_ui_state(
            |ui, reveal: &mut bool| {
                inspector_section_opening(ui, "History", *reveal, false, |ui| {
                    ui.label("Body");
                });
            },
            false,
        );
        harness.run();
        assert!(harness.query_by_label("Body").is_none());
        *harness.state_mut() = true;
        harness.run();
        assert!(harness.query_by_label("Body").is_some());
    }

    #[test]
    fn collapsible_returns_its_body_only_while_open() {
        let mut harness = Harness::builder().with_size([300.0, 200.0]).build_ui_state(
            |ui, seen: &mut Option<u8>| {
                *seen = collapsible(ui, "salt", "Advanced", false, |_| 7);
            },
            None,
        );
        harness.run();
        assert_eq!(*harness.state(), None);
        harness.get_by_label("Advanced").click();
        harness.run();
        assert_eq!(*harness.state(), Some(7));
    }
}
