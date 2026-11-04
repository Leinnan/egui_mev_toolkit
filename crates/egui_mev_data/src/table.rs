//! A list of results, sorted by clicking a column's header. Like forms, it is
//! data: no app code runs while it is open.
//!
//! Each row carries a payload `P`, the app's own value. [`TableView::contents_ui`]
//! returns it when the row is clicked. [`TableView::contents_with_menus_ui`] adds
//! context menus and opens rows on a double click, for tables where a row has more
//! than one thing to do.

use std::borrow::Borrow;
use std::cmp::Ordering;
use std::collections::BTreeMap;

use egui::{
    Align2, Color32, Context, FontId, Id, Painter, Pos2, Rect, RichText, Role, Sense, Shape,
    Stroke, Ui, WidgetInfo, vec2,
};
use egui_mev_core::format::format_bytes;
use egui_mev_theme::{Metrics, Palette};
use serde::Deserialize;
use web_time::Instant;

const VALUE_COLUMN_WIDTH: f32 = 96.0;
/// The space a header's sort arrow takes beside its label (px).
const SORT_ARROW_WIDTH: f32 = 8.0;
const SORT_ARROW_GAP: f32 = 4.0;

/// How a column's numbers read and sort.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColumnKind {
    /// As they are.
    #[default]
    Text,
    /// Bytes.
    Size,
    /// Seconds since 1970, shown as a UTC date.
    Time,
}

/// One column of a [`TableSpec`].
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColumnDef {
    /// The key of this column's cells in [`RowDef::cells`].
    pub id: String,
    /// The header's text.
    pub label: String,
    /// How its numbers read and sort.
    #[serde(default)]
    pub kind: ColumnKind,
}

/// One value in a row.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum Cell {
    /// Formatted by the column's kind.
    Number(f64),
    /// Shown as it is.
    Text(String),
}

/// One row of a [`TableSpec`].
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RowDef<P> {
    /// What a click on the row returns. A row without one does nothing when
    /// clicked. Read from `payload`, or from `path` where the payload is a path.
    #[serde(alias = "path")]
    pub payload: Option<P>,
    /// Cells by column id; a missing one shows as blank and sorts last.
    pub cells: BTreeMap<String, Cell>,
    /// Shown when hovering the row.
    pub tooltip: Option<String>,
    /// More entries for the row's context menu.
    #[serde(default)]
    pub menu: Vec<RowAction>,
}

/// A context menu entry that asks the app to act on the row. Unavailable while
/// the table's own job runs, since it may be reading what the row stands for.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RowAction {
    /// The entry's text.
    pub label: String,
    /// What [`RowClick::Action`] says was asked for.
    pub action: String,
    /// Draw it as a warning: it removes something.
    #[serde(default)]
    pub danger: bool,
}

/// A change to one row, found by its payload.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RowUpdate<P> {
    /// The row to change.
    #[serde(alias = "path")]
    pub payload: P,
    /// Cells to set; others keep their values.
    #[serde(default)]
    pub cells: BTreeMap<String, Cell>,
    /// Cells to blank.
    #[serde(default)]
    pub clear: Vec<String>,
    /// What the row stood for is gone: drop the row.
    #[serde(default)]
    pub remove: bool,
}

/// What was asked for on a row, from [`TableView::contents_with_menus_ui`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowClick<P> {
    /// A double click, Enter, or "Open" in its menu.
    Open(P),
    /// "Refresh" in its menu, offered when the spec has a `refresh_action`.
    Refresh(P),
    /// One of the row's [`RowAction`]s.
    Action {
        /// The entry's [`RowAction::action`].
        action: String,
        /// The row's payload.
        payload: P,
    },
}

/// How a table starts out sorted.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SortDef {
    /// The id of the column to sort by.
    pub column: String,
    /// Run from the largest down.
    #[serde(default)]
    pub descending: bool,
}

/// What an app passes to show a table.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TableSpec<P> {
    /// The title of [`TableView::window_ui`]'s window.
    pub title: String,
    /// The line above the table. `{count}` and `{total}` fill in from the rows
    /// now in the table: how many, and the sum of the first size column.
    pub subtitle: Option<String>,
    /// What is still happening, such as "Scanning 12 of 136". Leave it out of
    /// the last update.
    pub progress: Option<String>,
    /// An action of the app that works out one row again. Its presence adds
    /// "Refresh" to row menus.
    pub refresh_action: Option<String>,
    /// The columns, in order. The first takes the width the others leave.
    pub columns: Vec<ColumnDef>,
    // A plain `default` would have serde require `P: Default`, which no row needs.
    /// The rows, in the order they tie in.
    #[serde(default = "Vec::new")]
    pub rows: Vec<RowDef<P>>,
    /// How it starts out sorted.
    pub sort: Option<SortDef>,
}

