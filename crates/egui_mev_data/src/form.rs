//! Forms an app asks for before an action runs.
//!
//! A form is data: a list of fields, built in code or read from a file. This
//! module checks it, draws it with egui and validates the values. `visible_if`
//! and `enabled_if` are evaluated here too, so no app code has to run while the
//! dialog is open.

use std::collections::BTreeMap;

use egui::{Id, Key, Margin, RichText, Ui};
use egui_mev_theme::{Metrics, Palette};
use egui_mev_widgets::{DialogButton, dialog_buttons, text_field};
use serde::Deserialize;
use serde_json::{Map, Value};

/// Submitted values by field id: strings, numbers and booleans.
pub type Values = Map<String, Value>;

/// A dialog's fields and how it reads.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Form {
    /// The heading. [`FormState::ui`] falls back to its `default_title`.
    pub title: Option<String>,
    /// The submit button's text.
    pub submit: Option<String>,
    /// Start from the values used last time.
    #[serde(default)]
    pub remember: bool,
    /// The fields, in order.
    #[serde(default)]
    pub fields: Vec<FieldDef>,
}

/// What a field is, which decides how it draws and what value it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    /// Read-only text.
    Info,
    /// A line between fields.
    Separator,
    /// A labelled box around `fields`.
    Group,
    /// One line of text.
    Text,
    /// Several lines of text.
    Multiline,
    /// A whole number.
    Int,
    /// A number.
    Float,
    /// A checkbox.
    Bool,
    /// A drop-down.
    Choice,
    /// One of a row of radio buttons.
    Radio,
    /// A duration or position in seconds, typed as `hh:mm:ss.mmm`.
    Time,
    /// A path, typed as text. `mode` says what kind, for an app that adds a
    /// file dialog beside it.
    Path,
}

impl FieldType {
    /// Whether the field holds a value (and so needs an id).
    #[must_use]
    pub const fn is_input(self) -> bool {
        !matches!(self, Self::Info | Self::Separator | Self::Group)
    }
}

/// One entry of a `choice` or `radio` field.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ChoiceOption {
    /// `["value", "Label"]`
    Pair(String, String),
    /// `{ "value": "v", "label": "Label" }`
    Named {
        /// What is submitted.
        value: String,
        /// What is shown.
        label: String,
    },
    /// `"value"`, which is also its label.
    Plain(String),
}

impl ChoiceOption {
    /// What is submitted when this is picked.
    #[must_use]
    pub fn value(&self) -> &str {
        match self {
            Self::Pair(value, _) | Self::Named { value, .. } | Self::Plain(value) => value,
        }
    }

    /// What the option shows.
    #[must_use]
    pub fn label(&self) -> &str {
        match self {
            Self::Pair(_, label) | Self::Named { label, .. } => label,
            Self::Plain(value) => value,
        }
    }
}

/// `visible_if` / `enabled_if`: compares another field's value. Every check
/// that is present must hold.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Condition {
    /// The id of the field to read.
    pub field: String,
    /// Holds when the value is this.
    pub equals: Option<Value>,
    /// Holds when the value is not this.
    pub not_equals: Option<Value>,
    /// Holds when the value is one of these.
    #[serde(default, rename = "in")]
    pub one_of: Vec<Value>,
    /// Holds when the value is none of these.
    #[serde(default)]
    pub not_in: Vec<Value>,
}

impl Condition {
    fn holds(&self, values: &Values) -> bool {
        let value = values.get(&self.field).unwrap_or(&Value::Null);
        self.equals
            .as_ref()
            .is_none_or(|expected| same(value, expected))
            && self
                .not_equals
                .as_ref()
                .is_none_or(|other| !same(value, other))
            && (self.one_of.is_empty() || self.one_of.iter().any(|other| same(value, other)))
            && !self.not_in.iter().any(|other| same(value, other))
    }
}

/// Equality where `1` and `1.0` are the same number.
fn same(a: &Value, b: &Value) -> bool {
    match (a.as_f64(), b.as_f64()) {
        (Some(a), Some(b)) => (a - b).abs() < f64::EPSILON,
        _ => a == b,
    }
}

/// What a `path` field asks for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PathMode {
    /// An existing file.
    #[default]
    File,
    /// A file to write.
    Save,
    /// A folder.
    Folder,
}

