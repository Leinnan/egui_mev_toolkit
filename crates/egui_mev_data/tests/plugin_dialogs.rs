//! Custom dialogs keep using the shared form and table state.
use egui_kittest::{
    Harness,
    kittest::{NodeT as _, Queryable as _},
};
use egui_mev_data::{Form, FormState, Outcome, RowStatus, RowUpdate, TableSpec, TableView};
use serde_json::json;

fn form_harness(value: serde_json::Value) -> Harness<'static, (FormState, Outcome)> {
    let form: Form = serde_json::from_value(value).expect("form");
    form.check().expect("valid form");
    Harness::builder().with_size([560.0, 400.0]).build_ui_state(
        |ui, (state, outcome): &mut (FormState, Outcome)| {
            let now = state.ui(ui, "Confirm");
            if now != Outcome::Open {
                *outcome = now;
            }
        },
        (FormState::new(form, None), Outcome::Open),
    )
}

#[test]
fn confirmation_fields_choose_the_submit_label_and_show_paths() {
    let mut harness = form_harness(json!({
        "submit": "Move", "danger": true,
        "submit_when": [{"when": {"field": "how", "equals": "delete"}, "label": "Delete", "danger": true}],
        "fields": [
            {"type": "paths", "rows": [{"path": "/example", "size": "1 GB"}, {"path": "/other"}]},
            {"id": "how", "type": "radio", "default": "move", "options": ["move", "delete"]}
        ]
    }));
    harness.run();
    assert!(harness.query_by_label("/example").is_some());
    assert!(harness.query_by_label("1 GB").is_some());
    assert!(harness.query_by_label("Move").is_some());
    harness.get_by_label("delete").click();
    harness.run();
    assert!(harness.query_by_label("Delete").is_some());
    assert!(harness.query_by_label("Move").is_none());
}

#[test]
fn blocked_confirmation_cannot_submit_with_enter() {
    let mut harness = form_harness(json!({"blocked": true, "submit": "Delete", "fields": []}));
    harness.run();
    assert!(harness.query_by_label("Delete").is_none());
    assert!(harness.query_by_label("Close").is_some());
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert_eq!(harness.state().1, Outcome::Open);
    harness.get_by_label("Close").click();
    harness.run();
    assert_eq!(harness.state().1, Outcome::Cancel);
}

#[test]
fn toolbar_choices_update_values_and_honor_disabled_fields() {
    let form: Form = serde_json::from_value(json!({"fields": [
        {"id": "scope", "type": "radio", "options": ["all", "some"], "default": "all"},
        {"id": "disabled", "type": "text", "label": "Disabled", "default": "same", "enabled": false}
    ]}))
    .expect("form");
    let mut harness = Harness::new_ui_state(
        |ui, state: &mut FormState| {
            ui.horizontal(|ui| state.toolbar_ui(ui));
        },
        FormState::new(form, None),
    );
    harness.run();
    harness.get_by_label("some").click();
    harness.run();
    assert_eq!(harness.state().values["scope"], "some");
    assert_eq!(harness.state().values["disabled"], "same");
    assert!(
        harness
            .get_by_role_and_label(egui::Role::TextInput, "Disabled")
            .accesskit_node()
            .is_disabled()
    );
}

fn table_spec() -> TableSpec<String> {
    serde_json::from_value(json!({
        "title": "Results", "columns": [{"id": "name", "label": "Name"}],
        "rows": [{"payload": "one", "cells": {"name": "One"}, "menu": [
            {"label": "Remove", "action": "remove", "busy": "Removing", "title": "Remove one?", "danger": true}
        ]}]
    })).expect("spec")
}

#[test]
fn borrowed_status_clears_on_update_and_does_not_leak_to_removed_rows() {
    let mut table = TableView::new(table_spec());
    table.set_status("one", Some(RowStatus::Working("Removing".into())));
    assert_eq!(
        table.status("one"),
        Some(&RowStatus::Working("Removing".into()))
    );
    let update: RowUpdate<String> =
        serde_json::from_value(json!({"payload": "one", "cells": {"name": "Updated"}}))
            .expect("update");
    table.update_row(update);
    assert_eq!(table.status("one"), None);
    table.set_status("one", Some(RowStatus::Failed("Refused".into())));
    let mut spec = table_spec();
    spec.rows.clear();
    table.update(spec);
    assert_eq!(table.status("one"), None);
    table.set_status("missing", Some(RowStatus::Waiting));
    assert_eq!(table.status("missing"), None);
}

#[test]
fn custom_dialog_draws_status_and_disables_busy_row_actions() {
    let mut table = TableView::new(table_spec());
    table.set_status("one", Some(RowStatus::Working("Removing".into())));
    let mut harness = Harness::builder().with_size([560.0, 400.0]).build_ui_state(
        |ui, table: &mut TableView<String>| {
            table.summary_ui(ui);
            ui.horizontal(|ui| table.filter_ui(ui));
            table.body_ui(ui);
        },
        table,
    );
    harness.run_steps(8);
    harness.get_by_label("One, Removing").click_secondary();
    harness.run_steps(8);
    assert!(
        harness
            .get_by_label("Remove")
            .accesskit_node()
            .is_disabled()
    );
    // The status is associated with the payload rather than its display index.
    assert_eq!(
        harness.state().status("one"),
        Some(&RowStatus::Working("Removing".into()))
    );
}
