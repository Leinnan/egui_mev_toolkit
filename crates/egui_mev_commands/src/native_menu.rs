//! The native macOS menu bar, built with muda from the same [`Menu`] table as the
//! egui one.
//!
//! Menu clicks and key equivalents become commands. Shortcuts without modifiers,
//! such as Delete, stay with egui so they never take keys from a focused text
//! field, and an item can opt out with [`MenuItem::no_accelerator`].
//!
//! ⌘C, ⌘V, ⌘X, ⌘A and ⌘Z are key equivalents, so AppKit consumes them before egui
//! sees the keys. While a text field has focus, the items marked with
//! [`MenuItem::text_edit`] are turned back into the egui events the field expects
//! instead of running as commands. Feed those to the next frame:
//!
//! ```ignore
//! fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
//!     raw_input.events.extend(self.menu.drain_text_events());
//! }
//! ```
//!
//! Key equivalents of greyed out items aren't consumed, so they reach egui. Keep
//! them from also running through a keymap with
//! `keymap.retain(|binding| !bar.handles(&binding.shortcut))`.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, PoisonError, RwLock},
};

use egui::{Context, Event, Key, KeyboardShortcut};
use muda::{
    CheckMenuItem, IsMenuItem, Menu as NativeMenu, MenuEvent, MenuId, MenuItem as NativeItem,
    PredefinedMenuItem, Submenu,
    accelerator::{Accelerator, Code, Modifiers},
};

use crate::{
    keymap::Keymap,
    menu::{Availability, Menu, MenuEntry, MenuItem, TextEditAction},
    queue::CommandQueue,
};

/// What the app itself brings to the application menu, the first one on the bar.
/// AppKit's own About panel is used when `about` is `None`.
#[derive(Clone, Debug)]
pub struct NativeMenuOptions<C> {
    /// The application menu's title.
    pub app_name: String,
    /// Its own About window, rather than AppKit's panel.
    pub about: Option<C>,
    /// "About …".
    pub about_label: String,
    /// Open the settings, if the app has any.
    pub settings: Option<C>,
    /// "Settings…".
    pub settings_label: String,
    /// ⌘, by default.
    pub settings_shortcut: Option<KeyboardShortcut>,
    /// Quit the app the way it wants, so eframe shuts down cleanly and saves.
    pub quit: C,
    /// "Quit …".
    pub quit_label: String,
    /// ⌘Q by default.
    pub quit_shortcut: Option<KeyboardShortcut>,
    /// The title of the menu that holds AppKit's window items: one in the table
    /// with this title gets them first, and otherwise one is added at the end.
    pub window_menu: String,
}

impl<C> NativeMenuOptions<C> {
    /// Options for an app called `app_name` that quits with `quit`.
    #[must_use]
    pub fn new(app_name: impl Into<String>, quit: C) -> Self {
        let app_name = app_name.into();
        Self {
            about: None,
            about_label: format!("About {app_name}"),
            settings: None,
            settings_label: "Settings…".to_owned(),
            settings_shortcut: Some(KeyboardShortcut::new(egui::Modifiers::COMMAND, Key::Comma)),
            quit,
            quit_label: format!("Quit {app_name}"),
            quit_shortcut: Some(KeyboardShortcut::new(egui::Modifiers::COMMAND, Key::Q)),
            window_menu: "Window".to_owned(),
            app_name,
        }
    }

    /// With the app's own About window.
    #[must_use]
    pub fn about(mut self, command: C) -> Self {
        self.about = Some(command);
        self
    }

    /// With a Settings item.
    #[must_use]
    pub fn settings(mut self, command: C) -> Self {
        self.settings = Some(command);
        self
    }
}

/// Whether AppKit can take `shortcut` as a key equivalent, which takes it away
/// from egui.
#[must_use]
pub fn has_key_equivalent(shortcut: KeyboardShortcut) -> bool {
    accelerator(shortcut).is_some()
}

/// Convert an egui shortcut to a native key equivalent. Modifier-less shortcuts
/// return `None` so egui keeps handling them, and they can't steal keys from text
/// fields. Keys AppKit has no code for, like `+`, return `None` too.
#[must_use]
pub fn accelerator(shortcut: KeyboardShortcut) -> Option<Accelerator> {
    let held = shortcut.modifiers;
    let mut modifiers = Modifiers::empty();
    if held.command || held.mac_cmd {
        modifiers |= Modifiers::META;
    }
    if held.ctrl {
        modifiers |= Modifiers::CONTROL;
    }
    if held.shift {
        modifiers |= Modifiers::SHIFT;
    }
    if held.alt {
        modifiers |= Modifiers::ALT;
    }
    if modifiers.is_empty() {
        return None;
    }
    Some(Accelerator::new(modifiers, key_code(shortcut.logical_key)?))
}

