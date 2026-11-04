//! The user's own keyboard shortcuts: overrides of an app's defaults, saved with
//! its settings, and recording new ones.

use std::{collections::BTreeMap, fmt::Debug, marker::PhantomData};

use egui::{Context, Event, Key, KeyboardShortcut, Modifiers};

use crate::{
    keymap::{Binding, Keymap, clipboard_key},
    menu::{Menu, MenuEntry},
    shortcut::Shortcut,
};

/// The shortcuts someone chose: a command's name to its shortcut in
/// [`Shortcut::encode`]'s form, or to nothing for no shortcut at all. Commands
/// not listed keep their default.
///
/// The names are `C`'s `Debug` text, so renaming a command in the code forgets
/// its override. Entries for commands the app doesn't have (yet) are kept.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(transparent, bound = "")
)]
pub struct KeyOverrides<C> {
    overrides: BTreeMap<String, Option<String>>,
    #[cfg_attr(feature = "serde", serde(skip))]
    command: PhantomData<fn(&C)>,
}

impl<C> Default for KeyOverrides<C> {
    fn default() -> Self {
        Self::from_map(BTreeMap::new())
    }
}

impl<C> KeyOverrides<C> {
    /// No overrides.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Overrides as saved.
    #[must_use]
    pub const fn from_map(overrides: BTreeMap<String, Option<String>>) -> Self {
        Self {
            overrides,
            command: PhantomData,
        }
    }

    /// The overrides, as saved.
    #[must_use]
    pub const fn as_map(&self) -> &BTreeMap<String, Option<String>> {
        &self.overrides
    }

    /// Whether every command uses its default.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.overrides.is_empty()
    }

    /// Use every default again.
    pub fn clear(&mut self) {
        self.overrides.clear();
    }
}

impl<C: Debug> KeyOverrides<C> {
    fn id(command: &C) -> String {
        format!("{command:?}")
    }

    /// Give `command` this shortcut, or none at all.
    pub fn set(&mut self, command: &C, shortcut: Option<KeyboardShortcut>) {
        self.overrides.insert(
            Self::id(command),
            shortcut.map(|shortcut| Shortcut(shortcut).encode()),
        );
    }

    /// Use `command`'s default again.
    pub fn reset(&mut self, command: &C) {
        self.overrides.remove(&Self::id(command));
    }

    /// Whether `command` has an override.
    #[must_use]
    pub fn contains(&self, command: &C) -> bool {
        self.overrides.contains_key(&Self::id(command))
    }

    /// `command`'s shortcut: its override if it has one, else `default`. An
    /// override that doesn't read as a shortcut counts as `default`.
    #[must_use]
    pub fn apply(
        &self,
        command: &C,
        default: Option<KeyboardShortcut>,
    ) -> Option<KeyboardShortcut> {
        match self.overrides.get(&Self::id(command)) {
            Some(Some(text)) => Shortcut::decode(text).map(|s| s.0).or(default),
            Some(None) => None,
            None => default,
        }
    }

    /// Put the overrides on the items of `menus`.
    pub fn apply_to_menus(&self, menus: &mut [Menu<C>]) {
        fn walk<C: Debug>(overrides: &KeyOverrides<C>, entries: &mut [MenuEntry<C>]) {
            for entry in entries {
                match entry {
                    MenuEntry::Item(item) => {
                        item.shortcut = overrides.apply(&item.command, item.shortcut);
                    }
                    MenuEntry::Submenu { entries, .. } => walk(overrides, entries),
                    MenuEntry::Disabled { .. } | MenuEntry::Separator | MenuEntry::Section(_) => {}
                }
            }
        }
        for menu in menus {
            walk(self, &mut menu.entries);
        }
    }
}

impl<C: Debug + Clone + PartialEq> KeyOverrides<C> {
    /// `keymap` with the overrides on it. A command with an override keeps one
    /// binding, in the place of its first, and so loses any second keys.
    #[must_use]
    pub fn apply_to_keymap(&self, keymap: &Keymap<C>) -> Keymap<C> {
        let mut done: Vec<&C> = Vec::new();
        let mut bindings = Vec::new();
        for binding in keymap.bindings() {
            if !self.contains(&binding.command) {
                bindings.push(binding.clone());
            } else if !done.contains(&&binding.command) {
                done.push(&binding.command);
                if let Some(shortcut) = self.apply(&binding.command, Some(binding.shortcut)) {
                    bindings.push(Binding {
                        shortcut,
                        ..binding.clone()
                    });
                }
            }
        }
        Keymap::new(bindings)
    }
}

