//! The table of keyboard shortcuts: what runs when a key is pressed, and what
//! tooltips, menus and the help view say about it.

use egui::{Button, Context, Event, Key, KeyboardShortcut, Modifiers, Response, Ui};
use egui_mev_widgets::Tip;

use crate::{menu_bar::check_button, queue::CommandQueue};

/// A shortcut bound to a command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding<C> {
    /// The keys.
    pub shortcut: KeyboardShortcut,
    /// What they run.
    pub command: C,
    /// Whether the shortcut also fires while a text field has focus.
    pub in_text: bool,
}

impl<C> Binding<C> {
    /// `modifiers` and `key` run `command`, except while a text field has focus.
    #[must_use]
    pub const fn new(modifiers: Modifiers, key: Key, command: C) -> Self {
        Self {
            shortcut: KeyboardShortcut::new(modifiers, key),
            command,
            in_text: false,
        }
    }

    /// The shortcut also fires while a text field has focus.
    #[must_use]
    pub const fn in_text(mut self) -> Self {
        self.in_text = true;
        self
    }
}

/// The app's keyboard shortcuts, in one table, so the help view, menus and
/// tooltips show the same key combinations that actually run. The first binding
/// of a command is the one menus show.
#[derive(Clone, Debug)]
pub struct Keymap<C> {
    bindings: Vec<Binding<C>>,
    /// Indices into `bindings`, the most modifiers first.
    by_specificity: Vec<usize>,
}

impl<C> Default for Keymap<C> {
    fn default() -> Self {
        Self {
            bindings: Vec::new(),
            by_specificity: Vec::new(),
        }
    }
}

impl<C> FromIterator<Binding<C>> for Keymap<C> {
    fn from_iter<I: IntoIterator<Item = Binding<C>>>(bindings: I) -> Self {
        Self::new(bindings)
    }
}

/// How many modifiers a shortcut needs.
pub(crate) fn modifier_count(modifiers: Modifiers) -> u8 {
    u8::from(modifiers.alt)
        + u8::from(modifiers.ctrl)
        + u8::from(modifiers.shift)
        + u8::from(modifiers.mac_cmd || modifiers.command)
}

/// Keys that keyboards reach through Shift, so a press of one comes with Shift
/// held even though the shortcut doesn't ask for it: `Cmd+Plus` is ⇧⌘= on a US
/// layout.
const fn typed_with_shift(key: Key) -> bool {
    matches!(
        key,
        Key::Plus
            | Key::Colon
            | Key::Pipe
            | Key::Questionmark
            | Key::Exclamationmark
            | Key::OpenCurlyBracket
            | Key::CloseCurlyBracket
    )
}

/// Whether a press of `key` with `pressed` held is `shortcut`.
fn press_matches(shortcut: KeyboardShortcut, key: Key, pressed: Modifiers) -> bool {
    key == shortcut.logical_key
        && (pressed.matches_exact(shortcut.modifiers)
            || (typed_with_shift(key)
                && pressed.matches_exact(shortcut.modifiers.plus(Modifiers::SHIFT))))
}

/// The key press behind a clipboard event. egui-winit turns Ctrl+X, Ctrl+C,
/// Ctrl+V and, on Windows, Shift+Delete, Ctrl+Insert and Shift+Insert into
/// `Cut` / `Copy` / `Paste` without the key event, so outside text fields they
/// are turned back into keys for the table.
pub(crate) const fn clipboard_key(event: &Event, modifiers: Modifiers) -> Option<Event> {
    let (key, modifiers) = match event {
        Event::Cut if cfg!(windows) && modifiers.shift && !modifiers.command => {
            (Key::Delete, Modifiers::SHIFT)
        }
        Event::Cut => (Key::X, modifiers),
        Event::Copy => (Key::C, modifiers.plus(Modifiers::COMMAND)),
        Event::Paste(_) => (Key::V, Modifiers::COMMAND),
        _ => return None,
    };
    Some(Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    })
}

impl<C> Keymap<C> {
    /// A keymap of `bindings`, in the order menus should prefer them.
    #[must_use]
    pub fn new(bindings: impl IntoIterator<Item = Binding<C>>) -> Self {
        let bindings: Vec<Binding<C>> = bindings.into_iter().collect();
        let mut by_specificity: Vec<usize> = (0..bindings.len()).collect();
        // Shortcuts with more modifiers first, so ⌃⌘S is consumed before ⌘S:
        // `matches_exact` takes a press with Control as a match for Command.
        by_specificity.sort_by_key(|&index| {
            std::cmp::Reverse(modifier_count(bindings[index].shortcut.modifiers))
        });
        Self {
            bindings,
            by_specificity,
        }
    }

