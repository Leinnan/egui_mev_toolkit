//! Font setup: the platform's UI font when it's there, egui's built-ins
//! otherwise, then icon fonts as the last fallback.

use std::path::{Path, PathBuf};

use egui::{Context, FontData, FontDefinitions, FontFamily, FontId};

use crate::{IconFont, icons::FLUENT_ICONS_FAMILY};

/// Roboto Condensed SemiBold (SIL OFL, see `assets/fonts/OFL.txt`).
#[cfg(feature = "roboto")]
pub const ROBOTO_CONDENSED: &[u8] = include_bytes!("../assets/fonts/RobotoCondensed-SemiBold.ttf");

/// Name of the semibold family every setup here registers.
pub const SEMIBOLD: &str = "semibold";

/// UI font files to try, most preferred first.
const UI_FONTS: &[&str] = if cfg!(target_os = "macos") {
    &["SFNS.ttf", "SFNSText.ttf", "HelveticaNeue.ttc"]
} else if cfg!(windows) {
    &["SegUIVar.ttf", "segoeui.ttf"]
} else {
    &[
        "Inter-Regular.ttf",
        "NotoSans-Regular.ttf",
        "DejaVuSans.ttf",
        "LiberationSans-Regular.ttf",
    ]
};

/// Monospace font files to try, most preferred first.
const MONO_FONTS: &[&str] = if cfg!(target_os = "macos") {
    &["SFNSMono.ttf"]
} else if cfg!(windows) {
    &["CascadiaMono.ttf", "consola.ttf"]
} else {
    &["DejaVuSansMono.ttf", "LiberationMono-Regular.ttf"]
};

/// Semibold UI font files to try, most preferred first.
const SEMIBOLD_FONTS: &[&str] = if cfg!(target_os = "macos") {
    &["SFCompact.ttf"]
} else if cfg!(windows) {
    &["seguisb.ttf"]
} else {
    &["Inter-SemiBold.ttf", "NotoSans-SemiBold.ttf"]
};

/// Segoe Fluent Icons, from the system and never bundled.
const FLUENT_ICON_FONTS: &[&str] = &["SegoeIcons.ttf", "segmdl2.ttf"];

/// What an app adds to the platform's fonts.
#[derive(Clone, Debug, Default)]
pub struct FontOptions {
    /// Icon fonts, in fallback order.
    pub icons: Vec<IconFont>,
    /// A font the app bundles: the base of the `Proportional` chain, so the app
    /// looks the same where no system font is found.
    pub bundled_ui: Option<&'static [u8]>,
    /// Register Segoe Fluent Icons where the system has it, for
    /// [`crate::Glyph::themed`]. It keeps a family of its own and stays out of
    /// the `Proportional` chain, because its private-use range collides with Lucide.
    pub fluent_icons: bool,
}

/// Build the fonts, reading files through `load`, which gets candidate file
/// names and returns the first one it can read. Injecting it keeps the chain
/// testable on any machine.
pub fn definitions_with_loader(
    options: &FontOptions,
    mut load: impl FnMut(&[&str]) -> Option<Vec<u8>>,
) -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    if let Some(bytes) = options.bundled_ui {
        fonts
            .font_data
            .insert("bundled".to_owned(), FontData::from_static(bytes).into());
        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .insert(0, "bundled".to_owned());
    }
    if let Some(bytes) = load(UI_FONTS) {
        fonts
            .font_data
            .insert("system".to_owned(), FontData::from_owned(bytes).into());
        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .insert(0, "system".to_owned());
        // Monospace fonts often lack ⌘⌥⌃⇧, which shortcut labels need.
        fonts
            .families
            .entry(FontFamily::Monospace)
            .or_default()
            .push("system".to_owned());
    }
    if let Some(bytes) = load(MONO_FONTS) {
        fonts
            .font_data
            .insert("system-mono".to_owned(), FontData::from_owned(bytes).into());
        fonts
            .families
            .entry(FontFamily::Monospace)
            .or_default()
            .insert(0, "system-mono".to_owned());
    }
    let mut semibold = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    if let Some(bytes) = load(SEMIBOLD_FONTS) {
        fonts
            .font_data
            .insert(SEMIBOLD.to_owned(), FontData::from_owned(bytes).into());
        semibold.insert(0, SEMIBOLD.to_owned());
    }
    fonts
        .families
        .insert(FontFamily::Name(SEMIBOLD.into()), semibold);
    for icon_font in &options.icons {
        icon_font.register(&mut fonts);
    }
    if options.fluent_icons
        && let Some(bytes) = load(FLUENT_ICON_FONTS)
    {
        fonts.font_data.insert(
            FLUENT_ICONS_FAMILY.to_owned(),
            FontData::from_owned(bytes).into(),
        );
        fonts.families.insert(
            FontFamily::Name(FLUENT_ICONS_FAMILY.into()),
            vec![FLUENT_ICONS_FAMILY.to_owned()],
        );
    }
    fonts
}

