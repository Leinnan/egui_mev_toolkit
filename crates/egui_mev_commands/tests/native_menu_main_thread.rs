//! The real muda objects, on the main thread AppKit insists on, which libtest
//! doesn't use. Run with its own `main`, and answering just enough of libtest's
//! command line for nextest to find and run it.
//!
//! It builds menus and reads them back, but sends no clicks and needs no window.

#[cfg(target_os = "macos")]
mod real {
    use std::borrow::Cow;

    use egui::{Key, KeyboardShortcut, Modifiers};
    use egui_mev_commands::{
        Availability, Keymap, Menu, MenuEntry, MenuItem, NativeMenuBar, NativeMenuOptions,
        TextEditAction,
    };
    use muda::{MenuId, MenuItemKind, Submenu};

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Command {
        Save,
        Close,
        Copy,
        Delete,
        Grid,
        Open(usize),
        Clear,
        Plugin(usize),
        About,
        Settings,
        Quit,
    }

    const CMD: Modifiers = Modifiers::COMMAND;

    fn table() -> Vec<Menu<Command>> {
        vec![
            Menu::new(
                "File",
                vec![
                    MenuItem::new("Save", Command::Save)
                        .shortcut(KeyboardShortcut::new(CMD, Key::S))
                        .into(),
                    MenuEntry::submenu("Open Recent", vec![MenuEntry::Section("recent")]),
                    MenuEntry::Section("plugins_first"),
                    MenuEntry::Section("plugins_second"),
                    MenuItem::new("Close", Command::Close)
                        .shortcut(KeyboardShortcut::new(CMD, Key::W))
                        .into(),
                    MenuEntry::Separator,
                    MenuItem::new("Quit", Command::Quit)
                        .shortcut(KeyboardShortcut::new(CMD, Key::Q))
                        .into(),
                ],
            ),
            Menu::new(
                "Edit",
                vec![
                    MenuItem::new("Copy", Command::Copy)
                        .shortcut(KeyboardShortcut::new(CMD, Key::C))
                        .text_edit(TextEditAction::Copy)
                        .no_accelerator()
                        .into(),
                    MenuItem::new("Delete", Command::Delete)
                        .shortcut(KeyboardShortcut::new(Modifiers::NONE, Key::Delete))
                        .into(),
                ],
            ),
            Menu::new(
                "View",
                vec![MenuItem::new("Grid", Command::Grid).checkable().into()],
            ),
            // Only the app menu's own command: this menu goes.
            Menu::new("Help", vec![MenuItem::new("About", Command::About).into()]),
        ]
    }

    fn options() -> NativeMenuOptions<Command> {
        NativeMenuOptions::new("Example & Co", Command::Quit)
            .about(Command::About)
            .settings(Command::Settings)
    }

