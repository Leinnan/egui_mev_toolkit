//! One description of an app's menus, which the egui menu bar and the native
//! macOS one both draw.

use std::{borrow::Cow, collections::HashMap};

use egui::{Event, Key, KeyboardShortcut, Modifiers};

use crate::keymap::{Binding, Keymap};

/// A top-level menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Menu<C> {
    /// Visible title.
    pub label: Cow<'static, str>,
    /// Entries in order.
    pub entries: Vec<MenuEntry<C>>,
}

impl<C> Menu<C> {
    /// A menu titled `label`.
    #[must_use]
    pub fn new(label: impl Into<Cow<'static, str>>, entries: Vec<MenuEntry<C>>) -> Self {
        Self {
            label: label.into(),
            entries,
        }
    }
}

/// One entry in a menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuEntry<C> {
    /// A clickable command.
    Item(MenuItem<C>),
    /// An entry shown greyed out.
    Disabled {
        /// Visible label.
        label: Cow<'static, str>,
    },
    /// A separator line.
    Separator,
    /// A nested submenu.
    Submenu {
        /// Visible label.
        label: Cow<'static, str>,
        /// Entries of the submenu.
        entries: Vec<MenuEntry<C>>,
    },
    /// Where the entries an app sets at run time go, such as recent files. The
    /// name is the slot's: [`Sections::set`] and `NativeMenuBar::set_section` fill
    /// it. Slots only count in a menu table, not inside what fills one.
    Section(&'static str),
}

impl<C> MenuEntry<C> {
    /// A greyed out entry.
    #[must_use]
    pub fn disabled(label: impl Into<Cow<'static, str>>) -> Self {
        Self::Disabled {
            label: label.into(),
        }
    }

    /// A nested submenu.
    #[must_use]
    pub fn submenu(label: impl Into<Cow<'static, str>>, entries: Vec<Self>) -> Self {
        Self::Submenu {
            label: label.into(),
            entries,
        }
    }
}

impl<C> From<MenuItem<C>> for MenuEntry<C> {
    fn from(item: MenuItem<C>) -> Self {
        Self::Item(item)
    }
}

/// A menu item that runs a command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuItem<C> {
    /// Visible label.
    pub label: Cow<'static, str>,
    /// Command queued on click.
    pub command: C,
    /// Optional keyboard shortcut.
    pub shortcut: Option<KeyboardShortcut>,
    /// What the item does to a focused text field instead of running `command`.
    pub text_edit: Option<TextEditAction>,
    /// Whether the native menu may turn `shortcut` into a key equivalent, which
    /// takes the key away from egui. When false, the shortcut is only shown by the
    /// egui menu bar and handled through a [`Keymap`].
    pub accelerator: bool,
    /// Whether it is an on/off item, which shows [`Availability::checked`].
    pub checkable: bool,
}

impl<C> MenuItem<C> {
    /// An item labelled `label` that runs `command`.
    #[must_use]
    pub fn new(label: impl Into<Cow<'static, str>>, command: C) -> Self {
        Self {
            label: label.into(),
            command,
            shortcut: None,
            text_edit: None,
            accelerator: true,
            checkable: false,
        }
    }

    /// With a keyboard shortcut.
    #[must_use]
    pub const fn shortcut(mut self, shortcut: KeyboardShortcut) -> Self {
        self.shortcut = Some(shortcut);
        self
    }

    /// While a text field has focus, the item acts on the field.
    #[must_use]
    pub const fn text_edit(mut self, action: TextEditAction) -> Self {
        self.text_edit = Some(action);
        self
    }

    /// Never a native key equivalent: its keys keep going through egui, so a text
    /// field gets them without the text-field bridge.
    #[must_use]
    pub const fn no_accelerator(mut self) -> Self {
        self.accelerator = false;
        self
    }

    /// An on/off item.
    #[must_use]
    pub const fn checkable(mut self) -> Self {
        self.checkable = true;
        self
    }
}

/// What an Edit menu command does when a text field has focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextEditAction {
    /// Cut the selected text.
    Cut,
    /// Copy the selected text.
    Copy,
    /// Paste the clipboard's text.
    Paste,
    /// Select all the text.
    SelectAll,
    /// Undo the last edit.
    Undo,
    /// Redo the last undone edit.
    Redo,
}