    /// Every binding, in table order.
    #[must_use]
    pub fn bindings(&self) -> &[Binding<C>] {
        &self.bindings
    }

    /// Every binding in the order presses are tried: the most modifiers first, so
    /// an app that dispatches with `consume_shortcut` gets ⇧⌘S before ⌘S.
    pub fn by_specificity(&self) -> impl Iterator<Item = &Binding<C>> {
        self.by_specificity
            .iter()
            .map(|&index| &self.bindings[index])
    }

    /// Keep only the bindings `keep` accepts, such as those a native menu bar
    /// doesn't handle.
    pub fn retain(&mut self, keep: impl FnMut(&Binding<C>) -> bool) {
        let mut bindings = std::mem::take(&mut self.bindings);
        bindings.retain(keep);
        *self = Self::new(bindings);
    }

    /// A shortcut bound twice, if any. Apps assert this is `None` in a test.
    #[must_use]
    pub fn first_duplicate(&self) -> Option<KeyboardShortcut> {
        self.bindings
            .iter()
            .enumerate()
            .find_map(|(index, binding)| {
                self.bindings[..index]
                    .iter()
                    .any(|other| other.shortcut == binding.shortcut)
                    .then_some(binding.shortcut)
            })
    }
}

impl<C: Clone + PartialEq> Keymap<C> {
    /// Returns the commands whose shortcuts were pressed this frame and consumes
    /// their key events. Modifiers must match exactly, so Ctrl+Shift+Tab never
    /// also triggers Ctrl+Tab. Call it before drawing the widgets that would
    /// otherwise see those keys.
    ///
    /// Clipboard events become key presses again outside text fields, when a
    /// binding wants that key. Without one they stay as they are, for widgets
    /// that read `Event::Paste` themselves. Holding a key repeats its command.
    #[must_use]
    pub fn pressed(&self, ctx: &Context) -> Vec<C> {
        let in_text = ctx.text_edit_focused();
        let mut commands = Vec::new();
        ctx.input_mut(|input| {
            if !in_text {
                let modifiers = input.modifiers;
                for event in &mut input.events {
                    if let Some(key) = clipboard_key(event, modifiers)
                        && self.wants(&key)
                    {
                        *event = key;
                    }
                }
            }
            for binding in self.by_specificity() {
                if in_text && !binding.in_text {
                    continue;
                }
                let before = input.events.len();
                input.events.retain(|event| {
                    !matches!(
                        event,
                        Event::Key { key, modifiers, pressed: true, .. }
                            if press_matches(binding.shortcut, *key, *modifiers)
                    )
                });
                if input.events.len() != before && !commands.contains(&binding.command) {
                    commands.push(binding.command.clone());
                }
            }
        });
        commands
    }

    fn wants(&self, event: &Event) -> bool {
        let Event::Key { key, modifiers, .. } = event else {
            return false;
        };
        self.bindings
            .iter()
            .any(|binding| press_matches(binding.shortcut, *key, *modifiers))
    }

    /// The first shortcut bound to `command`.
    #[must_use]
    pub fn shortcut_for(&self, command: &C) -> Option<KeyboardShortcut> {
        self.bindings
            .iter()
            .find(|binding| binding.command == *command)
            .map(|binding| binding.shortcut)
    }

    /// Every shortcut bound to `command`.
    pub fn shortcuts_for<'a>(
        &'a self,
        command: &'a C,
    ) -> impl Iterator<Item = KeyboardShortcut> + 'a {
        self.bindings
            .iter()
            .filter(move |binding| binding.command == *command)
            .map(|binding| binding.shortcut)
    }

    /// The other command that already uses `shortcut`, if any, for refusing a key
    /// someone is recording for `except`.
    #[must_use]
    pub fn taken_by(&self, shortcut: KeyboardShortcut, except: &C) -> Option<&C> {
        self.bindings
            .iter()
            .find(|binding| binding.command != *except && binding.shortcut == shortcut)
            .map(|binding| &binding.command)
    }

    /// The first shortcut bound to `command`, formatted for this platform. Like
    /// `Context::format_shortcut`, it needs a frame to have run, for the fonts.
    #[must_use]
    pub fn shortcut_text(&self, ctx: &Context, command: &C) -> Option<String> {
        self.shortcut_for(command)
            .map(|shortcut| ctx.format_shortcut(&shortcut))
    }

    /// What an icon button for `command` shows on hover: `name`, then its
    /// shortcut. Hand it to `toolbar_icon` and the other icon buttons.
    #[must_use]
    pub fn tip<'a>(&self, ctx: &Context, name: &'a str, command: &C) -> Tip<'a> {
        self.shortcut_text(ctx, command)
            .map_or_else(|| Tip::from(name), |shortcut| Tip::hint(name, shortcut))
    }

    /// A tooltip like "Back  Alt+Left".
    #[must_use]
    pub fn tooltip(&self, ctx: &Context, name: &str, command: &C) -> String {
        self.tip(ctx, name, command).text()
    }
}

