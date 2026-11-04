//! Choosing and moving around: segmented controls, tab strips, navigation and
//! tree rows, search fields and split buttons.

use egui::{
    Align2, AsIdSalt, Button, Color32, CornerRadius, FontId, Frame, Margin, Painter, Popup, Pos2,
    Rect, Response, RichText, Role, Sense, Stroke, StrokeKind, TextEdit, TextFormat, TextStyle, Ui,
    Vec2, WidgetInfo, pos2, text::LayoutJob,
};
use egui_mev_theme::{Glyph, Metrics, Palette, ThemeFamily};

use crate::buttons::{ButtonLook, painted_button};
use crate::marks::{self, Chevron};

// ---------------------------------------------------------------------------
// Segmented control
// ---------------------------------------------------------------------------

/// Horizontal text padding inside each segment (px).
const SEGMENT_PAD_X: f32 = 12.0;

/// Minimum segment width in the macOS control (px).
const SEGMENT_MIN_WIDTH: f32 = 44.0;

/// Height of the macOS control, a regular-size `NSSegmentedControl` (px).
const SEGMENT_HEIGHT: f32 = 24.0;

/// Height of a `WinUI` `SelectorBar` item (px).
const SELECTOR_HEIGHT: f32 = 32.0;

/// Gap between `SelectorBar` items (px).
const SELECTOR_GAP: f32 = 4.0;

/// Size of the pill under the selected `SelectorBar` item (px).
const SELECTOR_PILL: Vec2 = Vec2::new(16.0, 3.0);

fn segment_font(family: ThemeFamily) -> FontId {
    FontId::proportional(match family {
        ThemeFamily::Graphite => 12.0,
        ThemeFamily::Fluent => 14.0,
    })
}

const fn segment_gap(family: ThemeFamily) -> f32 {
    match family {
        ThemeFamily::Graphite => 0.0,
        ThemeFamily::Fluent => SELECTOR_GAP,
    }
}

/// Measure each segment's width for `labels`.
fn segment_widths(ui: &Ui, labels: &[impl AsRef<str>]) -> Vec<f32> {
    let family = ThemeFamily::of(ui);
    let font = segment_font(family);
    let color = Palette::of(ui).text;
    let min_width = match family {
        ThemeFamily::Graphite => SEGMENT_MIN_WIDTH,
        ThemeFamily::Fluent => 0.0,
    };
    labels
        .iter()
        .map(|label| {
            let text_w = ui
                .fonts_mut(|f| f.layout_no_wrap(label.as_ref().to_owned(), font.clone(), color))
                .size()
                .x;
            (text_w + SEGMENT_PAD_X * 2.0).max(min_width).ceil()
        })
        .collect()
}

/// Total width (px) of `widths` with the gaps of `family` between them.
fn segments_total(widths: &[f32], family: ThemeFamily) -> f32 {
    let gaps = widths.len().saturating_sub(1) as f32;
    widths.iter().sum::<f32>() + segment_gap(family) * gaps
}

/// Total width (px) a [`segmented_control`] with `labels` will occupy.
#[must_use]
pub fn segmented_width(ui: &Ui, labels: &[impl AsRef<str>]) -> f32 {
    segments_total(&segment_widths(ui, labels), ThemeFamily::of(ui))
}