impl TextEditAction {
    /// The egui events a text field expects for this action. `clipboard` reads the
    /// text for [`Self::Paste`], and nothing else asks for it.
    #[must_use]
    pub fn events(self, clipboard: impl FnOnce() -> Option<String>) -> Vec<Event> {
        let key = |key, modifiers| {
            [true, false]
                .map(|pressed| Event::Key {
                    key,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers,
                })
                .to_vec()
        };
        match self {
            Self::Cut => vec![Event::Cut],
            Self::Copy => vec![Event::Copy],
            // As egui-winit does for the real keys.
            Self::Paste => clipboard()
                .map(|text| text.replace("\r\n", "\n"))
                .filter(|text| !text.is_empty())
                .map(Event::Paste)
                .into_iter()
                .collect(),
            Self::SelectAll => key(Key::A, Modifiers::COMMAND),
            Self::Undo => key(Key::Z, Modifiers::COMMAND),
            Self::Redo => key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT),
        }
    }
}

/// What the menus can offer right now. Both menu bars ask this, so commands that
/// wouldn't do anything are greyed out in either. `()` makes everything available.
pub trait Availability<C> {
    /// Whether `command` would do something now.
    fn enabled(&self, _command: &C) -> bool {
        true
    }

    /// The label `command`'s items show now instead of their own, such as "Undo
    /// Rename".
    fn label(&self, _command: &C) -> Option<Cow<'_, str>> {
        None
    }

    /// Whether `command`'s [checkable](MenuItem::checkable) items are checked.
    fn checked(&self, _command: &C) -> bool {
        false
    }
}

impl<C> Availability<C> for () {}

/// What fills the egui menu bar's [`MenuEntry::Section`] slots.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sections<C> {
    slots: HashMap<&'static str, Vec<MenuEntry<C>>>,
}

impl<C> Default for Sections<C> {
    fn default() -> Self {
        Self {
            slots: HashMap::new(),
        }
    }
}

impl<C> Sections<C> {
    /// Every slot empty.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Show `entries` in `slot`.
    pub fn set(&mut self, slot: &'static str, entries: Vec<MenuEntry<C>>) {
        self.slots.insert(slot, entries);
    }

    /// What `slot` shows.
    #[must_use]
    pub fn get(&self, slot: &str) -> &[MenuEntry<C>] {
        self.slots.get(slot).map_or(&[], Vec::as_slice)
    }
}

impl<C: Clone> Keymap<C> {
    /// The shortcuts of the items in `menus`, for dispatching key presses when no
    /// native menu bar handles them. While a text field has focus they keep
    /// firing, unless they would interfere with editing or the item is a
    /// [text-edit](MenuItem::text_edit) one.
    #[must_use]
    pub fn from_menus(menus: &[Menu<C>]) -> Self {
        fn collect<C: Clone>(entries: &[MenuEntry<C>], out: &mut Vec<Binding<C>>) {
            for entry in entries {
                match entry {
                    MenuEntry::Item(item) => {
                        if let Some(shortcut) = item.shortcut {
                            out.push(Binding {
                                shortcut,
                                command: item.command.clone(),
                                in_text: item.text_edit.is_none()
                                    && !shortcut.conflicts_with_text_editing(),
                            });
                        }
                    }
                    MenuEntry::Submenu { entries, .. } => collect(entries, out),
                    MenuEntry::Disabled { .. } | MenuEntry::Separator | MenuEntry::Section(_) => {}
                }
            }
        }
        let mut bindings = Vec::new();
        for menu in menus {
            collect(&menu.entries, &mut bindings);
        }
        Self::new(bindings)
    }
}

#[cfg(test)]
mod tests {
    use egui_kittest::{Harness, kittest::Queryable as _};

    use super::*;

    const CMD: Modifiers = Modifiers::COMMAND;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Command {
        Save,
        SaveAs,
        Copy,
        Delete,
    }

    fn menus() -> Vec<Menu<Command>> {
        vec![
            Menu::new(
                "File",
                vec![
                    MenuItem::new("Save", Command::Save)
                        .shortcut(KeyboardShortcut::new(CMD, Key::S))
                        .into(),
                    MenuEntry::submenu(
                        "More",
                        vec![
                            MenuItem::new("Save As", Command::SaveAs)
                                .shortcut(KeyboardShortcut::new(CMD | Modifiers::SHIFT, Key::S))
                                .into(),
                        ],
                    ),
                    MenuEntry::Separator,
                    MenuEntry::disabled("Print"),
                    MenuEntry::Section("recent"),
                ],
            ),
            Menu::new(
                "Edit",
                vec![
                    MenuItem::new("Copy", Command::Copy)
                        .shortcut(KeyboardShortcut::new(CMD, Key::C))
                        .text_edit(TextEditAction::Copy)
                        .into(),
                    MenuItem::new("Delete", Command::Delete)
                        .shortcut(KeyboardShortcut::new(Modifiers::NONE, Key::Delete))
                        .into(),
                ],
            ),
        ]
    }