/// `modifiers` as a table would spell them: the platform's command key as
/// `command`, with Control kept apart on macOS, where it is a key of its own.
#[must_use]
pub const fn normalize_modifiers(modifiers: Modifiers) -> Modifiers {
    let mac = cfg!(target_os = "macos");
    Modifiers {
        alt: modifiers.alt,
        shift: modifiers.shift,
        ctrl: mac && modifiers.ctrl,
        mac_cmd: false,
        command: if mac {
            modifiers.mac_cmd || modifiers.command
        } else {
            modifiers.ctrl || modifiers.command
        },
    }
}

/// Take this frame's next key press, with normalized modifiers, so it doesn't
/// also run a shortcut or type into a field. Call it before the widgets and the
/// keymap. Clipboard keys count: egui-winit reports ⌘C as `Event::Copy`.
#[must_use]
pub fn capture_key_press(ctx: &Context) -> Option<KeyboardShortcut> {
    ctx.input_mut(|input| {
        let held = input.modifiers;
        let found = input.events.iter().find_map(|event| {
            let key_press = clipboard_key(event, held);
            match key_press.as_ref().unwrap_or(event) {
                Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => Some((*key, *modifiers)),
                _ => None,
            }
        });
        if found.is_some() {
            input.events.retain(|event| {
                !matches!(
                    event,
                    Event::Key { .. } | Event::Text(_) | Event::Cut | Event::Copy | Event::Paste(_)
                )
            });
        }
        found.map(|(key, modifiers)| KeyboardShortcut::new(normalize_modifiers(modifiers), key))
    })
}

/// What [`KeyRecorder::record`] saw this frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Recording<C> {
    /// Not recording, or no key yet.
    Waiting,
    /// Escape: recording stopped and nothing changed.
    Cancelled,
    /// The new shortcut, which recording stops on. Save it with
    /// [`KeyOverrides::set`].
    Recorded {
        /// The command being recorded for.
        command: C,
        /// The keys pressed.
        shortcut: KeyboardShortcut,
    },
    /// Those keys belong to another command. Recording goes on, for another try.
    Taken {
        /// The command being recorded for.
        command: C,
        /// The keys pressed.
        shortcut: KeyboardShortcut,
        /// The command that has them.
        by: C,
    },
}

/// Waits for the keys of a new shortcut for one command at a time.
#[derive(Clone, Debug)]
pub struct KeyRecorder<C> {
    recording: Option<C>,
}

impl<C> Default for KeyRecorder<C> {
    fn default() -> Self {
        Self { recording: None }
    }
}

impl<C> KeyRecorder<C> {
    /// Record the next keys for `command`, instead of any other command.
    pub fn start(&mut self, command: C) {
        self.recording = Some(command);
    }

    /// Stop recording.
    pub fn stop(&mut self) {
        self.recording = None;
    }

    /// The command being recorded for.
    #[must_use]
    pub const fn recording(&self) -> Option<&C> {
        self.recording.as_ref()
    }
}

impl<C: Clone + PartialEq> KeyRecorder<C> {
    /// Takes this frame's key press while recording, and checks it against
    /// `keymap`. Call it before the widgets and the keymap's
    /// [`Keymap::pressed`], so the keys only go here.
    pub fn record(&mut self, ctx: &Context, keymap: &Keymap<C>) -> Recording<C> {
        let Some(command) = self.recording.clone() else {
            return Recording::Waiting;
        };
        let Some(shortcut) = capture_key_press(ctx) else {
            return Recording::Waiting;
        };
        if shortcut.logical_key == Key::Escape && shortcut.modifiers.is_none() {
            self.recording = None;
            return Recording::Cancelled;
        }
        if let Some(by) = keymap.taken_by(shortcut, &command) {
            return Recording::Taken {
                command,
                shortcut,
                by: by.clone(),
            };
        }
        self.recording = None;
        Recording::Recorded { command, shortcut }
    }
}

