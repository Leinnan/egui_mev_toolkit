//! Every widget on one page, shared by `examples/gallery.rs` and `tests/gallery.rs`.

use egui::{Color32, RichText, ScrollArea, Sense, Ui, Vec2};
use egui_mev_theme::{AccentShades, Glyph, Metrics, Palette, Theme, fluent, presets};
#[cfg(feature = "autocomplete")]
use egui_mev_widgets::autocomplete::{AutoCompleteTextEdit, complete_from_list};
#[cfg(feature = "picker")]
use egui_mev_widgets::picker::{PickerCategory, PickerItem, PickerMemory, SearchPicker};
use egui_mev_widgets::{
    BannerKind, DialogButton, DialogResult, Tip, ToastKind, Toasts, TreeRow, TreeRowTrailing,
    banner, card, collapsible, color_swatch_row, danger_button, destructive_button, dialog_buttons,
    empty_state, field_row, fit, group_title, grouped_form, inspector_section, key_caps, kv_row,
    kv_row_colored, kv_row_mono, nav_row, page_heading, painted_button, pill, primary_button,
    progress, search_field, secondary_button, section_header, segmented, settings_group, sheet,
    slider_row, slider_row_suffix, small_icon_button, small_text_button, split_button, tab_strip,
    text_field, text_fit, toggle_row, toggle_switch, tool_button, toolbar_icon,
    toolbar_icon_enabled, tree_row, ui_labeled_row,
};
use lucide_icons::Icon;

/// A look the gallery can be shown in. The example switches between them and the
/// snapshots cover each one, always with a fixed accent so that nothing depends on
/// the machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    GraphiteDark,
    MacosLight,
    GraphiteLight,
    SlateDark,
    FluentDark,
    FluentLight,
    PinkDark,
}

// Each of the two users of this file calls only some of these.
#[allow(dead_code)]
impl Look {
    pub const ALL: [Self; 7] = [
        Self::GraphiteDark,
        Self::MacosLight,
        Self::GraphiteLight,
        Self::SlateDark,
        Self::FluentDark,
        Self::FluentLight,
        Self::PinkDark,
    ];

    /// The look's name in a snapshot's file name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::GraphiteDark => "graphite_dark",
            Self::MacosLight => "macos_light",
            Self::GraphiteLight => "graphite_light",
            Self::SlateDark => "slate_dark",
            Self::FluentDark => "fluent_dark",
            Self::FluentLight => "fluent_light",
            Self::PinkDark => "pink_dark",
        }
    }

    /// The look's name on the theme switcher.
    pub const fn label(self) -> &'static str {
        match self {
            Self::GraphiteDark => "Graphite dark",
            Self::MacosLight => "macOS light",
            Self::GraphiteLight => "Graphite light",
            Self::SlateDark => "Slate dark",
            Self::FluentDark => "Fluent dark",
            Self::FluentLight => "Fluent light",
            Self::PinkDark => "Pink accent",
        }
    }

    pub fn theme(self) -> Theme {
        match self {
            Self::GraphiteDark | Self::MacosLight => Theme::graphite(AccentShades::MACOS_DEFAULT),
            Self::GraphiteLight => Theme::new(presets::GRAPHITE_DARK, presets::GRAPHITE_LIGHT),
            Self::SlateDark => Theme::dark_only(presets::SLATE_DARK),
            Self::FluentDark | Self::FluentLight => fluent::theme(AccentShades::WINDOWS_DEFAULT),
            Self::PinkDark => {
                let pink = Color32::from_rgb(0xF7, 0x4F, 0x9E);
                Theme::graphite(AccentShades::from_base(pink))
            }
        }
    }

    pub const fn mode(self) -> egui::Theme {
        match self {
            Self::MacosLight | Self::GraphiteLight | Self::FluentLight => egui::Theme::Light,
            Self::GraphiteDark | Self::SlateDark | Self::FluentDark | Self::PinkDark => {
                egui::Theme::Dark
            }
        }
    }
}

/// Which dialog the gallery shows over itself.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OpenDialog {
    #[default]
    None,
    Sheet,
    Progress,
}

/// What the gallery's controls change.
pub struct GalleryState {
    pub segment: u8,
    pub tab: u8,
    pub tool: u8,
    pub grid: bool,
    pub bones: bool,
    pub switch_on: bool,
    pub switch_off: bool,
    pub density: f32,
    pub distance: f32,
    pub search: String,
    pub field: String,
    pub color: [u8; 3],
    pub nav: u8,
    pub open_node: bool,
    pub visible: bool,
    pub dialog: OpenDialog,
    pub toasts_pushed: bool,
    #[cfg(feature = "autocomplete")]
    pub command: String,
    #[cfg(feature = "picker")]
    pub picker_memory: PickerMemory,
    #[cfg(feature = "picker")]
    pub picked: String,
}

