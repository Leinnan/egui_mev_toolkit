//! Rulers along the top and left of a canvas (OpenPDN `Core/Ruler.cs`),
//! in pixels, inches or centimeters, with a marker at the cursor.

use egui::{Align2, FontId, Painter, Pos2, Rect, Stroke, pos2};
use egui_mev_theme::Palette;

use crate::units::Unit;

/// Ruler thickness (points).
pub const RULER_SIZE: f32 = 18.0;

/// Distance between labelled ticks, in units: the smallest "nice" step
/// that keeps labels at least ~60 points apart.
#[must_use]
pub fn major_step(points_per_unit: f64) -> f64 {
    const NICE: [f64; 3] = [1.0, 2.0, 5.0];
    let mut magnitude = 1e-3;
    loop {
        for n in NICE {
            let step = n * magnitude;
            if step * points_per_unit >= 60.0 {
                return step;
            }
        }
        magnitude *= 10.0;
        if magnitude > 1e9 {
            return magnitude;
        }
    }
}

/// Where the rulers go: along the top and left of the canvas, and the
/// corner square between them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RulerLayout {
    /// Horizontal ruler.
    pub top: Rect,
    /// Vertical ruler.
    pub left: Rect,
    /// Square where they meet.
    pub corner: Rect,
}

impl RulerLayout {
    /// Rulers filling the space between the `full` canvas area and the
    /// image `viewport` inset from its top-left.
    #[must_use]
    pub fn new(full: Rect, viewport: Rect) -> Self {
        Self {
            top: Rect::from_min_max(
                pos2(viewport.left(), full.top()),
                pos2(full.right(), viewport.top()),
            ),
            left: Rect::from_min_max(
                pos2(full.left(), viewport.top()),
                pos2(viewport.left(), full.bottom()),
            ),
            corner: Rect::from_min_max(full.min, viewport.min),
        }
    }
}

/// How image pixels map to ruler marks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RulerScale {
    /// Screen position of image pixel (0, 0).
    pub origin: Pos2,
    /// Screen points per image pixel.
    pub zoom: f32,
    /// Unit the marks count in.
    pub unit: Unit,
    /// Image resolution (for inches and centimeters).
    pub dpi: f64,
}