/// A row of mutually exclusive segments: one rounded container with an inset
/// selected segment like macOS (Graphite), or a `WinUI` `SelectorBar` of separate
/// items with an accent pill under the selected one (Fluent). `selected` is the
/// index of the highlighted segment. Returns one [`Response`] per segment so
/// callers can handle clicks, context menus and tooltips individually.
pub fn segmented_control(
    ui: &mut Ui,
    id_salt: impl AsIdSalt,
    labels: &[impl AsRef<str>],
    selected: Option<usize>,
) -> Vec<Response> {
    let p = Palette::of(ui);
    let m = Metrics::of(ui);
    let family = ThemeFamily::of(ui);
    let height = match family {
        ThemeFamily::Graphite => SEGMENT_HEIGHT,
        ThemeFamily::Fluent => SELECTOR_HEIGHT,
    };
    let font = segment_font(family);
    let widths = segment_widths(ui, labels);
    let total_w = segments_total(&widths, family);
    let (outer, _) = ui.allocate_exact_size(Vec2::new(total_w, height), Sense::hover());

    let painter = ui.painter().clone();
    if family == ThemeFamily::Graphite {
        painter.rect(
            outer,
            CornerRadius::same(m.control_radius),
            p.card_bg,
            Stroke::new(1.0, p.border),
            StrokeKind::Inside,
        );
    }

    let base_id = ui.make_persistent_id(id_salt);
    let mut responses = Vec::with_capacity(labels.len());
    let mut x = outer.min.x;
    for (ix, (label, width)) in labels.iter().zip(&widths).enumerate() {
        let label = label.as_ref();
        let rect = Rect::from_min_size(Pos2::new(x, outer.min.y), Vec2::new(*width, height));
        let response = ui.interact(rect, base_id.with(ix), Sense::click());
        let is_selected = selected == Some(ix);
        response.widget_info(|| WidgetInfo::selected(Role::Button, true, is_selected, label));
        match family {
            ThemeFamily::Graphite => {
                let last = ix + 1 == labels.len();
                let next_selected = selected == Some(ix + 1);
                let separator = !last && !is_selected && !next_selected;
                paint_inset_segment(&painter, &p, &m, rect, &response, is_selected, separator);
            }
            ThemeFamily::Fluent => {
                paint_selector_item(&painter, &p, &m, rect, &response, is_selected)
            }
        }
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            label,
            font.clone(),
            if is_selected { p.text } else { p.text_weak },
        );
        responses.push(response);
        x += width + segment_gap(family);
    }
    responses
}

/// One segment inside the macOS container: an inset fill, and a hairline before
/// the next segment when neither of the two is selected.
fn paint_inset_segment(
    painter: &Painter,
    p: &Palette,
    m: &Metrics,
    rect: Rect,
    response: &Response,
    selected: bool,
    separator: bool,
) {
    let inner_rounding = CornerRadius::same(m.control_radius.saturating_sub(2));
    if selected {
        painter.rect_filled(rect.shrink(2.0), inner_rounding, p.selection_bg);
    } else if response.hovered() {
        painter.rect_filled(rect.shrink(2.0), inner_rounding, p.card_bg_hover);
    }
    if separator {
        painter.line_segment(
            [
                pos2(rect.max.x, rect.min.y + 5.0),
                pos2(rect.max.x, rect.max.y - 5.0),
            ],
            Stroke::new(1.0, p.border),
        );
    }
}

/// One `SelectorBar` item: a subtle hover fill and an accent pill when selected.
fn paint_selector_item(
    painter: &Painter,
    p: &Palette,
    m: &Metrics,
    rect: Rect,
    response: &Response,
    selected: bool,
) {
    if response.hovered() {
        painter.rect_filled(rect, CornerRadius::same(m.control_radius), p.subtle_hover);
    }
    if selected {
        let pill = Rect::from_center_size(
            pos2(rect.center().x, rect.bottom() - SELECTOR_PILL.y / 2.0),
            SELECTOR_PILL,
        );
        painter.rect_filled(pill, CornerRadius::same(2), p.accent);
    }
}

/// [`segmented_control`] over a short static set of mutually exclusive options.
/// Returns whether the selection changed this frame.
pub fn segmented<T: PartialEq + Copy>(
    ui: &mut Ui,
    id_salt: impl AsIdSalt,
    current: &mut T,
    options: &[(T, &str)],
) -> bool {
    let labels: Vec<&str> = options.iter().map(|(_, label)| *label).collect();
    let selected = options.iter().position(|(value, _)| value == current);
    let responses = segmented_control(ui, id_salt, &labels, selected);
    for (response, (value, _)) in responses.iter().zip(options) {
        if response.clicked() && value != current {
            *current = *value;
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Tab strip
// ---------------------------------------------------------------------------

/// A tab strip: the selected tab sits on a lighter fill with an accent
/// underline, the others are muted text. Returns whether the selection changed.
pub fn tab_strip<T: PartialEq + Copy>(
    ui: &mut Ui,
    id_salt: impl AsIdSalt,
    current: &mut T,
    options: &[(T, &str)],
) -> bool {
    let mut changed = false;
    ui.push_id(id_salt, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for (value, label) in options {
                if tab(ui, *current == *value, label).clicked() && *current != *value {
                    *current = *value;
                    changed = true;
                }
            }
        });
    });
    changed
}

