//! The egui-drawn menu bar, for platforms without a native one.

use egui::{Button, RichText, Ui};
use egui_mev_theme::Palette;

use crate::{
    menu::{Availability, Menu, MenuEntry, MenuItem, Sections},
    queue::CommandQueue,
};

/// Draw `menus` as a horizontal menu bar. Clicks go on the [`CommandQueue`],
/// `available` greys out items that wouldn't do anything, and `sections` fills the
/// [`MenuEntry::Section`] slots.
pub fn menu_bar<C: Clone + Send + Sync + 'static>(
    ui: &mut Ui,
    menus: &[Menu<C>],
    available: &dyn Availability<C>,
    sections: &Sections<C>,
) {
    egui::MenuBar::new().ui(ui, |ui| {
        for menu in menus {
            ui.menu_button(menu.label.as_ref(), |ui| {
                show_entries(ui, &menu.entries, available, Some(sections));
            });
        }
    });
}

/// `sections` is `None` inside a section, where slots don't count.
fn show_entries<C: Clone + Send + Sync + 'static>(
    ui: &mut Ui,
    entries: &[MenuEntry<C>],
    available: &dyn Availability<C>,
    sections: Option<&Sections<C>>,
) {
    for entry in entries {
        match entry {
            MenuEntry::Item(item) => show_item(ui, item, available),
            MenuEntry::Disabled { label } => {
                ui.add_enabled(false, Button::new(label.as_ref()));
            }
            MenuEntry::Separator => {
                ui.separator();
            }
            MenuEntry::Submenu { label, entries } => {
                ui.menu_button(label.as_ref(), |ui| {
                    show_entries(ui, entries, available, sections);
                });
            }
            MenuEntry::Section(slot) => {
                if let Some(sections) = sections {
                    show_entries(ui, sections.get(slot), available, None);
                }
            }
        }
    }
}

/// An on/off menu entry. egui's selected text is the accent on an accent fill,
/// so it gets the palette's text colour.
pub(crate) fn check_button<'a>(ui: &Ui, checked: bool, label: &str) -> Button<'a> {
    Button::selectable(checked, RichText::new(label).color(Palette::of(ui).text))
}

fn show_item<C: Clone + Send + Sync + 'static>(
    ui: &mut Ui,
    item: &MenuItem<C>,
    available: &dyn Availability<C>,
) {
    let label = available.label(&item.command);
    let label = label.as_deref().unwrap_or(item.label.as_ref());
    let mut button = if item.checkable {
        check_button(ui, available.checked(&item.command), label)
    } else {
        Button::new(label)
    };
    if let Some(shortcut) = &item.shortcut {
        button = button.shortcut_text(ui.ctx().format_shortcut(shortcut));
    }
    if ui
        .add_enabled(available.enabled(&item.command), button)
        .clicked()
    {
        CommandQueue::push(ui.ctx(), item.command.clone());
        ui.close();
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use egui::{Key, KeyboardShortcut, Modifiers};
    use egui_kittest::{
        Harness,
        kittest::{NodeT as _, Queryable as _},
    };

    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Command {
        Undo,
        Redo,
        Grid,
        Open(usize),
        Clear,
    }

    struct Now {
        can_redo: bool,
        grid: bool,
    }

    impl Availability<Command> for Now {
        fn enabled(&self, command: &Command) -> bool {
            *command != Command::Redo || self.can_redo
        }

        fn label(&self, command: &Command) -> Option<Cow<'_, str>> {
            (*command == Command::Undo).then_some(Cow::Borrowed("Undo Rename"))
        }

        fn checked(&self, command: &Command) -> bool {
            *command == Command::Grid && self.grid
        }
    }

    fn menus() -> Vec<Menu<Command>> {
        vec![
            Menu::new(
                "File",
                vec![MenuEntry::submenu(
                    "Open Recent",
                    vec![MenuEntry::Section("recent")],
                )],
            ),
            Menu::new(
                "Edit",
                vec![
                    MenuItem::new("Undo", Command::Undo)
                        .shortcut(KeyboardShortcut::new(Modifiers::COMMAND, Key::Z))
                        .into(),
                    MenuItem::new("Redo", Command::Redo).into(),
                    MenuEntry::Separator,
                    MenuItem::new("Pixel Grid", Command::Grid)
                        .checkable()
                        .into(),
                    MenuEntry::disabled("Print"),
                ],
            ),
        ]
    }

    fn sections() -> Sections<Command> {
        let mut sections = Sections::new();
        sections.set(
            "recent",
            vec![
                MenuItem::new("/tmp/a.png", Command::Open(0)).into(),
                MenuItem::new("/tmp/b.png", Command::Open(1)).into(),
                MenuEntry::Separator,
                MenuItem::new("Clear List", Command::Clear).into(),
                // Ignored: slots don't nest.
                MenuEntry::Section("recent"),
            ],
        );
        sections
    }

    fn harness(now: Now) -> Harness<'static> {
        let (menus, sections) = (menus(), sections());
        Harness::new_ui(move |ui| menu_bar(ui, &menus, &now, &sections))
    }

    #[test]
    fn items_and_sections_queue_commands() {
        let mut harness = harness(Now {
            can_redo: true,
            grid: false,
        });
        harness.get_by_label("Edit").click();
        harness.run_steps(3);
        // Labels include the shortcut text ("Undo Rename Ctrl+Z").
        harness.get_by_label_contains("Undo Rename").click();
        harness.run();
        assert_eq!(
            CommandQueue::<Command>::drain(&harness.ctx),
            vec![Command::Undo]
        );
        assert!(
            harness.query_by_label_contains("Pixel Grid").is_none(),
            "a click closes the menu"
        );

        harness.get_by_label("File").click();
        harness.run_steps(3);
        harness.get_by_label_contains("Open Recent").hover();
        harness.run_steps(3);
        harness.get_by_label_contains("/tmp/b.png").click();
        harness.run();
        assert_eq!(
            CommandQueue::<Command>::drain(&harness.ctx),
            vec![Command::Open(1)]
        );
    }

    #[test]
    fn unavailable_items_do_nothing() {
        let mut harness = harness(Now {
            can_redo: false,
            grid: false,
        });
        harness.get_by_label("Edit").click();
        harness.run_steps(3);
        assert!(harness.get_by_label("Redo").accesskit_node().is_disabled());
        assert!(harness.get_by_label("Print").accesskit_node().is_disabled());
        harness.get_by_label("Redo").click();
        harness.run();
        assert_eq!(CommandQueue::<Command>::drain(&harness.ctx), Vec::new());
    }

    #[test]
    fn checkable_items_show_their_state() {
        let selected = |grid| {
            let mut harness = harness(Now {
                can_redo: true,
                grid,
            });
            harness.get_by_label("Edit").click();
            harness.run_steps(3);
            harness
                .get_by_label("Pixel Grid")
                .accesskit_node()
                .toggled()
                == Some(egui::accesskit::Toggled::True)
        };
        assert!(selected(true));
        assert!(!selected(false));
    }
}