/// One field of a [`Form`].
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldDef {
    /// The field's type.
    #[serde(rename = "type")]
    pub kind: FieldType,
    /// The key of its value. Required for fields that hold one.
    pub id: Option<String>,
    /// The text beside it, or its id when there is none.
    pub label: Option<String>,
    /// For `info`: the text shown.
    pub text: Option<String>,
    /// The starting value.
    pub default: Option<Value>,
    /// Text must not be blank while the field is visible and enabled.
    #[serde(default)]
    pub required: bool,
    /// Shown when hovering the field.
    pub help: Option<String>,
    /// Shown in an empty text field.
    pub placeholder: Option<String>,
    /// `false` disables the field whatever `enabled_if` says.
    pub enabled: Option<bool>,
    /// Hide the field unless this holds.
    pub visible_if: Option<Condition>,
    /// Disable the field unless this holds.
    pub enabled_if: Option<Condition>,
    /// The smallest number or time.
    pub min: Option<f64>,
    /// The largest number or time.
    pub max: Option<f64>,
    /// How far a number moves when dragged.
    pub step: Option<f64>,
    /// Draw a number as a slider. Needs both `min` and `max`.
    #[serde(default)]
    pub slider: bool,
    /// The entries of a `choice` or `radio` field.
    #[serde(default)]
    pub options: Vec<ChoiceOption>,
    /// The fields of a `group`.
    #[serde(default)]
    pub fields: Vec<Self>,
    /// What a `path` field asks for.
    #[serde(default)]
    pub mode: PathMode,
}

impl FieldDef {
    fn id(&self) -> &str {
        self.id.as_deref().unwrap_or_default()
    }

    fn label(&self) -> &str {
        self.label
            .as_deref()
            .or(self.id.as_deref())
            .unwrap_or_default()
    }

    /// Whether `visible_if` lets the field show.
    #[must_use]
    pub fn visible(&self, values: &Values) -> bool {
        self.visible_if
            .as_ref()
            .is_none_or(|condition| condition.holds(values))
    }

    /// Whether `enabled` and `enabled_if` let the field be edited.
    #[must_use]
    pub fn enabled(&self, values: &Values) -> bool {
        self.enabled.unwrap_or(true)
            && self
                .enabled_if
                .as_ref()
                .is_none_or(|condition| condition.holds(values))
    }

    /// The value a field starts with when there is no default.
    fn empty_value(&self) -> Value {
        match self.kind {
            FieldType::Bool => Value::Bool(false),
            FieldType::Int => Value::from(self.min.map_or(0, |min| min.ceil() as i64).max(0)),
            FieldType::Float | FieldType::Time => Value::from(self.min.unwrap_or(0.0).max(0.0)),
            FieldType::Choice | FieldType::Radio => self
                .options
                .first()
                .map_or(Value::Null, |option| Value::from(option.value())),
            FieldType::Text | FieldType::Multiline | FieldType::Path => Value::from(""),
            FieldType::Info | FieldType::Separator | FieldType::Group => Value::Null,
        }
    }

    /// The default, where it fits the field, or else the empty value.
    fn start_value(&self) -> Value {
        self.default
            .as_ref()
            .and_then(|value| self.conform(value).ok())
            .unwrap_or_else(|| self.empty_value())
    }

    /// `value` converted to this field's type and range, or why it can't be.
    fn conform(&self, value: &Value) -> Result<Value, String> {
        match self.kind {
            FieldType::Bool => value
                .as_bool()
                .map(Value::Bool)
                .ok_or_else(|| "must be true or false".into()),
            FieldType::Int => {
                let number = value.as_f64().ok_or("must be a whole number")?;
                let clamped = self.clamp(number.round());
                Ok(Value::from(clamped as i64))
            }
            FieldType::Float | FieldType::Time => {
                let number = value.as_f64().filter(|n| n.is_finite()).ok_or(
                    if self.kind == FieldType::Time {
                        BAD_TIME
                    } else {
                        "must be a number"
                    },
                )?;
                Ok(Value::from(self.clamp(number)))
            }
            FieldType::Choice | FieldType::Radio => {
                let text = match value {
                    Value::String(text) => text.clone(),
                    Value::Number(number) => number.to_string(),
                    _ => return Err("must be one of the options".into()),
                };
                if self.options.iter().any(|option| option.value() == text) {
                    Ok(Value::String(text))
                } else {
                    Err(format!("\"{text}\" is not one of the options"))
                }
            }
            FieldType::Text | FieldType::Multiline | FieldType::Path => match value {
                Value::String(text) => Ok(Value::String(text.clone())),
                Value::Number(number) => Ok(Value::String(number.to_string())),
                _ => Err("must be text".into()),
            },
            FieldType::Info | FieldType::Separator | FieldType::Group => Ok(Value::Null),
        }
    }

    fn clamp(&self, number: f64) -> f64 {
        let number = self.min.map_or(number, |min| number.max(min));
        self.max.map_or(number, |max| number.min(max))
    }
}

/// What a time field says about text it can't read.
const BAD_TIME: &str = "Use hh:mm:ss.mmm or seconds";

