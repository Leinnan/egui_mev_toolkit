//! Driving forms and tables the way a user does, through the public API.

use std::path::{Path, PathBuf};

use egui::{Context, Event, Key, Modifiers, PointerButton, Pos2, Role};
use egui_kittest::{Harness, kittest::Queryable as _};
use egui_mev_data::{
    Form, FormState, Outcome, RowClick, RowUpdate, SizeFilter, SizeUnit, TableSpec, TableView,
};
use egui_mev_theme::{Theme, presets};
use serde::Deserialize;
use serde_json::{Value, json};

fn themed(ctx: &Context) {
    egui_mev_test::install(ctx, Theme::dark_only(presets::GRAPHITE_DARK));
}

// ---------------------------------------------------------------------------
// Forms
// ---------------------------------------------------------------------------

fn form_harness<'a>(value: Value) -> Harness<'a, (FormState, Outcome)> {
    let form: Form = serde_json::from_value(value).expect("form");
    form.check().expect("valid");
    Harness::builder()
        .with_size([520.0, 480.0])
        .with_context_setup(themed)
        .build_ui_state(
            |ui, (state, outcome): &mut (FormState, Outcome)| {
                let now = state.ui(ui, "Fallback title");
                if now != Outcome::Open {
                    *outcome = now;
                }
            },
            (FormState::new(form, None), Outcome::Open),
        )
}

#[test]
fn the_title_and_submit_text_have_fallbacks() {
    let mut harness = form_harness(json!({ "fields": [] }));
    harness.run();
    assert!(harness.query_by_label("Fallback title").is_some());
    assert!(harness.query_by_label("Run").is_some());

    let mut harness = form_harness(json!({ "title": "Convert", "submit": "Go", "fields": [] }));
    harness.run();
    assert!(harness.query_by_label("Convert").is_some());
    assert!(harness.query_by_label("Go").is_some());
    assert!(harness.query_by_label("Run").is_none());
}

#[test]
fn inputs_are_named_by_their_labels() {
    let mut harness = form_harness(json!({ "fields": [
        { "id": "name", "type": "text", "label": "Name" },
        { "id": "notes", "type": "multiline", "label": "Notes" },
        { "id": "count", "type": "int", "label": "Count" },
        { "id": "ratio", "type": "float", "label": "Ratio", "min": 0, "max": 1, "slider": true },
        { "id": "fast", "type": "bool", "label": "Fast" },
        { "id": "start", "type": "time", "label": "Start" },
        { "id": "kind", "type": "choice", "label": "Kind", "options": ["a", "b"] },
    ] }));
    harness.run();
    for (role, name) in [
        (Role::TextInput, "Name"),
        (Role::MultilineTextInput, "Notes"),
        (Role::SpinButton, "Count"),
        (Role::Slider, "Ratio"),
        (Role::CheckBox, "Fast"),
        (Role::TextInput, "Start"),
        (Role::ComboBox, "Kind"),
    ] {
        assert!(
            harness.query_by_role_and_label(role, name).is_some(),
            "{role:?} {name}"
        );
    }
}

#[test]
fn a_condition_shows_a_field_once_its_value_changes() {
    let mut harness = form_harness(json!({ "fields": [
        { "id": "range", "type": "radio", "options": [["all", "Whole"], ["part", "Fragment"]], "default": "all" },
        { "id": "start", "type": "text", "label": "Start", "visible_if": { "field": "range", "equals": "part" } },
    ] }));
    harness.run();
    assert!(harness.query_by_label("Start").is_none());
    harness.get_by_label("Fragment").click();
    harness.run();
    assert_eq!(harness.state().0.values["range"], json!("part"));
    assert!(harness.query_by_label("Start").is_some());
}

#[test]
fn a_disabled_field_takes_no_input_until_its_condition_holds() {
    let mut harness = form_harness(json!({ "fields": [
        { "id": "on", "type": "bool", "label": "On" },
        { "id": "extra", "type": "bool", "label": "Extra", "enabled_if": { "field": "on", "equals": true } },
    ] }));
    harness.run();
    let extra = |harness: &Harness<'_, (FormState, Outcome)>| {
        harness.state().0.values["extra"].as_bool().expect("bool")
    };
    harness
        .get_by_role_and_label(Role::CheckBox, "Extra")
        .click();
    harness.run();
    assert!(!extra(&harness), "disabled");
    harness.get_by_role_and_label(Role::CheckBox, "On").click();
    harness.run();
    harness
        .get_by_role_and_label(Role::CheckBox, "Extra")
        .click();
    harness.run();
    assert!(extra(&harness), "enabled now");
}

