//! Measurement units for rulers and readouts.

/// Measurement unit for rulers and readouts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Unit {
    /// Pixels.
    #[default]
    Pixels,
    /// Inches.
    Inches,
    /// Centimeters.
    Centimeters,
}

impl Unit {
    /// All units in menu order.
    pub const ALL: [Self; 3] = [Self::Pixels, Self::Inches, Self::Centimeters];

    /// Menu label.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Pixels => "Pixels",
            Self::Inches => "Inches",
            Self::Centimeters => "Centimeters",
        }
    }

    /// Short suffix.
    #[must_use]
    pub const fn suffix(self) -> &'static str {
        match self {
            Self::Pixels => "px",
            Self::Inches => "in",
            Self::Centimeters => "cm",
        }
    }

    /// Image pixels per unit at `dpi`.
    #[must_use]
    pub fn pixels_per_unit(self, dpi: f64) -> f64 {
        match self {
            Self::Pixels => 1.0,
            Self::Inches => dpi,
            Self::Centimeters => dpi / 2.54,
        }
    }

    /// `pixels` expressed in this unit, formatted.
    #[must_use]
    pub fn format(self, pixels: f64, dpi: f64) -> String {
        match self {
            Self::Pixels => format!("{pixels:.0}"),
            _ => format!("{:.2}", pixels / self.pixels_per_unit(dpi)),
        }
    }
}