impl Default for GalleryState {
    fn default() -> Self {
        Self {
            segment: 1,
            tab: 0,
            tool: 0,
            grid: true,
            bones: true,
            switch_on: true,
            switch_off: false,
            density: 0.4,
            distance: 120.0,
            search: "brush".to_owned(),
            field: "Untitled".to_owned(),
            color: [0x0A, 0x84, 0xFF],
            nav: 1,
            open_node: true,
            visible: true,
            dialog: OpenDialog::None,
            toasts_pushed: false,
            #[cfg(feature = "autocomplete")]
            command: String::new(),
            #[cfg(feature = "picker")]
            picker_memory: PickerMemory::default(),
            #[cfg(feature = "picker")]
            picked: "cpu".to_owned(),
        }
    }
}

/// Draw the whole gallery into `ui`.
pub fn gallery_ui(ui: &mut Ui, state: &mut GalleryState) {
    ScrollArea::vertical()
        .id_salt("gallery_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.columns(3, |columns| {
                buttons_column(&mut columns[0], state);
                forms_column(&mut columns[1], state);
                navigation_column(&mut columns[2], state);
            });
        });
    if !state.toasts_pushed {
        state.toasts_pushed = true;
        let ctx = ui.ctx();
        Toasts::push(ctx, "A note for you", ToastKind::Info);
        Toasts::push(ctx, "Saved to disk", ToastKind::Ok);
        Toasts::push(ctx, "Disk is almost full", ToastKind::Warn);
        Toasts::push(ctx, "Couldn't reach the server", ToastKind::Error);
    }
    Toasts::show(ui.ctx());
    dialogs(ui, state);
}

fn buttons_column(ui: &mut Ui, state: &mut GalleryState) {
    page_heading(ui, "Page heading", "A page title and its description.");
    card(ui, Some("Buttons"), |ui| {
        ui.horizontal_wrapped(|ui| {
            primary_button(ui, "Save");
            secondary_button(ui, "Cancel");
            danger_button(ui, "Reset");
            destructive_button(ui, "Delete");
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let glyph = |ui: &Ui, icon| Glyph::themed(ui.ctx(), icon);
            let grid = glyph(ui, Icon::Grid3x3);
            if toolbar_icon(ui, grid, state.grid, Tip::hint("Grid", "G")).clicked() {
                state.grid = !state.grid;
            }
            toolbar_icon(ui, glyph(ui, Icon::FolderOpen), false, "Open");
            toolbar_icon_enabled(ui, glyph(ui, Icon::Undo2), true, "Undo");
            toolbar_icon_enabled(ui, glyph(ui, Icon::Redo2), false, "Redo");
            small_icon_button(ui, glyph(ui, Icon::X), 12.0, true, "Close");
            small_text_button(ui, "B/W", "Black and white");
            let dots = painted_button(
                ui,
                Vec2::splat(24.0),
                egui_mev_widgets::ButtonLook::Plain,
                true,
                |painter, rect, color| {
                    painter.circle_filled(rect.center(), 3.0, color);
                },
            );
            dots.on_hover_text("Painted")
                .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Painted"));
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            for (index, (icon, name)) in [
                (Icon::Brush, "Brush"),
                (Icon::Eraser, "Eraser"),
                (Icon::Pipette, "Pipette"),
            ]
            .into_iter()
            .enumerate()
            {
                let index = index as u8;
                if tool_button(ui, icon, state.tool == index, name).clicked() {
                    state.tool = index;
                }
            }
        });
        ui.add_space(4.0);
        let main = egui_mev_widgets::secondary(ui, "Place");
        split_button(ui, main, "More place options", |ui| {
            ui.label("Place all");
        });
    });
    card(ui, Some("Status"), |ui| {
        let p = Palette::of(ui);
        ui.horizontal_wrapped(|ui| {
            pill(ui, "Connected", p.success);
            pill(ui, "2 notices", p.caution);
            pill(ui, "Idle", p.neutral);
        });
        ui.add_space(4.0);
        banner(
            ui,
            BannerKind::Info,
            "Flora is approximate, so check how it looks before you export the scene to the game.",
        );
        ui.add_space(4.0);
        banner(ui, BannerKind::Success, "The key is valid for this skin.");
        ui.add_space(4.0);
        banner(ui, BannerKind::Warning, "2 compatibility warnings.");
        ui.add_space(4.0);
        banner(ui, BannerKind::Error, "The loader rejected this file.");
    });
    card(ui, Some("Dialogs"), |ui| {
        ui.horizontal(|ui| {
            if secondary_button(ui, "Open sheet").clicked() {
                state.dialog = OpenDialog::Sheet;
            }
            if secondary_button(ui, "Show progress").clicked() {
                state.dialog = OpenDialog::Progress;
            }
        });
        let buttons = [
            DialogButton::primary("Apply"),
            DialogButton::danger("Discard"),
            DialogButton::plain("Later").enabled(false),
        ];
        if let Some(index) = dialog_buttons(ui, false, &buttons) {
            Toasts::info(ui.ctx(), format!("{} clicked", buttons[index].label));
        }
    });
    card(ui, Some("Empty state"), |ui| {
        empty_state(ui, "No file open", "Pick a file to browse its objects.");
    });
}

