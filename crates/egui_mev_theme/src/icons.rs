//! Icon fonts and the glyphs drawn from them.
//!
//! An icon font is registered as the last `Proportional` fallback, so its
//! codepoints render in any text. A named family alone isn't enough: it isn't
//! bound on the very first frame after `set_fonts`, and looking it up then panics.

use std::sync::Arc;

use egui::{FontData, FontDefinitions, FontFamily, FontId, RichText};

/// Name of the font family holding Segoe Fluent Icons, registered when
/// [`crate::FontOptions::fluent_icons`] finds the system's font.
pub const FLUENT_ICONS_FAMILY: &str = "fluent_icons";

/// Default size of an icon (px).
pub const ICON_SIZE: f32 = 16.0;

/// One icon: a codepoint and the family it lives in.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Glyph {
    /// The codepoint.
    pub ch: char,
    /// The family to draw it in.
    pub family: FontFamily,
}

impl Glyph {
    /// A glyph from a font registered as a `Proportional` fallback.
    #[must_use]
    pub const fn new(ch: char) -> Self {
        Self {
            ch,
            family: FontFamily::Proportional,
        }
    }

    /// The codepoint as text.
    #[must_use]
    pub fn text(&self) -> String {
        self.ch.to_string()
    }

    /// A font id for this glyph at `size`.
    #[must_use]
    pub fn font_id(&self, size: f32) -> FontId {
        FontId::new(size, self.family.clone())
    }

    /// The glyph as rich text at `size`.
    #[must_use]
    pub fn rich(&self, size: f32) -> RichText {
        RichText::new(self.text()).font(self.font_id(size))
    }
}

impl From<char> for Glyph {
    fn from(ch: char) -> Self {
        Self::new(ch)
    }
}

#[cfg(feature = "lucide")]
impl From<lucide_icons::Icon> for Glyph {
    fn from(icon: lucide_icons::Icon) -> Self {
        Self::new(char::from(icon))
    }
}

#[cfg(feature = "lucide")]
impl Glyph {
    /// `icon` in the set native to `ctx`'s theme: the Segoe Fluent Icons glyph
    /// under the Fluent family when the font is there, otherwise Lucide.
    #[must_use]
    pub fn themed(ctx: &egui::Context, icon: lucide_icons::Icon) -> Self {
        let family = FontFamily::Name(FLUENT_ICONS_FAMILY.into());
        match fluent_codepoint(icon) {
            Some(ch)
                if crate::ThemeFamily::of_ctx(ctx) == crate::ThemeFamily::Fluent
                    && ctx.fonts(|fonts| fonts.definitions().families.contains_key(&family)) =>
            {
                Self { ch, family }
            }
            _ => Self::from(icon),
        }
    }
}

/// The Segoe Fluent Icons codepoint matching a Lucide `icon`, for icons with a
/// clear match. `None` keeps the Lucide glyph.
#[cfg(feature = "lucide")]
#[must_use]
pub const fn fluent_codepoint(icon: lucide_icons::Icon) -> Option<char> {
    use lucide_icons::Icon;
    let codepoint = match icon {
        Icon::Plus => '\u{E710}',
        Icon::X => '\u{E711}',
        Icon::Settings => '\u{E713}',
        Icon::Video => '\u{E714}',
        Icon::Search => '\u{E721}',
        Icon::Cctv | Icon::Camera => '\u{E722}',
        Icon::RefreshCw => '\u{E72C}',
        Icon::Trash2 => '\u{E74D}',
        Icon::ChevronLeft => '\u{E76B}',
        Icon::ChevronRight => '\u{E76C}',
        Icon::Globe => '\u{E774}',
        Icon::ClipboardPaste => '\u{E77F}',
        Icon::Redo2 => '\u{E7A6}',
        Icon::Undo2 => '\u{E7A7}',
        Icon::TriangleAlert => '\u{E7BA}',
        Icon::Monitor => '\u{E7F4}',
        Icon::Laptop => '\u{E7F8}',
        Icon::Home | Icon::House => '\u{E80F}',
        Icon::FolderOpen => '\u{E838}',
        Icon::Eye => '\u{E890}',
        Icon::Download => '\u{E896}',
        Icon::FileText => '\u{E8A5}',
        Icon::Folder => '\u{E8B7}',
        Icon::Copy => '\u{E8C8}',
        Icon::Music => '\u{E8D6}',
        Icon::KeyRound => '\u{E8D7}',
        Icon::Check => '\u{E8FB}',
        Icon::Wrench => '\u{E90F}',
        Icon::CircleCheck => '\u{E930}',
        Icon::Info => '\u{E946}',
        Icon::SlidersHorizontal => '\u{E9E9}',
        Icon::ShieldCheck => '\u{EA18}',
        Icon::Image => '\u{EB9F}',
        Icon::Blocks => '\u{ECAA}',
        Icon::EyeOff => '\u{ED1A}',
        _ => return None,
    };
    Some(codepoint)
}