#[test]
fn a_group_disables_what_is_inside_it() {
    let mut harness = form_harness(json!({ "fields": [
        { "type": "group", "label": "Locked", "enabled": false, "fields": [
            { "id": "inner", "type": "bool", "label": "Inner" },
        ] },
    ] }));
    harness.run();
    harness
        .get_by_role_and_label(Role::CheckBox, "Inner")
        .click();
    harness.run();
    assert_eq!(harness.state().0.values["inner"], json!(false));
}

#[test]
fn the_buttons_submit_and_cancel() {
    let mut harness = form_harness(json!({ "fields": [] }));
    harness.run();
    harness.get_by_label("Run").click();
    harness.run();
    assert_eq!(harness.state().1, Outcome::Submit);

    let mut harness = form_harness(json!({ "fields": [] }));
    harness.run();
    harness.get_by_label("Cancel").click();
    harness.run();
    assert_eq!(harness.state().1, Outcome::Cancel);
}

#[test]
fn enter_submits_and_escape_cancels() {
    let mut harness = form_harness(json!({ "fields": [] }));
    harness.run();
    harness.key_press(Key::Enter);
    harness.run();
    assert_eq!(harness.state().1, Outcome::Submit);

    let mut harness = form_harness(json!({ "fields": [] }));
    harness.run();
    harness.key_press(Key::Escape);
    harness.run();
    assert_eq!(harness.state().1, Outcome::Cancel);
}

#[test]
fn enter_in_a_text_field_submits_and_in_a_multiline_one_does_not() {
    let mut harness = form_harness(json!({ "fields": [
        { "id": "name", "type": "text", "label": "Name" },
    ] }));
    harness.run();
    harness
        .get_by_role_and_label(Role::TextInput, "Name")
        .focus();
    harness.run();
    harness.key_press(Key::Enter);
    harness.run();
    assert_eq!(harness.state().1, Outcome::Submit);

    let mut harness = form_harness(json!({ "fields": [
        { "id": "notes", "type": "multiline", "label": "Notes" },
    ] }));
    harness.run();
    harness
        .get_by_role_and_label(Role::MultilineTextInput, "Notes")
        .focus();
    harness.run();
    harness.key_press(Key::Enter);
    harness.run();
    assert_eq!(harness.state().1, Outcome::Open);
}

#[test]
fn escape_and_enter_go_to_an_open_drop_down_first() {
    let mut harness = form_harness(json!({ "fields": [
        { "id": "kind", "type": "choice", "label": "Kind", "options": [["a", "First"], ["b", "Second"]] },
    ] }));
    harness.run();
    harness
        .get_by_role_and_label(Role::ComboBox, "Kind")
        .click();
    harness.run();
    assert!(egui::Popup::is_any_open(&harness.ctx));
    harness.key_press(Key::Enter);
    harness.step();
    assert_eq!(harness.state().1, Outcome::Open, "Enter didn't submit");
    harness.run();
    if !egui::Popup::is_any_open(&harness.ctx) {
        harness
            .get_by_role_and_label(Role::ComboBox, "Kind")
            .click();
        harness.run();
    }
    harness.key_press(Key::Escape);
    harness.run();
    assert_eq!(
        harness.state().1,
        Outcome::Open,
        "only the drop-down closed"
    );
    assert!(!egui::Popup::is_any_open(&harness.ctx));
    harness.key_press(Key::Escape);
    harness.run();
    assert_eq!(harness.state().1, Outcome::Cancel, "now the form goes");
}

#[test]
fn a_busy_form_refuses_to_submit_and_can_still_cancel() {
    let mut harness = form_harness(json!({ "fields": [] }));
    harness.state_mut().0.busy = true;
    // The spinner keeps asking for frames.
    harness.run_steps(3);
    harness.get_by_label("Run").click();
    harness.run_steps(3);
    harness.key_press(Key::Enter);
    harness.run_steps(3);
    assert_eq!(harness.state().1, Outcome::Open);
    harness.get_by_label("Cancel").click();
    harness.run_steps(3);
    assert_eq!(harness.state().1, Outcome::Cancel);
}

