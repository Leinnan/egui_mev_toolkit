//! A small app driven by one menu table: the native menu bar on macOS, and the
//! egui one with a note elsewhere.
//!
//! ```bash
//! cargo run -p egui_mev_commands --example native_menu --features native_menu
//! ```
//!
//! To try on macOS: the commands in the menus and through their keys, F5 (which
//! has no modifier, so it stays with egui), and ⌘A, ⌘X, ⌘C, ⌘V, ⌘Z and ⇧⌘Z inside the
//! text field, which must edit the text and not show up as the last command. With
//! the field unfocused, the same keys must show up as commands.

use std::borrow::Cow;

use egui::{Key, KeyboardShortcut, Modifiers};
use egui_mev_commands::{
    Availability, CommandQueue, Keymap, Menu, MenuEntry, MenuItem, TextEditAction,
};
use egui_mev_theme::{Theme, presets};

/// The slot Open Recent fills at run time.
const RECENT: &str = "recent";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Command {
    Count,
    CountDown,
    Toggle,
    AddRecent,
    OpenRecent(usize),
    ClearRecent,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
    About,
    Settings,
    Quit,
}

const fn key(modifiers: Modifiers, key: Key) -> KeyboardShortcut {
    KeyboardShortcut::new(modifiers, key)
}

fn menus() -> Vec<Menu<Command>> {
    const CMD: Modifiers = Modifiers::COMMAND;
    const CMD_SHIFT: Modifiers = Modifiers::COMMAND.plus(Modifiers::SHIFT);
    let edit = |label: &'static str, command, action, shortcut| {
        MenuItem::new(label, command)
            .shortcut(shortcut)
            .text_edit(action)
            .into()
    };
    vec![
        Menu::new(
            "File",
            vec![
                MenuItem::new("Add Recent File", Command::AddRecent)
                    .shortcut(key(CMD, Key::N))
                    .into(),
                MenuEntry::submenu("Open Recent", vec![MenuEntry::Section(RECENT)]),
                MenuEntry::Separator,
                // On macOS these move to the application menu.
                MenuItem::new("About", Command::About).into(),
                MenuItem::new("Settings…", Command::Settings).into(),
                MenuItem::new("Quit", Command::Quit)
                    .shortcut(key(CMD, Key::Q))
                    .into(),
            ],
        ),
        Menu::new(
            "Edit",
            vec![
                edit(
                    "Undo",
                    Command::Undo,
                    TextEditAction::Undo,
                    key(CMD, Key::Z),
                ),
                edit(
                    "Redo",
                    Command::Redo,
                    TextEditAction::Redo,
                    key(CMD_SHIFT, Key::Z),
                ),
                MenuEntry::Separator,
                edit("Cut", Command::Cut, TextEditAction::Cut, key(CMD, Key::X)),
                edit(
                    "Copy",
                    Command::Copy,
                    TextEditAction::Copy,
                    key(CMD, Key::C),
                ),
                edit(
                    "Paste",
                    Command::Paste,
                    TextEditAction::Paste,
                    key(CMD, Key::V),
                ),
                edit(
                    "Select All",
                    Command::SelectAll,
                    TextEditAction::SelectAll,
                    key(CMD, Key::A),
                ),
            ],
        ),
        Menu::new(
            "View",
            vec![
                MenuItem::new("Count", Command::Count)
                    .shortcut(key(CMD, Key::K))
                    .into(),
                // No modifier, so it isn't a key equivalent: a keymap runs it.
                MenuItem::new("Count Down", Command::CountDown)
                    .shortcut(key(Modifiers::NONE, Key::F5))
                    .into(),
                MenuItem::new("Toggle", Command::Toggle)
                    .shortcut(key(CMD, Key::T))
                    .checkable()
                    .into(),
            ],
        ),
    ]
}

#[derive(Default)]
struct State {
    text: String,
    count: u32,
    toggled: bool,
    recent: Vec<String>,
    last_command: Option<Command>,
    close: bool,
}

impl Availability<Command> for State {
    fn enabled(&self, command: &Command) -> bool {
        *command != Command::CountDown || self.count > 0
    }

    fn label(&self, command: &Command) -> Option<Cow<'_, str>> {
        (*command == Command::Count).then(|| format!("Count ({})", self.count).into())
    }

    fn checked(&self, command: &Command) -> bool {
        *command == Command::Toggle && self.toggled
    }
}

impl State {
    fn recent_entries(&self) -> Vec<MenuEntry<Command>> {
        if self.recent.is_empty() {
            return vec![MenuEntry::disabled("No Recent Files")];
        }
        let mut entries: Vec<MenuEntry<Command>> = self
            .recent
            .iter()
            .enumerate()
            .map(|(index, name)| MenuItem::new(name.clone(), Command::OpenRecent(index)).into())
            .collect();
        entries.push(MenuEntry::Separator);
        entries.push(MenuItem::new("Clear Menu", Command::ClearRecent).into());
        entries
    }

