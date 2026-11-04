//! Paint.NET-style color editor: a hue/saturation wheel, value and alpha bars,
//! and hex / RGB / HSV fields, plus a Krita-style hue ring and triangle.
//!
//! Colors are straight (unmultiplied) RGBA `[u8; 4]`. `egui::Color32` is
//! premultiplied, so it is only used for painting: going through it would lose
//! the color of low-alpha values.

use egui::{
    Color32, CornerRadius, DragValue, Mesh, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind,
    TextEdit, Ui, Vec2, pos2, vec2,
};
use egui_mev_theme::Palette;

/// A color in hue/saturation/value (+ alpha), kept separately from RGB so
/// hue survives fully desaturated or black colors while editing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hsva {
    /// Hue, degrees 0–360.
    pub h: f32,
    /// Saturation 0–1.
    pub s: f32,
    /// Value 0–1.
    pub v: f32,
    /// Alpha 0–255.
    pub a: u8,
}

impl Hsva {
    /// Convert from straight RGBA (hue 0 for grays).
    #[must_use]
    pub fn from_rgba([r, g, b, a]: [u8; 4]) -> Self {
        let (r, g, b) = (
            f32::from(r) / 255.0,
            f32::from(g) / 255.0,
            f32::from(b) / 255.0,
        );
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let d = max - min;
        let h = if d == 0.0 {
            0.0
        } else if max == r {
            60.0 * ((g - b) / d).rem_euclid(6.0)
        } else if max == g {
            60.0 * ((b - r) / d + 2.0)
        } else {
            60.0 * ((r - g) / d + 4.0)
        };
        Self {
            h,
            s: if max == 0.0 { 0.0 } else { d / max },
            v: max,
            a,
        }
    }

    /// Convert to straight RGBA.
    #[must_use]
    pub fn to_rgba(self) -> [u8; 4] {
        let c = self.v * self.s;
        let hp = (self.h.rem_euclid(360.0)) / 60.0;
        let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
        let (r, g, b) = match hp as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        let m = self.v - c;
        let to = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
        [to(r), to(g), to(b), self.a]
    }
}