/// The physical code of `key`, among those muda can turn into a key equivalent.
const fn key_code(key: Key) -> Option<Code> {
    Some(match key {
        Key::A => Code::KeyA,
        Key::B => Code::KeyB,
        Key::C => Code::KeyC,
        Key::D => Code::KeyD,
        Key::E => Code::KeyE,
        Key::F => Code::KeyF,
        Key::G => Code::KeyG,
        Key::H => Code::KeyH,
        Key::I => Code::KeyI,
        Key::J => Code::KeyJ,
        Key::K => Code::KeyK,
        Key::L => Code::KeyL,
        Key::M => Code::KeyM,
        Key::N => Code::KeyN,
        Key::O => Code::KeyO,
        Key::P => Code::KeyP,
        Key::Q => Code::KeyQ,
        Key::R => Code::KeyR,
        Key::S => Code::KeyS,
        Key::T => Code::KeyT,
        Key::U => Code::KeyU,
        Key::V => Code::KeyV,
        Key::W => Code::KeyW,
        Key::X => Code::KeyX,
        Key::Y => Code::KeyY,
        Key::Z => Code::KeyZ,
        Key::Num0 => Code::Digit0,
        Key::Num1 => Code::Digit1,
        Key::Num2 => Code::Digit2,
        Key::Num3 => Code::Digit3,
        Key::Num4 => Code::Digit4,
        Key::Num5 => Code::Digit5,
        Key::Num6 => Code::Digit6,
        Key::Num7 => Code::Digit7,
        Key::Num8 => Code::Digit8,
        Key::Num9 => Code::Digit9,
        Key::Backtick => Code::Backquote,
        Key::Backslash => Code::Backslash,
        Key::OpenBracket => Code::BracketLeft,
        Key::CloseBracket => Code::BracketRight,
        Key::Comma => Code::Comma,
        Key::Equals => Code::Equal,
        Key::Minus => Code::Minus,
        Key::Period => Code::Period,
        Key::Quote => Code::Quote,
        Key::Semicolon => Code::Semicolon,
        Key::Slash => Code::Slash,
        Key::Space => Code::Space,
        Key::Tab => Code::Tab,
        Key::Enter => Code::Enter,
        Key::Escape => Code::Escape,
        Key::Backspace => Code::Backspace,
        Key::Delete => Code::Delete,
        Key::Insert => Code::Insert,
        Key::Home => Code::Home,
        Key::End => Code::End,
        Key::PageUp => Code::PageUp,
        Key::PageDown => Code::PageDown,
        Key::ArrowUp => Code::ArrowUp,
        Key::ArrowDown => Code::ArrowDown,
        Key::ArrowLeft => Code::ArrowLeft,
        Key::ArrowRight => Code::ArrowRight,
        Key::F1 => Code::F1,
        Key::F2 => Code::F2,
        Key::F3 => Code::F3,
        Key::F4 => Code::F4,
        Key::F5 => Code::F5,
        Key::F6 => Code::F6,
        Key::F7 => Code::F7,
        Key::F8 => Code::F8,
        Key::F9 => Code::F9,
        Key::F10 => Code::F10,
        Key::F11 => Code::F11,
        Key::F12 => Code::F12,
        _ => return None,
    })
}

/// The key equivalent an item gets, if any.
fn item_accelerator<C>(item: &MenuItem<C>) -> Option<Accelerator> {
    item.shortcut
        .filter(|_| item.accelerator)
        .and_then(accelerator)
}

/// muda reads `&` as a mnemonic and drops it, and a recent file's name can have one.
fn native_label(label: &str) -> String {
    label.replace('&', "&&")
}

#[derive(Debug)]
struct TableEntry<C> {
    command: C,
    text_edit: Option<TextEditAction>,
}

/// Generated menu ids and the commands behind them.
#[derive(Debug)]
struct CommandTable<C> {
    entries: HashMap<MenuId, TableEntry<C>>,
    next: u64,
}

impl<C> Default for CommandTable<C> {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            next: 0,
        }
    }
}

impl<C> CommandTable<C> {
    fn insert(&mut self, command: C, text_edit: Option<TextEditAction>) -> MenuId {
        let id = MenuId::new(format!("egui_mev_commands::menu::{}", self.next));
        self.next += 1;
        self.entries
            .insert(id.clone(), TableEntry { command, text_edit });
        id
    }

    fn remove(&mut self, id: &MenuId) {
        self.entries.remove(id);
    }
}

#[derive(Debug)]
struct Shared<C> {
    table: RwLock<CommandTable<C>>,
    text_events: Mutex<Vec<Event>>,
}