impl<C: Clone + PartialEq + Send + Sync + 'static> Keymap<C> {
    /// A menu entry that queues `command` and shows its shortcut, so menus and the
    /// keyboard always do the same thing.
    pub fn menu_button(&self, ui: &mut Ui, label: &str, command: &C) -> Response {
        self.menu_button_enabled(ui, label, command, true)
    }

    /// Like [`Self::menu_button`], disabled when `enabled` is false. For a label
    /// that changes, such as "Undo rename".
    pub fn menu_button_enabled(
        &self,
        ui: &mut Ui,
        label: &str,
        command: &C,
        enabled: bool,
    ) -> Response {
        let button = self.with_shortcut(ui, Button::new(label), command);
        let response = ui.add_enabled(enabled, button);
        Self::run_on_click(ui, &response, command);
        response
    }

    /// Like [`Self::menu_button`], drawn as a check entry reflecting `checked`.
    pub fn menu_toggle(&self, ui: &mut Ui, label: &str, command: &C, checked: bool) -> Response {
        let button = self.with_shortcut(ui, check_button(ui, checked, label), command);
        let response = ui.add(button);
        Self::run_on_click(ui, &response, command);
        response
    }

    fn with_shortcut<'a>(&self, ui: &Ui, button: Button<'a>, command: &C) -> Button<'a> {
        match self.shortcut_text(ui.ctx(), command) {
            Some(shortcut) => button.shortcut_text(shortcut),
            None => button,
        }
    }

    fn run_on_click(ui: &Ui, response: &Response, command: &C) {
        if response.clicked() {
            CommandQueue::push(ui.ctx(), command.clone());
            ui.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use egui::RawInput;
    use egui_kittest::{Harness, kittest::Queryable as _};

    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Command {
        Save,
        SaveAs,
        Lock,
        Back,
        NextTab,
        PreviousTab,
        Cut,
        Copy,
        CopyAsPath,
        Paste,
        DeletePermanently,
        ZoomIn,
        Search,
    }

    const CMD: Modifiers = Modifiers::COMMAND;
    const CMD_SHIFT: Modifiers = Modifiers::COMMAND.plus(Modifiers::SHIFT);

    fn keymap() -> Keymap<Command> {
        Keymap::new([
            Binding::new(CMD, Key::S, Command::Save),
            Binding::new(CMD_SHIFT, Key::S, Command::SaveAs),
            Binding::new(CMD.plus(Modifiers::CTRL), Key::S, Command::Lock),
            Binding::new(Modifiers::ALT, Key::ArrowLeft, Command::Back),
            Binding::new(CMD, Key::Tab, Command::NextTab),
            Binding::new(CMD_SHIFT, Key::Tab, Command::PreviousTab),
            Binding::new(CMD, Key::X, Command::Cut),
            Binding::new(CMD_SHIFT, Key::C, Command::CopyAsPath),
            Binding::new(CMD, Key::C, Command::Copy),
            Binding::new(CMD, Key::V, Command::Paste),
            Binding::new(Modifiers::SHIFT, Key::Delete, Command::DeletePermanently),
            Binding::new(CMD, Key::Plus, Command::ZoomIn),
            Binding::new(CMD, Key::F, Command::Search).in_text(),
        ])
    }

    fn frame(ctx: &Context, events: Vec<Event>, run: impl FnOnce(&mut Ui)) {
        let input = RawInput {
            events,
            ..Default::default()
        };
        let mut run = Some(run);
        let mut output = ctx.run_ui(input, |ui| {
            if let Some(run) = run.take() {
                run(ui);
            }
        });
        output.textures_delta.clear();
    }

    fn press(ctx: &Context, modifiers: Modifiers, key: Key) -> Vec<Command> {
        let mut commands = Vec::new();
        let events = vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }];
        frame(ctx, events, |ui| commands = keymap().pressed(ui.ctx()));
        commands
    }

    #[test]
    fn modifiers_must_match_exactly() {
        let ctx = Context::default();
        assert_eq!(press(&ctx, CMD_SHIFT, Key::Tab), vec![Command::PreviousTab]);
        assert_eq!(press(&ctx, CMD, Key::Tab), vec![Command::NextTab]);
        assert_eq!(
            press(&ctx, Modifiers::ALT, Key::ArrowLeft),
            vec![Command::Back]
        );
        assert_eq!(press(&ctx, Modifiers::NONE, Key::ArrowLeft), Vec::new());
        assert_eq!(press(&ctx, Modifiers::ALT | CMD, Key::Tab), Vec::new());
    }

    #[test]
    fn the_most_specific_shortcut_wins() {
        let ctx = Context::default();
        assert_eq!(press(&ctx, CMD_SHIFT, Key::S), vec![Command::SaveAs]);
        assert_eq!(press(&ctx, CMD, Key::S), vec![Command::Save]);
        // A press with Control also counts as Command to egui, so only the order
        // keeps ⌃⌘S from running Save. On macOS Command reports `mac_cmd` too.
        let control_command = Modifiers {
            ctrl: true,
            mac_cmd: true,
            command: true,
            ..Modifiers::NONE
        };
        assert_eq!(press(&ctx, control_command, Key::S), vec![Command::Lock]);
    }

    #[test]
    fn presses_are_tried_most_modifiers_first_and_the_table_keeps_its_order() {
        let keymap = keymap();
        let counts: Vec<u8> = keymap
            .by_specificity()
            .map(|binding| modifier_count(binding.shortcut.modifiers))
            .collect();
        assert!(counts.is_sorted_by(|a, b| a >= b), "{counts:?}");
        assert_eq!(keymap.bindings()[0].command, Command::Save);
        // Equal counts stay in table order.
        let specific: Vec<Command> = keymap.by_specificity().map(|b| b.command).take(4).collect();
        assert_eq!(specific[0], Command::SaveAs);
        assert_eq!(specific[1], Command::Lock);
    }

    #[test]
    fn a_mac_command_press_matches_the_command_modifier() {
        let ctx = Context::default();
        let mac = Modifiers {
            mac_cmd: true,
            command: true,
            ..Modifiers::NONE
        };
        assert_eq!(press(&ctx, mac, Key::S), vec![Command::Save]);
    }

    #[test]
    fn a_symbol_typed_with_shift_matches_without_asking_for_it() {
        let ctx = Context::default();
        assert_eq!(press(&ctx, CMD_SHIFT, Key::Plus), vec![Command::ZoomIn]);
        assert_eq!(press(&ctx, CMD, Key::Plus), vec![Command::ZoomIn]);
    }

    #[test]
    fn the_key_event_is_consumed() {
        let ctx = Context::default();
        let event = Event::Key {
            key: Key::S,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: CMD,
        };
        frame(&ctx, vec![event], |ui| {
            assert_eq!(keymap().pressed(ui.ctx()), vec![Command::Save]);
            assert_eq!(keymap().pressed(ui.ctx()), Vec::new());
        });
    }

    fn clipboard(ctx: &Context, modifiers: Modifiers, event: Event) -> Vec<Command> {
        let mut commands = Vec::new();
        let events = vec![Event::ModifiersChanged(modifiers), event];
        frame(ctx, events, |ui| commands = keymap().pressed(ui.ctx()));
        commands
    }

    /// egui-winit sends these as clipboard events instead of key presses.
    #[test]
    fn clipboard_events_run_their_shortcuts() {
        let ctx = Context::default();
        assert_eq!(clipboard(&ctx, CMD, Event::Cut), vec![Command::Cut]);
        assert_eq!(clipboard(&ctx, CMD, Event::Copy), vec![Command::Copy]);
        assert_eq!(
            clipboard(&ctx, CMD_SHIFT, Event::Copy),
            vec![Command::CopyAsPath]
        );
        assert_eq!(
            clipboard(&ctx, CMD, Event::Paste("x".into())),
            vec![Command::Paste]
        );
        if cfg!(windows) {
            assert_eq!(
                clipboard(&ctx, Modifiers::SHIFT, Event::Cut),
                vec![Command::DeletePermanently]
            );
        }
    }

    #[test]
    fn a_clipboard_event_nobody_bound_is_left_alone() {
        let ctx = Context::default();
        let empty: Keymap<Command> = Keymap::default();
        let events = vec![
            Event::ModifiersChanged(CMD),
            Event::Paste("kept".to_owned()),
        ];
        frame(&ctx, events, |ui| {
            assert_eq!(empty.pressed(ui.ctx()), Vec::new());
            assert!(ui.input(|input| {
                input
                    .events
                    .iter()
                    .any(|event| matches!(event, Event::Paste(text) if text == "kept"))
            }));
        });
    }

    #[test]
    fn a_focused_text_field_keeps_its_keys() {
        let mut harness = Harness::new_ui_state(
            |ui, (text, commands): &mut (String, Vec<Command>)| {
                commands.extend(keymap().pressed(ui.ctx()));
                let label = ui.label("Name");
                ui.text_edit_singleline(text).labelled_by(label.id);
            },
            ("hello".to_owned(), Vec::new()),
        );
        harness.run();
        harness
            .get_by_role(egui::accesskit::Role::TextInput)
            .focus();
        harness.run();
        // In text: ⌘S isn't for the field's benefit, only `in_text` bindings run.
        harness.key_press_modifiers(CMD, Key::S);
        harness.key_press_modifiers(CMD, Key::F);
        harness.run();
        assert_eq!(harness.state().1, vec![Command::Search]);
        // The clipboard event stays a clipboard event, for the field.
        harness.state_mut().1.clear();
        harness.event(Event::Paste(" there".to_owned()));
        harness.run();
        assert_eq!(harness.state().1, Vec::new());
        assert!(harness.state().0.contains("there"), "{}", harness.state().0);
    }

    #[test]
    fn looks_up_shortcuts_and_conflicts() {
        let keymap = keymap();
        let save = KeyboardShortcut::new(CMD, Key::S);
        assert_eq!(keymap.shortcut_for(&Command::Save), Some(save));
        assert_eq!(keymap.taken_by(save, &Command::Back), Some(&Command::Save));
        // Its own shortcut is no conflict.
        assert_eq!(keymap.taken_by(save, &Command::Save), None);
        assert_eq!(
            keymap.taken_by(
                KeyboardShortcut::new(Modifiers::ALT, Key::M),
                &Command::Save
            ),
            None
        );
    }

    #[test]
    fn finds_a_shortcut_bound_twice() {
        assert_eq!(keymap().first_duplicate(), None);
        let twice = Keymap::new([
            Binding::new(CMD, Key::S, Command::Save),
            Binding::new(CMD, Key::S, Command::SaveAs),
        ]);
        assert_eq!(
            twice.first_duplicate(),
            Some(KeyboardShortcut::new(CMD, Key::S))
        );
    }

    #[test]
    fn retain_keeps_the_rest_working() {
        let mut keymap = keymap();
        keymap.retain(|binding| binding.command != Command::SaveAs);
        assert_eq!(keymap.shortcut_for(&Command::SaveAs), None);
        assert!(
            keymap
                .by_specificity()
                .all(|b| b.command != Command::SaveAs)
        );
        assert_eq!(keymap.bindings().len(), keymap.by_specificity().count());
    }

    #[test]
    fn tooltips_read_shortcuts_from_the_table() {
        let ctx = Context::default();
        // Formatting a shortcut needs the fonts of a first frame.
        frame(&ctx, Vec::new(), |_| {});
        let keymap = keymap();
        let shortcut = ctx.format_shortcut(&KeyboardShortcut::new(CMD, Key::S));
        assert_eq!(
            keymap.tooltip(&ctx, "Save", &Command::Save),
            format!("Save  {shortcut}")
        );
        let unbound: Keymap<Command> = Keymap::default();
        assert_eq!(unbound.tooltip(&ctx, "Save", &Command::Save), "Save");
        let tip = keymap.tip(&ctx, "Save", &Command::Save);
        assert_eq!(tip.name(), "Save");
    }

    #[test]
    fn menu_helpers_queue_their_command() {
        let keymap = keymap();
        let mut harness = Harness::new_ui(move |ui| {
            keymap.menu_button(ui, "Save", &Command::Save);
            keymap.menu_button_enabled(ui, "Save as", &Command::SaveAs, false);
            keymap.menu_toggle(ui, "Back", &Command::Back, true);
        });
        harness.run();
        // The shortcut is part of the button's text.
        harness.get_by_label("Save as Ctrl+Shift+S").click();
        harness.run();
        assert_eq!(CommandQueue::<Command>::drain(&harness.ctx), Vec::new());
        harness.get_by_label("Save Ctrl+S").click();
        harness.run();
        harness.get_by_label_contains("Back").click();
        harness.run();
        assert_eq!(
            CommandQueue::<Command>::drain(&harness.ctx),
            vec![Command::Save, Command::Back]
        );
    }
}
