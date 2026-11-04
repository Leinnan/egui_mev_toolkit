use egui::Color32;

/// The shades of an accent colour, lightest first, as Windows stores them in its
/// `AccentPalette` registry value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccentShades {
    /// `SystemAccentColorLight3`.
    pub light3: Color32,
    /// `SystemAccentColorLight2`: the accent fill in dark Fluent.
    pub light2: Color32,
    /// `SystemAccentColorLight1`.
    pub light1: Color32,
    /// `SystemAccentColor`: the accent itself.
    pub base: Color32,
    /// `SystemAccentColorDark1`: the accent fill in light Fluent.
    pub dark1: Color32,
    /// `SystemAccentColorDark2`.
    pub dark2: Color32,
    /// `SystemAccentColorDark3`.
    pub dark3: Color32,
}

impl AccentShades {
    /// The Windows 11 default blue accent.
    pub const WINDOWS_DEFAULT: Self = Self {
        light3: Color32::from_rgb(0x99, 0xEB, 0xFF),
        light2: Color32::from_rgb(0x60, 0xCD, 0xFF),
        light1: Color32::from_rgb(0x00, 0x91, 0xF8),
        base: Color32::from_rgb(0x00, 0x78, 0xD4),
        dark1: Color32::from_rgb(0x00, 0x5F, 0xB8),
        dark2: Color32::from_rgb(0x00, 0x3E, 0x92),
        dark3: Color32::from_rgb(0x00, 0x1A, 0x68),
    };

    /// The macOS default blue accent.
    pub const MACOS_DEFAULT: Self = Self {
        light3: Color32::from_rgb(0x9D, 0xCE, 0xFF),
        light2: Color32::from_rgb(0x6C, 0xB5, 0xFF),
        light1: Color32::from_rgb(0x3B, 0x9C, 0xFF),
        base: Color32::from_rgb(0x0A, 0x84, 0xFF),
        dark1: Color32::from_rgb(0x00, 0x70, 0xE8),
        dark2: Color32::from_rgb(0x00, 0x55, 0xB3),
        dark3: Color32::from_rgb(0x00, 0x3A, 0x7A),
    };

    /// Shades around one colour, for an OS or app that gives only one.
    #[must_use]
    pub fn from_base(base: Color32) -> Self {
        let tint = |amount| base.lerp_to_gamma(Color32::WHITE, amount);
        let shade = |amount| base.lerp_to_gamma(Color32::BLACK, amount);
        Self {
            light3: tint(0.6),
            light2: tint(0.4),
            light1: tint(0.2),
            base,
            dark1: shade(0.2),
            dark2: shade(0.4),
            dark3: shade(0.6),
        }
    }

    /// Whether this is the blue macOS uses by default, which reads differently
    /// in dark mode (`#0A84FF`) and light mode (`#007AFF`).
    #[must_use]
    pub fn is_macos_blue(&self) -> bool {
        self.base == Self::MACOS_DEFAULT.base || self.base == Color32::from_rgb(0x00, 0x7A, 0xFF)
    }

    /// Parses the 32-byte `AccentPalette` registry value: eight RGBA
    /// quadruplets, lightest first (the eighth entry is unused).
    ///
    /// Returns `None` when the value is shorter than seven colours.
    #[must_use]
    pub fn from_accent_palette_bytes(bytes: &[u8]) -> Option<Self> {
        let color = |index: usize| {
            let chunk = bytes.get(index * 4..index * 4 + 3)?;
            Some(Color32::from_rgb(chunk[0], chunk[1], chunk[2]))
        };
        Some(Self {
            light3: color(0)?,
            light2: color(1)?,
            light1: color(2)?,
            base: color(3)?,
            dark1: color(4)?,
            dark2: color(5)?,
            dark3: color(6)?,
        })
    }

    /// The default accent of the OS this was built for.
    #[must_use]
    pub const fn platform_default() -> Self {
        if cfg!(windows) {
            Self::WINDOWS_DEFAULT
        } else {
            Self::MACOS_DEFAULT
        }
    }

    /// The user's accent, read from the OS with the `native` feature on Windows
    /// and macOS. Otherwise, or when the OS doesn't answer, the platform's default.
    #[must_use]
    pub fn system() -> Self {
        #[cfg(all(feature = "native", windows))]
        if let Some(shades) = crate::win32::system_accent() {
            return shades;
        }
        #[cfg(all(feature = "native", target_os = "macos"))]
        if let Some(base) = macos_accent() {
            return Self::from_base(base);
        }
        Self::platform_default()
    }
}

/// `NSColor.controlAccentColor` in sRGB. It's a dynamic colour, so the result
/// reflects the appearance current when this runs.
#[cfg(all(feature = "native", target_os = "macos"))]
fn macos_accent() -> Option<Color32> {
    use objc2_app_kit::{NSColor, NSColorSpace};

    let color =
        NSColor::controlAccentColor().colorUsingColorSpace(&NSColorSpace::sRGBColorSpace())?;
    let channel = |value: f64| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    Some(Color32::from_rgb(
        channel(color.redComponent()),
        channel(color.greenComponent()),
        channel(color.blueComponent()),
    ))
}

#[cfg(test)]
mod tests {
    use egui::Color32;

    use super::AccentShades;

    #[test]
    fn parses_accent_palette_registry_bytes() {
        let bytes: [u8; 32] = [
            0x99, 0xEB, 0xFF, 0x00, 0x4C, 0xC2, 0xFF, 0x00, 0x00, 0x91, 0xF8, 0x00, 0x00, 0x78,
            0xD4, 0x00, 0x00, 0x67, 0xC0, 0x00, 0x00, 0x3E, 0x92, 0x00, 0x00, 0x1A, 0x68, 0x00,
            0xF7, 0x63, 0x0C, 0x00,
        ];
        let shades = AccentShades::from_accent_palette_bytes(&bytes).expect("valid palette");
        assert_eq!(shades.light2, Color32::from_rgb(0x4C, 0xC2, 0xFF));
        assert_eq!(shades.base, Color32::from_rgb(0x00, 0x78, 0xD4));
        assert_eq!(shades.dark3, Color32::from_rgb(0x00, 0x1A, 0x68));
    }

    #[test]
    fn rejects_truncated_accent_palette() {
        assert_eq!(AccentShades::from_accent_palette_bytes(&[0; 20]), None);
    }

    #[test]
    fn shades_from_one_colour_run_light_to_dark() {
        let shades = AccentShades::from_base(Color32::from_rgb(0x80, 0x40, 0xC0));
        let brightness = |c: Color32| u32::from(c.r()) + u32::from(c.g()) + u32::from(c.b());
        let ramp = [
            shades.light3,
            shades.light2,
            shades.light1,
            shades.base,
            shades.dark1,
            shades.dark2,
            shades.dark3,
        ];
        assert!(ramp.windows(2).all(|p| brightness(p[0]) > brightness(p[1])));
    }

    #[test]
    fn both_macos_blues_count_as_the_default() {
        assert!(AccentShades::MACOS_DEFAULT.is_macos_blue());
        assert!(AccentShades::from_base(Color32::from_rgb(0x00, 0x7A, 0xFF)).is_macos_blue());
        assert!(!AccentShades::WINDOWS_DEFAULT.is_macos_blue());
    }

    #[test]
    fn the_system_accent_always_answers() {
        // Whatever the OS says, or the platform's default.
        let shades = AccentShades::system();
        assert_ne!(shades.base, Color32::TRANSPARENT);
    }
}