/// The part of the bar that any thread can hold: from menu ids to commands, and
/// the events waiting for a focused text field. For an app that reads
/// `MenuEvent::receiver()` itself, such as one sharing it with a tray.
#[derive(Debug)]
pub struct MenuCommands<C> {
    shared: Arc<Shared<C>>,
}

impl<C> Clone for MenuCommands<C> {
    fn clone(&self) -> Self {
        Self {
            shared: Arc::clone(&self.shared),
        }
    }
}

impl<C> Default for MenuCommands<C> {
    fn default() -> Self {
        Self {
            shared: Arc::new(Shared {
                table: RwLock::new(CommandTable::default()),
                text_events: Mutex::new(Vec::new()),
            }),
        }
    }
}

impl<C> MenuCommands<C> {
    fn insert(&self, command: C, text_edit: Option<TextEditAction>) -> MenuId {
        self.shared
            .table
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(command, text_edit)
    }

    fn remove(&self, id: &MenuId) {
        self.shared
            .table
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(id);
    }

    /// Events for the focused text field, to feed into the next frame's input.
    #[must_use]
    pub fn drain_text_events(&self) -> Vec<Event> {
        std::mem::take(
            &mut self
                .shared
                .text_events
                .lock()
                .unwrap_or_else(PoisonError::into_inner),
        )
    }
}

impl<C: Clone> MenuCommands<C> {
    /// The command of the menu item `id`, if it is one of this bar's.
    #[must_use]
    pub fn command_for(&self, id: &MenuId) -> Option<C> {
        self.shared
            .table
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .entries
            .get(id)
            .map(|entry| entry.command.clone())
    }

    /// What to do about a click on the item `id`, with the text-field bridge: while
    /// a text field has focus a [text-edit](MenuItem::text_edit) item queues events
    /// for [`Self::drain_text_events`] and returns `None`, and otherwise this is
    /// [`Self::command_for`]. Asks for a frame when it queues anything.
    #[must_use]
    pub fn handle_event(&self, ctx: &Context, id: &MenuId) -> Option<C> {
        self.handle_event_with(ctx, id, read_clipboard)
    }

    fn handle_event_with(
        &self,
        ctx: &Context,
        id: &MenuId,
        clipboard: impl FnOnce() -> Option<String>,
    ) -> Option<C> {
        let (command, text_edit) = {
            let table = self
                .shared
                .table
                .read()
                .unwrap_or_else(PoisonError::into_inner);
            let entry = table.entries.get(id)?;
            (entry.command.clone(), entry.text_edit)
        };
        match text_edit.filter(|_| ctx.text_edit_focused()) {
            Some(action) => {
                self.shared
                    .text_events
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .extend(action.events(clipboard));
                ctx.request_repaint();
                None
            }
            None => Some(command),
        }
    }
}

/// The clipboard's text. Nothing to paste when it can't be read.
fn read_clipboard() -> Option<String> {
    arboard::Clipboard::new()
        .and_then(|mut clipboard| clipboard.get_text())
        .ok()
}

/// A native item, one kind or the other.
#[derive(Clone)]
enum ItemWidget {
    Plain(NativeItem),
    Check(CheckMenuItem),
}

impl ItemWidget {
    fn as_dyn(&self) -> &dyn IsMenuItem {
        match self {
            Self::Plain(item) => item,
            Self::Check(item) => item,
        }
    }

    fn id(&self) -> &MenuId {
        match self {
            Self::Plain(item) => item.id(),
            Self::Check(item) => item.id(),
        }
    }

    fn sync_enabled(&self, enabled: bool) {
        match self {
            Self::Plain(item) if item.is_enabled() != enabled => item.set_enabled(enabled),
            Self::Check(item) if item.is_enabled() != enabled => item.set_enabled(enabled),
            Self::Plain(_) | Self::Check(_) => {}
        }
    }

    /// muda hands back text without the `&&` of [`native_label`].
    fn sync_text(&self, text: &str) {
        match self {
            Self::Plain(item) if item.text() != text => item.set_text(native_label(text)),
            Self::Check(item) if item.text() != text => item.set_text(native_label(text)),
            Self::Plain(_) | Self::Check(_) => {}
        }
    }

    fn sync_checked(&self, checked: bool) {
        if let Self::Check(item) = self
            && item.is_checked() != checked
        {
            item.set_checked(checked);
        }
    }
}

/// A command item on the menu, kept for [`NativeMenuBar::set_availability`].
struct ShownItem<C> {
    command: C,
    label: String,
    widget: ItemWidget,
    /// The shortcut, when it became a key equivalent.
    key_equivalent: Option<KeyboardShortcut>,
}

/// Where [`NativeMenuBar::set_section`] puts entries.
struct Slot<C> {
    name: &'static str,
    parent: Submenu,
    /// How many of the parent's entries are always before it. Other slots in the
    /// same parent come on top of this.
    fixed_before: usize,
    /// How many native entries it shows directly in `parent`.
    len: usize,
    /// Every command item in it, nested ones too.
    items: Vec<ShownItem<C>>,
}