    #[test]
    fn a_keymap_reads_the_shortcuts_of_every_item() {
        let keymap = Keymap::from_menus(&menus());
        assert_eq!(keymap.bindings().len(), 4);
        // ⇧⌘S before ⌘S, wherever they are in the table.
        assert_eq!(
            keymap.by_specificity().next().unwrap().command,
            Command::SaveAs
        );
        assert_eq!(keymap.first_duplicate(), None);
    }

    #[test]
    fn menu_shortcuts_only_fire_in_text_when_they_leave_editing_alone() {
        let keymap = Keymap::from_menus(&menus());
        let in_text = |command| {
            keymap
                .bindings()
                .iter()
                .find(|binding| binding.command == command)
                .unwrap()
                .in_text
        };
        assert!(in_text(Command::Save));
        assert!(!in_text(Command::Copy), "the field does its own copying");
        assert!(!in_text(Command::Delete), "a plain key has to type");
    }

    #[test]
    fn text_edit_actions_ask_for_the_clipboard_only_to_paste() {
        let no_clipboard = || -> Option<String> { panic!("no need for the clipboard") };
        assert_eq!(TextEditAction::Cut.events(no_clipboard), vec![Event::Cut]);
        assert_eq!(TextEditAction::Copy.events(no_clipboard), vec![Event::Copy]);
        let redo = TextEditAction::Redo.events(no_clipboard);
        assert!(matches!(
            redo[0],
            Event::Key { key: Key::Z, pressed: true, modifiers, .. }
                if modifiers == CMD | Modifiers::SHIFT
        ));
        assert!(matches!(redo[1], Event::Key { pressed: false, .. }));
        assert_eq!(
            TextEditAction::Paste.events(|| Some("a\r\nb".to_owned())),
            vec![Event::Paste("a\nb".to_owned())]
        );
        assert_eq!(TextEditAction::Paste.events(|| None), Vec::new());
        assert_eq!(
            TextEditAction::Paste.events(|| Some(String::new())),
            Vec::new()
        );
    }

    fn apply_all(harness: &mut Harness<'_, String>, actions: &[TextEditAction]) {
        for action in actions {
            for event in action.events(|| Some("pasted".to_owned())) {
                harness.event(event);
            }
            harness.run();
        }
    }

    fn text_field_harness(text: &str) -> Harness<'static, String> {
        let mut harness = Harness::new_ui_state(
            |ui, text: &mut String| {
                let label = ui.label("Name");
                ui.text_edit_singleline(text).labelled_by(label.id);
            },
            text.to_owned(),
        );
        harness.run();
        harness
            .get_by_role(egui::accesskit::Role::TextInput)
            .focus();
        harness.run();
        harness
    }

    #[test]
    fn select_all_then_cut_edits_the_focused_field() {
        let mut harness = text_field_harness("hello");
        apply_all(
            &mut harness,
            &[TextEditAction::SelectAll, TextEditAction::Cut],
        );
        assert_eq!(harness.state(), "");
    }

    #[test]
    fn paste_replaces_the_selection() {
        let mut harness = text_field_harness("hello");
        apply_all(
            &mut harness,
            &[TextEditAction::SelectAll, TextEditAction::Paste],
        );
        assert_eq!(harness.state(), "pasted");
    }

    #[test]
    fn undo_and_redo_reach_the_focused_field() {
        let mut harness = text_field_harness("hello");
        apply_all(
            &mut harness,
            &[TextEditAction::SelectAll, TextEditAction::Paste],
        );
        assert_eq!(harness.state(), "pasted");
        apply_all(&mut harness, &[TextEditAction::Undo]);
        assert_eq!(harness.state(), "hello");
        apply_all(&mut harness, &[TextEditAction::Redo]);
        assert_eq!(harness.state(), "pasted");
    }

    #[test]
    fn sections_start_empty_and_replace_what_they_show() {
        let mut sections = Sections::new();
        assert!(sections.get("recent").is_empty());
        sections.set("recent", vec![MenuEntry::<u8>::Separator]);
        sections.set("recent", vec![MenuEntry::disabled("None")]);
        assert_eq!(sections.get("recent"), &[MenuEntry::disabled("None")]);
    }

    #[test]
    fn availability_defaults_to_everything() {
        let unit: &dyn Availability<Command> = &();
        assert!(unit.enabled(&Command::Save));
        assert!(!unit.checked(&Command::Save));
        assert_eq!(unit.label(&Command::Save), None);
    }
}