/// Draw both rulers, with a marker at `cursor` (screen space).
pub fn paint(
    painter: &Painter,
    palette: &Palette,
    layout: RulerLayout,
    scale: Option<RulerScale>,
    cursor: Option<Pos2>,
) {
    let RulerLayout { top, left, corner } = layout;
    painter.rect_filled(top, 0.0, palette.sidebar_bg);
    painter.rect_filled(left, 0.0, palette.sidebar_bg);
    painter.rect_filled(corner, 0.0, palette.sidebar_bg);
    painter.hline(
        top.x_range(),
        top.bottom() - 0.5,
        Stroke::new(1.0, palette.border_subtle),
    );
    painter.vline(
        left.right() - 0.5,
        left.y_range(),
        Stroke::new(1.0, palette.border_subtle),
    );
    // No scale (a turned view): blank rulers.
    let Some(RulerScale {
        origin,
        zoom,
        unit,
        dpi,
    }) = scale
    else {
        return;
    };
    let points_per_unit = unit.pixels_per_unit(dpi) * f64::from(zoom);
    let major = major_step(points_per_unit);
    let minor = major
        / if major / 5.0 * points_per_unit >= 6.0 {
            5.0
        } else {
            2.0
        };
    // No usable scale (an image with no resolution): the tick loop below would
    // otherwise run over the whole range of i64.
    let tick = minor * points_per_unit;
    if !tick.is_finite() || tick < 1.0 {
        return;
    }
    let font = FontId::proportional(10.0);
    for horizontal in [true, false] {
        let (rect, start, end, o) = if horizontal {
            (top, top.left(), top.right(), origin.x)
        } else {
            (left, left.top(), left.bottom(), origin.y)
        };
        let painter = painter.with_clip_rect(rect);
        let first = ((f64::from(start - o)) / points_per_unit / minor).floor() as i64;
        let last = ((f64::from(end - o)) / points_per_unit / minor).ceil() as i64;
        for i in first..=last {
            let value = i as f64 * minor;
            let pos = o + (value * points_per_unit) as f32;
            let is_major = ((value / major) - (value / major).round()).abs() < 1e-6;
            let len = if is_major {
                RULER_SIZE * 0.6
            } else {
                RULER_SIZE * 0.25
            };
            let color = if is_major {
                palette.text_weak
            } else {
                palette.hint_text
            };
            if horizontal {
                painter.vline(
                    pos,
                    (rect.bottom() - len)..=rect.bottom(),
                    Stroke::new(1.0, color),
                );
                if is_major {
                    painter.text(
                        pos2(pos + 2.0, rect.top() + 1.0),
                        Align2::LEFT_TOP,
                        label(value, major),
                        font.clone(),
                        palette.text_weak,
                    );
                }
            } else {
                painter.hline(
                    (rect.right() - len)..=rect.right(),
                    pos,
                    Stroke::new(1.0, color),
                );
                if is_major {
                    painter.text(
                        pos2(rect.left() + 1.0, pos + 2.0),
                        Align2::LEFT_TOP,
                        stacked(&label(value, major)),
                        font.clone(),
                        palette.text_weak,
                    );
                }
            }
        }
        if let Some(c) = cursor {
            let marker = palette.accent;
            if horizontal && top.x_range().contains(c.x) {
                painter.vline(c.x, rect.y_range(), Stroke::new(1.0, marker));
            } else if !horizontal && left.y_range().contains(c.y) {
                painter.hline(rect.x_range(), c.y, Stroke::new(1.0, marker));
            }
        }
    }
}

/// `value`, with enough decimals to tell labels `major` apart.
fn label(value: f64, major: f64) -> String {
    if (value - value.round()).abs() < 1e-6 {
        format!("{}", value.round() as i64)
    } else {
        let decimals = (-major.log10() - 1e-9).ceil().clamp(1.0, 3.0) as usize;
        format!("{value:.decimals$}")
    }
}