/// One tab of a [`tab_strip`].
pub fn tab(ui: &mut Ui, selected: bool, label: &str) -> Response {
    let p = Palette::of(ui);
    let m = Metrics::of(ui);
    let text = RichText::new(label).color(if selected { p.text } else { p.text_weak });
    let radius = m.control_radius;
    let response = ui.add(
        Button::new(text)
            .frame_when_inactive(selected)
            .fill(if selected {
                p.pane_tab_active
            } else {
                Color32::TRANSPARENT
            })
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius {
                nw: radius,
                ne: radius,
                sw: 0,
                se: 0,
            })
            .min_size(Vec2::new(0.0, m.interact_height)),
    );
    if selected {
        let rect = response.rect;
        ui.painter().line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            Stroke::new(2.0, p.accent),
        );
    }
    response
}

// ---------------------------------------------------------------------------
// Navigation rows
// ---------------------------------------------------------------------------

/// Size of the Fluent selection pill at a selected row's edge (px).
const NAV_PILL: Vec2 = Vec2::new(3.0, 16.0);

/// Width reserved for a navigation row's icon (px).
const NAV_ICON_WIDTH: f32 = 22.0;

/// A sidebar row: icon and one line of text, with hover, selection and focus
/// states. Graphite draws a macOS source-list row with a tinted selection; Fluent
/// draws a `NavigationViewItem` with a neutral fill and an accent pill at its edge.
pub fn nav_row(
    ui: &mut Ui,
    icon: impl Into<Glyph>,
    label: &str,
    selected: bool,
    sense: Sense,
) -> Response {
    let p = Palette::of(ui);
    let m = Metrics::of(ui);
    let family = ThemeFamily::of(ui);
    let icon = icon.into();
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), m.nav_row_height), sense);
    response.widget_info(|| WidgetInfo::selected(Role::Button, true, selected, label));
    let radius = CornerRadius::same(m.control_radius);
    let active = response.hovered() || response.has_focus();
    let fill = match family {
        ThemeFamily::Graphite if selected => Some(p.selection_bg),
        ThemeFamily::Graphite => active.then_some(p.card_bg_hover),
        ThemeFamily::Fluent if active => Some(p.subtle_hover),
        ThemeFamily::Fluent => selected.then_some(p.subtle_selected),
    };
    if let Some(fill) = fill {
        ui.painter().rect_filled(rect, radius, fill);
    }
    if selected && family == ThemeFamily::Fluent {
        ui.painter().rect_filled(
            Rect::from_center_size(
                pos2(rect.left() + NAV_PILL.x / 2.0, rect.center().y),
                NAV_PILL,
            ),
            2,
            p.accent,
        );
    }
    if response.has_focus() {
        ui.painter()
            .rect_stroke(rect, radius, Stroke::new(1.0, p.accent), StrokeKind::Inside);
    }
    let (icon_color, title_color) = match family {
        ThemeFamily::Graphite if selected => (p.text, p.text),
        ThemeFamily::Graphite => (p.text_weak, p.text_label),
        ThemeFamily::Fluent => (p.text, p.text),
    };
    let (icon_size, icon_x) = match family {
        ThemeFamily::Graphite => (14.0, 4.0),
        ThemeFamily::Fluent => (16.0, 12.0),
    };
    ui.painter().text(
        rect.left_center() + Vec2::new(icon_x + NAV_ICON_WIDTH / 2.0, 0.0),
        Align2::CENTER_CENTER,
        icon.ch,
        icon.font_id(icon_size),
        icon_color,
    );
    let text_x = icon_x * 2.0 + NAV_ICON_WIDTH;
    let mut job = LayoutJob::simple_singleline(
        label.to_owned(),
        TextStyle::Body.resolve(ui.style()),
        title_color,
    );
    job.wrap.max_width = (rect.width() - text_x - 10.0).max(0.0);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
    ui.painter().galley(
        rect.left_center() + Vec2::new(text_x, -galley.size().y / 2.0),
        galley,
        title_color,
    );
    response
}