/// For painting only: this premultiplies.
fn to_color32([r, g, b, a]: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

/// `RRGGBB`, or `RRGGBBAA` unless opaque.
fn to_hex([r, g, b, a]: [u8; 4]) -> String {
    if a == 255 {
        format!("{r:02X}{g:02X}{b:02X}")
    } else {
        format!("{r:02X}{g:02X}{b:02X}{a:02X}")
    }
}

/// Edit `color`, straight RGBA; returns true when it changed. `id` keys the
/// HSV state.
pub fn color_editor(ui: &mut Ui, id: egui::Id, color: &mut [u8; 4]) -> bool {
    let palette = Palette::of(ui);
    let mut hsva: Hsva = ui
        .data(|d| d.get_temp(id))
        .filter(|h: &Hsva| h.to_rgba() == *color)
        .unwrap_or_else(|| Hsva::from_rgba(*color));
    let before = hsva;
    let mut rgb_changed = false;

    ui.horizontal(|ui| {
        let size = (ui.available_width() - 64.0).clamp(96.0, 150.0);
        wheel(ui, size, &mut hsva);
        ui.add_space(4.0);
        let height = size;
        bar(
            ui,
            height,
            &mut hsva.v,
            |t| {
                to_color32(
                    Hsva {
                        v: t,
                        a: 255,
                        ..hsva
                    }
                    .to_rgba(),
                )
            },
            false,
            "Value",
        );
        let mut alpha = f32::from(hsva.a) / 255.0;
        if bar(
            ui,
            height,
            &mut alpha,
            |t| {
                to_color32(
                    Hsva {
                        a: (t * 255.0) as u8,
                        ..hsva
                    }
                    .to_rgba(),
                )
            },
            true,
            "Alpha",
        )
        .changed()
        {
            hsva.a = (alpha * 255.0).round() as u8;
        }
    });

    let mut rgb = hsva.to_rgba();
    ui.horizontal(|ui| {
        // While the field has focus it edits a draft, so partial input
        // ("F8") isn't replaced by the current color every frame; a valid
        // draft applies as it is typed, an invalid one is dropped on blur.
        let draft_id = id.with("hex_draft");
        let field_id = id.with("hex_field");
        let focused = ui.memory(|m| m.has_focus(field_id));
        let mut hex = if focused {
            ui.data(|d| d.get_temp::<String>(draft_id))
                .unwrap_or_else(|| to_hex(rgb))
        } else {
            to_hex(rgb)
        };
        let caption = ui.label(egui::RichText::new("Hex").small().color(palette.text_weak));
        let response = ui
            .add(
                TextEdit::singleline(&mut hex)
                    .id(field_id)
                    .desired_width(80.0)
                    .font(egui::TextStyle::Monospace),
            )
            .labelled_by(caption.id);
        if response.changed()
            && let Some(parsed) = parse_hex(&hex)
        {
            rgb = parsed;
            rgb_changed = true;
        }
        if response.has_focus() {
            ui.data_mut(|d| d.insert_temp(draft_id, hex));
        } else {
            ui.data_mut(|d| d.remove::<String>(draft_id));
        }
        let caption = ui.label(
            egui::RichText::new("Alpha")
                .small()
                .color(palette.text_weak),
        );
        rgb_changed |= ui
            .add(DragValue::new(&mut rgb[3]))
            .labelled_by(caption.id)
            .changed();
    });
    egui::Grid::new(id.with("fields"))
        .num_columns(6)
        // Without this the captions take a control's width, and the fields run
        // past a narrow panel.
        .min_col_width(0.0)
        .spacing([6.0, 4.0])
        .show(ui, |ui| {
            let [r, g, b, _] = &mut rgb;
            for (label, channel) in [("R", r), ("G", g), ("B", b)] {
                let caption = ui.label(egui::RichText::new(label).small().color(palette.text_weak));
                rgb_changed |= ui
                    .add(DragValue::new(channel))
                    .labelled_by(caption.id)
                    .changed();
            }
            ui.end_row();
            let caption = ui.label(egui::RichText::new("H").small().color(palette.text_weak));
            ui.add(
                DragValue::new(&mut hsva.h)
                    .range(0.0..=360.0)
                    .max_decimals(0),
            )
            .labelled_by(caption.id);
            let mut s = hsva.s * 100.0;
            let caption = ui.label(egui::RichText::new("S").small().color(palette.text_weak));
            if ui
                .add(DragValue::new(&mut s).range(0.0..=100.0).max_decimals(0))
                .labelled_by(caption.id)
                .changed()
            {
                hsva.s = s / 100.0;
            }
            let mut v = hsva.v * 100.0;
            let caption = ui.label(egui::RichText::new("V").small().color(palette.text_weak));
            if ui
                .add(DragValue::new(&mut v).range(0.0..=100.0).max_decimals(0))
                .labelled_by(caption.id)
                .changed()
            {
                hsva.v = v / 100.0;
            }
            ui.end_row();
        });

    if rgb_changed {
        let mut from_rgb = Hsva::from_rgba(rgb);
        // Keep hue for grays.
        if from_rgb.s == 0.0 {
            from_rgb.h = hsva.h;
        }
        hsva = from_rgb;
    }
    ui.data_mut(|d| d.insert_temp(id, hsva));
    if hsva != before || rgb_changed {
        let new = if rgb_changed { rgb } else { hsva.to_rgba() };
        if new != *color {
            *color = new;
            return true;
        }
    }
    false
}

fn parse_hex(text: &str) -> Option<[u8; 4]> {
    let text = text.trim().trim_start_matches('#');
    let value = u32::from_str_radix(text, 16).ok()?;
    match text.len() {
        3 => {
            // RGB shorthand: each digit doubled ("F80" is FF8800).
            let digit = |shift: u32| ((value >> shift) & 0xF) as u8 * 17;
            Some([digit(8), digit(4), digit(0), 255])
        }
        6 => {
            let [_, r, g, b] = value.to_be_bytes();
            Some([r, g, b, 255])
        }
        // RRGGBBAA as shown.
        8 => Some(value.to_be_bytes()),
        _ => None,
    }
}

/// Krita's default popup selector: a hue ring around a triangle whose
/// corners are the pure hue, white and black, turning with the hue. Pure
/// geometry, so it is unit tested.
pub mod triangle {
    use egui::{Pos2, vec2};

    use super::Hsva;

    /// Screen angle (radians, clockwise from the right as egui's y points
    /// down) of `hue` degrees: red at the top, hues running clockwise.
    #[must_use]
    pub fn hue_angle(hue: f32) -> f32 {
        (hue - 90.0).to_radians()
    }

    /// Hue (degrees) at screen angle `angle`.
    #[must_use]
    pub fn angle_hue(angle: f32) -> f32 {
        (angle.to_degrees() + 90.0).rem_euclid(360.0)
    }

    /// The point at `angle` and `radius` from `center`.
    #[must_use]
    pub fn at(center: Pos2, angle: f32, radius: f32) -> Pos2 {
        center + vec2(angle.cos(), angle.sin()) * radius
    }

    /// Corners for `hue`: pure hue, white, black.
    #[must_use]
    pub fn corners(center: Pos2, radius: f32, hue: f32) -> [Pos2; 3] {
        let a = hue_angle(hue);
        let third = std::f32::consts::TAU / 3.0;
        [
            at(center, a, radius),
            at(center, a + third, radius),
            at(center, a + 2.0 * third, radius),
        ]
    }

    /// Where saturation and value of `hsva` sit in the triangle: the mix
    /// of the corners that gives that color (hue s·v, white v·(1−s),
    /// black 1−v).
    #[must_use]
    pub fn point(center: Pos2, radius: f32, hsva: Hsva) -> Pos2 {
        let [h, w, b] = corners(center, radius, hsva.h);
        let (wh, ww, wb) = (hsva.s * hsva.v, hsva.v * (1.0 - hsva.s), 1.0 - hsva.v);
        (h.to_vec2() * wh + w.to_vec2() * ww + b.to_vec2() * wb).to_pos2()
    }

    /// Saturation and value at `p`, clamped into the triangle.
    #[must_use]
    pub fn sv_at(center: Pos2, radius: f32, hue: f32, p: Pos2) -> (f32, f32) {
        let [h, w, b] = corners(center, radius, hue);
        // Barycentric weights of p.
        let v0 = w - h;
        let v1 = b - h;
        let v2 = p - h;
        let den = v0.x * v1.y - v1.x * v0.y;
        // A triangle with no area, from a ring too small to leave room for one.
        if den == 0.0 {
            return (0.0, 0.0);
        }
        let mut ww = (v2.x * v1.y - v1.x * v2.y) / den;
        let mut wb = (v0.x * v2.y - v2.x * v0.y) / den;
        let mut wh = 1.0 - ww - wb;
        // Clamp: drop negative weights and renormalize.
        wh = wh.max(0.0);
        ww = ww.max(0.0);
        wb = wb.max(0.0);
        let sum = wh + ww + wb;
        let (wh, ww) = (wh / sum, ww / sum);
        let v = (wh + ww).clamp(0.0, 1.0);
        let s = if v <= f32::EPSILON {
            0.0
        } else {
            (wh / v).clamp(0.0, 1.0)
        };
        (s, v)
    }
}

/// What a drag on the hue ring and triangle is changing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TriangleDrag {
    /// The hue, on the ring.
    Hue,
    /// Saturation and value, in the triangle.
    Triangle,
}