/// Something that can be put in a submenu.
enum Native {
    Item(ItemWidget),
    Disabled(NativeItem),
    Separator(PredefinedMenuItem),
    Submenu(Submenu),
}

impl Native {
    fn as_dyn(&self) -> &dyn IsMenuItem {
        match self {
            Self::Item(item) => item.as_dyn(),
            Self::Disabled(item) => item,
            Self::Separator(item) => item,
            Self::Submenu(item) => item,
        }
    }
}

/// `entries` without the commands the application menu already has, and without
/// the separators and empty submenus that leaves behind.
fn tidy_entries<C: Clone + PartialEq>(entries: &[MenuEntry<C>], skip: &[C]) -> Vec<MenuEntry<C>> {
    let mut out: Vec<MenuEntry<C>> = Vec::with_capacity(entries.len());
    for entry in entries {
        match entry {
            MenuEntry::Item(item) if skip.contains(&item.command) => {}
            MenuEntry::Separator => {
                if !matches!(out.last(), None | Some(MenuEntry::Separator)) {
                    out.push(MenuEntry::Separator);
                }
            }
            MenuEntry::Submenu { label, entries } => {
                let entries = tidy_entries(entries, skip);
                if !entries.is_empty() {
                    out.push(MenuEntry::Submenu {
                        label: label.clone(),
                        entries,
                    });
                }
            }
            other => out.push(other.clone()),
        }
    }
    if matches!(out.last(), Some(MenuEntry::Separator)) {
        out.pop();
    }
    out
}

/// What building a menu collects.
struct Build<'a, C> {
    commands: &'a MenuCommands<C>,
    items: &'a mut Vec<ShownItem<C>>,
}

/// Make the native item for `item`.
fn build_item<C: Clone>(item: &MenuItem<C>, cx: &mut Build<'_, C>) -> ItemWidget {
    let accelerator = item_accelerator(item);
    let key_equivalent = accelerator.as_ref().and(item.shortcut);
    let id = cx.commands.insert(item.command.clone(), item.text_edit);
    let label = native_label(&item.label);
    let widget = if item.checkable {
        ItemWidget::Check(CheckMenuItem::with_id(id, &label, true, false, accelerator))
    } else {
        ItemWidget::Plain(NativeItem::with_id(id, &label, true, accelerator))
    };
    cx.items.push(ShownItem {
        command: item.command.clone(),
        label: item.label.to_string(),
        widget: widget.clone(),
        key_equivalent,
    });
    widget
}

/// Make the native entry for `entry`, or nothing for a slot, which is the caller's
/// to handle.
fn build_entry<C: Clone>(
    entry: &MenuEntry<C>,
    cx: &mut Build<'_, C>,
) -> Result<Option<Native>, muda::Error> {
    Ok(Some(match entry {
        MenuEntry::Item(item) => Native::Item(build_item(item, cx)),
        MenuEntry::Disabled { label } => {
            Native::Disabled(NativeItem::new(native_label(label), false, None))
        }
        MenuEntry::Separator => Native::Separator(PredefinedMenuItem::separator()),
        MenuEntry::Submenu { label, entries } => {
            let child = Submenu::new(native_label(label), true);
            for entry in entries {
                // Slots only count at the top of a menu or a table's submenu, which
                // `append_entries` handles.
                if let Some(native) = build_entry(entry, cx)? {
                    child.append(native.as_dyn())?;
                }
            }
            Native::Submenu(child)
        }
        MenuEntry::Section(_) => return Ok(None),
    }))
}

/// Append `entries` of the menu table to `parent`, noting the slots.
fn append_entries<C: Clone>(
    parent: &Submenu,
    entries: &[MenuEntry<C>],
    commands: &MenuCommands<C>,
    items: &mut Vec<ShownItem<C>>,
    slots: &mut Vec<Slot<C>>,
) -> Result<(), muda::Error> {
    for entry in entries {
        match entry {
            MenuEntry::Section(name) => slots.push(Slot {
                name,
                parent: parent.clone(),
                fixed_before: parent.items().len(),
                len: 0,
                items: Vec::new(),
            }),
            MenuEntry::Submenu { label, entries } => {
                let child = Submenu::new(native_label(label), true);
                append_entries(&child, entries, commands, items, slots)?;
                parent.append(&child)?;
            }
            other => {
                let mut cx = Build { commands, items };
                if let Some(native) = build_entry(other, &mut cx)? {
                    parent.append(native.as_dyn())?;
                }
            }
        }
    }
    Ok(())
}