/// A trailing on/off icon of a [`tree_row`], such as visibility.
#[derive(Clone, Debug)]
pub struct TreeRowTrailing<'a> {
    /// Whether the option is on.
    pub on: bool,
    /// Icon while on.
    pub icon_on: Glyph,
    /// Icon while off.
    pub icon_off: Glyph,
    /// Name for assistive technology and tests.
    pub name: &'a str,
}

/// What a [`tree_row`] shows.
#[derive(Clone, Debug, Default)]
pub struct TreeRow<'a> {
    /// Nesting depth (0: top level).
    pub depth: usize,
    /// `Some(open)` draws a caret that toggles children.
    pub expanded: Option<bool>,
    /// A leading icon.
    pub icon: Option<Glyph>,
    /// The icon's colour (severity); the label colour when `None`.
    pub icon_color: Option<Color32>,
    /// The row's text.
    pub label: &'a str,
    /// Char indices of `label` matched by a search, drawn in the accent.
    pub matched: &'a [usize],
    /// Muted text after the label (kind, folder, count).
    pub detail: Option<&'a str>,
    /// A leading status dot (unsaved, severity).
    pub dot: Option<Color32>,
    /// Draw as selected.
    pub selected: bool,
    /// Draw the label as hint text.
    pub dimmed: bool,
    /// A trailing icon toggle.
    pub trailing: Option<TreeRowTrailing<'a>>,
}

/// What happened to a [`tree_row`].
pub struct TreeRowResponse {
    /// The row itself: click, double-click, context menu, drag.
    pub row: Response,
    /// The caret was clicked.
    pub toggled: bool,
    /// The trailing toggle was clicked.
    pub trailing_clicked: bool,
}

/// One row of a tree or list: hover fill, an accent outline when selected,
/// indentation guides, an optional caret, icon, muted detail and trailing toggle.
pub fn tree_row(ui: &mut Ui, id_salt: impl AsIdSalt, row: &TreeRow<'_>) -> TreeRowResponse {
    let p = Palette::of(ui);
    let m = Metrics::of(ui);
    let id = ui.make_persistent_id(id_salt);
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, m.row_height), Sense::hover());
    let response = ui.interact(rect, id, Sense::click_and_drag());
    response.widget_info(|| WidgetInfo::selected(Role::Button, true, row.selected, row.label));
    let painter = ui.painter_at(rect.expand(1.0));
    let radius = CornerRadius::same(m.control_radius);
    if row.selected {
        painter.rect_filled(rect, radius, p.card_bg_selected);
        painter.rect_stroke(
            rect.shrink(0.5),
            radius,
            Stroke::new(1.0, p.accent),
            StrokeKind::Inside,
        );
    } else if response.hovered() {
        painter.rect_filled(rect, radius, p.card_bg_hover);
    }
    let indent = 14.0;
    let mut x = rect.left() + 6.0;
    for level in 0..row.depth {
        let guide_x = rect.left() + 6.0 + level as f32 * indent + 5.0;
        painter.line_segment(
            [pos2(guide_x, rect.top()), pos2(guide_x, rect.bottom())],
            Stroke::new(1.0, p.border),
        );
        x += indent;
    }
    let text_color = if row.dimmed { p.hint_text } else { p.text };
    let font = TextStyle::Body.resolve(ui.style());
    let center_y = rect.center().y;
    let mut toggled = false;
    if let Some(open) = row.expanded {
        let caret_rect = Rect::from_center_size(pos2(x + 6.0, center_y), Vec2::splat(16.0));
        let caret = ui.interact(caret_rect, id.with("caret"), Sense::click());
        caret.widget_info(|| {
            let name = if open { "Collapse" } else { "Expand" };
            WidgetInfo::labeled(Role::Button, true, format!("{name} {}", row.label))
        });
        toggled = caret.clicked();
        marks::chevron(
            &painter,
            caret_rect.center(),
            8.0,
            if open { Chevron::Down } else { Chevron::Right },
            if caret.hovered() { p.text } else { p.text_weak },
        );
    }
    // Flat lists (no carets, no nesting) start at the edge.
    if row.expanded.is_some() || row.depth > 0 {
        x += 16.0;
    }
    if let Some(dot) = row.dot {
        painter.circle_filled(pos2(x + 3.0, center_y), 3.0, dot);
        x += 10.0;
    }
    if let Some(glyph) = &row.icon {
        painter.text(
            pos2(x, center_y),
            Align2::LEFT_CENTER,
            glyph.ch,
            glyph.font_id(font.size),
            if row.selected {
                p.accent
            } else {
                row.icon_color.unwrap_or(p.text_label)
            },
        );
        x += 20.0;
    }
    let trailing_width = if row.trailing.is_some() { 24.0 } else { 0.0 };
    let max_x = rect.right() - 6.0 - trailing_width;
    let label = if row.matched.is_empty() {
        painter.layout_no_wrap(row.label.to_owned(), font.clone(), text_color)
    } else {
        let mut job = LayoutJob::default();
        for (i, c) in row.label.chars().enumerate() {
            let color = if row.matched.contains(&i) {
                p.accent
            } else {
                text_color
            };
            job.append(&c.to_string(), 0.0, TextFormat::simple(font.clone(), color));
        }
        painter.layout_job(job)
    };
    let label_rect = Rect::from_min_size(pos2(x, center_y - label.size().y / 2.0), label.size());
    let clip = Rect::from_min_max(rect.min, pos2(max_x, rect.max.y));
    painter
        .with_clip_rect(clip)
        .galley(label_rect.min, label, text_color);
    if let Some(detail) = row.detail {
        let detail_x = label_rect.right() + 6.0;
        if detail_x < max_x {
            let small = TextStyle::Small.resolve(ui.style());
            painter.with_clip_rect(clip).text(
                pos2(detail_x, center_y),
                Align2::LEFT_CENTER,
                detail,
                small,
                p.text_weak,
            );
        }
    }
    let mut trailing_clicked = false;
    if let Some(trailing_row) = &row.trailing {
        let trailing_rect =
            Rect::from_center_size(pos2(rect.right() - 16.0, center_y), Vec2::splat(18.0));
        let trailing = ui.interact(trailing_rect, id.with("trailing"), Sense::click());
        trailing.widget_info(|| {
            WidgetInfo::selected(Role::Button, true, trailing_row.on, trailing_row.name)
        });
        trailing_clicked = trailing.clicked();
        if trailing_row.on || response.hovered() || trailing.hovered() {
            let glyph = if trailing_row.on {
                &trailing_row.icon_on
            } else {
                &trailing_row.icon_off
            };
            painter.text(
                trailing_rect.center(),
                Align2::CENTER_CENTER,
                glyph.ch,
                glyph.font_id(font.size),
                if trailing.hovered() {
                    p.text
                } else {
                    p.text_weak
                },
            );
        }
    }
    TreeRowResponse {
        row: response,
        toggled,
        trailing_clicked,
    }
}

