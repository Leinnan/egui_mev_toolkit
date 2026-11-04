//! Snapshots of the gallery in each preset. Rewrite them with
//! `UPDATE_SNAPSHOTS=1`, and only after looking at the new images.

#[path = "support/gallery_ui.rs"]
mod gallery_ui;

use egui::{Context, Theme as Mode};
use egui_kittest::{Harness, SnapshotResults, kittest::Queryable as _};
use egui_mev_test::{install_with, settle, snapshot_lock, snapshot_options};
use egui_mev_theme::{FontOptions, IconFont, Palette, Theme};
use gallery_ui::{GalleryState, Look, OpenDialog, gallery_ui};

const WIDTH: f32 = 1200.0;
// Tall enough that the toasts in the corner cover nothing in any look.
const HEIGHT: f32 = 1120.0;

fn with_icons(ctx: &Context, theme: Theme) {
    install_with(
        ctx,
        theme,
        &FontOptions {
            icons: vec![IconFont::Lucide],
            ..Default::default()
        },
    );
}

fn harness(theme: Theme, mode: Mode) -> Harness<'static, GalleryState> {
    Harness::builder()
        .with_size([WIDTH, HEIGHT])
        .with_theme(mode)
        .with_context_setup(move |ctx| with_icons(ctx, theme))
        .build_ui_state(
            |ui, state: &mut GalleryState| {
                // The harness only fills what its contents take up.
                let background = Palette::of(ui).panel_bg;
                ui.painter()
                    .rect_filled(ui.ctx().content_rect(), 0.0, background);
                egui::CentralPanel::default().show(ui, |ui| gallery_ui(ui, state));
            },
            GalleryState::default(),
        )
}

fn look_harness(look: Look) -> Harness<'static, GalleryState> {
    // Fitted text belongs to the fonts of the context that laid it out.
    egui_mev_widgets::text_fit::clear_pool();
    harness(look.theme(), look.mode())
}

#[test]
fn gallery_snapshots() {
    let _guard = snapshot_lock();
    let options = snapshot_options();
    let mut results = SnapshotResults::new();
    for look in Look::ALL {
        let mut harness = look_harness(look);
        settle(&mut harness);
        harness.snapshot_options(format!("gallery_{}", look.name()), &options);
        results.extend_harness(&mut harness);
    }
    results.unwrap();
}

#[test]
fn sheet_snapshots() {
    let _guard = snapshot_lock();
    let options = snapshot_options();
    let mut results = SnapshotResults::new();
    for look in Look::ALL {
        let mut harness = look_harness(look);
        settle(&mut harness);
        harness.get_by_label("Open sheet").click();
        settle(&mut harness);
        settle(&mut harness);
        harness.snapshot_options(format!("sheet_{}", look.name()), &options);
        results.extend_harness(&mut harness);
    }
    results.unwrap();
}

#[test]
fn progress_snapshots() {
    let _guard = snapshot_lock();
    let options = snapshot_options();
    let mut results = SnapshotResults::new();
    for look in [Look::GraphiteDark, Look::FluentDark, Look::FluentLight] {
        let mut harness = look_harness(look);
        settle(&mut harness);
        harness.state_mut().dialog = OpenDialog::Progress;
        settle(&mut harness);
        settle(&mut harness);
        // The spinner keeps moving; hide it so the image is stable.
        let spacing = harness.ctx.global_style().spacing.clone();
        let size = spacing.interact_size.y;
        let label = harness.get_by_label("Applying Blur…").rect();
        harness.mask(egui::Rect::from_center_size(
            egui::pos2(
                label.left() - spacing.item_spacing.x - size / 2.0,
                label.center().y,
            ),
            egui::Vec2::splat(size + 4.0),
        ));
        harness.snapshot_options(format!("progress_{}", look.name()), &options);
        results.extend_harness(&mut harness);
    }
    results.unwrap();
}

#[test]
fn open_picker_snapshots() {
    let _guard = snapshot_lock();
    let options = snapshot_options();
    let mut results = SnapshotResults::new();
    for look in Look::ALL {
        let mut harness = look_harness(look);
        settle(&mut harness);
        harness.get_by_label("Widget").click();
        harness.run();
        assert!(
            harness.query_by_label("Memory Usage").is_some(),
            "{}",
            look.name()
        );
        harness.snapshot_options(format!("picker_{}", look.name()), &options);
        results.extend_harness(&mut harness);
    }
    results.unwrap();
}

#[test]
fn the_gallery_keeps_working_after_interaction() {
    for look in Look::ALL {
        let mut harness = look_harness(look);
        settle(&mut harness);
        harness.get_by_label("World").click();
        settle(&mut harness);
        assert_eq!(harness.state().segment, 2, "{}", look.name());
        harness.get_by_label("Open sheet").click();
        settle(&mut harness);
        // The last match is the sheet's: the gallery has a Cancel of its own.
        harness.get_all_by_label("Cancel").last().unwrap().click();
        settle(&mut harness);
        assert_eq!(harness.state().dialog, OpenDialog::None, "{}", look.name());
    }
}