impl<P> TableSpec<P> {
    /// Checks the columns and the sort.
    pub fn check(&self) -> Result<(), String> {
        if self.columns.is_empty() {
            return Err("a table needs at least one column".into());
        }
        if let Some(sort) = &self.sort
            && !self.columns.iter().any(|column| column.id == sort.column)
        {
            return Err(format!("the table has no column \"{}\"", sort.column));
        }
        Ok(())
    }
}

/// Units for the smallest size to show. They match how sizes are displayed,
/// where 1 GB is 1024 MB, so what is typed compares with what is read.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SizeUnit {
    /// 1024² bytes.
    Mb,
    /// 1024³ bytes.
    #[default]
    Gb,
}

impl SizeUnit {
    const fn bytes(self) -> f64 {
        match self {
            Self::Mb => 1024.0 * 1024.0,
            Self::Gb => 1024.0 * 1024.0 * 1024.0,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Mb => "MB",
            Self::Gb => "GB",
        }
    }
}

/// Hides rows smaller than this. Zero shows everything.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SizeFilter {
    /// How many of `unit`.
    pub amount: f64,
    /// The unit of `amount`.
    pub unit: SizeUnit,
}

impl SizeFilter {
    /// A row not measured yet stays: it isn't known to be small.
    fn keeps(self, cell: Option<&Cell>) -> bool {
        match cell {
            Some(Cell::Number(bytes)) => *bytes >= self.amount * self.unit.bytes(),
            _ => true,
        }
    }
}

/// A column, and whether it runs from the largest down.
pub type SortOrder = (usize, bool);

/// A table and how it is sorted now.
#[derive(Debug)]
pub struct TableView<P> {
    spec: TableSpec<P>,
    /// Row indices in display order.
    order: Vec<usize>,
    sorted_by: Option<SortOrder>,
    filter: SizeFilter,
    updated: Instant,
    /// Whether the app's job is still running. When it stops short of a last
    /// update, `progress` no longer describes anything.
    running: bool,
}

fn compare(a: Option<&Cell>, b: Option<&Cell>, descending: bool) -> Ordering {
    let direct = |ordering: Ordering| {
        if descending {
            ordering.reverse()
        } else {
            ordering
        }
    };
    match (a, b) {
        (None, None) => Ordering::Equal,
        // Blank cells stay at the end in both directions.
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(Cell::Number(a)), Some(Cell::Number(b))) => direct(a.total_cmp(b)),
        (Some(Cell::Text(a)), Some(Cell::Text(b))) => {
            direct(a.to_lowercase().cmp(&b.to_lowercase()))
        }
        (Some(Cell::Number(_)), Some(Cell::Text(_))) => direct(Ordering::Less),
        (Some(Cell::Text(_)), Some(Cell::Number(_))) => direct(Ordering::Greater),
    }
}

/// `YYYY-MM-DD` in UTC, or nothing for a date past what four digits can hold.
fn date_text(seconds: i64) -> Option<String> {
    // Howard Hinnant's `civil_from_days`, with March as the year's first month.
    let shifted = seconds.div_euclid(86_400) + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (0..=9999)
        .contains(&year)
        .then(|| format!("{year:04}-{month:02}-{day:02}"))
}

fn cell_text(kind: ColumnKind, cell: Option<&Cell>) -> String {
    match (kind, cell) {
        (_, None) => String::new(),
        (_, Some(Cell::Text(text))) => text.clone(),
        (ColumnKind::Size, Some(Cell::Number(bytes))) => format_bytes(bytes.max(0.0) as u64),
        (ColumnKind::Time, Some(Cell::Number(seconds))) => {
            date_text(*seconds as i64).unwrap_or_default()
        }
        (ColumnKind::Text, Some(Cell::Number(number))) => number.to_string(),
    }
}

/// An arrow centred at `center`, pointing down for a descending sort.
fn paint_sort_arrow(painter: &Painter, center: Pos2, descending: bool, color: Color32) {
    let stroke = Stroke::new(1.5, color);
    let direction = if descending { 1.0 } else { -1.0 };
    let tip = center + vec2(0.0, direction * 5.0);
    let tail = center - vec2(0.0, direction * 5.0);
    painter.line_segment([tail, tip], stroke);
    let head = vec![
        tip + vec2(-3.5, direction * -3.5),
        tip,
        tip + vec2(3.5, direction * -3.5),
    ];
    painter.add(Shape::line(head, stroke));
}

impl<P> TableView<P> {
    /// A view of `spec`, sorted the way it asks. It counts as running while the
    /// spec reports progress; [`Self::set_running`] says otherwise.
    #[must_use]
    pub fn new(spec: TableSpec<P>) -> Self {
        let mut view = Self {
            order: (0..spec.rows.len()).collect(),
            sorted_by: None,
            filter: SizeFilter::default(),
            updated: Instant::now(),
            running: spec.progress.is_some(),
            spec,
        };
        if let Some(sort) = &view.spec.sort
            && let Some(column) = view.spec.columns.iter().position(|c| c.id == sort.column)
        {
            let descending = sort.descending;
            view.sort(column, descending);
        }
        view
    }

