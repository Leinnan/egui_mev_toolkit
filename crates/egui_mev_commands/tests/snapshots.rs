//! Snapshots of the palette, the help view and the menu bar in both Graphite looks. Rewrite them
//! with `UPDATE_SNAPSHOTS=1`, and only after looking at the new images.

use egui::{Key, Modifiers, Theme as Mode};
use egui_kittest::{Harness, SnapshotResults, kittest::Queryable as _};
use egui_mev_commands::{
    Availability, Binding, CommandPalette, HelpSection, Keymap, Menu, MenuEntry, MenuItem,
    PaletteItem, Sections, help_ui, menu_bar,
};
use egui_mev_test::{install, settle, snapshot_lock, snapshot_options};
use egui_mev_theme::{Theme, presets};

const LOOKS: [(&str, Mode); 2] = [
    ("graphite_dark", Mode::Dark),
    ("graphite_light", Mode::Light),
];

fn theme() -> Theme {
    Theme::new(presets::GRAPHITE_DARK, presets::GRAPHITE_LIGHT)
}

fn palette() -> CommandPalette<u32> {
    let keys = |text: &str| Some(text.to_owned());
    CommandPalette::new(vec![
        PaletteItem::new("Open in new tab", 1).shortcut(keys("Ctrl+T")),
        PaletteItem::new("Open in new window", 2).shortcut(keys("Ctrl+N")),
        PaletteItem::new("Add to favorites", 3),
        PaletteItem::new("Save search", 4).shortcut(keys("Ctrl+S")),
        PaletteItem::new("Toggle sidebar", 5),
    ])
}

fn palette_harness(mode: Mode) -> Harness<'static, CommandPalette<u32>> {
    Harness::builder()
        .with_size([520.0, 340.0])
        .with_theme(mode)
        .with_context_setup(|ctx| install(ctx, theme()))
        .build_ui_state(
            |ui, palette: &mut CommandPalette<u32>| {
                palette.show(ui.ctx());
            },
            palette(),
        )
}

#[test]
fn palette_snapshots() {
    let _guard = snapshot_lock();
    let options = snapshot_options();
    let mut results = SnapshotResults::new();
    for (name, mode) in LOOKS {
        let mut harness = palette_harness(mode);
        settle(&mut harness);
        harness.key_press(Key::ArrowDown);
        settle(&mut harness);
        results.add(harness.try_snapshot_options(format!("palette_{name}"), &options));

        // A search, with a selection on the second best match.
        let mut harness = palette_harness(mode);
        settle(&mut harness);
        harness.event(egui::Event::Text("opn".to_owned()));
        settle(&mut harness);
        harness.key_press(Key::ArrowDown);
        settle(&mut harness);
        results.add(harness.try_snapshot_options(format!("palette_search_{name}"), &options));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Command {
    Back,
    Refresh,
    Search,
    Palette,
    Settings,
}

fn keymap() -> Keymap<Command> {
    const CMD: Modifiers = Modifiers::COMMAND;
    Keymap::new([
        Binding::new(Modifiers::ALT, Key::ArrowLeft, Command::Back),
        Binding::new(Modifiers::NONE, Key::Backspace, Command::Back),
        Binding::new(Modifiers::NONE, Key::F5, Command::Refresh),
        Binding::new(CMD, Key::R, Command::Refresh),
        Binding::new(CMD, Key::F, Command::Search),
        Binding::new(CMD.plus(Modifiers::SHIFT), Key::P, Command::Palette),
        Binding::new(CMD, Key::Comma, Command::Settings),
    ])
}

fn label(command: &Command) -> Option<std::borrow::Cow<'static, str>> {
    Some(
        match command {
            Command::Back => "Back",
            Command::Refresh => "Refresh",
            Command::Search => "Search",
            Command::Palette => "Commands",
            Command::Settings => "Settings",
        }
        .into(),
    )
}

#[test]
fn help_snapshots() {
    let _guard = snapshot_lock();
    let options = snapshot_options();
    let mut results = SnapshotResults::new();
    for (name, mode) in LOOKS {
        let keymap = keymap();
        let mut harness = Harness::builder()
            .with_size([640.0, 640.0])
            .with_theme(mode)
            .with_context_setup(|ctx| install(ctx, theme()))
            .build_ui(move |ui| {
                let sections = [
                    HelpSection::from_keymap(ui.ctx(), "Application", &keymap, label),
                    HelpSection::from_keys(
                        "Files",
                        &[
                            ("Shift + movement", "Extend the selection"),
                            ("Ctrl+Enter, middle-click", "Open folders in a new tab"),
                            ("Esc", "Clear the selection"),
                        ],
                    ),
                ];
                help_ui(ui.ctx(), "Keyboard shortcuts", "Close", &sections);
            });
        settle(&mut harness);
        results.add(harness.try_snapshot_options(format!("help_{name}"), &options));
    }
}

struct Checked;

impl Availability<Command> for Checked {
    fn enabled(&self, command: &Command) -> bool {
        *command != Command::Settings
    }

    fn checked(&self, command: &Command) -> bool {
        *command == Command::Refresh
    }
}

#[test]
fn menu_bar_snapshots() {
    let _guard = snapshot_lock();
    let options = snapshot_options();
    let mut results = SnapshotResults::new();
    for (name, mode) in LOOKS {
        let menus = vec![
            Menu::new(
                "View",
                vec![
                    MenuItem::new("Back", Command::Back)
                        .shortcut(egui::KeyboardShortcut::new(Modifiers::ALT, Key::ArrowLeft))
                        .into(),
                    MenuItem::new("Auto Refresh", Command::Refresh)
                        .checkable()
                        .into(),
                    MenuItem::new("Search", Command::Search).checkable().into(),
                    MenuEntry::Separator,
                    MenuItem::new("Settings", Command::Settings).into(),
                    MenuEntry::disabled("Print"),
                    MenuEntry::submenu("Recent", vec![MenuEntry::Section("recent")]),
                ],
            ),
            Menu::new(
                "Help",
                vec![MenuItem::new("Commands", Command::Palette).into()],
            ),
        ];
        let mut harness = Harness::builder()
            .with_size([360.0, 260.0])
            .with_theme(mode)
            .with_context_setup(|ctx| install(ctx, theme()))
            .build_ui(move |ui| menu_bar(ui, &menus, &Checked, &Sections::new()));
        settle(&mut harness);
        harness.get_by_label("View").click();
        settle(&mut harness);
        results.add(harness.try_snapshot_options(format!("menu_bar_{name}"), &options));
    }
}
