# egui_mev_toolkit

Crates shared by egui applications, on egui 0.37. Read `docs/crate-rules.md` before adding a crate or a dependency and `docs/consuming.md` for how projects use them.

## Commands

```bash
just check         # fmt, clippy -D warnings, crate boundaries
just test          # all tests, with nextest when installed
just check-alone   # each crate alone with no default features
just check-web     # crates meant to run on the web
just ci            # everything CI runs
```

## Rules

- Run one cargo command at a time, and never `cargo clean`.
- Prefer `--workspace` over `-p` for tests: `-p` changes feature unification and forces rebuilds.
- Projects pin revs, so a breaking change here reaches a project only when it moves its pin. Add the note to `docs/consuming.md` when a change needs edits in projects.
- Every crate's manifest has `exclude.workspace = true`, so snapshot images stay out of packages. `scripts/check-boundaries.sh` checks this.
- Add a new crate to the lists in `scripts/check-boundaries.sh` and the table in `README.md` when you add it to `crates/`.
- Widgets read colours through `Palette::of(ui)` and shared sizes through `Metrics::of(ui)`. Never add a theme colour to a widget as a constant. Colours that belong to the content can stay literal, like the corners of a colour triangle or a transparency checker, and so can layout numbers private to one widget, like a popup's width.
- Accept snapshot changes only after looking at the new image. `UPDATE_SNAPSHOTS=1 just test` rewrites them.
