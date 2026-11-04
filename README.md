# egui_mev_toolkit

Crates shared by egui applications and tools, on egui 0.37 and with no Bevy dependency.

| Crate | What it holds |
|---|---|
| `egui_mev_core` | Pure logic with no egui: undo history, atomic files, weighted LRU, list selection, splines, byte formatting |
| `egui_mev_theme` | Windows and macOS system themes, palette, metrics, light/dark, fonts, icon glyphs |
| `egui_mev_widgets` | Buttons drawn per OS look, cards, form rows, dialogs, toasts, text fitting, `SearchPicker` |
| `egui_mev_data` | `FormState` and `TableView`, driven by serde specs |
| `egui_mev_graphics` | Curve editor, color editor, rulers |
| `egui_mev_commands` | Shortcuts, keymaps, command queue, menus, command palette, native macOS menu |
| `egui_mev_tasks` | Worker-to-UI plumbing: repaint waker, event inbox, mailbox, keyed worker, poller |
| `egui_mev_app` | App shell: native window chrome, title bar, window drag, settings, paths, startup timing, log viewer, eframe presets |
| `egui_mev_host` | A raw winit window for what eframe can't host, such as Mica |
| `egui_mev_test` | `egui_kittest` helpers, used as a dev-dependency |
| `egui_mev_icons_subset` | A tool that cuts a Lucide font subset from the icons an app uses |
| `egui_mev_inspect` | Reflect-driven editor over `bevy_reflect`, without the `bevy` crate |

Read [docs/crate-rules.md](docs/crate-rules.md) before adding a crate or a dependency and [docs/consuming.md](docs/consuming.md) for how a project uses these crates.

Projects on egui 0.36, including everything on `bevy_egui`, can only use `egui_mev_core` until they move to 0.37.

## Commands

```sh
just check        # fmt, clippy -D warnings, crate boundaries
just test         # all tests, with nextest when installed
just check-alone  # each crate alone with no default features
just check-web    # crates meant to run on the web
just check-windows # Windows code, which only compiles from other OSes
just ci           # everything CI runs
```

## License

MIT OR Apache-2.0, at your option. Bundled fonts keep their own licenses: see the `assets/fonts` directories.
