//! A response-curve editor over 0–1 (tablet pressure, brush dynamics), in
//! the look of a Curves adjustment: click to add a point, drag to move one,
//! right-click to remove it. The end points stay at the edges.

use egui::{CornerRadius, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui, Vec2, pos2};
use egui_mev_core::spline::Curve;
use egui_mev_theme::Palette;

/// How close (points) the pointer must be to grab a control point.
const GRAB: f32 = 7.0;

/// Least horizontal gap between neighbouring control points (0–1).
const GAP: f32 = 0.01;

/// Corner radius of the graph. Its diagonal and curve run into the corners, so
/// it stays below the larger radius controls use.
const RADIUS: u8 = 2;

/// Edit `points` in a graph of `size`; `marker` draws a vertical line at
/// that input (the pen's live pressure). Returns whether they changed.
pub fn curve_editor(
    ui: &mut Ui,
    points: &mut Vec<[f32; 2]>,
    size: Vec2,
    marker: Option<f32>,
) -> bool {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let changed = interact(ui, &response, rect, points);
    paint(ui, rect, points, true, marker);
    changed
}

/// A small read-only drawing of `points`, clickable.
pub fn curve_thumbnail(ui: &mut Ui, points: &[[f32; 2]], size: Vec2) -> Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    paint(ui, rect, points, false, None);
    response
}

fn interact(ui: &Ui, response: &Response, rect: Rect, points: &mut Vec<[f32; 2]>) -> bool {
    let to_screen = |p: [f32; 2]| {
        pos2(
            rect.left() + p[0] * rect.width(),
            rect.bottom() - p[1] * rect.height(),
        )
    };
    let to_value = |p: Pos2| {
        [
            ((p.x - rect.left()) / rect.width()).clamp(0.0, 1.0),
            ((rect.bottom() - p.y) / rect.height()).clamp(0.0, 1.0),
        ]
    };
    let mut changed = false;
    if points.len() < 2 {
        *points = vec![[0.0, 0.0], [1.0, 1.0]];
        changed = true;
    }
    let drag_id = response.id.with("drag");
    let mut dragging: Option<usize> = ui.data(|d| d.get_temp(drag_id)).flatten();
    if let Some(pos) = response.interact_pointer_pos() {
        // A drag only starts once the pointer has travelled, by which time it
        // can be past `GRAB`, so a point is grabbed where the press began.
        let origin = ui.input(|i| i.pointer.press_origin()).unwrap_or(pos);
        let near = points
            .iter()
            .position(|&p| to_screen(p).distance(origin) < GRAB);
        if response.drag_started() || response.clicked() {
            dragging = near;
            if dragging.is_none() {
                let v = to_value(pos);
                let ix = points.partition_point(|p| p[0] < v[0]);
                if ix > 0
                    && ix < points.len()
                    && v[0] - points[ix - 1][0] >= GAP
                    && points[ix][0] - v[0] >= GAP
                {
                    points.insert(ix, v);
                    dragging = Some(ix);
                    changed = true;
                }
            }
        }
        if response.secondary_clicked()
            && let Some(i) = near
            && i != 0
            && i + 1 != points.len()
        {
            points.remove(i);
            dragging = None;
            changed = true;
        }
        if response.dragged()
            && let Some(i) = dragging
            && i < points.len()
        {
            let [mut x, y] = to_value(pos);
            let last = points.len() - 1;
            x = if i == 0 {
                0.0
            } else if i == last {
                1.0
            } else {
                // Points from elsewhere can sit closer together than the gap
                // allows, which leaves this one no room to move sideways.
                let (min, max) = (points[i - 1][0] + GAP, points[i + 1][0] - GAP);
                if min <= max {
                    x.clamp(min, max)
                } else {
                    points[i][0]
                }
            };
            if points[i] != [x, y] {
                points[i] = [x, y];
                changed = true;
            }
        }
    }
    if response.drag_stopped() {
        dragging = None;
    }
    ui.data_mut(|d| d.insert_temp(drag_id, dragging));
    changed
}

fn paint(ui: &Ui, rect: Rect, points: &[[f32; 2]], handles: bool, marker: Option<f32>) {
    let palette = Palette::of(ui);
    let painter = ui.painter();
    painter.rect(
        rect,
        CornerRadius::same(RADIUS),
        palette.extreme_bg,
        Stroke::new(1.0, palette.border),
        StrokeKind::Inside,
    );
    if handles {
        for i in 1..4 {
            let t = i as f32 / 4.0;
            painter.vline(
                rect.left() + t * rect.width(),
                rect.y_range(),
                Stroke::new(1.0, palette.border_subtle),
            );
            painter.hline(
                rect.x_range(),
                rect.top() + t * rect.height(),
                Stroke::new(1.0, palette.border_subtle),
            );
        }
    }
    painter.line_segment(
        [rect.left_bottom(), rect.right_top()],
        Stroke::new(1.0, palette.border),
    );
    let to_screen = |x: f32, y: f32| {
        pos2(
            rect.left() + x * rect.width(),
            rect.bottom() - y * rect.height(),
        )
    };
    let curve = Curve::new(points);
    let steps = (rect.width() / 3.0).ceil().max(8.0) as usize;
    let path: Vec<Pos2> = (0..=steps)
        .map(|i| {
            let x = i as f32 / steps as f32;
            to_screen(x, curve.eval(x))
        })
        .collect();
    painter.add(Shape::line(path, Stroke::new(1.5, palette.text)));
    if let Some(x) = marker {
        let x = x.clamp(0.0, 1.0);
        painter.vline(
            rect.left() + x * rect.width(),
            rect.y_range(),
            Stroke::new(1.0, palette.accent),
        );
        painter.circle_filled(to_screen(x, curve.eval(x)), 3.0, palette.accent);
    }
    if handles {
        for &p in curve.points() {
            painter.circle(
                to_screen(p[0], p[1]),
                4.0,
                palette.text,
                Stroke::new(1.0, palette.extreme_bg),
            );
        }
    }
}