// ---------------------------------------------------------------------------
// Search field and split button
// ---------------------------------------------------------------------------

/// A full-width search field: a magnifier, the text and a clear button. Returns
/// the text edit's response.
pub fn search_field(ui: &mut Ui, text: &mut String, hint: &str) -> Response {
    let p = Palette::of(ui);
    let m = Metrics::of(ui);
    Frame::new()
        .fill(p.extreme_bg)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(CornerRadius::same(m.control_radius))
        .inner_margin(Margin::symmetric(6, 0))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let height = ui.spacing().interact_size.y;
                let (slot, _) = ui.allocate_exact_size(Vec2::new(14.0, height), Sense::hover());
                marks::magnifier(
                    ui.painter(),
                    Rect::from_center_size(slot.center(), Vec2::splat(13.0)),
                    p.hint_text,
                );
                let clear = !text.is_empty();
                let width = ui.available_width() - if clear { 22.0 } else { 0.0 };
                let response = ui
                    .add(
                        TextEdit::singleline(text)
                            .hint_text(hint)
                            .frame(Frame::NONE)
                            .min_size(Vec2::new(0.0, height))
                            .align(Align2::LEFT_CENTER)
                            .desired_width(width.max(20.0)),
                    )
                    .accessible_name(hint);
                if clear {
                    let clear_button = painted_button(
                        ui,
                        Vec2::splat(18.0),
                        ButtonLook::Plain,
                        true,
                        |painter, rect, color| {
                            marks::cross(painter, rect.center(), 8.0, color);
                        },
                    );
                    clear_button.widget_info(|| WidgetInfo::labeled(Role::Button, true, "Clear"));
                    if clear_button.on_hover_text("Clear").clicked() {
                        text.clear();
                    }
                }
                response
            })
            .inner
        })
        .inner
}

