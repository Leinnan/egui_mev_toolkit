//! The config editor: files on disk, undo and redo. Runs in whichever format the
//! features pick, so run it with and without `configs_json`.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU32, Ordering},
    time::Duration,
};

use bevy_reflect::Reflect;
use egui::{Context, RawInput, Role};
use egui_kittest::{Harness, kittest::Queryable as _};
use egui_mev_inspect::{
    config::{ConfigBuilder, ConfigEditorApp, serialization::ConfigsSerializerExt as _},
    create_registry,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, PartialEq, Reflect, Serialize, Deserialize)]
struct Settings {
    name: String,
    age: u32,
}

#[derive(Debug, Default, Clone, PartialEq, Reflect, Serialize, Deserialize)]
enum Mode {
    #[default]
    Fast,
    Slow {
        delay: u32,
    },
}

/// Every kind of field a config file holds, nested tables after plain values.
#[derive(Debug, Default, Clone, PartialEq, Reflect, Serialize, Deserialize)]
struct Everything {
    title: String,
    ratio: f64,
    enabled: bool,
    timeout: Duration,
    nickname: Option<String>,
    missing: Option<String>,
    tags: Vec<String>,
    scores: HashMap<String, u32>,
    plain_mode: Mode,
    slow_mode: Mode,
    nested: Settings,
    people: Vec<Settings>,
}

fn test_dir() -> PathBuf {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "egui-mev-inspect-tests-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&dir).expect("should create directory");
    dir
}

fn file_names(dir: &Path) -> Vec<String> {
    let mut names = fs::read_dir(dir)
        .expect("should read directory")
        .map(|entry| {
            let name = entry.expect("should read entry").file_name();
            name.to_string_lossy().into_owned()
        })
        .collect::<Vec<_>>();
    names.sort();
    names
}

fn edited(path: PathBuf) -> ConfigEditorApp<Settings> {
    let mut app = ConfigEditorApp::<Settings>::at_path(path);
    app.config_mut().name = "Ada".to_owned();
    app.config_mut().age = 36;
    app
}

/// Runs one frame at `time` feeding the undo history.
fn feed_undo(app: &mut ConfigEditorApp<Settings>, ctx: &Context, time: f64) {
    let input = RawInput {
        time: Some(time),
        ..Default::default()
    };
    let mut output = ctx.run_ui(input, |ui| app.update_undo(ui.ctx()));
    output.textures_delta.clear();
}

#[test]
fn missing_file_gives_the_default_without_creating_it() {
    let dir = test_dir();
    let path = dir.join("settings");

    let app = ConfigEditorApp::<Settings>::at_path(path.clone());

    assert_eq!(app.config(), &Settings::default());
    assert!(!app.is_modified());
    assert!(!path.exists());
}

#[test]
fn saved_config_loads_back() {
    let dir = test_dir();
    let path = dir.join("settings");

    let mut app = edited(path.clone());
    assert!(app.is_modified());
    app.save_to_file().expect("should save");
    assert!(!app.is_modified());

    let loaded = ConfigEditorApp::<Settings>::at_path(path);
    assert_eq!(loaded.config(), app.config());
    assert!(!loaded.is_modified());
}

#[test]
fn every_kind_of_field_survives_a_round_trip() {
    let dir = test_dir();
    let path = dir.join("everything");
    let value = Everything {
        title: "A \"quoted\" title".to_owned(),
        ratio: 0.5,
        enabled: true,
        timeout: Duration::from_millis(1500),
        nickname: Some("nick".to_owned()),
        missing: None,
        tags: vec!["one".to_owned(), "two".to_owned()],
        scores: HashMap::from([("alice".to_owned(), 1), ("bob".to_owned(), 2)]),
        plain_mode: Mode::Fast,
        slow_mode: Mode::Slow { delay: 9 },
        nested: Settings {
            name: "inner".to_owned(),
            age: 3,
        },
        people: vec![Settings {
            name: "first".to_owned(),
            age: 1,
        }],
    };

    let registry = create_registry();
    registry
        .try_save_config(&value, &path)
        .expect("should save");

    assert_eq!(registry.try_load_config::<Everything>(&path), Some(value));
}