/// Install the platform's fonts on `ctx`.
pub fn install_system(ctx: &Context, options: &FontOptions) {
    ctx.set_fonts(definitions_with_loader(options, find_font));
}

/// Install fonts that don't depend on the machine, for snapshot tests.
pub fn install_test(ctx: &Context, options: &FontOptions) {
    ctx.set_fonts(definitions_with_loader(options, |_| None));
}

/// A semibold font of `size`. Every setup here registers the family, falling
/// back to the regular face. Before one has run, this is plain `Proportional`,
/// because drawing with an unregistered family panics.
#[must_use]
pub fn semibold(ctx: &Context, size: f32) -> FontId {
    let family = FontFamily::Name(SEMIBOLD.into());
    let registered = ctx.fonts(|fonts| fonts.definitions().families.contains_key(&family));
    FontId::new(
        size,
        if registered {
            family
        } else {
            FontFamily::Proportional
        },
    )
}

/// The system's font directories, user ones first so a developer can override
/// the look without touching the system installation.
#[must_use]
pub fn font_directories() -> Vec<PathBuf> {
    let mut directories = Vec::new();
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        directories.push(home.join("Library/Fonts"));
        directories.push(home.join(".local/share/fonts"));
        directories.push(home.join(".fonts"));
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        directories.push(local.join("Microsoft/Windows/Fonts"));
    }
    if let Some(windows) = std::env::var_os("WINDIR").map(PathBuf::from) {
        directories.push(windows.join("Fonts"));
    }
    for directory in [
        "/Library/Fonts",
        "/System/Library/Fonts",
        "/usr/share/fonts",
        "/usr/local/share/fonts",
    ] {
        directories.push(PathBuf::from(directory));
    }
    directories
}

/// The first file named one of `names` (case-insensitive), tried in order, in
/// the font directories or their subdirectories, read.
#[must_use]
pub fn find_font(names: &[&str]) -> Option<Vec<u8>> {
    let directories = font_directories();
    names.iter().find_map(|name| {
        directories
            .iter()
            .find_map(|directory| search(directory, name, 3))
            .and_then(|path| std::fs::read(path).ok())
    })
}

/// `name` in `directory` or below it, as Linux font trees nest.
fn search(directory: &Path, name: &str, depth: u8) -> Option<PathBuf> {
    let entries = std::fs::read_dir(directory).ok()?;
    let mut subdirectories = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            subdirectories.push(path);
        } else if path
            .file_name()
            .and_then(|file| file.to_str())
            .is_some_and(|file| file.eq_ignore_ascii_case(name))
        {
            return Some(path);
        }
    }
    if depth == 0 {
        return None;
    }
    subdirectories
        .iter()
        .find_map(|subdirectory| search(subdirectory, name, depth - 1))
}

#[cfg(test)]
mod tests {
    use egui::FontFamily;

    use super::{FontOptions, definitions_with_loader, install_test, search, semibold};
    use crate::IconFont;

