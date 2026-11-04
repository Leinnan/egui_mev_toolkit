# Crate rules

`scripts/check-boundaries.sh` enforces what a dependency tree can prove. The rest is for review.

1. **Admission.** A crate or item needs two or more projects that have that code today and no app types, or a place on the list below. One project's code stays in that project until a second one wants it.
2. **Features.**
   - The workspace has `egui` and `eframe` with no default features. Each crate lists the minimum it needs.
   - No crate requires a renderer. Only `egui_mev_app`'s `eframe` feature pulls in `eframe`, and its `wgpu` feature picks that backend. The app chooses otherwise.
   - Optional integrations stay behind features, off by default.
3. **No umbrella crate.** Projects depend on crates directly.
4. **Configuration comes from the app.** Palette, fonts, IDs, app names and env var names are passed in.
5. **No globals.** Per-app state lives in `Context` data, so parallel test harnesses stay isolated and one process can hold two looks. Caches are the exception: `text_fit` keeps its galley pool per thread.
6. **Naming.** `*_core` means no egui at all. Names carry no app branding.
7. **Not here:** app screens, document models, file-format loaders, and anything that needs an app type to make sense.

## Admitted by decision

These have one source project today, and were included on purpose on 2026-11-04:

- `egui_mev_core`: `selection`, `spline`
- `egui_mev_widgets`: `text_fit`, breadcrumb `fit`, `SearchPicker`
- `egui_mev_data`: `FormState`, `TableView`
- `egui_mev_graphics`: curve editor, color editor, rulers

## Parked

- **Waiting on egui 0.37 support upstream:** dock style (`egui_dock`), Phosphor constants (`egui-phosphor`), puffin helpers (`puffin_egui`). Phosphor already works through `IconFont::Custom`.
- **One consumer:** Lucide picker, `ViewTransform`, `TileCache`, panel-state repair, `RotatingLog`, `FailureRecord`.
- **App-bound:** `chip_input`, `path_editor`, breadcrumb `show`, `JobScheduler`, `AssetManager`, multi-window docking, `TrayController`, pen input, pixel undo.