#[cfg(not(feature = "configs_json"))]
#[test]
fn config_file_is_toml_without_configs_json() {
    let dir = test_dir();
    let path = dir.join("settings.toml");

    edited(path.clone()).save_to_file().expect("should save");

    let text = fs::read_to_string(&path).expect("should read");
    assert!(text.contains("name = \"Ada\""), "{text}");
}

#[cfg(feature = "configs_json")]
#[test]
fn config_file_is_json_with_configs_json() {
    let dir = test_dir();
    let path = dir.join("settings.json");

    edited(path.clone()).save_to_file().expect("should save");

    let text = fs::read_to_string(&path).expect("should read");
    let value: serde_json::Value = serde_json::from_str(&text).expect("should be JSON");
    assert_eq!(value["name"], "Ada");
}

#[test]
fn save_leaves_only_the_config_behind() {
    let dir = test_dir();
    let path = dir.join("settings");

    let mut app = edited(path);
    app.save_to_file().expect("should create");
    app.config_mut().age = 37;
    app.save_to_file().expect("should replace");

    assert_eq!(file_names(&dir), ["settings"]);
}

#[test]
fn save_creates_missing_parent_directories() {
    let dir = test_dir();
    let path = dir.join("not/there/yet/settings");

    edited(path.clone()).save_to_file().expect("should save");

    assert!(path.is_file());
}

#[test]
fn save_goes_through_a_temporary_file_and_a_failure_keeps_the_original() {
    let dir = test_dir();
    let path = dir.join("settings");
    let mut app = edited(path.clone());
    app.save_to_file().expect("should save");
    let original = fs::read(&path).expect("should read");

    // A directory at `write_atomic`'s temporary name fails the save. A plain
    // `fs::write` to the path would not notice it, and would have replaced the file.
    let name = format!(".settings.{}.tmp", std::process::id());
    let blocker = fs::canonicalize(&dir).expect("should resolve").join(name);
    fs::create_dir(&blocker).expect("should block");
    app.config_mut().age = 99;

    assert!(app.save_to_file().is_err());

    assert_eq!(fs::read(&path).expect("should read"), original);
    assert!(app.is_modified(), "a failed save must stay unsaved");
}

#[test]
fn corrupt_file_gives_the_default_and_survives_the_next_save() {
    let dir = test_dir();
    let path = dir.join("settings");
    fs::write(&path, b"\xff not a config {{{").expect("should seed");

    let mut app = ConfigEditorApp::<Settings>::at_path(path.clone());

    assert_eq!(app.config(), &Settings::default());
    assert!(!app.is_modified());
    let moved = app
        .set_aside()
        .expect("should say where it went")
        .to_owned();

    app.config_mut().age = 1;
    app.save_to_file().expect("should write a new file");

    assert_eq!(
        fs::read(&moved).expect("should keep the old contents"),
        b"\xff not a config {{{"
    );
    let loaded = ConfigEditorApp::<Settings>::at_path(path);
    assert_eq!(loaded.config().age, 1);
    assert_eq!(loaded.set_aside(), None);
}

#[test]
fn file_of_the_wrong_shape_gives_the_default() {
    let dir = test_dir();
    let path = dir.join("settings");
    fs::write(&path, "valid text = but not a config").expect("should seed");

    let app = ConfigEditorApp::<Settings>::at_path(path);

    assert_eq!(app.config(), &Settings::default());
}