/// `text` one character a line, for the vertical ruler: too narrow for "1000"
/// across, which clipped would read as "100". Major ticks leave room for it.
fn stacked(text: &str) -> String {
    let lines: Vec<String> = text.chars().map(String::from).collect();
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use egui::{Rect, pos2};
    use egui_mev_theme::Palette;

    use super::{RULER_SIZE, RulerLayout, RulerScale, Unit, label, major_step, paint, stacked};

    #[test]
    fn steps_and_units() {
        assert!((major_step(1.0) - 100.0).abs() < 1e-9);
        assert!((major_step(4.0) - 20.0).abs() < 1e-9);
        assert!((major_step(96.0) - 1.0).abs() < 1e-9);
        assert_eq!(Unit::Inches.format(192.0, 96.0), "2.00");
        assert_eq!(Unit::Pixels.format(12.4, 96.0), "12");
        assert!((Unit::Centimeters.pixels_per_unit(254.0) - 100.0).abs() < 1e-9);
    }

    #[test]
    fn major_steps_stay_nice_and_far_enough_apart() {
        for (points_per_unit, step) in [
            (0.1, 1000.0),
            (0.5, 200.0),
            (1.5, 50.0),
            (6.0, 10.0),
            (12.0, 5.0),
            (30.0, 2.0),
            (60.0, 1.0),
            (120.0, 0.5),
            (3072.0, 0.02),
            (1e6, 1e-3),
        ] {
            let found = major_step(points_per_unit);
            assert!((found - step).abs() < 1e-9, "{points_per_unit}: {found}");
            assert!(found * points_per_unit >= 60.0 - 1e-9, "{points_per_unit}");
        }
    }

    #[test]
    fn major_step_gives_up_on_a_useless_scale() {
        for points_per_unit in [0.0, -1.0, f64::NAN] {
            assert!(major_step(points_per_unit) > 1e9);
        }
    }

    #[test]
    fn layout_fills_the_inset() {
        let full = Rect::from_min_max(pos2(10.0, 20.0), pos2(310.0, 220.0));
        let viewport = Rect::from_min_max(pos2(28.0, 38.0), pos2(310.0, 220.0));
        let layout = RulerLayout::new(full, viewport);
        assert_eq!(
            layout.top,
            Rect::from_min_max(pos2(28.0, 20.0), pos2(310.0, 38.0))
        );
        assert_eq!(
            layout.left,
            Rect::from_min_max(pos2(10.0, 38.0), pos2(28.0, 220.0))
        );
        assert_eq!(
            layout.corner,
            Rect::from_min_max(pos2(10.0, 20.0), pos2(28.0, 38.0))
        );
        assert_eq!(layout.top.height(), RULER_SIZE);
        assert_eq!(layout.left.width(), RULER_SIZE);
    }

    #[test]
    fn labels_tell_small_steps_apart() {
        assert_eq!(label(3.0, 1.0), "3");
        assert_eq!(label(-100.0, 50.0), "-100");
        assert_eq!(label(0.5, 0.5), "0.5");
        assert_eq!(label(0.2, 0.1 + 0.1), "0.2");
        // With one decimal these two would both read 0.0.
        assert_eq!(label(0.02, 0.02), "0.02");
        assert_eq!(label(0.04, 0.02), "0.04");
        assert_eq!(label(0.05, major_step(1200.0)), "0.05");
        assert_eq!(label(0.1, major_step(600.0)), "0.1");
        assert_eq!(label(0.003, 0.001), "0.003");
    }

    #[test]
    fn vertical_labels_stack() {
        assert_eq!(stacked("1000"), "1\n0\n0\n0");
        assert_eq!(stacked("0"), "0");
    }

    /// Shapes [`paint`] adds. A scale with no ticks returning at all is half of
    /// what this checks: a zero scale used to loop over the range of `i64`.
    fn shapes_painted(zoom: f32, dpi: f64, unit: Unit) -> usize {
        let ctx = egui::Context::default();
        let full = Rect::from_min_max(pos2(0.0, 0.0), pos2(300.0, 200.0));
        let viewport = Rect::from_min_max(pos2(RULER_SIZE, RULER_SIZE), full.max);
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            paint(
                ui.painter(),
                &Palette::of(ui),
                RulerLayout::new(full, viewport),
                Some(RulerScale {
                    origin: viewport.min,
                    zoom,
                    unit,
                    dpi,
                }),
                None,
            );
        });
        output.textures_delta.clear();
        output.shapes.len()
    }

    #[test]
    fn scale_without_a_size_paints_blank_rulers() {
        let blank = shapes_painted(0.0, 96.0, Unit::Pixels);
        assert!(blank <= 8, "{blank} shapes");
        assert_eq!(shapes_painted(1.0, 0.0, Unit::Inches), blank);
        assert_eq!(shapes_painted(1.0, 0.0, Unit::Centimeters), blank);
        assert_eq!(shapes_painted(-1.0, 96.0, Unit::Pixels), blank);
        assert_eq!(shapes_painted(f32::NAN, 96.0, Unit::Pixels), blank);
        assert_eq!(shapes_painted(1e-30, 96.0, Unit::Pixels), blank);
        assert!(shapes_painted(1.0, 96.0, Unit::Pixels) > blank);
    }

    #[test]
    fn units_are_named_in_menu_order() {
        let names = Unit::ALL.map(Unit::name);
        assert_eq!(names, ["Pixels", "Inches", "Centimeters"]);
        let suffixes = Unit::ALL.map(Unit::suffix);
        assert_eq!(suffixes, ["px", "in", "cm"]);
        assert_eq!(Unit::default(), Unit::Pixels);
    }
}