/// A Lucide icon as rich text at the default size.
#[cfg(feature = "lucide")]
#[must_use]
pub fn glyph(icon: lucide_icons::Icon) -> RichText {
    Glyph::from(icon).rich(ICON_SIZE)
}

/// An icon font to register.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconFont {
    /// The whole Lucide font (about 900 KB), from `lucide-icons`.
    #[cfg(feature = "lucide")]
    Lucide,
    /// A subset of Lucide cut for the app. Lucide's font ids are stable, so
    /// `lucide_icons::Icon` values keep working.
    Subset(&'static [u8]),
    /// Any other icon font, such as Phosphor or Material, under `name`.
    Custom {
        /// Name of the font and of its family.
        name: &'static str,
        /// The font file.
        bytes: &'static [u8],
    },
}

impl IconFont {
    fn name(self) -> &'static str {
        match self {
            #[cfg(feature = "lucide")]
            Self::Lucide => "lucide",
            Self::Subset(_) => "lucide",
            Self::Custom { name, .. } => name,
        }
    }

    fn bytes(self) -> &'static [u8] {
        match self {
            #[cfg(feature = "lucide")]
            Self::Lucide => lucide_icons::LUCIDE_FONT_BYTES,
            Self::Subset(bytes) | Self::Custom { bytes, .. } => bytes,
        }
    }

    /// Add this font to `fonts`, as the last `Proportional` fallback and as a
    /// family of its own.
    pub fn register(self, fonts: &mut FontDefinitions) {
        let name = self.name();
        // Icon fonts ship no hinting instructions, so hinting only rounds their
        // phantom points, which jitters icon bounds by a pixel. Pinning it off
        // also makes a subset lay out exactly like the font it was cut from.
        let mut data = FontData::from_static(self.bytes());
        data.tweak.hinting = Some(false);
        fonts.font_data.insert(name.to_owned(), Arc::new(data));
        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .push(name.to_owned());
        fonts
            .families
            .entry(FontFamily::Name(name.into()))
            .or_default()
            .push(name.to_owned());
    }
}

#[cfg(all(test, feature = "lucide"))]
mod tests {
    use egui::{Context, FontFamily};
    use lucide_icons::Icon;

    use super::{FLUENT_ICONS_FAMILY, Glyph, fluent_codepoint};
    use crate::{AccentShades, FontOptions, fluent, fonts, run_frame};

    #[test]
    fn mapped_icons_are_in_segoe_private_use_area() {
        for icon in [Icon::Plus, Icon::Trash2, Icon::Search, Icon::EyeOff] {
            let codepoint = fluent_codepoint(icon).expect("mapped");
            assert!(('\u{E700}'..='\u{F8FF}').contains(&codepoint));
        }
    }

    #[test]
    fn graphite_keeps_lucide() {
        let ctx = Context::default();
        run_frame(&ctx, |_| {});
        assert_eq!(Glyph::themed(&ctx, Icon::Plus), Glyph::from(Icon::Plus));
    }

    #[test]
    fn fluent_falls_back_to_lucide_without_the_font() {
        let ctx = Context::default();
        fluent::theme(AccentShades::WINDOWS_DEFAULT).install(&ctx);
        fonts::install_test(&ctx, &FontOptions::default());
        run_frame(&ctx, |_| {});
        assert_eq!(Glyph::themed(&ctx, Icon::Plus), Glyph::from(Icon::Plus));
    }

    #[test]
    fn fluent_uses_the_font_when_it_is_there() {
        let ctx = Context::default();
        fluent::theme(AccentShades::WINDOWS_DEFAULT).install(&ctx);
        let mut definitions = egui::FontDefinitions::default();
        definitions.families.insert(
            FontFamily::Name(FLUENT_ICONS_FAMILY.into()),
            definitions.families[&FontFamily::Proportional].clone(),
        );
        ctx.set_fonts(definitions);
        run_frame(&ctx, |_| {});
        let glyph = Glyph::themed(&ctx, Icon::Plus);
        assert_eq!(glyph.ch, '\u{E710}');
        assert_eq!(glyph.family, FontFamily::Name(FLUENT_ICONS_FAMILY.into()));
        // An icon with no match stays Lucide.
        assert_eq!(Glyph::themed(&ctx, Icon::Anvil), Glyph::from(Icon::Anvil));
    }
}