/// Every field, groups flattened, in order.
fn walk<'a>(fields: &'a [FieldDef], out: &mut Vec<&'a FieldDef>) {
    for field in fields {
        out.push(field);
        walk(&field.fields, out);
    }
}

/// Validates `fields` into `out` and `errors`. A field in a hidden or disabled
/// group counts as hidden or disabled itself.
fn validate_fields(
    fields: &[FieldDef],
    values: &Values,
    parent_active: bool,
    out: &mut Values,
    errors: &mut BTreeMap<String, String>,
) {
    for field in fields {
        let active = parent_active && field.visible(values) && field.enabled(values);
        if field.kind.is_input() {
            let id = field.id().to_owned();
            let value = values
                .get(&id)
                .cloned()
                .unwrap_or_else(|| field.empty_value());
            match field.conform(&value) {
                Ok(value) => {
                    if active
                        && field.required
                        && value.as_str().is_some_and(|s| s.trim().is_empty())
                    {
                        errors.insert(id.clone(), "Required".into());
                    }
                    out.insert(id, value);
                }
                Err(error) if active => {
                    errors.insert(id, error);
                }
                // Nobody can see or fix what a hidden field holds.
                Err(_) => {
                    out.insert(id, field.start_value());
                }
            }
        }
        validate_fields(&field.fields, values, active, out, errors);
    }
}

impl Form {
    /// Every field, groups flattened, in order.
    #[must_use]
    pub fn all_fields(&self) -> Vec<&FieldDef> {
        let mut out = Vec::new();
        walk(&self.fields, &mut out);
        out
    }

    /// Checks the schema itself: ids, options, ranges and defaults.
    pub fn check(&self) -> Result<(), String> {
        let fields = self.all_fields();
        if fields.len() > 128 {
            return Err("a form can have at most 128 fields".into());
        }
        let mut ids: Vec<&str> = Vec::new();
        for field in &fields {
            if field.kind.is_input() {
                let id = field
                    .id
                    .as_deref()
                    .filter(|id| !id.is_empty())
                    .ok_or_else(|| {
                        format!("a {:?} field needs an id", field.kind).to_lowercase()
                    })?;
                if ids.contains(&id) {
                    return Err(format!("field id \"{id}\" is used twice"));
                }
                ids.push(id);
            }
            if field.kind == FieldType::Group && field.label.is_none() {
                return Err("a group needs a label".into());
            }
            if field.kind != FieldType::Group && !field.fields.is_empty() {
                return Err(format!("field \"{}\": only groups have fields", field.id()));
            }
            if matches!(field.kind, FieldType::Choice | FieldType::Radio)
                && field.options.is_empty()
            {
                return Err(format!("field \"{}\" needs options", field.id()));
            }
            if let (Some(min), Some(max)) = (field.min, field.max)
                && min > max
            {
                return Err(format!("field \"{}\": min is above max", field.id()));
            }
            if let Some(default) = &field.default
                && field.kind.is_input()
            {
                field
                    .conform(default)
                    .map_err(|error| format!("field \"{}\" default {error}", field.id()))?;
            }
        }
        for field in &fields {
            for condition in [&field.visible_if, &field.enabled_if].into_iter().flatten() {
                if !ids.contains(&condition.field.as_str()) {
                    return Err(format!(
                        "field \"{}\" depends on unknown field \"{}\"",
                        field.id(),
                        condition.field
                    ));
                }
            }
        }
        Ok(())
    }

    /// Starting values: the defaults, then values remembered from last time
    /// where they still fit the field.
    #[must_use]
    pub fn initial_values(&self, remembered: Option<&Values>) -> Values {
        let mut values = Values::new();
        for field in self.all_fields() {
            if !field.kind.is_input() {
                continue;
            }
            // A field the form disabled this time keeps its default.
            let value = remembered
                .filter(|_| self.remember && field.enabled != Some(false))
                .and_then(|values| values.get(field.id()))
                .and_then(|value| field.conform(value).ok())
                .unwrap_or_else(|| field.start_value());
            values.insert(field.id().to_owned(), value);
        }
        values
    }

    /// The values to submit, or an error per field. Hidden and disabled
    /// fields keep their value but aren't required.
    pub fn validate(&self, values: &Values) -> Result<Values, BTreeMap<String, String>> {
        let mut out = Values::new();
        let mut errors = BTreeMap::new();
        validate_fields(&self.fields, values, true, &mut out, &mut errors);
        if errors.is_empty() {
            Ok(out)
        } else {
            Err(errors)
        }
    }
}