/// A button with a caret that opens `add_menu`: the main part returns the click,
/// the caret holds the alternatives. `menu_name` names the caret for assistive
/// technology and tests.
pub fn split_button(
    ui: &mut Ui,
    main: Button<'_>,
    menu_name: &str,
    add_menu: impl FnOnce(&mut Ui),
) -> Response {
    let p = Palette::of(ui);
    let m = Metrics::of(ui);
    let radius = CornerRadius::same(m.control_radius);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 1.0;
        let response = ui.add(main);
        let caret = painted_button(
            ui,
            Vec2::new(20.0, response.rect.height()),
            ButtonLook::Plain,
            true,
            |painter, rect, color| {
                painter.rect_stroke(rect, radius, Stroke::new(1.0, p.border), StrokeKind::Inside);
                marks::chevron(painter, rect.center(), 8.0, Chevron::Down, color);
            },
        );
        caret.widget_info(|| WidgetInfo::labeled(Role::Button, true, menu_name));
        Popup::menu(&caret).show(add_menu);
        response
    })
    .inner
}

#[cfg(test)]
mod tests {
    use egui_kittest::{Harness, kittest::Queryable as _};

    use super::*;

    #[test]
    fn segmented_reports_one_change_per_click() {
        let mut harness = Harness::builder().with_size([300.0, 80.0]).build_ui_state(
            |ui, (current, changes): &mut (u8, u32)| {
                if segmented(ui, "lod", current, &[(0, "Low"), (1, "High")]) {
                    *changes += 1;
                }
            },
            (0, 0),
        );
        harness.run();
        harness.get_by_label("High").click();
        harness.run();
        assert_eq!(*harness.state(), (1, 1));
        harness.get_by_label("High").click();
        harness.run();
        assert_eq!(
            *harness.state(),
            (1, 1),
            "clicking the current one is no change"
        );
    }

    #[test]
    fn segment_widths_add_up_and_keep_a_minimum() {
        let ctx = crate::test_support::themed(egui::Theme::Dark);
        crate::test_support::frame_at(&ctx, 0.0, Vec::new(), |ui| {
            let labels = ["a", "a much longer label"];
            let widths = segment_widths(ui, &labels);
            assert_eq!(widths[0], SEGMENT_MIN_WIDTH);
            assert!(widths[1] > SEGMENT_MIN_WIDTH);
            assert_eq!(segmented_width(ui, &labels), widths[0] + widths[1]);
        });
    }

    #[test]
    fn the_segmented_control_has_each_familys_shape() {
        for (family, height, gap) in [
            (ThemeFamily::Graphite, SEGMENT_HEIGHT, 0.0),
            (ThemeFamily::Fluent, SELECTOR_HEIGHT, SELECTOR_GAP),
        ] {
            let ctx = crate::test_support::themed_family(family, egui::Theme::Dark);
            crate::test_support::frame_at(&ctx, 0.0, Vec::new(), |ui| {
                let labels = ["Assets", "Faces", "World"];
                let total = segmented_width(ui, &labels);
                let responses = segmented_control(ui, "s", &labels, Some(1));
                assert_eq!(responses.len(), 3);
                assert_eq!(responses[0].rect.height(), height, "{family:?}");
                assert_eq!(
                    responses[1].rect.left() - responses[0].rect.right(),
                    gap,
                    "{family:?}"
                );
                let span = responses[2].rect.right() - responses[0].rect.left();
                assert_eq!(span, total, "{family:?}");
            });
        }
    }

    #[test]
    fn only_the_macos_control_has_a_container_and_only_fluent_a_pill() {
        for (family, container, pill) in [
            (ThemeFamily::Graphite, true, false),
            (ThemeFamily::Fluent, false, true),
        ] {
            let ctx = crate::test_support::themed_family(family, egui::Theme::Dark);
            let p = egui_mev_theme::Theme::of(&ctx).dark;
            let output = crate::test_support::frame_output(&ctx, 0.0, Vec::new(), |ui| {
                segmented_control(ui, "s", &["Assets", "Faces", "World"], Some(1));
            });
            let rects = crate::test_support::painted_rects(&output);
            let has_container = rects.iter().any(|(_, fill)| *fill == p.card_bg);
            let has_pill = rects
                .iter()
                .any(|(rect, fill)| *fill == p.accent && rect.size() == SELECTOR_PILL);
            assert_eq!(has_container, container, "{family:?}");
            assert_eq!(has_pill, pill, "{family:?}");
        }
    }