/// The hue ring and triangle around `center` with outer `radius`, editing
/// `hsva`; a drag keeps changing the part it started on. Returns whether
/// the color changed.
pub fn hue_triangle(ui: &mut Ui, id: egui::Id, center: Pos2, radius: f32, hsva: &mut Hsva) -> bool {
    let ring_width = (radius * 0.18).clamp(8.0, 16.0);
    let inner = radius - ring_width - 3.0;
    let rect = Rect::from_center_size(center, Vec2::splat(radius * 2.0));
    // Rectangles are all egui hit-tests: take the pointer only over the
    // disc (or while dragging), so round neighbours keep their corners.
    let over = ui
        .input(|i| i.pointer.hover_pos())
        .is_some_and(|p| p.distance(center) <= radius);
    let sense = if over || ui.ctx().is_being_dragged(id) {
        Sense::click_and_drag()
    } else {
        Sense::hover()
    };
    let response = ui.interact(rect, id, sense);
    let before = *hsva;
    if (response.is_pointer_button_down_on() || response.clicked())
        && let Some(pos) = response.interact_pointer_pos()
    {
        // The part the press started on keeps the drag, wherever it goes.
        let origin = ui.input(|i| i.pointer.press_origin()).unwrap_or(pos);
        let r = origin.distance(center);
        let part = if r >= radius - ring_width - 1.5 {
            Some(TriangleDrag::Hue)
        } else if r <= inner + 1.0 {
            Some(TriangleDrag::Triangle)
        } else {
            None
        };
        let d = pos - center;
        match part {
            Some(TriangleDrag::Hue) => {
                hsva.h = triangle::angle_hue(d.y.atan2(d.x));
            }
            Some(TriangleDrag::Triangle) => {
                let (s, v) = triangle::sv_at(center, inner, hsva.h, pos);
                hsva.s = s;
                hsva.v = v;
            }
            None => {}
        }
    }

    let painter = ui.painter();
    // Hue ring.
    let mut mesh = Mesh::default();
    let segments = 96u32;
    let (r0, r1) = (radius - ring_width, radius);
    for seg in 0..segments {
        let (t0, t1) = (
            seg as f32 / segments as f32,
            (seg + 1) as f32 / segments as f32,
        );
        let (h0, h1) = (t0 * 360.0, t1 * 360.0);
        let color = |h: f32| {
            to_color32(
                Hsva {
                    h,
                    s: 1.0,
                    v: 1.0,
                    a: 255,
                }
                .to_rgba(),
            )
        };
        let (a0, a1) = (triangle::hue_angle(h0), triangle::hue_angle(h1));
        let base = mesh.vertices.len() as u32;
        mesh.colored_vertex(triangle::at(center, a0, r0), color(h0));
        mesh.colored_vertex(triangle::at(center, a1, r0), color(h1));
        mesh.colored_vertex(triangle::at(center, a0, r1), color(h0));
        mesh.colored_vertex(triangle::at(center, a1, r1), color(h1));
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base + 1, base + 3, base + 2);
    }
    // Triangle: the three corner colors blend to exactly the triangle's
    // colors (hue·s·v + white·v·(1−s) + black·(1−v)).
    let [h, w, b] = triangle::corners(center, inner, hsva.h);
    let pure = to_color32(
        Hsva {
            h: hsva.h,
            s: 1.0,
            v: 1.0,
            a: 255,
        }
        .to_rgba(),
    );
    let base = mesh.vertices.len() as u32;
    mesh.colored_vertex(h, pure);
    mesh.colored_vertex(w, Color32::WHITE);
    mesh.colored_vertex(b, Color32::BLACK);
    mesh.add_triangle(base, base + 1, base + 2);
    painter.add(Shape::mesh(mesh));
    // Markers.
    let hue_at = triangle::at(
        center,
        triangle::hue_angle(hsva.h),
        radius - ring_width / 2.0,
    );
    painter.circle_stroke(hue_at, ring_width / 2.0, Stroke::new(2.0, Color32::WHITE));
    painter.circle_stroke(
        hue_at,
        ring_width / 2.0 + 1.5,
        Stroke::new(1.0, Color32::BLACK),
    );
    let sv_at = triangle::point(center, inner, *hsva);
    painter.circle_stroke(sv_at, 5.0, Stroke::new(2.0, Color32::WHITE));
    painter.circle_stroke(sv_at, 6.5, Stroke::new(1.0, Color32::BLACK));
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::Unknown, true, "Color: hue ring and triangle")
    });
    *hsva != before
}