    /// The spec's title.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.spec.title
    }

    /// The column it is sorted by now.
    #[must_use]
    pub const fn sorted_by(&self) -> Option<SortOrder> {
        self.sorted_by
    }

    /// Sorts the way an earlier table was, if it has that column.
    pub fn apply_sort(&mut self, (column, descending): SortOrder) {
        if column < self.spec.columns.len() {
            self.sort(column, descending);
        }
    }

    /// The smallest size it shows.
    #[must_use]
    pub const fn filter(&self) -> SizeFilter {
        self.filter
    }

    /// Sets the smallest size to show.
    pub const fn set_filter(&mut self, filter: SizeFilter) {
        self.filter = filter;
    }

    /// The rows to show, in display order. The filter applies to the first
    /// size column.
    fn visible(&self) -> Vec<usize> {
        let column = self
            .spec
            .columns
            .iter()
            .find(|column| column.kind == ColumnKind::Size);
        self.order
            .iter()
            .copied()
            .filter(|&row| {
                column.is_none_or(|column| {
                    self.filter.keeps(self.spec.rows[row].cells.get(&column.id))
                })
            })
            .collect()
    }

    /// The control for the smallest size to show, if there are sizes at all.
    fn filter_ui(&mut self, ui: &mut Ui, shown: usize) {
        if !self
            .spec
            .columns
            .iter()
            .any(|column| column.kind == ColumnKind::Size)
        {
            return;
        }
        ui.horizontal(|ui| {
            let label = ui.label("Hide smaller than");
            ui.add(
                egui::DragValue::new(&mut self.filter.amount)
                    .range(0.0..=1_000_000.0)
                    .speed(1.0)
                    .max_decimals(2),
            )
            .labelled_by(label.id);
            let unit = egui::ComboBox::from_id_salt("egui_mev_data::table_size_unit")
                .selected_text(self.filter.unit.label())
                .width(56.0)
                .show_ui(ui, |ui| {
                    for unit in [SizeUnit::Mb, SizeUnit::Gb] {
                        ui.selectable_value(&mut self.filter.unit, unit, unit.label());
                    }
                });
            let _ = unit.response.accessible_name("Size unit");
            if shown < self.order.len() {
                let text = format!("Showing {shown} of {}", self.order.len());
                ui.label(RichText::new(text).small().color(Palette::of(ui).text_weak));
            }
        });
    }

    /// The spec's `refresh_action`.
    #[must_use]
    pub fn refresh_action(&self) -> Option<&str> {
        self.spec.refresh_action.as_deref()
    }

    /// Whether a row carries this payload. Takes a `&Path` where `P` is a
    /// `PathBuf`, as it does a `&str` for a `String`.
    pub fn has_row<Q>(&self, payload: &Q) -> bool
    where
        P: Borrow<Q>,
        Q: PartialEq + ?Sized,
    {
        self.spec.rows.iter().any(|row| {
            row.payload
                .as_ref()
                .is_some_and(|own| Borrow::<Q>::borrow(own) == payload)
        })
    }

    /// Changes one row, so its cells needn't wait for a whole new table.
    pub fn update_row(&mut self, update: RowUpdate<P>)
    where
        P: PartialEq,
    {
        let Some(index) = self
            .spec
            .rows
            .iter()
            .position(|row| row.payload.as_ref() == Some(&update.payload))
        else {
            return;
        };
        if update.remove {
            self.spec.rows.remove(index);
            self.order.retain(|&row| row != index);
            for row in &mut self.order {
                if *row > index {
                    *row -= 1;
                }
            }
        } else {
            let row = &mut self.spec.rows[index];
            row.cells.extend(update.cells);
            for cell in &update.clear {
                row.cells.remove(cell);
            }
        }
        if let Some((column, descending)) = self.sorted_by {
            self.sort(column, descending);
        }
    }

    /// The subtitle with `{count}` and `{total}` filled in from the rows now
    /// in the table, so they stay right as rows change.
    fn subtitle_text(&self) -> Option<String> {
        let subtitle = self.spec.subtitle.as_ref()?;
        if !subtitle.contains('{') {
            return Some(subtitle.clone());
        }
        let size = self
            .spec
            .columns
            .iter()
            .find(|column| column.kind == ColumnKind::Size);
        let total: f64 = size.map_or(0.0, |column| {
            self.spec
                .rows
                .iter()
                .filter_map(|row| match row.cells.get(&column.id) {
                    Some(Cell::Number(bytes)) => Some(*bytes),
                    _ => None,
                })
                .sum()
        });
        Some(
            subtitle
                .replace("{count}", &self.spec.rows.len().to_string())
                .replace("{total}", &format_bytes(total.max(0.0) as u64)),
        )
    }

    /// What is still happening, while the job runs.
    #[must_use]
    pub fn progress(&self) -> Option<&str> {
        self.spec.progress.as_deref()
    }

    /// Says whether the job filling the table still runs. Row actions wait for
    /// it to end, and progress left over from a job that stopped says so.
    pub const fn set_running(&mut self, running: bool) {
        self.running = running;
    }

    /// New contents for the open table. The order is kept, so ties stay as
    /// the user's last sort left them.
    pub fn update(&mut self, spec: TableSpec<P>) {
        if spec.rows.len() != self.spec.rows.len() {
            self.order = (0..spec.rows.len()).collect();
        }
        self.running = spec.progress.is_some();
        self.spec = spec;
        self.updated = Instant::now();
        match self.sorted_by {
            Some((column, descending)) if column < self.spec.columns.len() => {
                self.sort(column, descending);
            }
            // That column went away with the old spec.
            _ => self.sorted_by = None,
        }
    }

    /// Stable, so rows that tie stay in the order of the previous sort:
    /// clicking "Last change" and then "Size" means size, ties by date.
    fn sort(&mut self, column: usize, descending: bool) {
        let id = &self.spec.columns[column].id;
        let rows = &self.spec.rows;
        self.order
            .sort_by(|&a, &b| compare(rows[a].cells.get(id), rows[b].cells.get(id), descending));
        self.sorted_by = Some((column, descending));
    }

    /// A header was clicked: numbers start from the largest, and a second
    /// click reverses.
    fn header_clicked(&mut self, column: usize) {
        let descending = match self.sorted_by {
            Some((current, descending)) if current == column => !descending,
            _ => self.spec.columns[column].kind != ColumnKind::Text,
        };
        self.sort(column, descending);
    }

    fn updated_text(&self) -> String {
        let minutes = self.updated.elapsed().as_secs() / 60;
        match minutes {
            0 => "just now".to_owned(),
            1..60 => format!("{minutes} min ago"),
            _ => format!("{} h ago", minutes / 60),
        }
    }

    /// The column rectangles of a row: the first takes what the others leave.
    fn column_rects(&self, row: Rect) -> Vec<Rect> {
        let others = self.spec.columns.len().saturating_sub(1) as f32;
        let first = others.mul_add(-VALUE_COLUMN_WIDTH, row.width()).max(80.0);
        let mut left = row.left();
        (0..self.spec.columns.len())
            .map(|index| {
                let width = if index == 0 {
                    first
                } else {
                    VALUE_COLUMN_WIDTH
                };
                let rect =
                    Rect::from_min_size(egui::pos2(left, row.top()), vec2(width, row.height()));
                left += width;
                rect.shrink2(vec2(6.0, 0.0))
            })
            .collect()
    }

    fn paint_cell(ui: &Ui, rect: Rect, first: bool, text: &str, font: FontId, color: Color32) {
        let (anchor, position) = if first {
            (Align2::LEFT_CENTER, rect.left_center())
        } else {
            (Align2::RIGHT_CENTER, rect.right_center())
        };
        ui.painter()
            .with_clip_rect(rect.intersect(ui.clip_rect()))
            .text(position, anchor, text, font, color);
    }

    fn header_ui(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        let m = Metrics::of(ui);
        let (row, strip) =
            ui.allocate_exact_size(vec2(ui.available_width(), m.row_height), Sense::hover());
        let rects = self.column_rects(row);
        let mut clicked = None;
        for (index, rect) in rects.iter().enumerate() {
            let column = &self.spec.columns[index];
            let descending = match self.sorted_by {
                Some((current, descending)) if current == index => Some(descending),
                _ => None,
            };
            let response = ui.interact(
                rect.expand2(vec2(6.0, 0.0)),
                strip.id.with(("table_header", index)),
                Sense::click(),
            );
            if response.clicked() {
                clicked = Some(index);
            }
            response.widget_info(|| WidgetInfo::labeled(Role::Button, true, &column.label));
            let color = if response.hovered() {
                p.text
            } else {
                p.text_weak
            };
            let galley = ui.painter().layout_no_wrap(
                column.label.clone(),
                FontId::proportional(m.caption),
                color,
            );
            let arrow = if descending.is_some() {
                SORT_ARROW_GAP + SORT_ARROW_WIDTH
            } else {
                0.0
            };
            let left = if index == 0 {
                rect.left()
            } else {
                rect.right() - galley.size().x - arrow
            };
            let top = rect.center().y - galley.size().y / 2.0;
            let text_right = left + galley.size().x;
            ui.painter().galley(egui::pos2(left, top), galley, color);
            // Painted, so a sorted column shows without an icon font.
            if let Some(descending) = descending {
                let x = text_right + SORT_ARROW_GAP + SORT_ARROW_WIDTH / 2.0;
                paint_sort_arrow(
                    ui.painter(),
                    egui::pos2(x, rect.center().y),
                    descending,
                    color,
                );
            }
        }
        if let Some(index) = clicked {
            self.header_clicked(index);
        }
    }
}

