//! The widgets driven by pointer and keyboard, through a real egui frame loop.

use egui::{Id, Key, Modifiers, PointerButton, Pos2, Rect, Role, Vec2, pos2, vec2};
use egui_kittest::{Harness, kittest::Queryable};
use egui_mev_graphics::{Hsva, color_editor, curve_editor, hue_triangle};

const SIZE: Vec2 = vec2(200.0, 200.0);

struct Curve {
    points: Vec<[f32; 2]>,
    rect: Rect,
    changed: bool,
}

fn curve_harness(points: &[[f32; 2]]) -> Harness<'static, Curve> {
    Harness::builder()
        .with_size(vec2(240.0, 240.0))
        .build_ui_state(
            |ui, state: &mut Curve| {
                state.rect = Rect::from_min_size(ui.cursor().min, SIZE);
                state.changed |= curve_editor(ui, &mut state.points, SIZE, None);
            },
            Curve {
                points: points.to_vec(),
                rect: Rect::NOTHING,
                changed: false,
            },
        )
}

/// The screen position of a value in the graph.
fn at(harness: &Harness<'_, Curve>, [x, y]: [f32; 2]) -> Pos2 {
    let rect = harness.state().rect;
    pos2(
        rect.left() + x * rect.width(),
        rect.bottom() - y * rect.height(),
    )
}

/// Press at `from`, move through `path` a frame at a time, and release.
fn drag<S>(harness: &mut Harness<'_, S>, from: Pos2, path: &[Pos2]) {
    harness.hover_at(from);
    harness.step();
    harness.drag_at(from);
    harness.step();
    for &pos in path {
        harness.hover_at(pos);
        harness.step();
    }
    harness.drop_at(*path.last().unwrap_or(&from));
    harness.run();
}

fn click(harness: &mut Harness<'_, Curve>, pos: Pos2, button: PointerButton) {
    let event = |pressed| egui::Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: Modifiers::NONE,
    };
    harness.hover_at(pos);
    harness.step();
    harness.event(event(true));
    harness.step();
    harness.event(event(false));
    harness.run();
}

fn assert_near(actual: [f32; 2], expected: [f32; 2]) {
    let close = (actual[0] - expected[0]).abs() < 0.01 && (actual[1] - expected[1]).abs() < 0.01;
    assert!(close, "{actual:?} is not near {expected:?}");
}

const THREE: [[f32; 2]; 3] = [[0.0, 0.0], [0.5, 0.5], [1.0, 1.0]];

#[test]
fn dragging_a_point_moves_it() {
    let mut harness = curve_harness(&THREE);
    harness.run();
    let (from, mid, to) = (
        at(&harness, [0.5, 0.5]),
        at(&harness, [0.55, 0.6]),
        at(&harness, [0.7, 0.8]),
    );
    drag(&mut harness, from, &[mid, to]);
    let state = harness.state();
    assert!(state.changed);
    assert_eq!(state.points.len(), 3);
    assert_eq!(state.points[0], [0.0, 0.0]);
    assert_near(state.points[1], [0.7, 0.8]);
    assert_eq!(state.points[2], [1.0, 1.0]);
}

/// The drag starts on the frame the pointer is already past `GRAB`: it must
/// still take the point under the press, not add one under the pointer.
#[test]
fn a_fast_drag_moves_the_point_it_started_on() {
    let mut harness = curve_harness(&THREE);
    harness.run();
    let (from, to) = (at(&harness, [0.5, 0.5]), at(&harness, [0.7, 0.2]));
    drag(&mut harness, from, &[to]);
    let state = harness.state();
    assert_eq!(state.points.len(), 3, "{:?}", state.points);
    assert_near(state.points[1], [0.7, 0.2]);
}

#[test]
fn a_dragged_point_stops_short_of_its_neighbours() {
    let mut harness = curve_harness(&[[0.0, 0.0], [0.3, 0.3], [0.6, 0.6], [1.0, 1.0]]);
    harness.run();
    let (from, to) = (at(&harness, [0.3, 0.3]), at(&harness, [0.9, 0.5]));
    drag(&mut harness, from, &[to]);
    let points = &harness.state().points;
    assert!((points[1][0] - 0.59).abs() < 1e-4, "{points:?}");
    assert_eq!(points[2], [0.6, 0.6]);
}

