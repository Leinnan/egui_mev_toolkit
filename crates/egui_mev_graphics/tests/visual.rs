//! How each widget looks in both graphite palettes. After changing one, rerun
//! with `UPDATE_SNAPSHOTS=1` and look at the new images.

use egui::{Id, Rect, Ui, Vec2, pos2, vec2};
use egui_kittest::{Harness, SnapshotResults};
use egui_mev_graphics::{
    Hsva, Unit, color_editor, curve_editor, curve_thumbnail, hue_triangle,
    rulers::{self, RULER_SIZE, RulerLayout, RulerScale},
};
use egui_mev_theme::{Palette, Theme, presets};

const MODES: [(egui::Theme, &str); 2] =
    [(egui::Theme::Dark, "dark"), (egui::Theme::Light, "light")];

const SOFT: [[f32; 2]; 4] = [[0.0, 0.1], [0.3, 0.2], [0.7, 0.85], [1.0, 1.0]];

fn snapshot(name: &str, size: Vec2, pixels_per_point: f32, mut show: impl FnMut(&mut Ui)) {
    let mut results = SnapshotResults::new();
    for (mode, suffix) in MODES {
        let mut harness = Harness::builder()
            .with_size(size)
            .with_pixels_per_point(pixels_per_point)
            .with_theme(mode)
            .with_context_setup(|ctx| {
                Theme::new(presets::GRAPHITE_DARK, presets::GRAPHITE_LIGHT).install(ctx);
            })
            .build_ui(|ui| {
                // The harness only fills what its contents take up, and some
                // of these paint without taking any.
                let background = Palette::of(ui).panel_bg;
                ui.painter()
                    .rect_filled(ui.ctx().content_rect(), 0.0, background);
                show(ui);
            });
        harness.run();
        harness.snapshot(format!("{name}_{suffix}"));
        results.extend_harness(&mut harness);
    }
}

#[test]
fn curve_editor_with_marker() {
    snapshot("curve_editor", vec2(236.0, 236.0), 1.0, |ui| {
        let mut points = SOFT.to_vec();
        curve_editor(ui, &mut points, vec2(220.0, 220.0), Some(0.45));
    });
}

#[test]
fn curve_thumbnails() {
    snapshot("curve_thumbnail", vec2(120.0, 40.0), 2.0, |ui| {
        ui.horizontal(|ui| {
            curve_thumbnail(ui, &SOFT, vec2(22.0, 18.0));
            curve_thumbnail(ui, &[], vec2(22.0, 18.0));
            curve_thumbnail(ui, &[[0.0, 0.5], [1.0, 0.5]], vec2(44.0, 24.0));
        });
    });
}

#[test]
fn color_editor_translucent() {
    snapshot("color_editor", vec2(260.0, 280.0), 1.0, |ui| {
        let mut color = [0xE0, 0x60, 0x20, 0x90];
        color_editor(ui, Id::unique("color"), &mut color);
    });
}

#[test]
fn hue_triangle_ring() {
    snapshot("hue_triangle", vec2(200.0, 200.0), 1.0, |ui| {
        let mut hsva = Hsva {
            h: 200.0,
            s: 0.7,
            v: 0.8,
            a: 255,
        };
        hue_triangle(
            ui,
            Id::unique("triangle"),
            pos2(100.0, 100.0),
            90.0,
            &mut hsva,
        );
    });
}

fn show_rulers(ui: &mut Ui, unit: Unit, zoom: f32, origin: Vec2) {
    let full = Rect::from_min_size(ui.max_rect().min, vec2(400.0, 160.0));
    let viewport = Rect::from_min_max(full.min + Vec2::splat(RULER_SIZE), full.max);
    let cursor = viewport.min + vec2(137.0, 64.0);
    rulers::paint(
        ui.painter(),
        &Palette::of(ui),
        RulerLayout::new(full, viewport),
        Some(RulerScale {
            origin: viewport.min + origin,
            zoom,
            unit,
            dpi: 96.0,
        }),
        Some(cursor),
    );
}

#[test]
fn rulers_in_pixels() {
    snapshot("rulers_pixels", vec2(416.0, 176.0), 1.0, |ui| {
        show_rulers(ui, Unit::Pixels, 1.0, vec2(-40.0, 12.0));
    });
}

#[test]
fn rulers_in_inches_zoomed_in() {
    snapshot("rulers_inches", vec2(416.0, 176.0), 1.0, |ui| {
        show_rulers(ui, Unit::Inches, 32.0, vec2(-40.0, 12.0));
    });
}

/// Labels too wide to fit across the vertical ruler.
#[test]
fn rulers_with_long_labels() {
    snapshot("rulers_long_labels", vec2(416.0, 176.0), 1.0, |ui| {
        show_rulers(ui, Unit::Pixels, 0.2, vec2(-40.0, -100.0));
    });
}

#[test]
fn rulers_without_a_scale() {
    snapshot("rulers_blank", vec2(216.0, 96.0), 1.0, |ui| {
        let full = Rect::from_min_size(ui.max_rect().min, vec2(200.0, 80.0));
        let viewport = Rect::from_min_max(full.min + Vec2::splat(RULER_SIZE), full.max);
        rulers::paint(
            ui.painter(),
            &Palette::of(ui),
            RulerLayout::new(full, viewport),
            None,
            None,
        );
    });
}