    fn run(&mut self, command: Command) {
        self.last_command = Some(command);
        match command {
            Command::Count => self.count += 1,
            Command::CountDown => self.count = self.count.saturating_sub(1),
            Command::Toggle => self.toggled = !self.toggled,
            Command::AddRecent => self
                .recent
                .push(format!("R&D notes {}.txt", self.recent.len() + 1)),
            Command::ClearRecent => self.recent.clear(),
            Command::Quit => self.close = true,
            Command::OpenRecent(_)
            | Command::Undo
            | Command::Redo
            | Command::Cut
            | Command::Copy
            | Command::Paste
            | Command::SelectAll
            | Command::About
            | Command::Settings => {}
        }
    }
}

struct App {
    state: State,
    keymap: Keymap<Command>,
    #[cfg(target_os = "macos")]
    bar: egui_mev_commands::NativeMenuBar<Command>,
    #[cfg(not(target_os = "macos"))]
    menus: Vec<Menu<Command>>,
    #[cfg(not(target_os = "macos"))]
    sections: egui_mev_commands::Sections<Command>,
}

impl App {
    fn new(
        cc: &eframe::CreationContext<'_>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Theme::new(presets::GRAPHITE_DARK, presets::GRAPHITE_LIGHT).install(&cc.egui_ctx);
        let menus = menus();
        let state = State::default();
        #[cfg(target_os = "macos")]
        {
            use egui_mev_commands::{NativeMenuBar, NativeMenuOptions};
            let options = NativeMenuOptions::new("Native Menu Example", Command::Quit)
                .about(Command::About)
                .settings(Command::Settings);
            let mut bar = NativeMenuBar::install(&options, &menus)?;
            bar.install_event_handler(&cc.egui_ctx);
            bar.set_section(RECENT, &state.recent_entries())?;
            let mut keymap = Keymap::from_menus(&menus);
            keymap.retain(|binding| !bar.handles(&binding.shortcut));
            Ok(Self { state, keymap, bar })
        }
        #[cfg(not(target_os = "macos"))]
        {
            let mut sections = egui_mev_commands::Sections::new();
            sections.set(RECENT, state.recent_entries());
            Ok(Self {
                keymap: Keymap::from_menus(&menus),
                state,
                menus,
                sections,
            })
        }
    }

    fn run_commands(&mut self, ctx: &egui::Context) {
        let recent_before = self.state.recent.len();
        let mut commands = CommandQueue::<Command>::drain(ctx);
        commands.extend(self.keymap.pressed(ctx));
        for command in commands {
            self.state.run(command);
        }
        if self.state.close {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        let entries = self.state.recent_entries();
        let recent_changed = self.state.recent.len() != recent_before;
        #[cfg(target_os = "macos")]
        {
            if recent_changed && let Err(error) = self.bar.set_section(RECENT, &entries) {
                eprintln!("Could not update Open Recent: {error}");
            }
            self.bar.set_availability(&self.state);
        }
        #[cfg(not(target_os = "macos"))]
        if recent_changed {
            self.sections.set(RECENT, entries);
        }
    }
}

impl eframe::App for App {
    #[cfg(target_os = "macos")]
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        raw_input.events.extend(self.bar.drain_text_events());
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.run_commands(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        #[cfg(not(target_os = "macos"))]
        egui::Panel::top("menu_bar").show(ui, |ui| {
            egui_mev_commands::menu_bar(ui, &self.menus, &self.state, &self.sections);
        });
        egui::CentralPanel::default().show(ui, |ui| {
            if cfg!(target_os = "macos") {
                ui.label("The application menu and the rest are in the menu bar at the top of the screen.");
            } else {
                ui.label("The native menu is macOS only. This is the same table, drawn by egui.");
            }
            ui.add_space(12.0);
            let label = ui.label("Text field");
            ui.text_edit_singleline(&mut self.state.text).labelled_by(label.id);
            ui.add_space(12.0);
            ui.label(format!("Count: {}", self.state.count));
            ui.label(format!("Toggle: {}", if self.state.toggled { "on" } else { "off" }));
            ui.label(format!("Recent files: {}", self.state.recent.len()));
            ui.add_space(12.0);
            match self.state.last_command {
                Some(command) => ui.label(format!("Last command received: {command:?}")),
                None => ui.label("Last command received: none yet"),
            };
        });
    }
}

fn main() -> eframe::Result {
    eframe::run_native(
        "Native Menu Example",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([480.0, 320.0]),
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(App::new(cc)?))),
    )
}