/// Hue (angle) / saturation (radius) wheel at the current value.
fn wheel(ui: &mut Ui, size: f32, hsva: &mut Hsva) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click_and_drag());
    let center = rect.center();
    let radius = size / 2.0 - 2.0;
    if let Some(pos) = response.interact_pointer_pos()
        && (response.dragged() || response.clicked())
    {
        let d = pos - center;
        hsva.h = d.y.atan2(d.x).to_degrees().rem_euclid(360.0);
        hsva.s = (d.length() / radius).clamp(0.0, 1.0);
    }
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Unknown, true, "Hue and saturation"));
    let mut mesh = Mesh::default();
    let (segments, rings) = (72u32, 8u32);
    let color_at = |angle: f32, s: f32| {
        to_color32(
            Hsva {
                h: angle.to_degrees(),
                s,
                // Full brightness like Paint.NET's wheel; value has its own bar.
                v: 1.0,
                a: 255,
            }
            .to_rgba(),
        )
    };
    let point = |angle: f32, r: f32| center + vec2(angle.cos(), angle.sin()) * r;
    for ring in 0..rings {
        let (s0, s1) = (ring as f32 / rings as f32, (ring + 1) as f32 / rings as f32);
        for seg in 0..segments {
            let (a0, a1) = (
                seg as f32 / segments as f32 * std::f32::consts::TAU,
                (seg + 1) as f32 / segments as f32 * std::f32::consts::TAU,
            );
            let base = mesh.vertices.len() as u32;
            mesh.colored_vertex(point(a0, s0 * radius), color_at(a0, s0));
            mesh.colored_vertex(point(a1, s0 * radius), color_at(a1, s0));
            mesh.colored_vertex(point(a0, s1 * radius), color_at(a0, s1));
            mesh.colored_vertex(point(a1, s1 * radius), color_at(a1, s1));
            mesh.add_triangle(base, base + 1, base + 2);
            mesh.add_triangle(base + 1, base + 3, base + 2);
        }
    }
    let border = Palette::of(ui).border;
    let painter = ui.painter();
    painter.add(Shape::mesh(mesh));
    painter.circle_stroke(center, radius, Stroke::new(1.0, border));
    let marker: Pos2 = point(hsva.h.to_radians(), hsva.s * radius);
    painter.circle_stroke(marker, 5.0, Stroke::new(2.0, Color32::WHITE));
    painter.circle_stroke(marker, 6.5, Stroke::new(1.0, Color32::BLACK));
    response
}

