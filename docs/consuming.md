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

Call this once, from the eframe app creator, for every egui context:

```rust
use egui_mev_theme::{Theme, presets};

Theme::new(presets::GRAPHITE_DARK, presets::GRAPHITE_LIGHT).install(&cc.egui_ctx);
```

`Theme::dark_only(palette)` uses one palette whatever the system asks for.

## Renderers

`egui_mev_app` picks no renderer. Its `eframe` feature doesn't compile on its own, because eframe 0.37 needs one, so the app enables `eframe/glow` or `eframe/wgpu_no_default_features` itself, or uses the `wgpu` feature for per-OS backend presets.

## Icon fonts

`egui-mev-icons-subset --scan src --out assets/fonts/lucide-subset.ttf` cuts a Lucide subset, and `--check` fails when the committed one is stale. Subsets depend on the `lucide-icons` version, so a project moving to this toolkit's version regenerates its subset once.

## Access while the repository is private

**On your machine**, in `~/.gitconfig` and `~/.cargo/config.toml`:

```ini
[url "git@github.com:Leinnan/egui_mev_toolkit"]
    insteadOf = https://github.com/Leinnan/egui_mev_toolkit
```

```toml
[net]
git-fetch-with-cli = true
```

Going public later changes no manifest or lockfile.

## Trying toolkit changes in a project

From the project's checkout:

```bash
cargo --config "$(/path/to/egui_mev_toolkit/scripts/toolkit-dev-config.sh)" check
```

This rewrites `Cargo.lock`. Restore it before committing, then pin the new rev.

`clippy` and `nextest` are external subcommands, so cargo ignores a `--config` placed before them and silently builds the pinned rev. Put it after: `cargo clippy --config "$config"`, `cargo nextest run --config "$config"`.

## Coming from egui 0.36 code

- `Id::new` is deprecated: use `Id::unique` where the id must be stable and unique across the app.
- `WidgetInfo::labeled` and `selected` take `Role`, not `WidgetType`. `TextEdit` became `TextInput`.
- `Response::accessible_name` is new, and it's the way to name icon-only controls.