/// Formats seconds as `h:mm:ss.mmm` (hours only when needed).
#[must_use]
pub fn format_time(seconds: f64) -> String {
    let millis = (seconds.max(0.0) * 1000.0).round() as u64;
    let (hours, rest) = (millis / 3_600_000, millis % 3_600_000);
    let (minutes, rest) = (rest / 60_000, rest % 60_000);
    let (secs, millis) = (rest / 1000, rest % 1000);
    let base = if hours > 0 {
        format!("{hours}:{minutes:02}:{secs:02}")
    } else {
        format!("{minutes:02}:{secs:02}")
    };
    if millis == 0 {
        base
    } else {
        format!("{base}.{millis:03}")
    }
}

/// Parses `ss`, `mm:ss`, `hh:mm:ss`, each with optional `.fraction`.
#[must_use]
pub fn parse_time(text: &str) -> Option<f64> {
    let parts: Vec<&str> = text.trim().split(':').collect();
    if parts.is_empty() || parts.len() > 3 || parts.iter().any(|part| part.is_empty()) {
        return None;
    }
    let mut seconds: f64 = 0.0;
    for (index, part) in parts.iter().enumerate() {
        let last = index + 1 == parts.len();
        let value: f64 = if last {
            part.parse().ok()?
        } else {
            f64::from(part.parse::<u32>().ok()?)
        };
        if !value.is_finite() || value < 0.0 || (index > 0 && value >= 60.0) {
            return None;
        }
        seconds = seconds.mul_add(60.0, value);
    }
    Some(seconds)
}

/// What happened in the dialog this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Nothing yet.
    Open,
    /// Submit was pressed. Run [`Form::validate`] on the values next.
    Submit,
    /// Cancel or Escape was pressed.
    Cancel,
}

/// The margin of a text field, for the multi-line one the widgets crate doesn't
/// build. It matches [`egui_mev_widgets::text_field`].
fn text_field_margin(ui: &Ui) -> Margin {
    let margin_x = if Metrics::of(ui) == Metrics::FLUENT {
        4
    } else {
        8
    };
    Margin::symmetric(margin_x, 2)
}

/// An open form dialog.
#[derive(Debug, Clone)]
pub struct FormState {
    /// The form being shown.
    pub form: Form,
    /// The current values. A time field whose text can't be read holds null, so
    /// that [`Form::validate`] refuses it.
    pub values: Values,
    /// Errors to show beside their fields, by id.
    pub errors: BTreeMap<String, String>,
    /// An error for the whole form, from the app's own checks or a failed call.
    pub message: Option<String>,
    /// Text being typed into time fields, by id.
    time_text: BTreeMap<String, String>,
    /// The app is checking the values: submitting is off and a spinner shows.
    pub busy: bool,
}

impl FormState {
    /// Opens `form` with its starting values.
    #[must_use]
    pub fn new(form: Form, remembered: Option<&Values>) -> Self {
        let values = form.initial_values(remembered);
        Self {
            form,
            values,
            errors: BTreeMap::new(),
            message: None,
            time_text: BTreeMap::new(),
            busy: false,
        }
    }