    fn submenu(kinds: &[MenuItemKind], title: &str) -> Submenu {
        kinds
            .iter()
            .find_map(|kind| match kind {
                MenuItemKind::Submenu(submenu) if submenu.text() == title => Some(submenu.clone()),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no submenu {title}"))
    }

    fn text(kind: &MenuItemKind) -> String {
        match kind {
            MenuItemKind::MenuItem(item) => item.text(),
            MenuItemKind::Check(item) => item.text(),
            MenuItemKind::Submenu(item) => item.text(),
            MenuItemKind::Predefined(item) => item.text(),
            MenuItemKind::Icon(item) => item.text(),
        }
    }

    fn texts(submenu: &Submenu) -> Vec<String> {
        submenu.items().iter().map(text).collect()
    }

    fn id_of(submenu: &Submenu, label: &str) -> MenuId {
        submenu
            .items()
            .iter()
            .find(|kind| text(kind) == label)
            .unwrap_or_else(|| panic!("no item {label}"))
            .id()
            .clone()
    }

    struct Now;

    impl Availability<Command> for Now {
        fn enabled(&self, command: &Command) -> bool {
            *command != Command::Save
        }

        fn label(&self, command: &Command) -> Option<Cow<'_, str>> {
            (*command == Command::Close).then_some(Cow::Borrowed("Close R&D"))
        }

        fn checked(&self, command: &Command) -> bool {
            *command == Command::Grid
        }
    }

    pub fn run() -> Result<(), muda::Error> {
        let table = table();
        let mut bar = NativeMenuBar::install(&options(), &table)?;
        let top = bar.menu().items();

        let titles: Vec<String> = top.iter().map(text).collect();
        assert_eq!(titles, ["Example & Co", "File", "Edit", "View", "Window"]);

        let app = submenu(&top, "Example & Co");
        let app_items = texts(&app);
        assert_eq!(app_items[0], "About Example & Co");
        assert!(app_items.iter().any(|item| item == "Settings…"));
        assert_eq!(app_items.last().unwrap(), "Quit Example & Co");
        for (label, command) in [
            ("About Example & Co", Command::About),
            ("Settings…", Command::Settings),
            ("Quit Example & Co", Command::Quit),
        ] {
            assert_eq!(
                bar.command_for(&id_of(&app, label)),
                Some(command),
                "{label}"
            );
        }

        let file = submenu(&top, "File");
        assert_eq!(texts(&file), ["Save", "Open Recent", "Close"], "Quit moved");
        assert_eq!(bar.command_for(&id_of(&file, "Save")), Some(Command::Save));

        // The table has its own ⌘W, so AppKit's Close Window stays out.
        let window = submenu(&top, "Window");
        assert_eq!(window.items().len(), 3, "{:?}", texts(&window));

        // Key equivalents: not for plain keys, not for an item opting out.
        assert!(bar.handles(&KeyboardShortcut::new(CMD, Key::S)));
        assert!(bar.handles(&KeyboardShortcut::new(CMD, Key::Q)));
        assert!(bar.handles(&KeyboardShortcut::new(CMD, Key::Comma)));
        assert!(!bar.handles(&KeyboardShortcut::new(CMD, Key::C)));
        assert!(!bar.handles(&KeyboardShortcut::new(Modifiers::NONE, Key::Delete)));
        let mut keymap = Keymap::from_menus(&table);
        keymap.retain(|binding| !bar.handles(&binding.shortcut));
        let left: Vec<Command> = keymap.bindings().iter().map(|b| b.command).collect();
        assert_eq!(left, [Command::Copy, Command::Delete]);

        // Sections, replacing what they showed.
        let recent = submenu(&file.items(), "Open Recent");
        assert!(texts(&recent).is_empty());
        let shown = vec![
            MenuItem::new("R&D.png", Command::Open(0)).into(),
            MenuItem::new("b.png", Command::Open(1)).into(),
            MenuEntry::Separator,
            MenuItem::new("Clear", Command::Clear).into(),
        ];
        assert!(bar.set_section("recent", &shown)?);
        assert_eq!(texts(&recent)[0], "R&D.png", "the & is not a mnemonic");
        assert_eq!(recent.items().len(), 4);
        let old = id_of(&recent, "b.png");
        assert_eq!(bar.command_for(&old), Some(Command::Open(1)));

        assert!(bar.set_section("recent", &[MenuItem::new("c.png", Command::Open(0)).into()])?);
        assert_eq!(texts(&recent), ["c.png"]);
        assert_eq!(bar.command_for(&old), None, "old ids stop working");
        assert_eq!(
            bar.command_for(&id_of(&recent, "c.png")),
            Some(Command::Open(0))
        );
        assert!(!bar.set_section("nope", &[])?, "no such slot");

        // Sections among other entries, and two side by side, stay in their place.
        assert!(bar.set_section(
            "plugins_second",
            &[MenuItem::new("Two", Command::Plugin(2)).into()]
        )?);
        assert!(bar.set_section(
            "plugins_first",
            &[MenuItem::new("One", Command::Plugin(1)).into()]
        )?);
        assert_eq!(texts(&file), ["Save", "Open Recent", "One", "Two", "Close"]);
        assert!(bar.set_section(
            "plugins_first",
            &[
                MenuItem::new("One A", Command::Plugin(1)).into(),
                MenuItem::new("One B", Command::Plugin(3)).into(),
            ],
        )?);
        assert_eq!(
            texts(&file),
            ["Save", "Open Recent", "One A", "One B", "Two", "Close"]
        );
        assert!(bar.set_section("plugins_first", &[])?);
        assert_eq!(texts(&file), ["Save", "Open Recent", "Two", "Close"]);
        assert!(bar.set_section("plugins_second", &[])?);
        assert_eq!(texts(&file), ["Save", "Open Recent", "Close"]);

        // Availability: enabled, label and checked, on what is shown now.
        bar.set_availability(&Now);
        let view = submenu(&top, "View");
        let MenuItemKind::Check(grid) = &view.items()[0] else {
            panic!("Grid is a check item");
        };
        assert!(grid.is_checked());
        let items = file.items();
        let MenuItemKind::MenuItem(save) = &items[0] else {
            panic!("Save is a plain item");
        };
        assert!(!save.is_enabled());
        assert_eq!(text(&items[2]), "Close R&D");
        bar.set_availability(&());
        assert!(!grid.is_checked());
        assert!(save.is_enabled());
        assert_eq!(text(&items[2]), "Close");

        // The handle reaches the same table from another thread.
        let handle = bar.commands();
        let id = id_of(&file, "Save");
        let from_thread = std::thread::spawn(move || handle.command_for(&id))
            .join()
            .expect("the thread finishes");
        assert_eq!(from_thread, Some(Command::Save));

        // A table with its own Window menu, and no ⌘W: it gets AppKit's items first.
        let with_window = vec![Menu::new(
            "Window",
            vec![MenuItem::new("Palette", Command::Grid).into()],
        )];
        let other =
            NativeMenuBar::install(&NativeMenuOptions::new("B", Command::Quit), &with_window)?;
        let top = other.menu().items();
        let window = submenu(&top, "Window");
        let window_items = texts(&window);
        assert_eq!(window_items.last().unwrap(), "Palette");
        assert!(window_items.len() > 3, "{window_items:?}");
        assert_eq!(
            top.iter().filter(|kind| text(kind) == "Window").count(),
            1,
            "no second Window menu"
        );
        Ok(())
    }
}

const NAME: &str = "native_menu_on_the_main_thread";

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let has = |flag: &str| args.iter().any(|arg| arg == flag);
    if has("--list") {
        // Nextest asks for the ignored tests separately, and there are none.
        if cfg!(target_os = "macos") && !has("--ignored") {
            println!("{NAME}: test");
        }
        return std::process::ExitCode::SUCCESS;
    }
    #[cfg(target_os = "macos")]
    {
        // A filter that isn't this test: nothing to run.
        let filters: Vec<&String> = args[1..]
            .iter()
            .filter(|arg| !arg.starts_with('-'))
            .collect();
        if !filters.is_empty() && !filters.iter().any(|filter| NAME.contains(filter.as_str())) {
            return std::process::ExitCode::SUCCESS;
        }
        if let Err(error) = real::run() {
            eprintln!("muda refused: {error}");
            return std::process::ExitCode::FAILURE;
        }
        println!("test {NAME} ... ok");
    }
    std::process::ExitCode::SUCCESS
}