#[test]
fn undo_and_redo_restore_settled_states() {
    let dir = test_dir();
    let mut app = ConfigEditorApp::<Settings>::at_path(dir.join("settings"));
    let ctx = Context::default();

    feed_undo(&mut app, &ctx, 0.0);
    assert!(!app.has_undo());
    assert!(!app.undo());

    app.config_mut().age = 1;
    feed_undo(&mut app, &ctx, 10.0);
    // Stable for longer than the undoer's settling time, so it becomes an undo point.
    feed_undo(&mut app, &ctx, 20.0);
    assert!(app.has_undo());
    assert!(!app.has_redo());

    assert!(app.undo());
    assert_eq!(app.config().age, 0);
    assert!(app.has_redo());

    assert!(app.redo());
    assert_eq!(app.config().age, 1);
    assert!(!app.redo());
}

#[test]
fn a_new_edit_after_undo_drops_the_redo_state() {
    let dir = test_dir();
    let mut app = ConfigEditorApp::<Settings>::at_path(dir.join("settings"));
    let ctx = Context::default();
    feed_undo(&mut app, &ctx, 0.0);
    app.config_mut().age = 1;
    feed_undo(&mut app, &ctx, 10.0);
    feed_undo(&mut app, &ctx, 20.0);
    assert!(app.undo());

    app.config_mut().age = 2;
    feed_undo(&mut app, &ctx, 30.0);

    assert!(!app.has_redo());
    assert!(!app.redo());
    assert_eq!(app.config().age, 2);
}

fn editor_harness(app: ConfigEditorApp<Settings>) -> Harness<'static, ConfigEditorApp<Settings>> {
    let mut harness = Harness::builder()
        .with_size([500.0, 400.0])
        .build_ui_state(|ui, app: &mut ConfigEditorApp<Settings>| app.show(ui), app);
    harness.run_steps(2);
    harness
}

#[test]
fn toolbar_buttons_save_undo_and_redo() {
    let dir = test_dir();
    let path = dir.join("settings");
    let mut harness = editor_harness(ConfigEditorApp::at_path(path.clone()));

    // Each frame is a quarter of a second, so this outlasts the settling time.
    harness.state_mut().config_mut().age = 5;
    harness.run_steps(12);

    harness.get_by_label("Undo").click();
    harness.run_steps(2);
    assert_eq!(harness.state().config().age, 0);

    harness.get_by_label("Redo").click();
    harness.run_steps(2);
    assert_eq!(harness.state().config().age, 5);

    assert!(harness.state().is_modified());
    harness.get_by_label("Save").click();
    harness.run_steps(2);
    assert!(!harness.state().is_modified());
    let loaded = ConfigEditorApp::<Settings>::at_path(path);
    assert_eq!(loaded.config().age, 5);
}

#[test]
fn save_button_does_nothing_until_the_config_changes() {
    let dir = test_dir();
    let path = dir.join("settings");
    let mut harness = editor_harness(ConfigEditorApp::at_path(path.clone()));

    harness.get_by_label("Save").click();
    harness.run_steps(2);

    assert!(!path.exists());
}

#[test]
fn editor_fields_edit_the_config() {
    let dir = test_dir();
    let mut harness = editor_harness(ConfigEditorApp::at_path(dir.join("settings")));

    harness.get_by_role(Role::TextInput).click();
    harness.run_steps(2);
    harness.get_by_role(Role::TextInput).type_text("Ada");
    harness.run_steps(2);

    assert_eq!(harness.state().config().name, "Ada");
    assert!(harness.state().is_modified());
}

#[test]
fn builder_keeps_its_window_name_and_registry() {
    let dir = test_dir();

    let named = ConfigBuilder::new()
        .with_window_name("Preferences")
        .with_type::<Settings>()
        .build::<Settings>(dir.join("settings"));
    assert_eq!(named.get_window_name(), "Preferences");

    let default = ConfigBuilder::new().build::<Settings>(dir.join("settings"));
    assert_eq!(default.get_window_name(), "Settings");

    let plain = ConfigEditorApp::<Settings>::at_path(dir.join("settings"));
    assert_eq!(plain.get_window_name(), "Configuration editor");
}