    /// The fields in their grid.
    fn grid_ui(&mut self, ui: &mut Ui) {
        let fields = std::mem::take(&mut self.form.fields);
        egui::Grid::new("egui_mev_data::form")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| self.fields_ui(ui, &fields, true));
        self.form.fields = fields;
    }

    fn message_ui(&self, ui: &mut Ui) {
        if let Some(message) = &self.message {
            ui.colored_label(Palette::of(ui).critical, message);
        }
    }

    /// Draws only the fields and any message, for a dialog with its own
    /// buttons.
    pub fn fields_only(&mut self, ui: &mut Ui) {
        self.grid_ui(ui);
        self.message_ui(ui);
    }

    /// Draws the dialog's fields and buttons.
    pub fn ui(&mut self, ui: &mut Ui, default_title: &str) -> Outcome {
        // A key pressed while a drop-down is open belongs to the drop-down. The
        // fields below close it, so look now.
        let popup_open = egui::Popup::is_any_open(ui.ctx());
        let title = self
            .form
            .title
            .clone()
            .unwrap_or_else(|| default_title.to_owned());
        ui.label(RichText::new(title).heading().strong());
        ui.add_space(6.0);
        egui::ScrollArea::vertical()
            .max_height((ui.ctx().content_rect().height() - 200.0).max(160.0))
            .show(ui, |ui| self.grid_ui(ui));
        self.message_ui(ui);
        ui.separator();
        let submit = self.form.submit.clone().unwrap_or_else(|| "Run".into());
        let clicked = ui
            .horizontal(|ui| {
                if self.busy {
                    ui.spinner();
                }
                dialog_buttons(
                    ui,
                    false,
                    &[
                        DialogButton::primary(&submit).enabled(!self.busy),
                        DialogButton::plain("Cancel"),
                    ],
                )
            })
            .inner;
        // A focused control takes Enter for itself; a text field gives up its
        // focus on Enter, so that one still submits.
        let enter = !popup_open
            && !self.busy
            && ui.input(|i| i.key_pressed(Key::Enter) && !i.modifiers.shift)
            && ui.memory(|memory| memory.focused().is_none());
        let escape =
            !popup_open && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape));
        match clicked {
            Some(1) => Outcome::Cancel,
            _ if escape => Outcome::Cancel,
            Some(_) => Outcome::Submit,
            None if enter => Outcome::Submit,
            None => Outcome::Open,
        }
    }

    fn fields_ui(&mut self, ui: &mut Ui, fields: &[FieldDef], parent_enabled: bool) {
        let p = Palette::of(ui);
        for field in fields {
            if !field.visible(&self.values) {
                continue;
            }
            let enabled = parent_enabled && field.enabled(&self.values);
            match field.kind {
                FieldType::Info => {
                    if let Some(text) = field.text.as_deref().filter(|text| !text.is_empty()) {
                        ui.label("");
                        ui.add(egui::Label::new(RichText::new(text).color(p.text_weak)).wrap());
                        ui.end_row();
                    }
                }
                FieldType::Separator => {
                    ui.separator();
                    ui.separator();
                    ui.end_row();
                }
                FieldType::Group => {
                    ui.label(RichText::new(field.label()).strong());
                    ui.label("");
                    ui.end_row();
                    self.fields_ui(ui, &field.fields, enabled);
                }
                _ => {
                    let label = ui.label(RichText::new(field.label()).color(p.text_label));
                    let label_id = label.id;
                    if let Some(help) = &field.help {
                        label.on_hover_text(help);
                    }
                    ui.add_enabled_ui(enabled, |ui| self.input_ui(ui, field, label_id));
                    ui.end_row();
                    // On a row of its own, so the label stays beside its control.
                    if let Some(error) = self.errors.get(field.id()) {
                        ui.label("");
                        ui.colored_label(p.critical, error);
                        ui.end_row();
                    }
                }
            }
        }
    }

    fn input_ui(&mut self, ui: &mut Ui, field: &FieldDef, label: Id) {
        let id = field.id().to_owned();
        let value = self.values.get(&id).cloned().unwrap_or(Value::Null);
        let changed = match field.kind {
            FieldType::Text | FieldType::Path => {
                let mut text = value.as_str().unwrap_or_default().to_owned();
                let mut edit = text_field(ui, &mut text).desired_width(260.0);
                if let Some(hint) = &field.placeholder {
                    edit = edit.hint_text(hint.as_str());
                }
                let changed = ui.add(edit).labelled_by(label).changed();
                changed.then_some(Value::String(text))
            }
            FieldType::Multiline => {
                let mut text = value.as_str().unwrap_or_default().to_owned();
                let mut edit = egui::TextEdit::multiline(&mut text)
                    .margin(text_field_margin(ui))
                    .desired_width(260.0)
                    .desired_rows(4);
                if let Some(hint) = &field.placeholder {
                    edit = edit.hint_text(hint.as_str());
                }
                let changed = ui.add(edit).labelled_by(label).changed();
                changed.then_some(Value::String(text))
            }
            FieldType::Bool => {
                let mut on = value.as_bool().unwrap_or(false);
                ui.checkbox(&mut on, "")
                    .labelled_by(label)
                    .changed()
                    .then_some(Value::Bool(on))
            }
            FieldType::Int => {
                let mut number = value.as_i64().unwrap_or(0);
                let min = field.min.map_or(i64::MIN, |min| min.ceil() as i64);
                let max = field.max.map_or(i64::MAX, |max| max.floor() as i64);
                let response = if field.slider && field.min.is_some() && field.max.is_some() {
                    ui.add(egui::Slider::new(&mut number, min..=max))
                } else {
                    ui.add(
                        egui::DragValue::new(&mut number)
                            .range(min..=max)
                            .speed(field.step.unwrap_or(1.0)),
                    )
                };
                let changed = response.labelled_by(label).changed();
                changed.then(|| Value::from(number))
            }
            FieldType::Float => {
                let mut number = value.as_f64().unwrap_or(0.0);
                let min = field.min.unwrap_or(f64::MIN);
                let max = field.max.unwrap_or(f64::MAX);
                let response = if field.slider && field.min.is_some() && field.max.is_some() {
                    ui.add(egui::Slider::new(&mut number, min..=max))
                } else {
                    ui.add(
                        egui::DragValue::new(&mut number)
                            .range(min..=max)
                            .speed(field.step.unwrap_or(0.1)),
                    )
                };
                let changed = response.labelled_by(label).changed();
                changed.then(|| Value::from(number))
            }
            FieldType::Choice => {
                let current = value.as_str().unwrap_or_default().to_owned();
                let selected = field
                    .options
                    .iter()
                    .find(|option| option.value() == current)
                    .map_or(current.as_str(), ChoiceOption::label)
                    .to_owned();
                let mut picked = None;
                let combo = egui::ComboBox::from_id_salt(("egui_mev_data::choice", &id))
                    .selected_text(selected)
                    .width(200.0)
                    .show_ui(ui, |ui| {
                        for option in &field.options {
                            if ui
                                .selectable_label(option.value() == current, option.label())
                                .clicked()
                            {
                                picked = Some(Value::from(option.value()));
                            }
                        }
                    });
                let _ = combo.response.labelled_by(label);
                picked
            }
            FieldType::Radio => {
                let current = value.as_str().unwrap_or_default().to_owned();
                let mut picked = None;
                ui.horizontal_wrapped(|ui| {
                    for option in &field.options {
                        if ui
                            .radio(option.value() == current, option.label())
                            .clicked()
                        {
                            picked = Some(Value::from(option.value()));
                        }
                    }
                });
                picked
            }
            FieldType::Time => {
                let seconds = value.as_f64().unwrap_or(0.0);
                let text = self
                    .time_text
                    .entry(id.clone())
                    .or_insert_with(|| format_time(seconds));
                let edit = text_field(ui, text)
                    .desired_width(120.0)
                    .hint_text("hh:mm:ss.mmm");
                // As tall as the field from the start, or the grid would centre a
                // shorter row and the field would hang below its label.
                let row = egui::vec2(ui.available_width(), Metrics::of(ui).control_height);
                let response = ui
                    .allocate_ui_with_layout(
                        row,
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            let response = ui.add(edit).labelled_by(label);
                            if let Some(max) = field.max {
                                let color = Palette::of(ui).text_weak;
                                let text = format!("of {}", format_time(max));
                                ui.label(RichText::new(text).color(color));
                            }
                            response
                        },
                    )
                    .inner;
                let mut result = None;
                if response.changed() {
                    match parse_time(text) {
                        Some(parsed) => {
                            self.errors.remove(&id);
                            result = Some(Value::from(parsed));
                        }
                        None => {
                            self.errors.insert(id.clone(), BAD_TIME.into());
                            // The last good value must not be submitted in its
                            // place, with no sign that it was.
                            result = Some(Value::Null);
                        }
                    }
                }
                if response.lost_focus() && parse_time(text).is_some() {
                    *text = format_time(field.clamp(parse_time(text).unwrap_or(seconds)));
                }
                result
            }
            FieldType::Info | FieldType::Separator | FieldType::Group => None,
        };
        if let Some(value) = changed {
            if field.kind != FieldType::Time {
                self.errors.remove(&id);
            }
            self.values.insert(id, value);
            self.message = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn form(value: Value) -> Form {
        serde_json::from_value(value).expect("form")
    }

    fn convert_form() -> Form {
        form(json!({
            "remember": true,
            "fields": [
                { "type": "info", "text": "1920x1080" },
                { "id": "format", "type": "choice", "options": [["mp4", "MP4"], ["webm", "WebM"]], "default": "mp4" },
                { "id": "range", "type": "radio", "options": [["all", "Whole"], ["part", "Fragment"]], "default": "all" },
                { "id": "start", "type": "time", "default": 0, "max": 60, "visible_if": { "field": "range", "equals": "part" } },
                { "id": "crf", "type": "int", "min": 0, "max": 51, "default": 23 },
                { "id": "name", "type": "text", "required": true, "visible_if": { "field": "format", "not_in": ["webm"] } }
            ]
        }))
    }

    #[test]
    fn checks_schema() {
        convert_form().check().expect("valid");
        let missing_id = form(json!({ "fields": [{ "type": "text" }] }));
        assert!(missing_id.check().is_err());
        let bad_default = form(
            json!({ "fields": [{ "id": "a", "type": "choice", "options": ["x"], "default": "y" }] }),
        );
        assert!(bad_default.check().is_err());
        let unknown_dep = form(
            json!({ "fields": [{ "id": "a", "type": "bool", "visible_if": { "field": "b", "equals": true } }] }),
        );
        assert!(unknown_dep.check().is_err());
        assert!(
            serde_json::from_value::<Form>(json!({ "fields": [{ "type": "text", "colour": 1 }] }))
                .is_err()
        );
    }

    #[test]
    fn check_refuses_the_rest_of_a_bad_schema() {
        let twice = form(json!({ "fields": [
            { "id": "a", "type": "text" },
            { "type": "group", "label": "G", "fields": [{ "id": "a", "type": "text" }] },
        ] }));
        assert_eq!(
            twice.check().expect_err("duplicate"),
            "field id \"a\" is used twice"
        );
        let group = form(json!({ "fields": [{ "type": "group" }] }));
        assert_eq!(group.check().expect_err("label"), "a group needs a label");
        let children = form(json!({ "fields": [
            { "id": "a", "type": "text", "fields": [{ "id": "b", "type": "text" }] },
        ] }));
        assert!(children.check().is_err());
        let options = form(json!({ "fields": [{ "id": "a", "type": "radio" }] }));
        assert!(options.check().is_err());
        let range = form(json!({ "fields": [{ "id": "a", "type": "int", "min": 2, "max": 1 }] }));
        assert!(range.check().is_err());
        let many: Vec<Value> = (0..129)
            .map(|index| json!({ "id": format!("f{index}"), "type": "bool" }))
            .collect();
        assert!(form(json!({ "fields": many })).check().is_err());
    }

    #[test]
    fn initial_values_use_defaults_and_matching_memory() {
        let form = convert_form();
        let values = form.initial_values(None);
        assert_eq!(values["format"], json!("mp4"));
        assert_eq!(values["crf"], json!(23));
        assert_eq!(values["name"], json!(""));
        let mut remembered = Values::new();
        remembered.insert("format".into(), json!("webm"));
        remembered.insert("crf".into(), json!(99));
        remembered.insert("gone".into(), json!(1));
        let values = form.initial_values(Some(&remembered));
        assert_eq!(values["format"], json!("webm"));
        assert_eq!(values["crf"], json!(51), "clamped to max");
        assert!(!values.contains_key("gone"));
    }

    #[test]
    fn memory_is_ignored_unless_asked_for_and_for_a_disabled_field() {
        let mut remembered = Values::new();
        remembered.insert("a".into(), json!("old"));
        let forgetful =
            form(json!({ "fields": [{ "id": "a", "type": "text", "default": "new" }] }));
        assert_eq!(
            forgetful.initial_values(Some(&remembered))["a"],
            json!("new")
        );
        let disabled = form(json!({ "remember": true, "fields": [
            { "id": "a", "type": "text", "default": "new", "enabled": false },
        ] }));
        assert_eq!(
            disabled.initial_values(Some(&remembered))["a"],
            json!("new")
        );
    }

    #[test]
    fn visibility_and_required() {
        let form = convert_form();
        let mut values = form.initial_values(None);
        let start = &form.all_fields()[3];
        assert!(!start.visible(&values));
        values.insert("range".into(), json!("part"));
        assert!(start.visible(&values));
        let errors = form.validate(&values).expect_err("name required");
        assert_eq!(errors["name"], "Required");
        values.insert("format".into(), json!("webm"));
        let ok = form.validate(&values).expect("hidden field isn't required");
        assert_eq!(ok["range"], json!("part"));
    }

    #[test]
    fn enabling_follows_the_flag_and_the_condition() {
        let form = form(json!({ "fields": [
            { "id": "on", "type": "bool" },
            { "id": "level", "type": "int", "enabled_if": { "field": "on", "equals": true } },
            { "id": "never", "type": "text", "enabled": false, "enabled_if": { "field": "on", "equals": true } },
            { "id": "note", "type": "text", "required": true, "enabled_if": { "field": "on", "equals": true } },
        ] }));
        let fields = form.all_fields();
        let mut values = form.initial_values(None);
        assert!(!fields[1].enabled(&values));
        assert!(form.validate(&values).is_ok(), "disabled isn't required");
        values.insert("on".into(), json!(true));
        assert!(fields[1].enabled(&values));
        assert!(!fields[2].enabled(&values), "the flag wins");
        assert_eq!(
            form.validate(&values).expect_err("now required")["note"],
            "Required"
        );
    }

    #[test]
    fn conditions_compare_every_way() {
        let values: Values = serde_json::from_value(json!({ "n": 1, "s": "x" })).expect("values");
        let holds = |condition: Value| -> bool {
            serde_json::from_value::<Condition>(condition)
                .expect("condition")
                .holds(&values)
        };
        assert!(holds(json!({ "field": "n", "equals": 1.0 })), "1 is 1.0");
        assert!(!holds(json!({ "field": "n", "not_equals": 1 })));
        assert!(holds(json!({ "field": "s", "in": ["x", "y"] })));
        assert!(!holds(json!({ "field": "s", "in": ["y"] })));
        assert!(!holds(json!({ "field": "s", "not_in": ["x"] })));
        assert!(holds(json!({ "field": "missing", "not_equals": 1 })));
        assert!(holds(json!({ "field": "s" })), "no check always holds");
    }

    #[test]
    fn a_group_hides_and_disables_its_fields() {
        let form = form(json!({ "fields": [
            { "id": "advanced", "type": "bool" },
            { "type": "group", "label": "Advanced", "visible_if": { "field": "advanced", "equals": true }, "fields": [
                { "id": "key", "type": "text", "required": true },
            ] },
            { "type": "group", "label": "Locked", "enabled": false, "fields": [
                { "id": "locked", "type": "text", "required": true },
            ] },
        ] }));
        form.check().expect("valid");
        let mut values = form.initial_values(None);
        let ok = form.validate(&values).expect("nothing active is blank");
        assert_eq!(ok["key"], json!(""), "it keeps a value");
        values.insert("advanced".into(), json!(true));
        let errors = form.validate(&values).expect_err("the group shows now");
        assert_eq!(errors["key"], "Required");
        assert!(!errors.contains_key("locked"));
    }

    #[test]
    fn validation_converts_and_clamps() {
        let form = form(json!({ "fields": [
            { "id": "whole", "type": "int", "min": 0, "max": 10 },
            { "id": "time", "type": "time", "max": 60 },
            { "id": "text", "type": "text" },
            { "id": "pick", "type": "choice", "options": ["1", "2"] },
        ] }));
        let values: Values = serde_json::from_value(json!({
            "whole": 99.4, "time": 75, "text": 5, "pick": 2,
        }))
        .expect("values");
        let ok = form.validate(&values).expect("all convert");
        assert_eq!(ok["whole"], json!(10));
        assert_eq!(ok["time"], json!(60.0));
        assert_eq!(ok["text"], json!("5"));
        assert_eq!(ok["pick"], json!("2"));

        let values: Values = serde_json::from_value(json!({
            "whole": "many", "time": null, "text": true, "pick": "3",
        }))
        .expect("values");
        let errors = form.validate(&values).expect_err("none convert");
        assert_eq!(errors["whole"], "must be a whole number");
        assert_eq!(errors["time"], BAD_TIME);
        assert_eq!(errors["text"], "must be text");
        assert_eq!(errors["pick"], "\"3\" is not one of the options");
    }

    #[test]
    fn a_hidden_field_with_a_bad_value_falls_back_to_its_default() {
        let form = form(json!({ "fields": [
            { "id": "on", "type": "bool" },
            { "id": "start", "type": "time", "default": 5, "visible_if": { "field": "on", "equals": true } },
        ] }));
        let mut values = form.initial_values(None);
        values.insert("start".into(), Value::Null);
        let ok = form.validate(&values).expect("nobody can fix it");
        assert_eq!(ok["start"], json!(5.0));
        values.insert("on".into(), json!(true));
        assert_eq!(
            form.validate(&values).expect_err("it shows")["start"],
            BAD_TIME
        );
    }

    #[test]
    fn choice_options_read_all_three_ways() {
        let form = form(json!({ "fields": [
            { "id": "a", "type": "choice", "options": [
                ["v1", "One"],
                { "value": "v2", "label": "Two" },
                "v3",
            ] },
        ] }));
        let options = &form.fields[0].options;
        let read: Vec<(&str, &str)> = options
            .iter()
            .map(|option| (option.value(), option.label()))
            .collect();
        assert_eq!(read, [("v1", "One"), ("v2", "Two"), ("v3", "v3")]);
    }

    #[test]
    fn times_round_trip() {
        assert_eq!(parse_time("90"), Some(90.0));
        assert_eq!(parse_time("1:30"), Some(90.0));
        assert_eq!(parse_time("1:00:00.5"), Some(3600.5));
        assert_eq!(parse_time("1:60"), None);
        assert_eq!(parse_time("x"), None);
        assert_eq!(format_time(3600.5), "1:00:00.500");
        assert_eq!(format_time(75.0), "01:15");
    }

    #[test]
    fn formatted_times_parse_back_to_the_same_seconds() {
        for seconds in [
            0.0,
            0.001,
            0.5,
            1.0,
            59.999,
            60.0,
            75.25,
            3599.999,
            3600.0,
            3600.5,
            86_400.0,
            360_000.125,
        ] {
            let text = format_time(seconds);
            let parsed = parse_time(&text).expect(&text);
            assert!((parsed - seconds).abs() < 1e-9, "{seconds} went to {text}");
        }
    }

    #[test]
    fn time_edges() {
        assert_eq!(format_time(-5.0), "00:00", "never negative");
        assert_eq!(format_time(59.9996), "01:00", "rounds up across a minute");
        for bad in [
            "", " ", ":", "1:", ":1", "1:2:3:4", "-1", "1:-1", "inf", "NaN", "1:60:00",
        ] {
            assert_eq!(parse_time(bad), None, "{bad:?}");
        }
        assert_eq!(parse_time(" 90:00 "), Some(5400.0), "minutes may be many");
    }
}