fn forms_column(ui: &mut Ui, state: &mut GalleryState) {
    card(ui, Some("Rows"), |ui| {
        let caution = Palette::of(ui).caution;
        kv_row(ui, "Vertices", "538");
        kv_row_mono(ui, "Flags", "0x00000000");
        kv_row_colored(ui, "Missing", "2 textures", caution);
        field_row(ui, "Name", |ui| {
            let edit = text_field(ui, &mut state.field).desired_width(f32::INFINITY);
            ui.add(edit).accessible_name("Name");
        });
        toggle_row(ui, "Show bones", &mut state.bones);
        slider_row(ui, "Density", &mut state.density, 0.0..=1.0);
        slider_row_suffix(ui, "Distance", &mut state.distance, 0.0..=500.0, " m");
    });
    ui.add_space(Metrics::of(ui).card_gap);
    grouped_form(ui, Some("Grouped form"), |ui| {
        ui_labeled_row(ui, "Label", |ui| {
            ui.label("Beside its control");
        });
        color_swatch_row(ui, "Accent", &mut state.color);
    });
    settings_group(ui, "Settings group", |group| {
        group.row("Sounds", Some("Play a sound on events."), |ui| {
            toggle_switch(ui, &mut state.switch_on, "Sounds");
        });
        group.row("Animations", None, |ui| {
            toggle_switch(ui, &mut state.switch_off, "Animations");
        });
        group.row("Find", None, |ui| {
            key_caps(ui, &["F3", "Ctrl+F"]);
        });
        group.note("Changes apply at once.");
    });
    card(ui, Some("Text fitting"), |ui| {
        let p = Palette::of(ui);
        let font = egui::FontId::proportional(Metrics::of(ui).body);
        let name = "A rather long screenshot name 2026-03-16 at 12.45.04.png";
        let plain = ui
            .painter()
            .layout_no_wrap(name.to_owned(), font.clone(), p.text);
        show_galley(ui, text_fit::fit_single_line(ui, plain, 220.0, 0));
        show_galley(ui, text_fit::fit_two_lines(ui, name, &font, p.text, 220.0));
        show_galley(
            ui,
            text_fit::fit_path_start(
                ui,
                "very/long/path/to/some/deeply/nested/folder",
                &font,
                p.text_weak,
                220.0,
            ),
        );
        ui.add_space(4.0);
        // Breadcrumbs measured by `fit`: the current folder lasts longest.
        let crumbs = ["Home", "Documents", "Projects", "toolkit", "crates"];
        let widths: Vec<f32> = crumbs
            .iter()
            .map(|crumb| {
                ui.painter()
                    .layout_no_wrap((*crumb).to_owned(), font.clone(), p.text)
                    .size()
                    .x
            })
            .collect();
        for available in [320.0, 200.0, 60.0] {
            let fitted = fit(&widths, 16.0, 12.0, 14.0, available);
            let hidden = if fitted.hidden.is_empty() {
                ""
            } else {
                "… › "
            };
            let shown: Vec<&str> = fitted.shown.iter().map(|index| crumbs[*index]).collect();
            ui.label(RichText::new(format!("{hidden}{}", shown.join(" › "))).color(p.text_weak));
        }
    });
}

fn show_galley(ui: &mut Ui, galley: std::sync::Arc<egui::Galley>) {
    let color = Palette::of(ui).text;
    let (rect, _) = ui.allocate_exact_size(galley.size(), Sense::hover());
    // Centred text lays out around its origin.
    let origin = if galley.job.halign == egui::Align::Center {
        rect.center_top()
    } else {
        rect.min
    };
    ui.painter().galley(origin, galley, color);
}

