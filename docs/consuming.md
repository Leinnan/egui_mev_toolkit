# Using the toolkit from a project

## Dependencies

List the crates in the project's `[workspace.dependencies]`, all at one rev:

```toml
egui_mev_theme = { git = "https://github.com/Leinnan/egui_mev_toolkit", rev = "..." }
egui_mev_widgets = { git = "https://github.com/Leinnan/egui_mev_toolkit", rev = "..." }
```

Always use this exact URL. Cargo treats a different spelling as a different source and rejects the `git@github.com:` form.

Use one rev for every toolkit crate. Two revs compile two copies, and their `Palette` types won't match.

## Installing the theme

Call this once, from the eframe app creator, for every egui context. To look like the OS it runs on, following its light or dark setting:

```rust
use egui_mev_theme::{FontOptions, Theme};

Theme::system().install_with_fonts(&cc.egui_ctx, &FontOptions::default());
```

Enable the `native` feature to read the user's accent colour from the OS. Without it, `Theme::system()` uses the platform's default accent.

Other choices:

- `Theme::for_family(family, accent)`: either look on any OS, which suits a look setting. `ThemeFamily::Graphite` is macOS-style and `ThemeFamily::Fluent` is Windows 11 style.
- `Theme::new(dark, light)`: palettes of your own, drawn like Graphite.
- `Theme::dark_only(palette)`: one palette, whatever the system asks for.
- `.with_metrics(Metrics::COMFORTABLE)`: other sizes, without changing the look.

`Appearance` is a ready-made System, Light or Dark setting. Install `Theme::system()` again to pick up a changed accent. Nothing watches for it.

Widgets take their structure from the family, so a dialog's buttons are listed with the main action first and the look decides where each goes. To use Segoe Fluent Icons where they exist, pass `Glyph::themed(ctx, icon)` to icon widgets. They only appear on Windows under Fluent, and Lucide stands in elsewhere.

After changing fonts at run time, call `egui_mev_widgets::text_fit::clear_pool()`. That cache is per thread, and cached text from the old fonts would come out garbled. Changing colours or metrics alone doesn't need it.

For snapshot tests, use fixed accents and machine-independent fonts, so a result doesn't depend on who runs it:

```rust
fluent::theme(AccentShades::WINDOWS_DEFAULT).install(&ctx);
fonts::install_test(&ctx, &FontOptions::default());
```

## Native window chrome

Fluent looks closest with Windows' rounded corners and a matching caption. Enable `egui_mev_app`'s `chrome` feature, and either wrap an eframe app in `ChromeApp`, or hold a `NativeChrome` per window and call `sync` each frame. Both are no-ops off Windows and off Fluent.

On macOS, `native::unified_titlebar(viewport)` extends content under the title bar, and `title_bar` leaves room for the traffic lights and drags the window. Both follow the platform rather than the look, so Fluent simulated on a Mac gets the Mac's window.

## Mica

Mica needs a transparent DX12 swapchain and `WS_EX_NOREDIRECTIONBITMAP`, which eframe can't set, so eframe apps stay solid. `egui_mev_host` is a raw winit window that can host it, falling back to the opaque look where the OS refuses. It runs a `HostApp` rather than an `eframe::App`, and `run` is the simple way in:

```rust
egui_mev_host::run(MyApp::default(), HostOptions::new("My app"))?;
```

The host doesn't enable `egui/default_fonts`. An app that may run where no system font is found needs `FontOptions::bundled_ui` or that feature.

## Renderers

`ChromeApp` doesn't forward `App::on_exit`, so a wrapped app cleans up in `Drop`.

`egui_mev_app` picks no renderer. Its `eframe` feature doesn't compile on its own, because eframe 0.37 needs one, so the app enables `eframe/glow` or `eframe/wgpu_no_default_features` itself, or uses the `wgpu` feature for per-OS backend presets.

## Icon fonts

`egui-mev-icons-subset --scan src --out assets/fonts/lucide-subset.ttf` cuts a Lucide subset, and `--check` fails when the committed one is stale. Subsets depend on the `lucide-icons` version, so a project moving to this toolkit's version regenerates its subset once.

## Trying toolkit changes in a project

From the project's checkout:

```bash
cargo --config "$(/path/to/egui_mev_toolkit/scripts/toolkit-dev-config.sh)" check
```

This rewrites `Cargo.lock`. Restore it before committing, then pin the new rev.

`clippy` and `nextest` are external subcommands, so cargo ignores a `--config` placed before them and silently builds the pinned rev. Put it after: `cargo clippy --config "$config"`, `cargo nextest run --config "$config"`.

## Breaking changes

- **Data dialogs:** `Form` now accepts `danger`, `submit_when`, `blocked` and read-only `paths` fields. `FormState::toolbar_ui` and `message_ui`, and `TableView::filter_ui`, `summary_ui` and `body_ui` let a project compose its own dialog around the shared state. `TableLabels` customizes menu text and `payload_text` enables copying a payload. `RowAction` accepts optional `busy` and `title` metadata; `RowClick::Action` includes the full entry in `item` as well as its existing `action` id. Exhaustive patterns add `item` or `..`, and Rust struct literals for the extended specs add the new fields. Table drawing and whole-table updates now require `P: PartialEq` so row statuses follow payloads rather than display indices. `set_status` and `status` accept borrowed payloads such as `&Path` for a `PathBuf`. Row updates clear their status, removed rows discard it, and busy rows disable actions until their work finishes.

- **`PickerIcon` and `PickerItem` have a lifetime**, so a thumbnail resolver can borrow app state for the frame. A signature naming them changes: `fn items() -> Vec<PickerItem>` becomes `Vec<PickerItem<'static>>`, or `<'_>` when it borrows. Code that only builds them doesn't change.

## Coming from egui 0.36 code

- `Id::new` is deprecated: use `Id::unique` where the id must be stable and unique across the app.
- `WidgetInfo::labeled` and `selected` take `Role`, not `WidgetType`. `TextEdit` became `TextInput`.
- `Response::accessible_name` is new, and it's the way to name icon-only controls.
