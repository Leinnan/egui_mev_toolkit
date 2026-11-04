//! Small marks the widgets paint for themselves, so they need no icon font.

use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, vec2};

use crate::containers::BannerKind;

const LINE: f32 = 1.5;

/// Which way a chevron points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chevron {
    Right,
    Down,
}

/// A chevron centred at `center` that spans `size`.
pub fn chevron(painter: &Painter, center: Pos2, size: f32, direction: Chevron, color: Color32) {
    let (long, short) = (size / 2.0, size / 4.0);
    let points = match direction {
        Chevron::Right => vec![
            center + vec2(-short, -long),
            center + vec2(short, 0.0),
            center + vec2(-short, long),
        ],
        Chevron::Down => vec![
            center + vec2(-long, -short),
            center + vec2(0.0, short),
            center + vec2(long, -short),
        ],
    };
    painter.add(Shape::line(points, Stroke::new(LINE, color)));
}

/// An × centred at `center` that spans `size`.
pub fn cross(painter: &Painter, center: Pos2, size: f32, color: Color32) {
    let half = size / 2.0;
    let stroke = Stroke::new(LINE, color);
    painter.line_segment(
        [center + vec2(-half, -half), center + vec2(half, half)],
        stroke,
    );
    painter.line_segment(
        [center + vec2(-half, half), center + vec2(half, -half)],
        stroke,
    );
}

/// A magnifying glass filling `rect`.
pub fn magnifier(painter: &Painter, rect: Rect, color: Color32) {
    let stroke = Stroke::new(LINE, color);
    let radius = rect.width() * 0.32;
    let center = rect.min + vec2(radius + 0.5, radius + 0.5);
    painter.circle_stroke(center, radius, stroke);
    let handle = radius * std::f32::consts::FRAC_1_SQRT_2;
    painter.line_segment(
        [center + vec2(handle, handle), rect.max - vec2(0.5, 0.5)],
        stroke,
    );
}

/// The severity icon of `kind`: a ring with a mark inside, in `color`.
pub fn severity(painter: &Painter, rect: Rect, kind: BannerKind, color: Color32) {
    let center = rect.center();
    let radius = rect.width().min(rect.height()) / 2.0 - 0.75;
    let stroke = Stroke::new(LINE, color);
    painter.circle_stroke(center, radius, stroke);
    let unit = radius * 0.5;
    match kind {
        BannerKind::Info => {
            painter.circle_filled(center - vec2(0.0, unit), 0.9, color);
            painter.line_segment(
                [center - vec2(0.0, unit * 0.2), center + vec2(0.0, unit)],
                stroke,
            );
        }
        BannerKind::Success => {
            let points = vec![
                center + vec2(-unit, 0.0),
                center + vec2(-unit * 0.2, unit * 0.8),
                center + vec2(unit, -unit * 0.7),
            ];
            painter.add(Shape::line(points, stroke));
        }
        BannerKind::Warning => {
            painter.line_segment(
                [center - vec2(0.0, unit), center + vec2(0.0, unit * 0.2)],
                stroke,
            );
            painter.circle_filled(center + vec2(0.0, unit), 0.9, color);
        }
        BannerKind::Error => {
            cross(painter, center, unit * 1.5, color);
        }
    }
}