fn navigation_column(ui: &mut Ui, state: &mut GalleryState) {
    card(ui, Some("Choices"), |ui| {
        segmented(
            ui,
            "gallery_segment",
            &mut state.segment,
            &[(0, "Assets"), (1, "Faces"), (2, "World")],
        );
        ui.add_space(4.0);
        tab_strip(
            ui,
            "gallery_tabs",
            &mut state.tab,
            &[(0, "Meshes"), (1, "Materials"), (2, "Bodies")],
        );
        ui.add_space(4.0);
        search_field(ui, &mut state.search, "Filter meshes");
    });
    card(ui, Some("Navigation"), |ui| {
        for (index, (icon, label)) in [
            (Icon::House, "Home"),
            (Icon::Folder, "Documents"),
            (Icon::Settings, "Settings"),
        ]
        .into_iter()
        .enumerate()
        {
            let index = index as u8;
            if nav_row(ui, icon, label, state.nav == index, Sense::click()).clicked() {
                state.nav = index;
            }
        }
        ui.add_space(4.0);
        let root = tree_row(
            ui,
            "gallery_root",
            &TreeRow {
                expanded: Some(state.open_node),
                icon: Some(Icon::Folder.into()),
                label: "scene",
                detail: Some("3 objects"),
                trailing: Some(TreeRowTrailing {
                    on: state.visible,
                    icon_on: Icon::Eye.into(),
                    icon_off: Icon::EyeOff.into(),
                    name: "Scene visible",
                }),
                ..Default::default()
            },
        );
        if root.toggled {
            state.open_node = !state.open_node;
        }
        if root.trailing_clicked {
            state.visible = !state.visible;
        }
        if state.open_node {
            let p = Palette::of(ui);
            tree_row(
                ui,
                "gallery_child",
                &TreeRow {
                    depth: 1,
                    icon: Some(Icon::Box.into()),
                    label: "mesh_lod0",
                    matched: &[0, 1, 2, 3],
                    selected: true,
                    ..Default::default()
                },
            );
            tree_row(
                ui,
                "gallery_dimmed",
                &TreeRow {
                    depth: 1,
                    icon: Some(Icon::Triangle.into()),
                    icon_color: Some(p.caution),
                    dot: Some(p.critical),
                    label: "hidden_part",
                    dimmed: true,
                    ..Default::default()
                },
            );
        }
    });
    card(ui, Some("Sections"), |ui| {
        section_header(ui, "Section header");
        group_title(ui, "Group title");
        inspector_section(ui, "Inspector section", false, |ui| {
            ui.label("Open by default.");
        });
        collapsible(ui, "gallery_collapsible", "Collapsible", true, |ui| {
            ui.label("A collapsible body.");
        });
    });
    #[cfg(any(feature = "autocomplete", feature = "picker"))]
    card(ui, Some("Pickers"), |ui| {
        #[cfg(feature = "autocomplete")]
        {
            let words = ["addItem", "addGold", "godmode", "heal"];
            AutoCompleteTextEdit::new(&mut state.command, |text, _| {
                complete_from_list(&words, text)
            })
            .id_salt("gallery_command")
            .text_edit(|edit| edit.hint_text("Run a command"))
            .show(ui)
            .response
            .accessible_name("Run a command");
            ui.add_space(4.0);
        }
        #[cfg(feature = "picker")]
        picker(ui, state);
    });
}

#[cfg(feature = "picker")]
fn picker(ui: &mut Ui, state: &mut GalleryState) {
    let categories = [
        PickerCategory::new("System", Icon::Cpu),
        PickerCategory::new("Time", Icon::Clock),
    ];
    let items = || {
        let mut cpu = PickerItem::new("cpu", "CPU Usage", Icon::Cpu);
        cpu.category = Some(0);
        cpu.keywords = "processor".to_owned();
        cpu.detail = Some("12%".to_owned());
        let mut memory = PickerItem::new("memory", "Memory Usage", Icon::MemoryStick);
        memory.category = Some(0);
        memory.keywords = "ram".to_owned();
        memory.needs_setup = true;
        let mut clock = PickerItem::new("clock", "Clock", Icon::Clock);
        clock.category = Some(1);
        vec![cpu, memory, clock]
    };
    let label = items()
        .into_iter()
        .find(|item| item.id == state.picked)
        .map_or_else(|| state.picked.clone(), |item| item.label);
    let picked = SearchPicker::new("gallery_picker", "Widget", &label, Icon::Cpu)
        .hint("Search widgets")
        .selected_id(&state.picked)
        .categories(&categories)
        .memory_key("gallery")
        .show(ui, &state.picker_memory, items);
    if let Some(picked) = picked {
        picked.apply_to(&mut state.picker_memory);
        if let Some(id) = picked.entry() {
            state.picked = id.to_owned();
        }
    }
}

fn dialogs(ui: &mut Ui, state: &mut GalleryState) {
    match state.dialog {
        OpenDialog::None => {}
        OpenDialog::Sheet => {
            let result = sheet(ui.ctx(), "Resize image", 320.0, |ui| {
                kv_row(ui, "Current size", "1280 × 720");
                ui.label(RichText::new("Return accepts and Escape cancels.").weak());
            });
            if result != DialogResult::Open {
                state.dialog = OpenDialog::None;
            }
        }
        OpenDialog::Progress => {
            if progress(ui.ctx(), "Applying Blur…", Some(0.4), true) {
                state.dialog = OpenDialog::None;
            }
        }
    }
}