/// Keeps the native menu alive for the lifetime of the app.
pub struct NativeMenuBar<C> {
    menu: NativeMenu,
    commands: MenuCommands<C>,
    items: Vec<ShownItem<C>>,
    slots: Vec<Slot<C>>,
}

impl<C: Clone + PartialEq> NativeMenuBar<C> {
    /// Install `menus` as the application's menu bar. Must run on the main thread
    /// after `NSApp` exists, so from the eframe app creator. Items with the app's
    /// About, Settings or Quit command are left out of the table's menus, since
    /// the application menu has them, and so are menus that leaves empty.
    ///
    /// Nothing listens yet: call [`Self::install_event_handler`], or read
    /// `MenuEvent::receiver()` yourself and use [`Self::commands`].
    ///
    /// # Errors
    /// When muda refuses a menu item.
    ///
    /// # Panics
    /// Off the main thread, in muda.
    pub fn install(options: &NativeMenuOptions<C>, menus: &[Menu<C>]) -> Result<Self, muda::Error> {
        let commands = MenuCommands::default();
        let menu = NativeMenu::new();
        let mut items = Vec::new();
        let mut slots = Vec::new();

        menu.append(&app_menu(options, &commands, &mut items)?)?;

        let skip: Vec<C> = [
            Some(&options.quit),
            options.about.as_ref(),
            options.settings.as_ref(),
        ]
        .into_iter()
        .flatten()
        .cloned()
        .collect();
        let close = KeyboardShortcut::new(egui::Modifiers::COMMAND, Key::W);
        let table_has_close = Keymap::from_menus(menus)
            .bindings()
            .iter()
            .any(|binding| binding.shortcut == close);
        let mut window = None;
        for top in menus {
            let entries = tidy_entries(&top.entries, &skip);
            let is_window = *top.label == *options.window_menu;
            if entries.is_empty() && !is_window {
                continue;
            }
            let submenu = Submenu::new(native_label(&top.label), true);
            if is_window {
                append_window_items(&submenu, table_has_close)?;
                if !entries.is_empty() {
                    submenu.append(&PredefinedMenuItem::separator())?;
                }
                window = Some(submenu.clone());
            }
            append_entries(&submenu, &entries, &commands, &mut items, &mut slots)?;
            menu.append(&submenu)?;
        }
        let window = match window {
            Some(window) => window,
            None => {
                let submenu = Submenu::new(native_label(&options.window_menu), true);
                append_window_items(&submenu, table_has_close)?;
                menu.append(&submenu)?;
                submenu
            }
        };
        window.set_as_windows_menu_for_nsapp();
        menu.init_for_nsapp();

        Ok(Self {
            menu,
            commands,
            items,
            slots,
        })
    }
}

impl<C: Clone> NativeMenuBar<C> {
    /// The native menu itself, for what this type doesn't cover.
    #[must_use]
    pub const fn menu(&self) -> &NativeMenu {
        &self.menu
    }

    /// A handle any thread can hold, for an app reading `MenuEvent::receiver()`.
    #[must_use]
    pub fn commands(&self) -> MenuCommands<C> {
        self.commands.clone()
    }

    /// The command of the menu item `id`, if it is one of this bar's.
    #[must_use]
    pub fn command_for(&self, id: &MenuId) -> Option<C> {
        self.commands.command_for(id)
    }

    /// Events for the focused text field, to feed into the next frame's input from
    /// `eframe::App::raw_input_hook`.
    #[must_use]
    pub fn drain_text_events(&self) -> Vec<Event> {
        self.commands.drain_text_events()
    }

    /// Whether a key equivalent on this bar has `shortcut`, so AppKit takes the
    /// keys before egui. Key equivalents of disabled items aren't taken, so a
    /// keymap should leave these out to keep those from running.
    #[must_use]
    pub fn handles(&self, shortcut: &KeyboardShortcut) -> bool {
        self.all_items()
            .any(|item| item.key_equivalent.as_ref() == Some(shortcut))
    }

    fn all_items(&self) -> impl Iterator<Item = &ShownItem<C>> {
        self.items
            .iter()
            .chain(self.slots.iter().flat_map(|slot| &slot.items))
    }

    /// Grey out, relabel and check items for what the app can do now. Cheap to call
    /// every frame: only changes reach AppKit. A click on a check item flips it
    /// natively, so this also puts it back when the app says otherwise.
    pub fn set_availability(&self, available: &dyn Availability<C>) {
        for item in self.all_items() {
            item.widget.sync_enabled(available.enabled(&item.command));
            let label = available.label(&item.command);
            item.widget
                .sync_text(label.as_deref().unwrap_or(item.label.as_str()));
            item.widget.sync_checked(available.checked(&item.command));
        }
    }