#[test]
fn a_time_typed_badly_is_flagged_and_can_not_be_submitted() {
    let mut harness = form_harness(json!({ "fields": [
        { "id": "start", "type": "time", "label": "Start", "default": 30, "max": 90 },
    ] }));
    harness.run();
    assert!(harness.query_by_label("of 01:30").is_some());
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, "Start")
            .value()
            .as_deref(),
        Some("00:30")
    );

    harness
        .get_by_role_and_label(Role::TextInput, "Start")
        .focus();
    harness.run();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.event(Event::Text("1:xx".to_owned()));
    harness.run();
    assert!(
        harness
            .query_by_label("Use hh:mm:ss.mmm or seconds")
            .is_some()
    );
    let state = &harness.state().0;
    assert!(
        state.form.validate(&state.values).is_err(),
        "not the last good value, silently"
    );

    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.event(Event::Text("1:15".to_owned()));
    harness.run();
    assert!(
        harness
            .query_by_label("Use hh:mm:ss.mmm or seconds")
            .is_none()
    );
    let state = &harness.state().0;
    let ok = state.form.validate(&state.values).expect("readable again");
    assert_eq!(ok["start"], json!(75.0));
}

#[test]
fn errors_and_messages_show_and_editing_clears_them() {
    let mut harness = form_harness(json!({ "fields": [
        { "id": "name", "type": "text", "label": "Name", "required": true },
    ] }));
    harness.run();
    let state = &mut harness.state_mut().0;
    state.errors = state.form.validate(&state.values).expect_err("blank");
    state.message = Some("That went badly".into());
    harness.run();
    assert!(harness.query_by_label("Required").is_some());
    assert!(harness.query_by_label("That went badly").is_some());

    harness
        .get_by_role_and_label(Role::TextInput, "Name")
        .focus();
    harness.run();
    harness.event(Event::Text("x".to_owned()));
    harness.run();
    assert_eq!(harness.state().0.values["name"], json!("x"));
    assert!(harness.query_by_label("Required").is_none());
    assert!(harness.query_by_label("That went badly").is_none());
}

#[test]
fn fields_only_draws_no_buttons() {
    let form: Form = serde_json::from_value(json!({ "fields": [
        { "id": "name", "type": "text", "label": "Name" },
    ] }))
    .expect("form");
    let mut harness = Harness::builder()
        .with_size([400.0, 200.0])
        .with_context_setup(themed)
        .build_ui_state(
            |ui, state: &mut FormState| state.fields_only(ui),
            FormState::new(form, None),
        );
    harness.run();
    assert!(harness.query_by_label("Name").is_some());
    assert!(harness.query_by_label("Run").is_none());
    assert!(harness.query_by_label("Cancel").is_none());
}

// ---------------------------------------------------------------------------
// Tables
// ---------------------------------------------------------------------------

/// A payload that is no path, and has no `Default`, unlike a `PathBuf`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct Track {
    album: String,
    number: u32,
}

fn track(album: &str, number: u32) -> Track {
    Track {
        album: album.to_owned(),
        number,
    }
}

fn track_spec() -> TableSpec<Track> {
    serde_json::from_value(json!({
        "title": "Tracks",
        "columns": [
            { "id": "name", "label": "Name" },
            { "id": "size", "label": "Size", "kind": "size" },
        ],
        "rows": [
            { "payload": { "album": "A", "number": 1 }, "cells": { "name": "one", "size": 10 } },
            { "payload": { "album": "A", "number": 2 }, "cells": { "name": "two", "size": 20 },
              "tooltip": "The second", "menu": [
                { "label": "Remove", "action": "remove", "danger": true },
            ] },
            { "cells": { "name": "no payload", "size": 30 } },
        ],
    }))
    .expect("spec")
}

type TableState = (TableView<Track>, Option<Track>);

fn table_harness<'a>() -> Harness<'a, TableState> {
    Harness::builder()
        .with_size([500.0, 300.0])
        .with_context_setup(themed)
        .build_ui_state(
            |ui, (table, clicked): &mut TableState| {
                if let Some(payload) = table.contents_ui(ui) {
                    *clicked = Some(payload);
                }
            },
            (TableView::new(track_spec()), None),
        )
}

