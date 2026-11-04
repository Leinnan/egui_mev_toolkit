//! Snapshots of the gallery in each preset. Rewrite them with
//! `UPDATE_SNAPSHOTS=1`, and only after looking at the new images.

#[path = "support/gallery_ui.rs"]
mod gallery_ui;

use egui::{Context, Theme as Mode};
use egui_kittest::{Harness, SnapshotResults, kittest::Queryable as _};
use egui_mev_test::{install_with, settle, snapshot_lock, snapshot_options};
use egui_mev_theme::{FontOptions, IconFont, Theme, presets};
use gallery_ui::{GalleryState, OpenDialog, gallery_ui};

const WIDTH: f32 = 1200.0;
const HEIGHT: f32 = 900.0;

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
                egui::CentralPanel::default().show(ui, |ui| gallery_ui(ui, state));
            },
            GalleryState::default(),
        )
}

fn looks() -> [(&'static str, Theme, Mode); 3] {
    let graphite = Theme::new(presets::GRAPHITE_DARK, presets::GRAPHITE_LIGHT);
    [
        ("graphite_dark", graphite.clone(), Mode::Dark),
        ("graphite_light", graphite, Mode::Light),
        (
            "slate_dark",
            Theme::dark_only(presets::SLATE_DARK),
            Mode::Dark,
        ),
    ]
}

#[test]
fn gallery_snapshots() {
    let _guard = snapshot_lock();
    let options = snapshot_options();
    let mut results = SnapshotResults::new();
    for (name, theme, mode) in looks() {
        let mut harness = harness(theme, mode);
        settle(&mut harness);
        harness.snapshot_options(format!("gallery_{name}"), &options);
        results.extend_harness(&mut harness);
    }
    results.unwrap();
}

#[test]
fn sheet_snapshots() {
    let _guard = snapshot_lock();
    let options = snapshot_options();
    let mut results = SnapshotResults::new();
    for (name, theme, mode) in looks() {
        let mut harness = harness(theme, mode);
        settle(&mut harness);
        harness.get_by_label("Open sheet").click();
        settle(&mut harness);
        settle(&mut harness);
        harness.snapshot_options(format!("sheet_{name}"), &options);
        results.extend_harness(&mut harness);
    }
    results.unwrap();
}

#[test]
fn progress_snapshot() {
    let _guard = snapshot_lock();
    let (name, theme, mode) = looks().into_iter().next().unwrap();
    let mut harness = harness(theme, mode);
    settle(&mut harness);
    harness.state_mut().dialog = OpenDialog::Progress;
    settle(&mut harness);
    settle(&mut harness);
    // The spinner keeps moving; hide it so the image is stable.
    let label = harness.get_by_label("Applying Blur…").rect();
    harness.mask(egui::Rect::from_center_size(
        egui::pos2(label.left() - 19.0, label.center().y),
        egui::Vec2::splat(26.0),
    ));
    harness.snapshot_options(format!("progress_{name}"), &snapshot_options());
}

#[test]
fn open_picker_snapshots() {
    let _guard = snapshot_lock();
    let options = snapshot_options();
    let mut results = SnapshotResults::new();
    for (name, theme, mode) in looks() {
        let mut harness = harness(theme, mode);
        settle(&mut harness);
        harness.get_by_label("Widget").click();
        harness.run();
        assert!(harness.query_by_label("Memory Usage").is_some(), "{name}");
        harness.snapshot_options(format!("picker_{name}"), &options);
        results.extend_harness(&mut harness);
    }
    results.unwrap();
}

#[test]
fn the_gallery_keeps_working_after_interaction() {
    let mut harness = harness(Theme::default(), Mode::Dark);
    settle(&mut harness);
    harness.get_by_label("World").click();
    settle(&mut harness);
    assert_eq!(harness.state().segment, 2);
    harness.get_by_label("Open sheet").click();
    settle(&mut harness);
    // The last match is the sheet's: the gallery has a Cancel of its own.
    harness.get_all_by_label("Cancel").last().unwrap().click();
    settle(&mut harness);
    assert_eq!(harness.state().dialog, OpenDialog::None);
}
