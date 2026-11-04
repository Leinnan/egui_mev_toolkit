//! Icon fonts and the glyphs drawn from them.
//!
//! An icon font is registered as the last `Proportional` fallback, so its
//! codepoints render in any text. A named family alone isn't enough: it isn't
//! bound on the very first frame after `set_fonts`, and looking it up then panics.

use std::sync::Arc;

use egui::{FontData, FontDefinitions, FontFamily, FontId, RichText};

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