#[test]
fn clicking_a_row_returns_its_payload() {
    let mut harness = table_harness();
    harness.run();
    assert_eq!(harness.state().1, None);
    harness.get_by_label("two, 20 B").click();
    harness.run();
    assert_eq!(harness.state().1, Some(track("A", 2)));
}

#[test]
fn a_row_without_a_payload_returns_nothing() {
    let mut harness = table_harness();
    harness.run();
    let row = harness.get_by_label("no payload, 30 B");
    let center = row.rect().center();
    harness.event(Event::PointerMoved(center));
    harness.run();
    press_and_release(&harness, center);
    harness.run();
    assert_eq!(harness.state().1, None);
}

#[test]
fn a_path_payload_still_works() {
    let spec: TableSpec<PathBuf> = serde_json::from_value(json!({
        "title": "Folders",
        "columns": [{ "id": "name", "label": "Name" }],
        "rows": [{ "path": "/tmp/a", "cells": { "name": "a" } }],
    }))
    .expect("spec");
    let table = TableView::new(spec);
    assert!(table.has_row(Path::new("/tmp/a")));
    assert!(!table.has_row(Path::new("/tmp/b")));
    let mut harness = Harness::builder()
        .with_size([300.0, 200.0])
        .with_context_setup(themed)
        .build_ui_state(
            |ui, (table, clicked): &mut (TableView<PathBuf>, Option<PathBuf>)| {
                if let Some(path) = table.contents_ui(ui) {
                    *clicked = Some(path);
                }
            },
            (table, None),
        );
    harness.run();
    harness.get_by_label("a").click();
    harness.run();
    assert_eq!(harness.state().1.as_deref(), Some(Path::new("/tmp/a")));
}

#[test]
fn clicking_a_header_sorts_and_clicking_again_reverses() {
    let mut harness = table_harness();
    harness.run();
    assert_eq!(harness.state().0.sorted_by(), None);
    harness.get_by_label("Size").click();
    harness.run();
    assert_eq!(harness.state().0.sorted_by(), Some((1, true)));
    harness.get_by_label("Size").click();
    harness.run();
    assert_eq!(harness.state().0.sorted_by(), Some((1, false)));
    harness.get_by_label("Name").click();
    harness.run();
    assert_eq!(harness.state().0.sorted_by(), Some((0, false)));
    // Sorting isn't a row click.
    assert_eq!(harness.state().1, None);
}

#[test]
fn the_size_filter_is_named_and_says_what_it_hides() {
    let mut harness = table_harness();
    harness.run();
    assert!(harness.query_by_label("Size unit").is_some());
    assert!(harness.query_by_label("Hide smaller than").is_some());
    assert!(harness.query_by_label("Showing 0 of 3").is_none());
    harness.state_mut().0.set_filter(SizeFilter {
        amount: 1.0,
        unit: SizeUnit::Mb,
    });
    harness.run();
    assert!(harness.query_by_label("Showing 0 of 3").is_some());
    assert!(harness.query_by_label("Nothing is this large.").is_some());
    assert!(harness.query_by_label("two, 20 B").is_none());
}

#[test]
fn updating_a_row_by_its_payload_shows_at_once() {
    let mut harness = table_harness();
    harness.run();
    harness.state_mut().0.update_row(RowUpdate {
        payload: track("A", 1),
        cells: [("name".to_owned(), egui_mev_data::Cell::Text("uno".into()))].into(),
        clear: vec!["size".to_owned()],
        remove: false,
    });
    harness.run();
    assert!(harness.query_by_label("uno").is_some());
    harness.state_mut().0.update_row(RowUpdate {
        payload: track("A", 1),
        cells: Default::default(),
        clear: Vec::new(),
        remove: true,
    });
    harness.run();
    assert!(harness.query_by_label("uno").is_none());
    assert!(!harness.state().0.has_row(&track("A", 1)));
    assert!(harness.state().0.has_row(&track("A", 2)));
}