/// Vertical 0–1 slider painted with `color(t)`, optionally over a checker.
fn bar(
    ui: &mut Ui,
    height: f32,
    value: &mut f32,
    color: impl Fn(f32) -> Color32,
    checker: bool,
    label: &str,
) -> Response {
    let (rect, mut response) = ui.allocate_exact_size(vec2(16.0, height), Sense::click_and_drag());
    response = response.on_hover_text(label);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Slider, true, label));
    if let Some(pos) = response.interact_pointer_pos()
        && (response.dragged() || response.clicked())
    {
        *value = (1.0 - (pos.y - rect.top()) / rect.height()).clamp(0.0, 1.0);
        response.mark_changed();
    }
    let border = Palette::of(ui).border;
    let painter = ui.painter();
    if checker {
        let cell = 4.0;
        for (i, y) in (0..(rect.height() / cell).ceil() as i32).enumerate() {
            for x in 0..4 {
                if (x + i as i32) % 2 == 0 {
                    let min = pos2(rect.left() + x as f32 * cell, rect.top() + y as f32 * cell);
                    painter.rect_filled(
                        Rect::from_min_size(min, Vec2::splat(cell)).intersect(rect),
                        CornerRadius::ZERO,
                        Color32::from_gray(191),
                    );
                } else {
                    let min = pos2(rect.left() + x as f32 * cell, rect.top() + y as f32 * cell);
                    painter.rect_filled(
                        Rect::from_min_size(min, Vec2::splat(cell)).intersect(rect),
                        CornerRadius::ZERO,
                        Color32::WHITE,
                    );
                }
            }
        }
    }
    let mut mesh = Mesh::default();
    let steps = 16;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let y = rect.bottom() - t * rect.height();
        let c = color(t);
        mesh.colored_vertex(pos2(rect.left(), y), c);
        mesh.colored_vertex(pos2(rect.right(), y), c);
        if i > 0 {
            let b = (i as u32 - 1) * 2;
            mesh.add_triangle(b, b + 1, b + 2);
            mesh.add_triangle(b + 1, b + 3, b + 2);
        }
    }
    painter.add(Shape::mesh(mesh));
    painter.rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, border),
        StrokeKind::Outside,
    );
    let y = rect.bottom() - *value * rect.height();
    painter.line_segment(
        [pos2(rect.left() - 3.0, y), pos2(rect.right() + 3.0, y)],
        Stroke::new(2.0, Color32::WHITE),
    );
    painter.line_segment(
        [pos2(rect.left() - 3.0, y), pos2(rect.right() + 3.0, y)],
        Stroke::new(1.0, Color32::BLACK),
    );
    response
}

#[cfg(test)]
mod tests {
    use egui::Color32;

    use super::{Hsva, parse_hex, to_hex};