#[test]
fn end_points_keep_to_the_edges() {
    let mut harness = curve_harness(&THREE);
    harness.run();
    let (from, to) = (at(&harness, [0.0, 0.0]), at(&harness, [0.2, 0.4]));
    drag(&mut harness, from, &[to]);
    assert_near(harness.state().points[0], [0.0, 0.4]);
    let (from, to) = (at(&harness, [1.0, 1.0]), at(&harness, [0.8, 0.6]));
    drag(&mut harness, from, &[to]);
    assert_near(harness.state().points[2], [1.0, 0.6]);
}

/// Neighbours nearer than two gaps leave no range to clamp into, which panics
/// in `f32::clamp` unless handled.
#[test]
fn a_point_between_close_neighbours_can_still_be_dragged() {
    let crowded = [
        [0.0, 0.0],
        [0.5, 0.1],
        [0.505, 0.5],
        [0.51, 0.9],
        [1.0, 1.0],
    ];
    let mut harness = curve_harness(&crowded);
    harness.run();
    let (from, to) = (at(&harness, [0.505, 0.5]), at(&harness, [0.8, 0.7]));
    drag(&mut harness, from, &[to]);
    assert_near(harness.state().points[2], [0.505, 0.7]);
}

#[test]
fn clicking_adds_a_point_in_order() {
    let mut harness = curve_harness(&THREE);
    harness.run();
    let pos = at(&harness, [0.25, 0.75]);
    click(&mut harness, pos, PointerButton::Primary);
    let state = harness.state();
    assert!(state.changed);
    assert_eq!(state.points.len(), 4);
    assert_near(state.points[1], [0.25, 0.75]);
    assert_eq!(state.points[2], [0.5, 0.5]);
}

/// Far from any handle, yet too close sideways to be a separate point of the
/// curve.
#[test]
fn clicking_right_above_a_point_adds_nothing() {
    let mut harness = curve_harness(&THREE);
    harness.run();
    for x in [0.5, 0.502] {
        let pos = at(&harness, [x, 0.95]);
        click(&mut harness, pos, PointerButton::Primary);
        assert_eq!(harness.state().points, THREE, "at {x}");
    }
    assert!(!harness.state().changed);
}

#[test]
fn right_click_removes_a_point_but_not_an_end() {
    let mut harness = curve_harness(&THREE);
    harness.run();
    for end in [[0.0, 0.0], [1.0, 1.0]] {
        let pos = at(&harness, end);
        click(&mut harness, pos, PointerButton::Secondary);
        assert_eq!(harness.state().points.len(), 3);
    }
    assert!(!harness.state().changed);
    let pos = at(&harness, [0.5, 0.5]);
    click(&mut harness, pos, PointerButton::Secondary);
    assert_eq!(harness.state().points, [[0.0, 0.0], [1.0, 1.0]]);
    assert!(harness.state().changed);
}

#[test]
fn too_few_points_become_the_identity_and_say_so() {
    let mut harness = curve_harness(&[]);
    harness.run();
    assert_eq!(harness.state().points, [[0.0, 0.0], [1.0, 1.0]]);
    assert!(harness.state().changed);
}

struct Color {
    color: [u8; 4],
    changed: bool,
    wheel: Rect,
}

fn color_harness(color: [u8; 4]) -> Harness<'static, Color> {
    Harness::builder()
        .with_size(vec2(300.0, 300.0))
        .build_ui_state(
            |ui, state: &mut Color| {
                // The wheel leads the row, at its full size in this width.
                state.wheel = Rect::from_min_size(ui.cursor().min, Vec2::splat(150.0));
                state.changed |= color_editor(ui, Id::unique("color"), &mut state.color);
            },
            Color {
                color,
                changed: false,
                wheel: Rect::NOTHING,
            },
        )
}

#[test]
fn an_untouched_low_alpha_color_stays_exactly_as_it_is() {
    let color = [200, 100, 50, 8];
    let mut harness = color_harness(color);
    harness.run();
    harness.run();
    assert_eq!(harness.state().color, color);
    assert!(!harness.state().changed);
}

#[test]
fn the_wheel_changes_the_color_but_not_a_low_alpha() {
    let mut harness = color_harness([200, 100, 50, 8]);
    harness.run();
    // Half way out towards red, at hue 0.
    let center = harness.state().wheel.center();
    let radius = 73.0;
    drag(
        &mut harness,
        center + vec2(radius / 2.0 - 8.0, 0.0),
        &[center + vec2(radius / 2.0, 0.0)],
    );
    let [r, g, b, a] = harness.state().color;
    assert!(harness.state().changed);
    assert_eq!(a, 8);
    assert_eq!(r, 200, "value is kept");
    assert_eq!(g, b, "hue 0 has no green or blue of its own");
    assert!((i32::from(g) - 100).abs() <= 2, "half saturated: {g}");
}