#[test]
fn the_window_shows_the_title_returns_the_payload_and_can_close() {
    let id = egui::Id::unique("egui_mev_data::test_window");
    let mut harness = Harness::builder()
        .with_size([700.0, 600.0])
        .with_context_setup(themed)
        .build_ui_state(
            |ui, (table, clicked, open): &mut (TableView<Track>, Option<Track>, bool)| {
                if let Some(payload) = table.window_ui(ui.ctx(), id, open) {
                    *clicked = Some(payload);
                }
            },
            (TableView::new(track_spec()), None, true),
        );
    harness.run();
    assert!(harness.query_by_label("Tracks").is_some());
    assert_eq!(harness.state().0.title(), "Tracks");
    harness.get_by_label("one, 10 B").click();
    harness.run();
    assert_eq!(harness.state().1, Some(track("A", 1)));

    harness.state_mut().2 = false;
    harness.run();
    assert!(harness.query_by_label("one, 10 B").is_none());
}

// With menus, rows answer differently: double clicks and their context menu.

type MenuState = (TableView<Track>, Option<RowClick<Track>>);

fn menu_spec() -> TableSpec<Track> {
    let mut spec = track_spec();
    spec.refresh_action = Some("measure".to_owned());
    spec
}

fn menu_harness<'a>(running: bool) -> Harness<'a, MenuState> {
    let mut table = TableView::new(menu_spec());
    table.set_running(running);
    Harness::builder()
        .with_size([500.0, 300.0])
        // Each queued event takes a frame, and a double click has 0.3 s to finish.
        .with_step_dt(0.05)
        .with_context_setup(themed)
        .build_ui_state(
            |ui, (table, click): &mut MenuState| {
                if let Some(asked) = table.contents_with_menus_ui(ui) {
                    *click = Some(asked);
                }
            },
            (table, None),
        )
}