    #[test]
    fn segmented_reports_clicks_in_both_families() {
        for family in [ThemeFamily::Graphite, ThemeFamily::Fluent] {
            let mut harness = Harness::builder()
                .with_size([400.0, 80.0])
                .with_context_setup(move |ctx| {
                    egui_mev_test::install(ctx, crate::test_support::family_theme(family));
                })
                .build_ui_state(
                    |ui, current: &mut u8| {
                        segmented(ui, "lod", current, &[(0, "Low"), (1, "High")]);
                    },
                    0,
                );
            harness.run();
            harness.get_by_label("High").click();
            harness.run();
            assert_eq!(*harness.state(), 1, "{family:?}");
        }
    }

    #[test]
    fn segmented_ids_are_scoped_to_their_parent() {
        // Two controls with one salt in separate scopes: no id clash, and each
        // gets its own click.
        let mut harness = Harness::builder().with_size([300.0, 120.0]).build_ui_state(
            |ui, (a, b): &mut (u8, u8)| {
                ui.push_id("left", |ui| segmented(ui, "s", a, &[(0, "A0"), (1, "A1")]));
                ui.push_id("right", |ui| segmented(ui, "s", b, &[(0, "B0"), (1, "B1")]));
            },
            (0, 0),
        );
        harness.run();
        harness.get_by_label("B1").click();
        harness.run();
        assert_eq!(*harness.state(), (0, 1));
    }

    #[test]
    fn a_tab_strip_changes_the_current_tab() {
        let mut harness = Harness::builder().with_size([300.0, 80.0]).build_ui_state(
            |ui, current: &mut u8| {
                tab_strip(ui, "tabs", current, &[(0, "Interior"), (1, "Exterior")]);
            },
            0,
        );
        harness.run();
        harness.get_by_label("Exterior").click();
        harness.run();
        assert_eq!(*harness.state(), 1);
    }

    #[test]
    fn a_tree_row_has_the_metrics_height_and_reports_its_parts() {
        let mut harness = Harness::builder().with_size([300.0, 120.0]).build_ui_state(
            |ui, (toggled, trailing, height): &mut (u32, u32, f32)| {
                let row = tree_row(
                    ui,
                    "row",
                    &TreeRow {
                        depth: 2,
                        expanded: Some(true),
                        icon: Some(Glyph::new('n')),
                        label: "a node",
                        detail: Some("mesh"),
                        selected: true,
                        trailing: Some(TreeRowTrailing {
                            on: true,
                            icon_on: Glyph::new('e'),
                            icon_off: Glyph::new('o'),
                            name: "Visible",
                        }),
                        ..Default::default()
                    },
                );
                *height = row.row.rect.height();
                *toggled += u32::from(row.toggled);
                *trailing += u32::from(row.trailing_clicked);
            },
            (0, 0, 0.0),
        );
        harness.run();
        assert_eq!(harness.state().2, Metrics::GRAPHITE.row_height);
        harness.get_by_label("Collapse a node").click();
        harness.run();
        harness.get_by_label("Visible").click();
        harness.run();
        assert_eq!((harness.state().0, harness.state().1), (1, 1));
    }

    #[test]
    fn search_field_clears_with_its_button() {
        let mut harness = Harness::builder().with_size([300.0, 80.0]).build_ui_state(
            |ui, text: &mut String| {
                search_field(ui, text, "Filter");
            },
            "abc".to_owned(),
        );
        harness.run();
        harness.get_by_label("Clear").click();
        harness.run();
        assert!(harness.state().is_empty());
        assert!(harness.query_by_label("Clear").is_none());
    }