    /// Show `entries` in the [`MenuEntry::Section`] slot named `slot`, instead of
    /// what it showed. Their items get fresh ids, and the old ones stop mapping to
    /// commands. Returns whether the menu table has that slot.
    ///
    /// # Errors
    /// When muda refuses a menu item.
    pub fn set_section(
        &mut self,
        slot: &str,
        entries: &[MenuEntry<C>],
    ) -> Result<bool, muda::Error> {
        let Some(index) = self.slots.iter().position(|shown| shown.name == slot) else {
            return Ok(false);
        };
        let start = section_start(&self.slots, index);
        let shown = &mut self.slots[index];
        for _ in 0..shown.len {
            shown.parent.remove_at(start);
        }
        shown.len = 0;
        for item in shown.items.drain(..) {
            self.commands.remove(item.widget.id());
        }
        let mut cx = Build {
            commands: &self.commands,
            items: &mut shown.items,
        };
        for entry in entries {
            if let Some(native) = build_entry(entry, &mut cx)? {
                shown.parent.insert(native.as_dyn(), start + shown.len)?;
                shown.len += 1;
            }
        }
        Ok(true)
    }
}

/// The position of `slots[index]`'s first entry in its parent.
fn section_start<C>(slots: &[Slot<C>], index: usize) -> usize {
    let parent = slots[index].parent.id();
    slots[index].fixed_before
        + slots[..index]
            .iter()
            .filter(|earlier| earlier.parent.id() == parent)
            .map(|earlier| earlier.len)
            .sum::<usize>()
}

impl<C: Clone + Send + Sync + 'static> NativeMenuBar<C> {
    /// Listen for menu events with muda's handler: commands go on the
    /// [`CommandQueue`] and wake egui. muda keeps only the first handler set in a
    /// process, and setting one stops `MenuEvent::receiver()`. An app sharing that
    /// receiver with a tray icon should skip this and read it itself.
    pub fn install_event_handler(&self, ctx: &Context) {
        let queue = ctx.clone();
        self.install_event_handler_with(ctx, move |command| {
            CommandQueue::push(&queue, command);
        });
    }

    /// Like [`Self::install_event_handler`], handing commands to `on_command`.
    pub fn install_event_handler_with(
        &self,
        ctx: &Context,
        on_command: impl Fn(C) + Send + Sync + 'static,
    ) {
        let commands = self.commands();
        let ctx = ctx.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            if let Some(command) = commands.handle_event(&ctx, event.id()) {
                on_command(command);
            }
            ctx.request_repaint();
        }));
    }
}

/// The application menu: About, Settings, AppKit's own items and Quit.
fn app_menu<C: Clone>(
    options: &NativeMenuOptions<C>,
    commands: &MenuCommands<C>,
    items: &mut Vec<ShownItem<C>>,
) -> Result<Submenu, muda::Error> {
    let submenu = Submenu::new(native_label(&options.app_name), true);
    let mut cx = Build { commands, items };
    if let Some(about) = &options.about {
        let item = MenuItem::new(options.about_label.clone(), about.clone());
        submenu.append(build_item(&item, &mut cx).as_dyn())?;
    } else {
        submenu.append(&PredefinedMenuItem::about(
            Some(&native_label(&options.about_label)),
            None,
        ))?;
    }
    submenu.append(&PredefinedMenuItem::separator())?;
    if let Some(settings) = &options.settings {
        let mut item = MenuItem::new(options.settings_label.clone(), settings.clone());
        item.shortcut = options.settings_shortcut;
        submenu.append(build_item(&item, &mut cx).as_dyn())?;
        submenu.append(&PredefinedMenuItem::separator())?;
    }
    submenu.append_items(&[
        &PredefinedMenuItem::services(None),
        &PredefinedMenuItem::separator(),
        &PredefinedMenuItem::hide(None),
        &PredefinedMenuItem::hide_others(None),
        &PredefinedMenuItem::show_all(None),
        &PredefinedMenuItem::separator(),
    ])?;
    let mut quit = MenuItem::new(options.quit_label.clone(), options.quit.clone());
    quit.shortcut = options.quit_shortcut;
    submenu.append(build_item(&quit, &mut cx).as_dyn())?;
    Ok(submenu)
}