    /// Any parseable font works as a stand-in here.
    fn stand_in() -> &'static [u8] {
        let fonts = egui::FontDefinitions::default();
        let name = &fonts.families[&FontFamily::Proportional][0];
        let data = &fonts.font_data[name];
        let bytes: &[u8] = (*data.font).as_ref();
        Box::leak(bytes.to_vec().into_boxed_slice())
    }

    #[test]
    fn missing_system_fonts_keep_the_defaults() {
        let defaults = egui::FontDefinitions::default();
        let fonts = definitions_with_loader(&FontOptions::default(), |_| None);
        assert_eq!(
            fonts.families[&FontFamily::Proportional],
            defaults.families[&FontFamily::Proportional]
        );
        // The family exists either way, so drawing with it can't panic.
        assert!(
            fonts
                .families
                .contains_key(&FontFamily::Name("semibold".into()))
        );
    }

    #[test]
    fn a_system_font_leads_the_chain_and_backs_up_monospace() {
        let bytes = stand_in();
        let fonts = definitions_with_loader(&FontOptions::default(), |names| {
            (names == super::UI_FONTS).then(|| bytes.to_vec())
        });
        assert_eq!(fonts.families[&FontFamily::Proportional][0], "system");
        assert_eq!(
            fonts.families[&FontFamily::Monospace]
                .last()
                .map(String::as_str),
            Some("system")
        );
    }

    #[test]
    fn a_bundled_font_sits_behind_the_system_font() {
        let bytes = stand_in();
        let options = FontOptions {
            bundled_ui: Some(bytes),
            ..FontOptions::default()
        };
        let without_system = definitions_with_loader(&options, |_| None);
        assert_eq!(
            without_system.families[&FontFamily::Proportional][0],
            "bundled"
        );
        let with_system = definitions_with_loader(&options, |names| {
            (names == super::UI_FONTS).then(|| bytes.to_vec())
        });
        assert_eq!(with_system.families[&FontFamily::Proportional][0], "system");
        assert_eq!(
            with_system.families[&FontFamily::Proportional][1],
            "bundled"
        );
    }

    #[test]
    fn icon_fonts_come_last_with_hinting_off_and_a_family_of_their_own() {
        let options = FontOptions {
            icons: vec![IconFont::Subset(stand_in())],
            ..FontOptions::default()
        };
        let fonts = definitions_with_loader(&options, |_| None);
        assert_eq!(
            fonts.families[&FontFamily::Proportional]
                .last()
                .map(String::as_str),
            Some("lucide")
        );
        assert_eq!(
            fonts.families[&FontFamily::Name("lucide".into())],
            ["lucide"]
        );
        assert_eq!(fonts.font_data["lucide"].tweak.hinting, Some(false));
    }

    #[test]
    fn semibold_is_safe_before_and_after_fonts_are_set() {
        let ctx = egui::Context::default();
        crate::run_frame(&ctx, |_| {});
        assert_eq!(semibold(&ctx, 14.0).family, FontFamily::Proportional);
        install_test(&ctx, &FontOptions::default());
        crate::run_frame(&ctx, |_| {});
        assert_eq!(
            semibold(&ctx, 14.0).family,
            FontFamily::Name("semibold".into())
        );
    }

    #[test]
    fn fluent_icons_get_their_own_family_and_stay_out_of_the_text_chain() {
        let bytes = stand_in();
        let options = FontOptions {
            fluent_icons: true,
            ..FontOptions::default()
        };
        let fonts = definitions_with_loader(&options, |names| {
            (names == super::FLUENT_ICON_FONTS).then(|| bytes.to_vec())
        });
        let family = crate::icons::FLUENT_ICONS_FAMILY;
        assert_eq!(fonts.families[&FontFamily::Name(family.into())], [family]);
        assert!(
            !fonts.families[&FontFamily::Proportional]
                .iter()
                .any(|n| n == family)
        );
        // Absent from the system, the family isn't registered at all.
        let missing = definitions_with_loader(&options, |_| None);
        assert!(
            !missing
                .families
                .contains_key(&FontFamily::Name(family.into()))
        );
    }

    #[test]
    fn search_finds_nested_files_case_insensitively_within_its_depth() {
        let root = std::env::temp_dir().join(format!("egui-mev-fonts-{}", std::process::id()));
        let nested = root.join("a/b");
        std::fs::create_dir_all(&nested).expect("should create");
        std::fs::write(nested.join("Thing.TTF"), b"x").expect("should write");

        assert_eq!(
            search(&root, "thing.ttf", 3),
            Some(nested.join("Thing.TTF"))
        );
        assert_eq!(search(&root, "thing.ttf", 1), None);
        assert_eq!(search(&root, "other.ttf", 3), None);

        std::fs::remove_dir_all(&root).expect("should clean up");
    }

    #[cfg(feature = "lucide")]
    #[test]
    fn lucide_glyphs_lay_out_in_the_first_frame() {
        let ctx = egui::Context::default();
        let options = FontOptions {
            icons: vec![IconFont::Lucide],
            ..FontOptions::default()
        };
        install_test(&ctx, &options);
        crate::run_frame(&ctx, |ui| {
            ui.label(crate::icons::glyph(lucide_icons::Icon::Folder));
        });
    }
}