    #[test]
    fn hsv_round_trip() {
        for c in [
            [0xFF, 0x00, 0x00, 0xFF],
            [0x00, 0xFF, 0x80, 0xFF],
            [0x12, 0x34, 0x56, 0x80],
            [0x00, 0x00, 0x00, 0xFF],
            [0xFF, 0xFF, 0xFF, 0xFF],
            [0x80, 0x80, 0x80, 0xFF],
        ] {
            assert_eq!(Hsva::from_rgba(c).to_rgba(), c, "{c:02X?}");
        }
    }

    #[test]
    fn every_color_survives_the_round_trip() {
        for r in (0..=255).step_by(5) {
            for g in (0..=255).step_by(5) {
                for b in (0..=255).step_by(5) {
                    let c = [r, g, b, 200];
                    assert_eq!(Hsva::from_rgba(c).to_rgba(), c);
                }
            }
        }
    }

    #[test]
    fn low_alpha_keeps_its_rgb() {
        for c in [[200, 100, 50, 8], [255, 0, 128, 1], [12, 34, 56, 0]] {
            // What the model avoids: premultiplied storage loses the color.
            let [r, g, b, a] = c;
            assert_ne!(
                Color32::from_rgba_unmultiplied(r, g, b, a).to_srgba_unmultiplied(),
                c,
                "{c:?}"
            );
            assert_eq!(Hsva::from_rgba(c).to_rgba(), c, "{c:?}");
            assert_eq!(parse_hex(&to_hex(c)), Some(c), "{c:?}");
        }
    }

    #[test]
    fn hex_parsing() {
        assert_eq!(parse_hex("#FF6A00"), Some([0xFF, 0x6A, 0x00, 0xFF]));
        assert_eq!(parse_hex("FF6A0080"), Some([0xFF, 0x6A, 0x00, 0x80]));
        assert_eq!(parse_hex("zz"), None);
        assert_eq!(parse_hex("F80"), Some([0xFF, 0x88, 0x00, 0xFF]));
        assert_eq!(parse_hex("F8"), None, "partial");
    }

    #[test]
    fn hex_formatting() {
        assert_eq!(to_hex([0xFF, 0x6A, 0x00, 0xFF]), "FF6A00");
        assert_eq!(to_hex([0xFF, 0x6A, 0x00, 0x08]), "FF6A0008");
    }
}

#[cfg(test)]
mod triangle_tests {
    use egui::pos2;

    use super::{Hsva, triangle};

    #[test]
    fn corners_round_trip_through_saturation_and_value() {
        let c = pos2(100.0, 100.0);
        for hue in [0.0, 75.0, 200.0, 359.0] {
            for (s, v) in [(1.0, 1.0), (0.0, 1.0), (0.3, 0.0), (0.5, 0.5), (0.8, 0.25)] {
                let hsva = Hsva {
                    h: hue,
                    s,
                    v,
                    a: 255,
                };
                let p = triangle::point(c, 50.0, hsva);
                let (s2, v2) = triangle::sv_at(c, 50.0, hue, p);
                assert!((v2 - v).abs() < 1e-3, "v {v} -> {v2}");
                if v > 0.01 {
                    assert!((s2 - s).abs() < 1e-3, "s {s} -> {s2}");
                }
            }
        }
    }

    #[test]
    fn outside_points_clamp_to_the_edge() {
        let c = pos2(0.0, 0.0);
        // Far past the pure-hue corner: fully saturated, full value.
        let hue_corner = triangle::corners(c, 50.0, 120.0)[0];
        let far = c + (hue_corner - c) * 3.0;
        let (s, v) = triangle::sv_at(c, 50.0, 120.0, far);
        assert!((s - 1.0).abs() < 1e-3 && (v - 1.0).abs() < 1e-3);
    }

    #[test]
    fn a_triangle_with_no_area_gives_a_color() {
        let (s, v) = triangle::sv_at(pos2(0.0, 0.0), 0.0, 120.0, pos2(5.0, 5.0));
        assert!(s.is_finite() && v.is_finite());
    }

    #[test]
    fn hue_angles_round_trip() {
        for hue in [0.0, 90.0, 181.5, 359.0] {
            let back = triangle::angle_hue(triangle::hue_angle(hue));
            assert!((back - hue).abs() < 1e-3);
        }
        // Red at the top.
        let top = triangle::at(pos2(0.0, 0.0), triangle::hue_angle(0.0), 10.0);
        assert!(top.y < -9.99 && top.x.abs() < 1e-3);
    }
}