fn press_and_release<S>(harness: &Harness<'_, S>, pos: Pos2) {
    for pressed in [true, false] {
        harness.event(Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
    }
}

#[test]
fn with_menus_a_click_does_nothing_and_a_double_click_opens() {
    let mut harness = menu_harness(false);
    harness.run();
    let center = harness.get_by_label("two, 20 B").rect().center();
    harness.event(Event::PointerMoved(center));
    harness.run();
    press_and_release(&harness, center);
    harness.step();
    assert_eq!(harness.state().1, None, "a single click");
    press_and_release(&harness, center);
    harness.step();
    assert_eq!(harness.state().1, Some(RowClick::Open(track("A", 2))));
}

#[test]
fn with_menus_enter_opens_the_focused_row() {
    let mut harness = menu_harness(false);
    harness.run();
    harness.get_by_label("one, 10 B").focus();
    harness.run();
    harness.key_press(Key::Enter);
    harness.run();
    assert_eq!(harness.state().1, Some(RowClick::Open(track("A", 1))));
}

#[test]
fn the_context_menu_asks_for_each_thing_with_the_payload() {
    for (entry, expected) in [
        ("Open", RowClick::Open(track("A", 2))),
        ("Refresh", RowClick::Refresh(track("A", 2))),
        (
            "Remove",
            RowClick::Action {
                action: "remove".to_owned(),
                item: serde_json::from_value(
                    json!({ "label": "Remove", "action": "remove", "danger": true }),
                )
                .expect("action"),
                payload: track("A", 2),
            },
        ),
    ] {
        let mut harness = menu_harness(false);
        harness.run();
        harness.get_by_label("two, 20 B").click_secondary();
        harness.run();
        harness.get_by_label(entry).click();
        harness.run();
        assert_eq!(harness.state().1.as_ref(), Some(&expected), "{entry}");
        assert!(harness.query_by_label(entry).is_none(), "the menu closed");
    }
}

#[test]
fn row_actions_wait_for_the_job_but_open_does_not() {
    for entry in ["Refresh", "Remove"] {
        let mut harness = menu_harness(true);
        harness.run();
        harness.get_by_label("two, 20 B").click_secondary();
        harness.run();
        harness.get_by_label(entry).click();
        harness.run();
        assert_eq!(harness.state().1, None, "{entry} is off");
    }
    let mut harness = menu_harness(true);
    harness.run();
    harness.get_by_label("two, 20 B").click_secondary();
    harness.run();
    harness.get_by_label("Open").click();
    harness.run();
    assert_eq!(harness.state().1, Some(RowClick::Open(track("A", 2))));
}

#[test]
fn contents_ui_offers_no_menu_it_could_not_report() {
    let mut harness = table_harness();
    harness.run();
    harness.get_by_label("two, 20 B").click_secondary();
    harness.run();
    assert!(harness.query_by_label("Remove").is_none());
    assert!(harness.query_by_label("Open").is_none());
}

#[test]
fn hovering_a_row_shows_its_tooltip() {
    let mut harness = table_harness();
    harness.run();
    harness.get_by_label("two, 20 B").hover();
    // Tooltips wait for the pointer to rest.
    for _ in 0..20 {
        harness.step();
    }
    assert!(harness.query_by_label("The second").is_some());
}

#[test]
fn progress_left_by_a_stopped_job_says_so() {
    let mut spec = track_spec();
    spec.progress = Some("Scanning".to_owned());
    let mut harness = Harness::builder()
        .with_size([500.0, 300.0])
        .with_context_setup(themed)
        .build_ui_state(
            |ui, table: &mut TableView<Track>| {
                table.contents_ui(ui);
            },
            TableView::new(spec),
        );
    harness.run();
    assert!(harness.query_by_label("Stopped early").is_none());
    harness.state_mut().set_running(false);
    harness.run();
    assert!(harness.query_by_label("Stopped early").is_some());
}

// ---------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------

#[test]
fn a_spec_with_paths_deserializes_under_either_key() {
    let spec: TableSpec<PathBuf> = serde_json::from_value(json!({
        "title": "T",
        "subtitle": "Sub",
        "progress": "Going",
        "refresh_action": "again",
        "columns": [
            { "id": "a", "label": "A" },
            { "id": "b", "label": "B", "kind": "size" },
            { "id": "c", "label": "C", "kind": "time" },
        ],
        "rows": [
            { "path": "/x", "cells": { "a": "text", "b": 1.5 } },
            { "payload": "/y", "cells": {} },
            { "cells": {} },
        ],
        "sort": { "column": "b", "descending": true },
    }))
    .expect("spec");
    spec.check().expect("valid");
    assert_eq!(spec.rows[0].payload.as_deref(), Some(Path::new("/x")));
    assert_eq!(spec.rows[1].payload.as_deref(), Some(Path::new("/y")));
    assert_eq!(spec.rows[2].payload, None);
    assert_eq!(
        spec.rows[0].cells["a"],
        egui_mev_data::Cell::Text("text".into())
    );
    assert_eq!(spec.rows[0].cells["b"], egui_mev_data::Cell::Number(1.5));
    assert_eq!(spec.columns[1].kind, egui_mev_data::ColumnKind::Size);
    assert_eq!(spec.columns[0].kind, egui_mev_data::ColumnKind::Text);
    let sort = spec.sort.expect("sort");
    assert_eq!(sort.column, "b");
    assert!(sort.descending);
}

#[test]
fn a_payload_type_needs_no_default_and_rows_may_be_left_out() {
    let spec: TableSpec<Track> = serde_json::from_value(json!({
        "title": "T",
        "columns": [{ "id": "a", "label": "A" }],
    }))
    .expect("spec");
    assert!(spec.rows.is_empty());
    assert_eq!(track_spec().rows[1].payload, Some(track("A", 2)));
    assert_eq!(track_spec().rows[1].menu[0].action, "remove");
}

#[test]
fn unknown_keys_in_a_spec_are_refused() {
    let bad_row = serde_json::from_value::<TableSpec<u32>>(json!({
        "title": "T",
        "columns": [{ "id": "a", "label": "A" }],
        "rows": [{ "cells": {}, "colour": "red" }],
    }));
    assert!(bad_row.is_err());
    let bad_spec = serde_json::from_value::<TableSpec<u32>>(json!({
        "title": "T",
        "columns": [{ "id": "a", "label": "A" }],
        "footer": "no",
    }));
    assert!(bad_spec.is_err());
    let bad_payload = serde_json::from_value::<TableSpec<u32>>(json!({
        "title": "T",
        "columns": [{ "id": "a", "label": "A" }],
        "rows": [{ "payload": "not a number", "cells": {} }],
    }));
    assert!(bad_payload.is_err());
}