#[test]
fn the_alpha_bar_leaves_rgb_alone_all_the_way_down() {
    let mut harness = color_harness([200, 100, 50, 255]);
    harness.run();
    let bar = harness.get_by_role_and_label(Role::Slider, "Alpha").rect();
    drag(
        &mut harness,
        bar.center(),
        &[pos2(bar.center().x, bar.bottom() - 2.0)],
    );
    let [r, g, b, a] = harness.state().color;
    assert_eq!([r, g, b], [200, 100, 50]);
    assert!((1..=8).contains(&a), "alpha {a}");

    let bar = harness.get_by_role_and_label(Role::Slider, "Alpha").rect();
    drag(
        &mut harness,
        bar.center(),
        &[pos2(bar.center().x, bar.bottom() + 20.0)],
    );
    assert_eq!(harness.state().color, [200, 100, 50, 0]);
}

#[test]
fn the_value_bar_keeps_alpha() {
    let mut harness = color_harness([200, 100, 50, 8]);
    harness.run();
    let bar = harness.get_by_role_and_label(Role::Slider, "Value").rect();
    drag(
        &mut harness,
        bar.center(),
        &[pos2(bar.center().x, bar.bottom() + 20.0)],
    );
    assert_eq!(harness.state().color, [0, 0, 0, 8]);
}

#[test]
fn typing_hex_sets_straight_rgba() {
    let mut harness = color_harness([200, 100, 50, 255]);
    harness.run();
    harness
        .get_by_role_and_label(Role::TextInput, "Hex")
        .focus();
    harness.run();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.run();
    harness
        .get_by_role_and_label(Role::TextInput, "Hex")
        .type_text("12345608");
    harness.run();
    assert_eq!(harness.state().color, [0x12, 0x34, 0x56, 0x08]);
    assert!(harness.state().changed);
}

struct Triangle {
    hsva: Hsva,
    /// Frames on which the widget reported a change.
    changes: usize,
}

const CENTER: Pos2 = pos2(120.0, 120.0);
const RADIUS: f32 = 100.0;

fn triangle_harness() -> Harness<'static, Triangle> {
    Harness::builder()
        .with_size(vec2(240.0, 240.0))
        .build_ui_state(
            |ui, state: &mut Triangle| {
                let id = Id::unique("triangle");
                if hue_triangle(ui, id, CENTER, RADIUS, &mut state.hsva) {
                    state.changes += 1;
                }
            },
            Triangle {
                hsva: Hsva {
                    h: 0.0,
                    s: 0.5,
                    v: 0.5,
                    a: 8,
                },
                changes: 0,
            },
        )
}

#[test]
fn the_ring_sets_hue_and_holds_the_drag_inside() {
    let mut harness = triangle_harness();
    harness.run();
    // The right of the ring is hue 90, and the bottom 180. The drag ends over
    // the triangle, and stays on the ring it started on.
    drag(
        &mut harness,
        CENTER + vec2(RADIUS - 5.0, 0.0),
        &[CENTER + vec2(0.0, RADIUS - 5.0), CENTER + vec2(0.0, 20.0)],
    );
    let hsva = harness.state().hsva;
    assert!((hsva.h - 180.0).abs() < 0.5, "{hsva:?}");
    assert_eq!((hsva.s, hsva.v, hsva.a), (0.5, 0.5, 8));
}

#[test]
fn the_triangle_sets_saturation_and_value() {
    let mut harness = triangle_harness();
    harness.run();
    drag(&mut harness, CENTER, &[CENTER + vec2(0.0, -40.0)]);
    let hsva = harness.state().hsva;
    assert_eq!((hsva.h, hsva.a), (0.0, 8));
    // Towards the hue corner at the top, from the middle.
    assert!(hsva.s > 0.5 && hsva.v > 0.5, "{hsva:?}");
}

#[test]
fn holding_still_reports_one_change() {
    let mut harness = triangle_harness();
    harness.run();
    let pos = CENTER + vec2(RADIUS - 5.0, 0.0);
    harness.hover_at(pos);
    harness.step();
    harness.drag_at(pos);
    harness.run_steps(5);
    assert_eq!(harness.state().changes, 1);
    assert!((harness.state().hsva.h - 90.0).abs() < 0.5);
}