#[cfg(test)]
mod tests {
    use egui::RawInput;

    use super::*;
    use crate::menu::MenuItem;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Command {
        Save,
        Redo,
        Rename(u8),
    }

    const CMD: Modifiers = Modifiers::COMMAND;

    fn shortcut(modifiers: Modifiers, key: Key) -> KeyboardShortcut {
        KeyboardShortcut::new(modifiers, key)
    }

    #[test]
    fn overrides_replace_the_default_and_none_removes_it() {
        let default = Some(shortcut(CMD, Key::S));
        let mut overrides = KeyOverrides::new();
        assert_eq!(overrides.apply(&Command::Save, default), default);

        let chosen = shortcut(CMD.plus(Modifiers::ALT), Key::S);
        overrides.set(&Command::Save, Some(chosen));
        assert_eq!(overrides.apply(&Command::Save, default), Some(chosen));
        assert!(overrides.contains(&Command::Save));

        overrides.set(&Command::Save, None);
        assert_eq!(overrides.apply(&Command::Save, default), None);

        overrides.reset(&Command::Save);
        assert_eq!(overrides.apply(&Command::Save, default), default);
        assert!(overrides.is_empty());
    }

    #[test]
    fn saves_encoded_text_keyed_by_the_command_name() {
        let mut overrides = KeyOverrides::new();
        overrides.set(&Command::Rename(3), Some(shortcut(CMD, Key::F2)));
        overrides.set(&Command::Save, None);
        let saved = overrides.as_map().clone();
        assert_eq!(saved["Rename(3)"], Some("Cmd+F2".to_owned()));
        assert_eq!(saved["Save"], None);
        assert_eq!(KeyOverrides::<Command>::from_map(saved), overrides);
    }

    #[test]
    fn a_broken_override_counts_as_the_default() {
        let default = Some(shortcut(CMD, Key::S));
        let map = BTreeMap::from([("Save".to_owned(), Some("Hyper+S".to_owned()))]);
        let overrides = KeyOverrides::from_map(map);
        assert_eq!(overrides.apply(&Command::Save, default), default);
    }

    #[test]
    fn overrides_land_on_menu_items_including_nested_ones() {
        let mut menus = vec![Menu::new(
            "Edit",
            vec![
                MenuItem::new("Save", Command::Save)
                    .shortcut(shortcut(CMD, Key::S))
                    .into(),
                MenuEntry::submenu(
                    "More",
                    vec![
                        MenuItem::new("Redo", Command::Redo)
                            .shortcut(shortcut(CMD, Key::Y))
                            .into(),
                    ],
                ),
            ],
        )];
        let mut overrides = KeyOverrides::new();
        overrides.set(&Command::Save, None);
        overrides.set(&Command::Redo, Some(shortcut(CMD, Key::U)));
        overrides.apply_to_menus(&mut menus);
        let keymap = Keymap::from_menus(&menus);
        assert_eq!(keymap.shortcut_for(&Command::Save), None);
        assert_eq!(
            keymap.shortcut_for(&Command::Redo),
            Some(shortcut(CMD, Key::U))
        );
    }

    #[test]
    fn an_override_drops_a_commands_second_keys() {
        let keymap = Keymap::new([
            Binding::new(CMD, Key::Y, Command::Redo).in_text(),
            Binding::new(CMD.plus(Modifiers::SHIFT), Key::Z, Command::Redo),
            Binding::new(CMD, Key::S, Command::Save),
        ]);
        let mut overrides = KeyOverrides::new();
        overrides.set(&Command::Redo, Some(shortcut(CMD, Key::U)));
        let applied = overrides.apply_to_keymap(&keymap);
        let redo: Vec<_> = applied.shortcuts_for(&Command::Redo).collect();
        assert_eq!(redo, vec![shortcut(CMD, Key::U)]);
        assert!(applied.bindings()[0].in_text, "the binding keeps its rule");
        assert_eq!(
            applied.shortcut_for(&Command::Save),
            Some(shortcut(CMD, Key::S))
        );

        overrides.set(&Command::Save, None);
        assert_eq!(
            overrides
                .apply_to_keymap(&keymap)
                .shortcut_for(&Command::Save),
            None
        );
    }