impl<P: Clone> TableView<P> {
    /// The context menu of a row with a payload.
    fn row_menu_ui(&self, ui: &mut Ui, row: &RowDef<P>, payload: &P) -> Option<RowClick<P>> {
        let p = Palette::of(ui);
        let mut click = None;
        if ui.button("Open").clicked() {
            click = Some(RowClick::Open(payload.clone()));
            ui.close();
        }
        if self.spec.refresh_action.is_some() {
            let again = ui
                .add_enabled(!self.running, egui::Button::new("Refresh"))
                .on_disabled_hover_text("Not available while running");
            if again.clicked() {
                click = Some(RowClick::Refresh(payload.clone()));
                ui.close();
            }
        }
        if !row.menu.is_empty() {
            ui.separator();
        }
        for item in &row.menu {
            let mut text = RichText::new(&item.label);
            if item.danger && !self.running {
                text = text.color(p.critical);
            }
            let button = ui
                .add_enabled(!self.running, egui::Button::new(text))
                .on_disabled_hover_text("Not available while running");
            if button.clicked() {
                click = Some(RowClick::Action {
                    action: item.action.clone(),
                    payload: payload.clone(),
                });
                ui.close();
            }
        }
        click
    }

    fn rows_ui(&self, ui: &mut Ui, visible: &[usize], menus: bool) -> Option<RowClick<P>> {
        let p = Palette::of(ui);
        let m = Metrics::of(ui);
        let mut open = None;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show_rows(ui, m.row_height, visible.len(), |ui, range| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for &index in &visible[range] {
                    let row = &self.spec.rows[index];
                    let clickable = row.payload.is_some();
                    let (rect, response) = ui.allocate_exact_size(
                        vec2(ui.available_width(), m.row_height),
                        if clickable {
                            Sense::click()
                        } else {
                            Sense::hover()
                        },
                    );
                    // The cells are painted, so screen readers need the row's text.
                    response.widget_info(|| {
                        let text: Vec<String> = self
                            .spec
                            .columns
                            .iter()
                            .map(|column| cell_text(column.kind, row.cells.get(&column.id)))
                            .filter(|text| !text.is_empty())
                            .collect();
                        let role = if clickable { Role::Button } else { Role::Label };
                        WidgetInfo::labeled(role, true, text.join(", "))
                    });
                    if response.hovered() {
                        ui.painter()
                            .rect_filled(rect, m.control_radius, p.subtle_hover);
                    }
                    for (position, (cell_rect, column)) in self
                        .column_rects(rect)
                        .into_iter()
                        .zip(&self.spec.columns)
                        .enumerate()
                    {
                        let text = cell_text(column.kind, row.cells.get(&column.id));
                        Self::paint_cell(
                            ui,
                            cell_rect,
                            position == 0,
                            &text,
                            FontId::proportional(m.body),
                            p.text,
                        );
                    }
                    let response = match &row.tooltip {
                        Some(tooltip) => response.on_hover_text(tooltip),
                        None => response,
                    };
                    let Some(payload) = &row.payload else {
                        continue;
                    };
                    let activated = if menus {
                        // With a menu to offer, a click alone asks for nothing.
                        response.double_clicked()
                            || (response.has_focus()
                                && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                    } else {
                        response.clicked()
                    };
                    if activated {
                        open = Some(RowClick::Open(payload.clone()));
                    }
                    if menus {
                        response.context_menu(|ui| {
                            if let Some(click) = self.row_menu_ui(ui, row, payload) {
                                open = Some(click);
                            }
                        });
                    }
                }
            });
        open
    }

