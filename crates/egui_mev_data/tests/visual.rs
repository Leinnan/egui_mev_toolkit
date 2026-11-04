//! How a form and a table look in the graphite and Fluent palettes. After changing
//! one, rerun with `UPDATE_SNAPSHOTS=1` and look at the new images.

use egui::{Ui, Vec2, vec2};
use egui_kittest::{Harness, SnapshotResults};
use egui_mev_data::{Form, FormState, TableSpec, TableView};
use egui_mev_test::{install, settle, snapshot_lock, snapshot_options};
use egui_mev_theme::{AccentShades, Palette, Theme, fluent, presets};
use serde_json::json;

fn graphite() -> Theme {
    Theme::new(presets::GRAPHITE_DARK, presets::GRAPHITE_LIGHT)
}

fn windows() -> Theme {
    fluent::theme(AccentShades::WINDOWS_DEFAULT)
}

type Look = (fn() -> Theme, egui::Theme, &'static str);

const LOOKS: [Look; 4] = [
    (graphite, egui::Theme::Dark, "graphite_dark"),
    (graphite, egui::Theme::Light, "graphite_light"),
    (windows, egui::Theme::Dark, "fluent_dark"),
    (windows, egui::Theme::Light, "fluent_light"),
];

fn snapshot<S>(name: &str, size: Vec2, state: impl Fn() -> S, show: impl Fn(&mut Ui, &mut S)) {
    let _guard = snapshot_lock();
    let options = snapshot_options();
    let mut results = SnapshotResults::new();
    for (theme, mode, suffix) in LOOKS {
        let mut harness = Harness::builder()
            .with_size(size)
            .with_theme(mode)
            .with_context_setup(move |ctx| install(ctx, theme()))
            .build_ui_state(
                |ui, state: &mut S| {
                    // The harness only fills what its contents take up.
                    let background = Palette::of(ui).panel_bg;
                    ui.painter()
                        .rect_filled(ui.ctx().content_rect(), 0.0, background);
                    show(ui, state);
                },
                state(),
            );
        settle(&mut harness);
        harness.snapshot_options(format!("{name}_{suffix}"), &options);
        results.extend_harness(&mut harness);
    }
    results.unwrap();
}

fn sample_form() -> FormState {
    let form: Form = serde_json::from_value(json!({
        "title": "Convert video",
        "submit": "Convert",
        "fields": [
            { "type": "info", "text": "1920x1080, 24 fps" },
            { "id": "name", "type": "text", "label": "Output name", "required": true, "placeholder": "movie.mp4" },
            { "id": "notes", "type": "multiline", "label": "Notes", "default": "First pass\nSecond pass" },
            { "type": "separator" },
            { "id": "format", "type": "choice", "label": "Format", "options": [["mp4", "MP4"], ["webm", "WebM"]], "default": "webm" },
            { "id": "range", "type": "radio", "label": "Range", "options": [["all", "Whole"], ["part", "Fragment"]], "default": "part" },
            { "id": "start", "type": "time", "label": "Start", "default": 75.5, "max": 3600, "visible_if": { "field": "range", "equals": "part" } },
            { "id": "hidden", "type": "text", "label": "Never shown", "visible_if": { "field": "range", "equals": "all" } },
            { "type": "group", "label": "Quality", "fields": [
                { "id": "crf", "type": "int", "label": "Level", "min": 0, "max": 51, "default": 23, "slider": true },
                { "id": "speed", "type": "float", "label": "Speed", "default": 1.5 },
                { "id": "fast", "type": "bool", "label": "Fast start", "default": true },
                { "id": "locked", "type": "text", "label": "Disabled", "default": "Can't change", "enabled": false },
            ] },
        ],
    }))
    .expect("form");
    form.check().expect("valid");
    let mut state = FormState::new(form, None);
    state.errors = state.form.validate(&state.values).expect_err("blank name");
    state.message = Some("The output folder is full.".to_owned());
    state
}

fn sample_table() -> TableView<u32> {
    let spec: TableSpec<u32> = serde_json::from_value(json!({
        "title": "Folder sizes",
        "subtitle": "{count} folders, {total}",
        "columns": [
            { "id": "name", "label": "Name" },
            { "id": "size", "label": "Size", "kind": "size" },
            { "id": "time", "label": "Last change", "kind": "time" },
        ],
        "rows": [
            { "payload": 1, "cells": { "name": "node_modules", "size": 1_288_490_188u64, "time": 1_700_000_000 } },
            { "payload": 2, "cells": { "name": "target", "size": 8_589_934_592u64, "time": 1_709_164_800 } },
            { "payload": 3, "cells": { "name": "A folder whose name is long enough to run into the columns beside it and be cut off there", "size": 52_428_800, "time": 1_600_000_000 } },
            { "payload": 4, "cells": { "name": "Not measured yet" } },
            { "cells": { "name": "No payload", "size": 512, "time": 0 } },
        ],
        "sort": { "column": "size", "descending": true },
    }))
    .expect("spec");
    spec.check().expect("valid");
    TableView::new(spec)
}

#[test]
fn form_with_every_field_type() {
    snapshot("form", vec2(520.0, 780.0), sample_form, |ui, state| {
        state.ui(ui, "Fallback");
    });
}

#[test]
fn table_sorted_descending() {
    snapshot("table", vec2(620.0, 400.0), sample_table, |ui, table| {
        table.contents_ui(ui);
    });
}

#[test]
fn table_sorted_ascending_on_the_first_column() {
    let sorted = || {
        let mut table = sample_table();
        table.apply_sort((0, false));
        table
    };
    snapshot("table_by_name", vec2(620.0, 400.0), sorted, |ui, table| {
        table.contents_ui(ui);
    });
}