    #[test]
    fn normalizing_merges_the_command_key() {
        let mac_press = Modifiers {
            mac_cmd: true,
            command: true,
            ..Modifiers::NONE
        };
        let other_press = Modifiers {
            ctrl: true,
            command: true,
            ..Modifiers::NONE
        };
        let press = if cfg!(target_os = "macos") {
            mac_press
        } else {
            other_press
        };
        assert_eq!(normalize_modifiers(press), Modifiers::COMMAND);
        assert_eq!(normalize_modifiers(Modifiers::ALT), Modifiers::ALT);
    }

    fn key_event(modifiers: Modifiers, key: Key) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    fn with_events<R>(events: Vec<Event>, run: impl FnOnce(&Context) -> R) -> R {
        let ctx = Context::default();
        let input = RawInput {
            events,
            ..Default::default()
        };
        let mut run = Some(run);
        let mut result = None;
        let mut output = ctx.run_ui(input, |ui| {
            if let Some(run) = run.take() {
                result = Some(run(ui.ctx()));
            }
        });
        output.textures_delta.clear();
        result.expect("the frame ran")
    }

    #[test]
    fn capture_takes_the_key_and_the_text_it_would_type() {
        let events = vec![
            key_event(Modifiers::NONE, Key::A),
            Event::Text("a".to_owned()),
        ];
        with_events(events, |ctx| {
            assert_eq!(
                capture_key_press(ctx),
                Some(shortcut(Modifiers::NONE, Key::A))
            );
            assert!(ctx.input(|input| input.events.is_empty()));
            assert_eq!(capture_key_press(ctx), None);
        });
    }

    #[test]
    fn capture_sees_a_clipboard_key() {
        let events = vec![Event::ModifiersChanged(CMD), Event::Copy];
        with_events(events, |ctx| {
            assert_eq!(capture_key_press(ctx), Some(shortcut(CMD, Key::C)));
        });
    }

    fn keymap() -> Keymap<Command> {
        Keymap::new([
            Binding::new(CMD, Key::S, Command::Save),
            Binding::new(CMD, Key::Y, Command::Redo),
        ])
    }

    #[test]
    fn recording_stores_nothing_until_started() {
        let mut recorder = KeyRecorder::default();
        let result = with_events(vec![key_event(CMD, Key::U)], |ctx| {
            recorder.record(ctx, &keymap())
        });
        assert_eq!(result, Recording::Waiting);
    }

    #[test]
    fn recording_stops_on_a_free_shortcut() {
        let mut recorder = KeyRecorder::default();
        recorder.start(Command::Save);
        let result = with_events(vec![key_event(CMD, Key::U)], |ctx| {
            recorder.record(ctx, &keymap())
        });
        assert_eq!(
            result,
            Recording::Recorded {
                command: Command::Save,
                shortcut: shortcut(CMD, Key::U),
            }
        );
        assert_eq!(recorder.recording(), None);
    }

    #[test]
    fn recording_refuses_a_taken_shortcut_and_goes_on() {
        let mut recorder = KeyRecorder::default();
        recorder.start(Command::Save);
        let result = with_events(vec![key_event(CMD, Key::Y)], |ctx| {
            recorder.record(ctx, &keymap())
        });
        assert_eq!(
            result,
            Recording::Taken {
                command: Command::Save,
                shortcut: shortcut(CMD, Key::Y),
                by: Command::Redo,
            }
        );
        assert_eq!(recorder.recording(), Some(&Command::Save));
    }

    #[test]
    fn a_commands_own_shortcut_is_free_for_it() {
        let mut recorder = KeyRecorder::default();
        recorder.start(Command::Save);
        let result = with_events(vec![key_event(CMD, Key::S)], |ctx| {
            recorder.record(ctx, &keymap())
        });
        assert!(matches!(result, Recording::Recorded { .. }), "{result:?}");
    }

    #[test]
    fn escape_cancels_recording() {
        let mut recorder = KeyRecorder::default();
        recorder.start(Command::Save);
        let result = with_events(vec![key_event(Modifiers::NONE, Key::Escape)], |ctx| {
            recorder.record(ctx, &keymap())
        });
        assert_eq!(result, Recording::Cancelled);
        assert_eq!(recorder.recording(), None);
    }
}