    #[test]
    fn a_split_button_clicks_and_opens_its_menu() {
        let mut harness = Harness::builder().with_size([300.0, 200.0]).build_ui_state(
            |ui, clicks: &mut u32| {
                let main = crate::buttons::primary(ui, "Place");
                let response = split_button(ui, main, "More", |ui| {
                    ui.label("Alternative");
                });
                *clicks += u32::from(response.clicked());
            },
            0,
        );
        harness.run();
        harness.get_by_label("Place").click();
        harness.run();
        assert_eq!(*harness.state(), 1);
        assert!(harness.query_by_label("Alternative").is_none());
        harness.get_by_label("More").click();
        harness.run();
        assert!(harness.query_by_label("Alternative").is_some());
    }

    #[test]
    fn a_nav_row_is_selectable_by_its_label() {
        let mut harness = Harness::builder().with_size([300.0, 120.0]).build_ui_state(
            |ui, clicks: &mut u32| {
                let row = nav_row(ui, 'h', "Home", true, Sense::click());
                *clicks += u32::from(row.clicked());
                assert_eq!(row.rect.height(), Metrics::of(ui).nav_row_height);
            },
            0,
        );
        harness.run();
        harness.get_by_label("Home").click();
        harness.run();
        assert_eq!(*harness.state(), 1);
    }

    /// Rects of accent fill a selected navigation row paints in `ctx`'s look.
    fn nav_pills(ctx: &egui::Context, selected: bool) -> Vec<Rect> {
        let accent = egui_mev_theme::Theme::of(ctx).dark.accent;
        let output = crate::test_support::frame_output(ctx, 0.0, Vec::new(), |ui| {
            nav_row(ui, 'h', "Home", selected, Sense::click());
        });
        crate::test_support::painted_rects(&output)
            .into_iter()
            .filter(|(_, fill)| *fill == accent)
            .map(|(rect, _)| rect)
            .collect()
    }

    #[test]
    fn only_a_selected_fluent_nav_row_has_a_pill() {
        let graphite = crate::test_support::themed_family(ThemeFamily::Graphite, egui::Theme::Dark);
        assert!(nav_pills(&graphite, true).is_empty());
        let fluent = crate::test_support::themed_family(ThemeFamily::Fluent, egui::Theme::Dark);
        assert!(nav_pills(&fluent, false).is_empty());
        let pills = nav_pills(&fluent, true);
        assert_eq!(pills.len(), 1);
        assert_eq!(pills[0].size(), NAV_PILL);
    }

    #[test]
    fn a_fluent_theme_with_custom_metrics_keeps_fluent_drawing() {
        let ctx = egui::Context::default();
        egui_mev_theme::fluent::theme(egui_mev_theme::AccentShades::WINDOWS_DEFAULT)
            .with_metrics(Metrics {
                card_radius: 9,
                nav_row_height: 40.0,
                ..Metrics::FLUENT
            })
            .install(&ctx);
        egui_mev_theme::fonts::install_test(&ctx, &egui_mev_theme::FontOptions::default());
        ctx.set_theme(egui::Theme::Dark);
        crate::test_support::frame_at(&ctx, 0.0, Vec::new(), |_| {});
        assert_eq!(nav_pills(&ctx, true).len(), 1);
        crate::test_support::frame_at(&ctx, 0.0, Vec::new(), |ui| {
            let responses = segmented_control(ui, "s", &["One", "Two"], Some(0));
            assert_eq!(responses[0].rect.height(), SELECTOR_HEIGHT);
            let mut on = true;
            let switch = crate::forms::toggle_switch(ui, &mut on, "Switch");
            assert_eq!(switch.rect.size(), Vec2::new(40.0, 20.0));
        });
    }

    #[test]
    fn nav_row_colours_follow_the_family() {
        for family in [ThemeFamily::Graphite, ThemeFamily::Fluent] {
            let ctx = crate::test_support::themed_family(family, egui::Theme::Dark);
            let p = egui_mev_theme::Theme::of(&ctx).dark;
            let fill = match family {
                ThemeFamily::Graphite => p.selection_bg,
                ThemeFamily::Fluent => p.subtle_selected,
            };
            let output = crate::test_support::frame_output(&ctx, 0.0, Vec::new(), |ui| {
                nav_row(ui, 'h', "Home", true, Sense::click());
            });
            let colors = crate::test_support::painted_colors(&output);
            assert!(colors.contains(&fill), "{family:?}");
        }
    }
}