    fn draw(&mut self, ui: &mut Ui, menus: bool) -> Option<RowClick<P>> {
        let weak = Palette::of(ui).text_weak;
        ui.horizontal(|ui| {
            if let Some(subtitle) = &self.subtitle_text() {
                ui.label(RichText::new(subtitle).small().color(weak));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                match &self.spec.progress {
                    // The app's own progress line says what is happening.
                    Some(_) if self.running => {}
                    Some(_) => {
                        ui.label(RichText::new("Stopped early").small().color(weak));
                    }
                    None => {
                        let text = format!("Updated {}", self.updated_text());
                        ui.label(RichText::new(text).small().color(weak));
                    }
                }
            });
        });
        let shown = self.visible().len();
        self.filter_ui(ui, shown);
        ui.add_space(4.0);
        self.header_ui(ui);
        ui.separator();
        // After the controls, so that a change shows this very frame.
        let visible = self.visible();
        if visible.is_empty() {
            let text = if self.order.is_empty() {
                "Nothing to show."
            } else {
                "Nothing is this large."
            };
            ui.label(RichText::new(text).color(weak));
            return None;
        }
        self.rows_ui(ui, &visible, menus)
    }

    /// Draws the table. Returns the payload of the row clicked this frame.
    pub fn contents_ui(&mut self, ui: &mut Ui) -> Option<P> {
        match self.draw(ui, false) {
            Some(RowClick::Open(payload)) => Some(payload),
            _ => None,
        }
    }

    /// Draws the table with a context menu on each row that has a payload, built
    /// from "Open", "Refresh" when the spec asks for it, and the row's own
    /// [`RowAction`]s. Rows open on a double click or Enter, since a single click
    /// on a list like this usually isn't meant to act.
    pub fn contents_with_menus_ui(&mut self, ui: &mut Ui) -> Option<RowClick<P>> {
        self.draw(ui, true)
    }

    /// Draws [`Self::contents_ui`] in a window titled from the spec, whose close
    /// button clears `open`. `id` must be unique to this window.
    pub fn window_ui(&mut self, ctx: &Context, id: Id, open: &mut bool) -> Option<P> {
        let mut clicked = None;
        egui::Window::new(self.spec.title.as_str())
            .id(id)
            .open(open)
            .default_size([560.0, 420.0])
            .show(ctx, |ui| clicked = self.contents_ui(ui));
        clicked
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;

    fn view(rows: &serde_json::Value) -> TableView<PathBuf> {
        let spec: TableSpec<PathBuf> = serde_json::from_value(serde_json::json!({
            "title": "T",
            "columns": [
                { "id": "name", "label": "Name" },
                { "id": "size", "label": "Size", "kind": "size" },
                { "id": "time", "label": "Time", "kind": "time" },
            ],
            "rows": rows,
            "sort": { "column": "size", "descending": true },
        }))
        .expect("spec");
        spec.check().expect("valid");
        TableView::new(spec)
    }

    fn names<P>(view: &TableView<P>) -> Vec<String> {
        view.order
            .iter()
            .map(|&row| cell_text(ColumnKind::Text, view.spec.rows[row].cells.get("name")))
            .collect()
    }

    fn sample() -> TableView<PathBuf> {
        view(&serde_json::json!([
            { "cells": { "name": "b", "size": 10, "time": 200 } },
            { "cells": { "name": "a", "size": 30, "time": 100 } },
            { "cells": { "name": "c", "size": 10, "time": 300 } },
            { "cells": { "name": "blank" } },
        ]))
    }

    #[test]
    fn sorts_and_keeps_blanks_last() {
        let mut view = sample();
        assert_eq!(names(&view), ["a", "b", "c", "blank"]);
        view.header_clicked(1);
        assert_eq!(
            names(&view),
            ["b", "c", "a", "blank"],
            "second click reverses"
        );
        view.header_clicked(0);
        assert_eq!(
            names(&view),
            ["a", "b", "blank", "c"],
            "text starts ascending"
        );
    }

    #[test]
    fn ties_keep_the_previous_order() {
        let mut view = sample();
        view.header_clicked(2);
        assert_eq!(names(&view), ["c", "b", "a", "blank"]);
        view.header_clicked(1);
        assert_eq!(names(&view), ["a", "c", "b", "blank"], "size, ties by date");
    }

    #[test]
    fn each_kind_sorts_both_ways_with_missing_values_last() {
        let mut table: TableView<PathBuf> = TableView::new(
            serde_json::from_value(serde_json::json!({
                "title": "T",
                "columns": [
                    { "id": "name", "label": "Name" },
                    { "id": "size", "label": "Size", "kind": "size" },
                    { "id": "time", "label": "Time", "kind": "time" },
                ],
                "rows": [
                    { "cells": { "name": "Beta", "size": 2, "time": 300 } },
                    { "cells": { "name": "alpha", "time": 100 } },
                    { "cells": { "size": 3 } },
                    { "cells": { "name": "gamma", "size": 1, "time": 200 } },
                ],
            }))
            .expect("spec"),
        );
        assert_eq!(table.sorted_by(), None, "the spec asked for no sort");
        assert_eq!(names(&table), ["Beta", "alpha", "", "gamma"]);

        // Text ignores case, and starts ascending.
        table.header_clicked(0);
        assert_eq!(table.sorted_by(), Some((0, false)));
        assert_eq!(names(&table), ["alpha", "Beta", "gamma", ""]);
        table.header_clicked(0);
        assert_eq!(table.sorted_by(), Some((0, true)));
        assert_eq!(names(&table), ["gamma", "Beta", "alpha", ""]);

        // Sizes start from the largest.
        table.header_clicked(1);
        assert_eq!(table.sorted_by(), Some((1, true)));
        assert_eq!(names(&table), ["", "Beta", "gamma", "alpha"]);
        table.header_clicked(1);
        assert_eq!(table.sorted_by(), Some((1, false)));
        assert_eq!(names(&table), ["gamma", "Beta", "", "alpha"]);

        // So do times.
        table.header_clicked(2);
        assert_eq!(table.sorted_by(), Some((2, true)));
        assert_eq!(names(&table), ["Beta", "gamma", "alpha", ""]);
        table.header_clicked(2);
        assert_eq!(table.sorted_by(), Some((2, false)));
        assert_eq!(names(&table), ["alpha", "gamma", "Beta", ""]);
    }

    #[test]
    fn numbers_and_text_in_one_column_stay_apart() {
        let cell = |text: &str| Some(Cell::Text(text.to_owned()));
        let number = Some(Cell::Number(1.0));
        assert_eq!(
            compare(number.as_ref(), cell("x").as_ref(), false),
            Ordering::Less
        );
        assert_eq!(
            compare(number.as_ref(), cell("x").as_ref(), true),
            Ordering::Greater
        );
        assert_eq!(compare(None, None, true), Ordering::Equal);
    }

    #[test]
    fn updates_keep_the_sort_and_rows_fill_in() {
        let mut table = view(&serde_json::json!([
            { "cells": { "name": "b" } },
            { "cells": { "name": "a" } },
        ]));
        table.header_clicked(1); // Ascending, from descending.
        let spec: TableSpec<PathBuf> = serde_json::from_value(serde_json::json!({
            "title": "T",
            "columns": table.spec.columns.iter().map(|column| serde_json::json!({
                "id": column.id, "label": column.label,
                "kind": if column.id == "size" { "size" } else { "text" },
            })).collect::<Vec<_>>(),
            "rows": [
                { "cells": { "name": "b", "size": 10 } },
                { "cells": { "name": "a", "size": 5 } },
            ],
            "progress": "Scanning",
        }))
        .expect("spec");
        table.update(spec);
        assert_eq!(names(&table), ["a", "b"], "sorted the user's way");
        assert_eq!(table.sorted_by, Some((1, false)));
        assert_eq!(table.spec.progress.as_deref(), Some("Scanning"));
    }

    #[test]
    fn an_update_that_drops_the_sorted_column_forgets_the_sort() {
        let mut table = view(&serde_json::json!([
            { "payload": "/a", "cells": { "name": "a", "size": 1 } },
        ]));
        assert_eq!(table.sorted_by(), Some((1, true)));
        let spec: TableSpec<PathBuf> = serde_json::from_value(serde_json::json!({
            "title": "T",
            "columns": [{ "id": "name", "label": "Name" }],
            "rows": [{ "payload": "/a", "cells": { "name": "a" } }],
        }))
        .expect("spec");
        table.update(spec);
        assert_eq!(table.sorted_by(), None);
        // This sorted by the old column's index, and panicked.
        table.update_row(RowUpdate {
            payload: PathBuf::from("/a"),
            cells: BTreeMap::new(),
            clear: Vec::new(),
            remove: false,
        });
        assert_eq!(names(&table), ["a"]);
    }

    #[test]
    fn apply_sort_ignores_a_column_it_lacks() {
        let mut table = sample();
        table.apply_sort((9, false));
        assert_eq!(table.sorted_by(), Some((1, true)));
        table.apply_sort((0, false));
        assert_eq!(table.sorted_by(), Some((0, false)));
    }

    #[test]
    fn filter_hides_small_rows_but_not_unmeasured_ones() {
        const MB: i64 = 1024 * 1024;
        let mut table = view(&serde_json::json!([
            { "cells": { "name": "big", "size": 2048 * MB } },
            { "cells": { "name": "exact", "size": 1024 * MB } },
            { "cells": { "name": "small", "size": 1023 * MB } },
            { "cells": { "name": "unmeasured" } },
        ]));
        let shown = |table: &TableView<PathBuf>| -> Vec<String> {
            table
                .visible()
                .iter()
                .map(|&row| cell_text(ColumnKind::Text, table.spec.rows[row].cells.get("name")))
                .collect()
        };
        assert_eq!(shown(&table).len(), 4, "off by default");
        table.set_filter(SizeFilter {
            amount: 1.0,
            unit: SizeUnit::Gb,
        });
        assert_eq!(shown(&table), ["big", "exact", "unmeasured"]);
        table.set_filter(SizeFilter {
            amount: 1023.0,
            unit: SizeUnit::Mb,
        });
        assert_eq!(shown(&table).len(), 4);
        // An update keeps it, and sorting still applies underneath.
        table.set_filter(SizeFilter {
            amount: 1.5,
            unit: SizeUnit::Gb,
        });
        table.header_clicked(1);
        assert_eq!(shown(&table), ["big", "unmeasured"]);
        table.update(table.spec.clone());
        assert_eq!(shown(&table), ["big", "unmeasured"]);
    }

    #[test]
    fn rows_update_and_go_and_the_subtitle_follows() {
        let mut table = view(&serde_json::json!([
            { "path": "/a", "cells": { "name": "a", "size": 100 }, "menu": [
                { "label": "Clear", "action": "p.clear", "danger": true }
            ] },
            { "path": "/b", "cells": { "name": "b", "size": 50 } },
            { "path": "/c", "cells": { "name": "c", "size": 10 } },
        ]));
        table.spec.subtitle = Some("Items: {count} · {total}".into());
        assert_eq!(table.subtitle_text().as_deref(), Some("Items: 3 · 160 B"));
        assert_eq!(names(&table), ["a", "b", "c"]);

        let update = |json: serde_json::Value| -> RowUpdate<PathBuf> {
            serde_json::from_value(json).expect("update")
        };
        table.update_row(update(serde_json::json!({
            "path": "/a", "cells": { "size": 1 }, "clear": ["time"],
        })));
        assert_eq!(names(&table), ["b", "c", "a"], "sorted again");
        assert_eq!(table.spec.rows[0].menu.len(), 1, "the menu stays");
        assert_eq!(table.subtitle_text().as_deref(), Some("Items: 3 · 61 B"));

        table.update_row(update(serde_json::json!({ "path": "/b", "remove": true })));
        assert_eq!(names(&table), ["c", "a"]);
        assert!(!table.has_row(Path::new("/b")));
        assert!(table.has_row(Path::new("/a")));
        assert_eq!(table.subtitle_text().as_deref(), Some("Items: 2 · 11 B"));
        table.update_row(update(
            serde_json::json!({ "path": "/gone", "remove": true }),
        ));
        assert_eq!(names(&table), ["c", "a"]);
    }

    #[test]
    fn cells_are_formatted_by_kind() {
        let bytes = Cell::Number(1_048_576.0);
        assert_eq!(
            cell_text(ColumnKind::Size, Some(&bytes)),
            format_bytes(1_048_576)
        );
        assert_eq!(
            cell_text(ColumnKind::Time, Some(&Cell::Number(1_700_000_000.0))),
            "2023-11-14"
        );
        assert_eq!(cell_text(ColumnKind::Size, None), "");
        assert_eq!(cell_text(ColumnKind::Text, Some(&Cell::Number(10.0))), "10");
        assert_eq!(
            cell_text(ColumnKind::Size, Some(&Cell::Text("n/a".into()))),
            "n/a"
        );
    }

    #[test]
    fn dates_read_in_utc_across_leap_days_and_the_epoch() {
        let date = |seconds| date_text(seconds).unwrap_or_default();
        assert_eq!(date(0), "1970-01-01");
        assert_eq!(date(86_399), "1970-01-01", "the last second of the day");
        assert_eq!(date(-1), "1969-12-31");
        assert_eq!(date(951_782_400), "2000-02-29", "a 400-year leap day");
        assert_eq!(date(951_868_800), "2000-03-01");
        assert_eq!(date(1_709_164_800), "2024-02-29");
        assert_eq!(date(-2_203_977_600), "1900-02-28");
        assert_eq!(date(-2_203_891_200), "1900-03-01", "1900 was no leap year");
        assert_eq!(date(1_735_689_599), "2024-12-31");
        assert_eq!(date(1_735_689_600), "2025-01-01");
        assert_eq!(date_text(i64::MAX), None);
        assert_eq!(date_text(i64::MIN), None);
    }

    #[test]
    fn a_table_counts_as_running_only_while_it_reports_progress() {
        let spec = |progress: Option<&str>| -> TableSpec<PathBuf> {
            serde_json::from_value(serde_json::json!({
                "title": "T",
                "columns": [{ "id": "a", "label": "A" }],
                "progress": progress,
            }))
            .expect("spec")
        };
        let mut table = TableView::new(spec(None));
        assert!(!table.running, "row actions stay available");
        table.update(spec(Some("Scanning")));
        assert!(table.running);
        assert_eq!(table.progress(), Some("Scanning"));
        table.set_running(false);
        assert!(!table.running);
        table.update(spec(None));
        assert!(!table.running);
        assert!(TableView::new(spec(Some("Scanning"))).running);
    }

    #[test]
    fn bad_specs_are_refused() {
        let spec: TableSpec<PathBuf> = serde_json::from_value(serde_json::json!({
            "title": "T",
            "columns": [{ "id": "a", "label": "A" }],
            "sort": { "column": "missing" },
        }))
        .expect("spec");
        assert!(spec.check().is_err());
        let spec: TableSpec<PathBuf> = serde_json::from_value(serde_json::json!({
            "title": "T",
            "columns": [],
        }))
        .expect("spec");
        assert!(spec.check().is_err());
    }
}