/// AppKit's window items. Close Window is left out when the table has its own ⌘W,
/// which two items can't share.
fn append_window_items(submenu: &Submenu, table_has_close: bool) -> Result<(), muda::Error> {
    submenu.append_items(&[
        &PredefinedMenuItem::minimize(None),
        &PredefinedMenuItem::maximize(Some("Zoom")),
        &PredefinedMenuItem::fullscreen(None),
    ])?;
    if !table_has_close {
        submenu.append_items(&[
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::close_window(None),
        ])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use egui::Modifiers as Mods;
    use egui_kittest::{Harness, kittest::Queryable as _};

    use super::*;

    const CMD: Mods = Mods::COMMAND;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Command {
        Save,
        Copy,
        Paste,
        Quit,
        About,
    }

    fn shortcut(modifiers: Mods, key: Key) -> KeyboardShortcut {
        KeyboardShortcut::new(modifiers, key)
    }

    #[test]
    fn modifiers_map_to_their_native_flags() {
        let native = accelerator(shortcut(CMD, Key::S)).expect("has modifiers");
        assert_eq!(native.modifiers(), Modifiers::META);
        assert_eq!(native.key(), Code::KeyS);
        let every = Mods {
            alt: true,
            ctrl: true,
            shift: true,
            mac_cmd: false,
            command: true,
        };
        assert_eq!(
            accelerator(shortcut(every, Key::Z))
                .expect("has modifiers")
                .modifiers(),
            Modifiers::META | Modifiers::CONTROL | Modifiers::SHIFT | Modifiers::ALT
        );
        let mac_press = Mods {
            mac_cmd: true,
            ..Mods::NONE
        };
        assert_eq!(
            accelerator(shortcut(mac_press, Key::Z))
                .expect("has modifiers")
                .modifiers(),
            Modifiers::META
        );
    }

    /// They would take keys away from a focused text field.
    #[test]
    fn plain_keys_never_become_key_equivalents() {
        for key in [Key::Delete, Key::Backspace, Key::F5, Key::A, Key::Num4] {
            assert_eq!(accelerator(shortcut(Mods::NONE, key)), None, "{key:?}");
            assert!(!has_key_equivalent(shortcut(Mods::NONE, key)));
        }
    }

    /// Everything with a key cap that can sit in a shortcut, so the table can't
    /// lose a key equivalent to a gap here.
    #[test]
    fn every_ordinary_key_has_a_code() {
        let no_code_on_purpose = |key: Key| {
            let name = key.name();
            matches!(
                key,
                // Reached through Shift, so no physical key of their own.
                Key::Plus
                    | Key::Colon
                    | Key::Pipe
                    | Key::Questionmark
                    | Key::Exclamationmark
                    | Key::OpenCurlyBracket
                    | Key::CloseCurlyBracket
                    | Key::Copy
                    | Key::Cut
                    | Key::Paste
            ) || name.starts_with("Browser")
                || name.starts_with("Media")
                || name.starts_with("Audio")
                || name.starts_with("Launch")
                // F13 and up: kept out with the rest of the rare keys.
                || (name.starts_with('F') && name[1..].parse::<u8>().is_ok_and(|n| n > 12))
                || [
                    Key::ShiftLeft,
                    Key::ShiftRight,
                    Key::ControlLeft,
                    Key::ControlRight,
                    Key::AltLeft,
                    Key::AltRight,
                    Key::SuperLeft,
                    Key::SuperRight,
                    Key::IntlBackslash,
                    Key::CapsLock,
                    Key::NumLock,
                    Key::ScrollLock,
                    Key::PrintScreen,
                    Key::Pause,
                    Key::Menu,
                    Key::Fn,
                    Key::Eject,
                    Key::Help,
                    Key::Power,
                    Key::Sleep,
                    Key::Clear,
                ]
                .contains(&key)
        };
        for &key in Key::ALL {
            if !no_code_on_purpose(key) {
                assert!(
                    has_key_equivalent(shortcut(CMD, key)),
                    "{key:?} has no code"
                );
            }
        }
    }

    #[test]
    fn an_item_can_opt_out_of_its_key_equivalent() {
        let item = MenuItem::new("Copy", Command::Copy).shortcut(shortcut(CMD, Key::C));
        assert!(item_accelerator(&item).is_some());
        assert_eq!(item_accelerator(&item.no_accelerator()), None);
        assert_eq!(
            item_accelerator(&MenuItem::new("Copy", Command::Copy)),
            None
        );
    }

    #[test]
    fn ids_are_generated_and_map_to_commands() {
        let commands = MenuCommands::default();
        let save = commands.insert(Command::Save, None);
        // Two items can run one command, each with its own id.
        let save_again = commands.insert(Command::Save, None);
        let copy = commands.insert(Command::Copy, Some(TextEditAction::Copy));
        assert_ne!(save, save_again);
        assert_eq!(commands.command_for(&save), Some(Command::Save));
        assert_eq!(commands.command_for(&save_again), Some(Command::Save));
        assert_eq!(commands.command_for(&copy), Some(Command::Copy));
        // Nothing in an id comes from the command, so nothing parses it back.
        assert!(!save.as_ref().contains("Save"), "{}", save.as_ref());
        assert_eq!(commands.command_for(&MenuId::new("someone else's")), None);

        commands.remove(&save);
        assert_eq!(commands.command_for(&save), None);
        assert_eq!(commands.command_for(&save_again), Some(Command::Save));
    }

    #[test]
    fn a_handle_works_from_another_thread() {
        let commands = MenuCommands::default();
        let id = commands.insert(Command::Save, None);
        let handle = commands.clone();
        let found = std::thread::spawn(move || handle.command_for(&id))
            .join()
            .expect("the thread finishes");
        assert_eq!(found, Some(Command::Save));
    }

    struct Bridge {
        commands: MenuCommands<Command>,
        copy: MenuId,
        paste: MenuId,
        save: MenuId,
    }

    fn bridge() -> Bridge {
        let commands = MenuCommands::default();
        Bridge {
            copy: commands.insert(Command::Copy, Some(TextEditAction::Copy)),
            paste: commands.insert(Command::Paste, Some(TextEditAction::Paste)),
            save: commands.insert(Command::Save, None),
            commands,
        }
    }

    fn field_harness(focus: bool) -> Harness<'static, String> {
        let mut harness = Harness::new_ui_state(
            |ui, text: &mut String| {
                let label = ui.label("Name");
                ui.text_edit_singleline(text).labelled_by(label.id);
            },
            String::new(),
        );
        harness.run();
        if focus {
            harness
                .get_by_role(egui::accesskit::Role::TextInput)
                .focus();
            harness.run();
        }
        harness
    }

    #[test]
    fn without_a_focused_field_edit_items_run_as_commands() {
        let bridge = bridge();
        let harness = field_harness(false);
        assert_eq!(
            bridge.commands.handle_event(&harness.ctx, &bridge.copy),
            Some(Command::Copy)
        );
        assert_eq!(bridge.commands.drain_text_events(), Vec::new());
    }

    #[test]
    fn a_focused_field_gets_edit_items_as_events() {
        let bridge = bridge();
        let harness = field_harness(true);
        assert_eq!(
            bridge.commands.handle_event(&harness.ctx, &bridge.copy),
            None
        );
        assert_eq!(bridge.commands.drain_text_events(), vec![Event::Copy]);
        assert_eq!(bridge.commands.drain_text_events(), Vec::new(), "drained");
    }

    #[test]
    fn other_items_stay_commands_while_typing() {
        let bridge = bridge();
        let harness = field_harness(true);
        assert_eq!(
            bridge.commands.handle_event(&harness.ctx, &bridge.save),
            Some(Command::Save)
        );
        assert_eq!(bridge.commands.drain_text_events(), Vec::new());
    }

    #[test]
    fn paste_reads_the_clipboard_and_reaches_the_field() {
        let bridge = bridge();
        let mut harness = field_harness(true);
        let command = bridge
            .commands
            .handle_event_with(&harness.ctx, &bridge.paste, || {
                Some("from the clipboard".to_owned())
            });
        assert_eq!(command, None);
        for event in bridge.commands.drain_text_events() {
            harness.event(event);
        }
        harness.run();
        assert_eq!(harness.state(), "from the clipboard");
    }

    #[test]
    fn unknown_ids_do_nothing() {
        let bridge = bridge();
        let harness = field_harness(true);
        let id = MenuId::new("tray::show");
        assert_eq!(bridge.commands.handle_event(&harness.ctx, &id), None);
        assert_eq!(bridge.commands.drain_text_events(), Vec::new());
    }

    #[test]
    fn labels_keep_their_ampersands() {
        assert_eq!(native_label("R&D.png"), "R&&D.png");
        assert_eq!(native_label("Plain"), "Plain");
    }

    #[test]
    fn the_app_menus_commands_leave_the_table_and_no_stray_separators_stay() {
        let skip = [Command::Quit, Command::About];
        let entries = vec![
            MenuEntry::Separator,
            MenuItem::new("Save", Command::Save).into(),
            MenuEntry::Separator,
            MenuItem::new("Exit", Command::Quit).into(),
            MenuEntry::Separator,
            MenuEntry::Section("recent"),
            MenuEntry::Separator,
            MenuEntry::submenu(
                "Only About",
                vec![MenuItem::new("About", Command::About).into()],
            ),
            MenuEntry::Separator,
        ];
        assert_eq!(
            tidy_entries(&entries, &skip),
            vec![
                MenuItem::new("Save", Command::Save).into(),
                MenuEntry::Separator,
                MenuEntry::Section("recent"),
            ]
        );
    }

    #[test]
    fn options_name_the_app_in_their_labels_and_have_the_usual_keys() {
        let options = NativeMenuOptions::new("Mazak", Command::Quit).about(Command::About);
        assert_eq!(options.about_label, "About Mazak");
        assert_eq!(options.quit_label, "Quit Mazak");
        assert_eq!(options.quit_shortcut, Some(shortcut(CMD, Key::Q)));
        assert_eq!(options.settings, None);
        assert_eq!(options.about, Some(Command::About));
    }
}
