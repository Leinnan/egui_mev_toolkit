//! A dropdown with search, a category rail, favorites and recents, for choosing
//! from lists too long to scan.
//!
//! The closed control shows the current entry. Items are only built while the
//! popup is open, and only the rows in view draw or resolve a thumbnail.
//!
//! The picker's own icons are Lucide, so the app must register
//! `IconFont::Lucide` or a subset that includes them.
//!
//! ```ignore
//! let picked = SearchPicker::new("widget", "Widget", &label, Icon::Box)
//!     .categories(&categories)
//!     .memory_key("widgets")
//!     .show(ui, &self.memory, || build_items());
//! if let Some(picked) = picked {
//!     picked.apply_to(&mut self.memory);
//!     if let Some(id) = picked.entry() {
//!         self.selected = id.to_owned();
//!     }
//! }
//! ```

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::rc::Rc;

use egui::{
    Align2, AsIdSalt, Color32, Context, CornerRadius, DragAndDrop, FontId, Frame, Id, Image, Key,
    Modifiers, Popup, PopupCloseBehavior, Rect, RectAlign, Response, RichText, Role, ScrollArea,
    Sense, Stroke, TextEdit, TextureId, Ui, Vec2, WidgetInfo, load::SizedTexture,
    scroll_area::ScrollSource,
};
use egui_mev_theme::{Glyph, Metrics, Palette};
use lucide_icons::Icon;

/// Width of the popup with a category rail, and of the list without one.
const POPUP_WIDTH: f32 = 420.0;
const LIST_ONLY_WIDTH: f32 = 300.0;
const RAIL_WIDTH: f32 = 118.0;
/// Tallest the list area gets, and shortest before it stops being useful.
const LIST_HEIGHT: f32 = 340.0;
const MIN_LIST_HEIGHT: f32 = 120.0;
/// Room the search field, footer, and margins take around the list.
const POPUP_CHROME_HEIGHT: f32 = 96.0;
/// Clear space kept between the popup and the window edge.
const WINDOW_MARGIN: f32 = 8.0;
const HEADER_HEIGHT: f32 = 24.0;
const ROW_HEIGHT: f32 = 28.0;
const THUMBNAIL_ROW_HEIGHT: f32 = 40.0;
const THUMBNAIL_SIZE: f32 = 34.0;
/// How many recent picks each picker keeps.
const MAX_RECENT_PICKS: usize = 8;

/// What the user favorited and picked recently, by picker and entry id. Ids that
/// no longer exist are skipped when listing. The app owns this, and saves it with
/// its settings if it wants it to last.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct PickerMemory {
    /// Favorite entry ids per picker.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "BTreeMap::is_empty")
    )]
    pub favorites: BTreeMap<String, Vec<String>>,
    /// Recently picked entry ids per picker, most recent first.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "BTreeMap::is_empty")
    )]
    pub recents: BTreeMap<String, Vec<String>>,
}

impl PickerMemory {
    /// Favorite entries of a picker.
    #[must_use]
    pub fn favorites_of(&self, picker: &str) -> &[String] {
        self.favorites.get(picker).map_or(&[], Vec::as_slice)
    }

    /// Recent entries of a picker, most recent first.
    #[must_use]
    pub fn recents_of(&self, picker: &str) -> &[String] {
        self.recents.get(picker).map_or(&[], Vec::as_slice)
    }

    /// Add or remove a favorite.
    pub fn toggle_favorite(&mut self, picker: &str, id: &str) {
        let favorites = self.favorites.entry(picker.to_owned()).or_default();
        if let Some(position) = favorites.iter().position(|existing| existing == id) {
            favorites.remove(position);
        } else {
            favorites.push(id.to_owned());
        }
    }

    /// Note that an entry was just picked.
    pub fn remember(&mut self, picker: &str, id: &str) {
        let recents = self.recents.entry(picker.to_owned()).or_default();
        recents.retain(|existing| existing != id);
        recents.insert(0, id.to_owned());
        recents.truncate(MAX_RECENT_PICKS);
    }
}

/// What a row or the closed control shows.
#[derive(Clone)]
pub enum PickerIcon<'a> {
    /// A glyph, drawn in the row's colour.
    Glyph(Glyph),
    /// A picture. `resolve` runs only for rows in view, so it can load lazily; the
    /// row shows `fallback` until it returns a texture. It may borrow for the frame:
    /// to share a `&mut dyn FnMut` between rows, put it in a `RefCell`.
    Texture {
        /// The texture, once it exists.
        resolve: Rc<dyn Fn() -> Option<TextureId> + 'a>,
        /// Shown while there is no texture.
        fallback: Glyph,
    },
}

impl<'a> PickerIcon<'a> {
    /// A picture that is already loaded.
    #[must_use]
    pub fn texture(texture: TextureId, fallback: impl Into<Glyph>) -> Self {
        Self::Texture {
            resolve: Rc::new(move || Some(texture)),
            fallback: fallback.into(),
        }
    }

    fn glyph(&self) -> &Glyph {
        match self {
            Self::Glyph(glyph)
            | Self::Texture {
                fallback: glyph, ..
            } => glyph,
        }
    }

    fn resolved(&self) -> Option<TextureId> {
        match self {
            Self::Glyph(_) => None,
            Self::Texture { resolve, .. } => resolve(),
        }
    }

    const fn is_texture(&self) -> bool {
        matches!(self, Self::Texture { .. })
    }
}

impl fmt::Debug for PickerIcon<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Glyph(glyph) => f.debug_tuple("Glyph").field(glyph).finish(),
            Self::Texture { fallback, .. } => f
                .debug_struct("Texture")
                .field("fallback", fallback)
                .finish_non_exhaustive(),
        }
    }
}

impl<T: Into<Glyph>> From<T> for PickerIcon<'_> {
    fn from(glyph: T) -> Self {
        Self::Glyph(glyph.into())
    }
}

/// One group in the category rail.
#[derive(Clone, Debug)]
pub struct PickerCategory {
    /// Name in the rail and on section headers.
    pub label: String,
    /// Icon in the rail.
    pub icon: Glyph,
}

impl PickerCategory {
    /// A category.
    #[must_use]
    pub fn new(label: impl Into<String>, icon: impl Into<Glyph>) -> Self {
        Self {
            label: label.into(),
            icon: icon.into(),
        }
    }
}

/// One entry in the list.
#[derive(Clone, Debug)]
pub struct PickerItem<'a> {
    /// Stable identifier, returned when picked.
    pub id: String,
    /// Name shown in the list.
    pub label: String,
    /// Index into the picker's categories.
    pub category: Option<usize>,
    /// The row's picture.
    pub icon: PickerIcon<'a>,
    /// Colour of a glyph icon; the weak text colour when `None`.
    pub color: Option<Color32>,
    /// Extra words search should find this entry by.
    pub keywords: String,
    /// Small text at the end of the row, such as a live value.
    pub detail: Option<String>,
    /// Whether to mark the row as needing setup first.
    pub needs_setup: bool,
    /// Whether a texture shows sample data rather than real values; draws it dimmed.
    pub sample: bool,
    /// Text shown when hovering the row.
    pub tooltip: Option<String>,
}

impl<'a> PickerItem<'a> {
    /// An entry with the given id, label and icon.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        icon: impl Into<PickerIcon<'a>>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            category: None,
            icon: icon.into(),
            color: None,
            keywords: String::new(),
            detail: None,
            needs_setup: false,
            sample: false,
            tooltip: None,
        }
    }
}

/// A row being dragged out of a picker, as a [`DragAndDrop`] payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickerDrag {
    /// The name [`SearchPicker::draggable`] gave, which tells pickers apart.
    pub source: String,
    /// The entry id.
    pub id: String,
}

/// What the user did this frame. Call [`Self::apply_to`] to keep favorites and
/// recents up to date.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Picked {
    /// A listed entry was chosen.
    Entry {
        /// The entry's id.
        id: String,
        /// The picker's memory key: where to record this pick as recent.
        remember: Option<String>,
    },
    /// Text typed into the search field was chosen, for pickers that allow it.
    Custom(String),
    /// A row's star was used. Nothing was chosen, and the list stays open.
    ToggleFavorite {
        /// The picker's memory key.
        memory_key: String,
        /// The entry's id.
        id: String,
    },
}

impl Picked {
    /// The id of the chosen entry, if one was chosen.
    #[must_use]
    pub fn entry(&self) -> Option<&str> {
        match self {
            Self::Entry { id, .. } => Some(id),
            Self::Custom(_) | Self::ToggleFavorite { .. } => None,
        }
    }

    /// The typed text, if that was chosen.
    #[must_use]
    pub fn custom(&self) -> Option<&str> {
        match self {
            Self::Custom(text) => Some(text),
            Self::Entry { .. } | Self::ToggleFavorite { .. } => None,
        }
    }

    /// Record this in `memory`. Returns whether `memory` changed, so it needs saving.
    pub fn apply_to(&self, memory: &mut PickerMemory) -> bool {
        match self {
            Self::Entry {
                id,
                remember: Some(key),
            } => {
                memory.remember(key, id);
                true
            }
            Self::ToggleFavorite { memory_key, id } => {
                memory.toggle_favorite(memory_key, id);
                true
            }
            Self::Entry { remember: None, .. } | Self::Custom(_) => false,
        }
    }
}

/// Describes one picker. Build it each frame and call [`Self::show`].
pub struct SearchPicker<'a> {
    id_salt: egui::IdSalt,
    name: &'a str,
    hint: &'a str,
    selected_label: &'a str,
    selected_icon: PickerIcon<'a>,
    selected_color: Option<Color32>,
    selected_id: &'a str,
    categories: &'a [PickerCategory],
    memory_key: Option<&'a str>,
    drag_source: Option<&'a str>,
    custom_label: Option<&'a str>,
    footer: Option<&'a str>,
}

impl<'a> SearchPicker<'a> {
    /// A picker. `id_salt` tells it apart from others in the scope, `name` names it
    /// for assistive technology and tests, and the closed control shows
    /// `selected_label` and `selected_icon`.
    #[must_use]
    pub fn new(
        id_salt: impl AsIdSalt,
        name: &'a str,
        selected_label: &'a str,
        selected_icon: impl Into<PickerIcon<'a>>,
    ) -> Self {
        Self {
            id_salt: egui::IdSalt::new(id_salt),
            name,
            hint: "Search",
            selected_label,
            selected_icon: selected_icon.into(),
            selected_color: None,
            selected_id: "",
            categories: &[],
            memory_key: None,
            drag_source: None,
            custom_label: None,
            footer: None,
        }
    }

    /// Search field hint.
    #[must_use]
    pub const fn hint(mut self, hint: &'a str) -> Self {
        self.hint = hint;
        self
    }

    /// Colour of the closed control's glyph.
    #[must_use]
    pub const fn selected_color(mut self, color: Color32) -> Self {
        self.selected_color = Some(color);
        self
    }

    /// The entry to mark as current in the list.
    #[must_use]
    pub const fn selected_id(mut self, id: &'a str) -> Self {
        self.selected_id = id;
        self
    }

    /// Groups for the rail; none hides the rail.
    #[must_use]
    pub const fn categories(mut self, categories: &'a [PickerCategory]) -> Self {
        self.categories = categories;
        self
    }

    /// Key under which favorites and recents are kept; none disables them.
    #[must_use]
    pub const fn memory_key(mut self, key: &'a str) -> Self {
        self.memory_key = Some(key);
        self
    }

    /// Allows dragging rows out, as a [`PickerDrag`] with this `source`.
    #[must_use]
    pub const fn draggable(mut self, source: &'a str) -> Self {
        self.drag_source = Some(source);
        self
    }

    /// Label for the row that accepts the typed text, such as "Use key".
    #[must_use]
    pub const fn custom_label(mut self, label: &'a str) -> Self {
        self.custom_label = Some(label);
        self
    }

    /// A line under the list, such as "Click to use here · drag onto any key".
    #[must_use]
    pub const fn footer(mut self, text: &'a str) -> Self {
        self.footer = Some(text);
        self
    }
}

/// Where the list is filtered to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Rail {
    #[default]
    All,
    Favorites,
    Recent,
    Category(usize),
}

#[derive(Clone, Debug, Default)]
struct PickerState {
    query: String,
    rail: Rail,
    /// Row index of the keyboard highlight.
    highlight: usize,
    /// Scroll the highlight into view on the next frame.
    reveal: bool,
    /// Whether the popup was open last frame.
    was_open: bool,
}

#[derive(Clone, Debug, PartialEq)]
enum Row {
    Header(String),
    Item { section: usize, item: usize },
}

#[derive(Debug, Default, PartialEq)]
struct RowList {
    rows: Vec<Row>,
    /// The query had no match in the chosen category, so all are listed.
    widened: bool,
}

/// How well an item matches; `None` when some search term isn't found.
fn match_score(item: &PickerItem<'_>, category: &str, query: &str) -> Option<u32> {
    let label = item.label.to_lowercase();
    let query = query.trim().to_lowercase();
    let haystack = format!(
        "{label} {} {}",
        item.keywords.to_lowercase(),
        category.to_lowercase()
    );
    if !query.split_whitespace().all(|term| haystack.contains(term)) {
        return None;
    }
    Some(if label.starts_with(&query) {
        4
    } else if label
        .split(|c: char| !c.is_alphanumeric())
        .any(|word| word.starts_with(&query))
    {
        3
    } else if label.contains(&query) {
        2
    } else {
        1
    })
}

fn items_by_ids(items: &[PickerItem<'_>], ids: &[String]) -> Vec<usize> {
    let positions: HashMap<&str, usize> = items
        .iter()
        .enumerate()
        .map(|(index, item)| (item.id.as_str(), index))
        .collect();
    ids.iter()
        .filter_map(|id| positions.get(id.as_str()).copied())
        .collect()
}

fn search_rows(
    items: &[PickerItem<'_>],
    categories: &[PickerCategory],
    query: &str,
    only_category: Option<usize>,
) -> Vec<Row> {
    let mut matches: Vec<(u32, usize)> = items
        .iter()
        .enumerate()
        .filter(|(_, item)| only_category.is_none() || item.category == only_category)
        .filter_map(|(index, item)| {
            let category = item
                .category
                .and_then(|category| categories.get(category))
                .map_or("", |category| category.label.as_str());
            match_score(item, category, query).map(|score| (score, index))
        })
        .collect();
    matches.sort_by_key(|(score, index)| (std::cmp::Reverse(*score), *index));
    matches
        .into_iter()
        .map(|(_, item)| Row::Item { section: 0, item })
        .collect()
}

/// The rows to list for a query and rail position.
fn build_rows(
    items: &[PickerItem<'_>],
    categories: &[PickerCategory],
    query: &str,
    rail: Rail,
    favorites: &[String],
    recents: &[String],
) -> RowList {
    if !query.trim().is_empty() {
        let only_category = match rail {
            Rail::Category(category) => Some(category),
            _ => None,
        };
        let rows = search_rows(items, categories, query, only_category);
        if rows.is_empty() && only_category.is_some() {
            return RowList {
                rows: search_rows(items, categories, query, None),
                widened: true,
            };
        }
        return RowList {
            rows,
            widened: false,
        };
    }
    let list =
        |section, ids: Vec<usize>| ids.into_iter().map(move |item| Row::Item { section, item });
    let mut rows = Vec::new();
    match rail {
        Rail::Favorites => rows.extend(list(0, items_by_ids(items, favorites))),
        Rail::Recent => rows.extend(list(0, items_by_ids(items, recents))),
        Rail::Category(category) => {
            let ids = (0..items.len())
                .filter(|&index| items[index].category == Some(category))
                .collect();
            rows.extend(list(0, ids));
        }
        Rail::All => {
            let mut section = 0;
            for (title, ids) in [
                ("Favorites", items_by_ids(items, favorites)),
                ("Recent", items_by_ids(items, recents)),
            ] {
                if !ids.is_empty() {
                    rows.push(Row::Header(title.to_owned()));
                    rows.extend(list(section, ids));
                }
                section += 1;
            }
            if categories.is_empty() {
                rows.extend(list(section, (0..items.len()).collect()));
            }
            for (category, details) in categories.iter().enumerate() {
                section += 1;
                let ids: Vec<usize> = (0..items.len())
                    .filter(|&index| items[index].category == Some(category))
                    .collect();
                if !ids.is_empty() {
                    rows.push(Row::Header(details.label.to_uppercase()));
                    rows.extend(list(section, ids));
                }
            }
        }
    }
    RowList {
        rows,
        widened: false,
    }
}

fn first_item_row(rows: &[Row]) -> usize {
    rows.iter()
        .position(|row| matches!(row, Row::Item { .. }))
        .unwrap_or(0)
}

/// The next item row from `from`, stepping over headers.
fn step_highlight(rows: &[Row], from: usize, down: bool) -> usize {
    let mut position = from;
    loop {
        position = if down {
            position.saturating_add(1)
        } else if position == 0 {
            return from;
        } else {
            position - 1
        };
        match rows.get(position) {
            Some(Row::Item { .. }) => return position,
            Some(Row::Header(_)) => {}
            None => return from,
        }
    }
}

/// Frame number of the last frame a list with thumbnails was on screen.
fn thumbnails_id() -> Id {
    Id::NULL.with("search_picker_thumbnails")
}

/// Whether a list showing textures is open, for apps that only do the work behind
/// live thumbnails while someone can see them.
#[must_use]
pub fn thumbnails_visible(ctx: &Context) -> bool {
    ctx.data(|data| data.get_temp::<u64>(thumbnails_id()))
        .is_some_and(|frame| ctx.cumulative_frame_nr().saturating_sub(frame) <= 1)
}

impl SearchPicker<'_> {
    /// Show the control and, when open, the list from `items`.
    pub fn show<'i>(
        self,
        ui: &mut Ui,
        memory: &PickerMemory,
        items: impl FnOnce() -> Vec<PickerItem<'i>>,
    ) -> Option<Picked> {
        let response = self.closed_control(ui);
        let ctx = ui.ctx().clone();
        let popup_id = Popup::default_response_id(&response);
        let is_open = Popup::is_id_open(&ctx, popup_id);
        let state_id = popup_id.with("state");

        let dragging = DragAndDrop::has_payload_of_type::<PickerDrag>(&ctx);
        let mut picked = None;
        let width = if self.categories.is_empty() {
            LIST_ONLY_WIDTH
        } else {
            POPUP_WIDTH
        };
        // Opens on the side with room and shrinks to fit, so it never jumps
        // around between frames.
        let window = ctx.content_rect();
        let below = window.bottom() - response.rect.bottom() - WINDOW_MARGIN;
        let above = response.rect.top() - window.top() - WINDOW_MARGIN;
        let needed = LIST_HEIGHT + POPUP_CHROME_HEIGHT;
        let (align, room) = if below >= needed || below >= above {
            (RectAlign::BOTTOM_END, below)
        } else {
            (RectAlign::TOP_END, above)
        };
        let list_height = (room - POPUP_CHROME_HEIGHT).clamp(MIN_LIST_HEIGHT, LIST_HEIGHT);
        let mut popup = Popup::from_toggle_button_response(&response)
            .id(popup_id)
            .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
            .align(align)
            .align_alternatives(&[])
            .width(width);
        if dragging && is_open {
            // Reveal what's under the list during a drag.
            popup = popup.interactable(false).frame(Frame::NONE);
        }
        popup.show(|ui| {
            let items = items();
            if dragging {
                // No rows at all: even unseen, they would catch the pointer and
                // block drops on what's underneath.
                if let Some(drag) = DragAndDrop::payload::<PickerDrag>(&ctx)
                    && let Some(item) = items.iter().find(|item| item.id == drag.id)
                {
                    paint_drag_ghost(ui, item);
                }
                return;
            }
            ui.set_width(width);
            let mut state: PickerState = ctx
                .data_mut(|data| data.get_temp::<PickerState>(state_id))
                .unwrap_or_default();
            picked = self.popup_contents(ui, &mut state, memory, &items, list_height);
            ctx.data_mut(|data| data.insert_temp(state_id, state));
        });
        if !is_open {
            ctx.data_mut(|data| data.remove::<PickerState>(state_id));
        }
        if matches!(picked, Some(Picked::Entry { .. } | Picked::Custom(_))) {
            Popup::close_id(&ctx, popup_id);
        }
        picked
    }

    fn entry(&self, id: &str) -> Picked {
        Picked::Entry {
            id: id.to_owned(),
            remember: self.memory_key.map(str::to_owned),
        }
    }

    fn closed_control(&self, ui: &mut Ui) -> Response {
        let p = Palette::of(ui);
        let m = Metrics::of(ui);
        let height = m.interact_height.max(26.0);
        let size = Vec2::new(ui.available_width(), height);
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        let response = ui.interact(rect, ui.scope_id().with_salt(self.id_salt), Sense::click());
        response.widget_info(|| {
            let mut info = WidgetInfo::labeled(Role::ComboBox, true, self.name);
            info.current_text_value = Some(self.selected_label.to_owned());
            info
        });
        let fill = if response.hovered() {
            p.card_bg_hover
        } else {
            p.card_bg
        };
        let painter = ui.painter();
        painter.rect(
            rect,
            CornerRadius::same(m.control_radius),
            fill,
            Stroke::new(1.0, p.border),
            egui::StrokeKind::Inside,
        );
        let icon_center = rect.left_center() + Vec2::new(14.0, 0.0);
        if let Some(texture) = self.selected_icon.resolved() {
            let image = Image::new(SizedTexture::new(texture, Vec2::splat(16.0)));
            image.paint_at(ui, Rect::from_center_size(icon_center, Vec2::splat(16.0)));
        } else {
            let glyph = self.selected_icon.glyph();
            painter.text(
                icon_center,
                Align2::CENTER_CENTER,
                glyph.ch,
                glyph.font_id(14.0),
                self.selected_color.unwrap_or(p.text_weak),
            );
        }
        let mut job = egui::text::LayoutJob::simple_singleline(
            self.selected_label.to_owned(),
            FontId::proportional(m.body),
            p.text,
        );
        job.wrap = egui::text::TextWrapping::truncate_at_width(rect.width() - 52.0);
        let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
        ui.painter().galley(
            rect.left_center() + Vec2::new(28.0, -galley.size().y / 2.0),
            galley,
            p.text,
        );
        let chevron = Glyph::from(Icon::ChevronDown);
        ui.painter().text(
            rect.right_center() - Vec2::new(12.0, 0.0),
            Align2::CENTER_CENTER,
            chevron.ch,
            chevron.font_id(13.0),
            p.text_weak,
        );
        response
    }

    #[allow(clippy::too_many_lines)]
    fn popup_contents(
        &self,
        ui: &mut Ui,
        state: &mut PickerState,
        memory: &PickerMemory,
        items: &[PickerItem<'_>],
        height: f32,
    ) -> Option<Picked> {
        let p = Palette::of(ui);
        let ctx = ui.ctx().clone();
        let favorites = self
            .memory_key
            .map_or(&[][..], |key| memory.favorites_of(key));
        let recents = self
            .memory_key
            .map_or(&[][..], |key| memory.recents_of(key));
        let just_opened = !state.was_open;
        state.was_open = true;
        let mut picked = None;

        let list = build_rows(
            items,
            self.categories,
            &state.query,
            state.rail,
            favorites,
            recents,
        );
        if just_opened {
            state.highlight = list
                .rows
                .iter()
                .position(|row| {
                    matches!(row, Row::Item { item, .. } if items[*item].id == self.selected_id)
                })
                .unwrap_or_else(|| first_item_row(&list.rows));
            state.reveal = true;
        }

        // Taken before the search field sees them, which would drop focus on
        // Enter.
        let (down, up, enter, star) = ui.input_mut(|input| {
            (
                input.consume_key(Modifiers::NONE, Key::ArrowDown),
                input.consume_key(Modifiers::NONE, Key::ArrowUp),
                input.consume_key(Modifiers::NONE, Key::Enter),
                input.consume_key(Modifiers::COMMAND, Key::D),
            )
        });
        if down || up {
            state.highlight = step_highlight(&list.rows, state.highlight, down);
            state.reveal = true;
        }
        let highlighted = list.rows.get(state.highlight).and_then(|row| match row {
            Row::Item { item, .. } => Some(*item),
            Row::Header(_) => None,
        });
        if enter && let Some(item) = highlighted {
            picked = Some(self.entry(&items[item].id));
        } else if enter && self.custom_label.is_some() && !state.query.trim().is_empty() {
            picked = Some(Picked::Custom(state.query.trim().to_owned()));
        }
        if star && let (Some(item), Some(key)) = (highlighted, self.memory_key) {
            picked = picked.or_else(|| {
                Some(Picked::ToggleFavorite {
                    memory_key: key.to_owned(),
                    id: items[item].id.clone(),
                })
            });
        }

        // ── Search ──────────────────────────────────────────────────────────
        ui.horizontal(|ui| {
            let search = Glyph::from(Icon::Search);
            ui.label(search.rich(14.0).color(p.text_weak));
            let edit = ui
                .add(
                    TextEdit::singleline(&mut state.query)
                        .id(ui.scope_id().with("query"))
                        .hint_text(self.hint)
                        .frame(Frame::NONE)
                        .min_size(Vec2::new(0.0, ui.spacing().interact_size.y))
                        .align(egui::Align2::LEFT_CENTER)
                        .desired_width(ui.available_width()),
                )
                .accessible_name(self.hint);
            if just_opened {
                edit.request_focus();
            }
            if edit.changed() {
                state.highlight = usize::MAX;
            }
        });
        ui.separator();
        if state.highlight == usize::MAX {
            let rows = build_rows(
                items,
                self.categories,
                &state.query,
                state.rail,
                favorites,
                recents,
            );
            state.highlight = first_item_row(&rows.rows);
            state.reveal = true;
        }

        // ── Rail + list ─────────────────────────────────────────────────────
        ui.horizontal_top(|ui| {
            if !self.categories.is_empty() {
                ui.vertical(|ui| {
                    ui.set_width(RAIL_WIDTH);
                    ui.set_height(height);
                    self.rail(ui, state, items, favorites, recents);
                });
                // A plain `separator` would stretch to the popup's height and
                // make it grow every frame.
                let (divider, _) = ui.allocate_exact_size(Vec2::new(1.0, height), Sense::hover());
                ui.painter().vline(
                    divider.center().x,
                    divider.y_range(),
                    Stroke::new(1.0, p.border),
                );
            }
            ui.vertical(|ui| {
                ui.set_height(height);
                let list = build_rows(
                    items,
                    self.categories,
                    &state.query,
                    state.rail,
                    favorites,
                    recents,
                );
                if let Some(choice) = self.list(ui, state, &list, memory, items, height) {
                    picked = Some(choice);
                }
            });
        });
        if let Some(footer) = self.footer {
            ui.separator();
            ui.label(RichText::new(footer).small().color(p.section_label));
        }
        if items.iter().any(|item| item.icon.is_texture()) {
            let frame = ctx.cumulative_frame_nr();
            ctx.data_mut(|data| data.insert_temp(thumbnails_id(), frame));
        }
        picked
    }

    fn rail(
        &self,
        ui: &mut Ui,
        state: &mut PickerState,
        items: &[PickerItem<'_>],
        favorites: &[String],
        recents: &[String],
    ) {
        ui.spacing_mut().item_spacing.y = 2.0;
        let mut row = |ui: &mut Ui, rail: Rail, icon: &Glyph, label: &str, count: usize| {
            if rail_row(ui, icon, label, count, state.rail == rail).clicked() {
                state.rail = rail;
                state.highlight = usize::MAX;
            }
        };
        let favorite_count = items_by_ids(items, favorites).len();
        if favorite_count > 0 {
            row(
                ui,
                Rail::Favorites,
                &Icon::Star.into(),
                "Favorites",
                favorite_count,
            );
        }
        let recent_count = items_by_ids(items, recents).len();
        if recent_count > 0 {
            row(
                ui,
                Rail::Recent,
                &Icon::History.into(),
                "Recent",
                recent_count,
            );
        }
        row(ui, Rail::All, &Icon::List.into(), "All", items.len());
        for (index, category) in self.categories.iter().enumerate() {
            let count = items
                .iter()
                .filter(|item| item.category == Some(index))
                .count();
            if count > 0 {
                row(
                    ui,
                    Rail::Category(index),
                    &category.icon,
                    &category.label,
                    count,
                );
            }
        }
    }

    fn list(
        &self,
        ui: &mut Ui,
        state: &mut PickerState,
        list: &RowList,
        memory: &PickerMemory,
        items: &[PickerItem<'_>],
        height: f32,
    ) -> Option<Picked> {
        let p = Palette::of(ui);
        let m = Metrics::of(ui);
        let mut picked = None;
        let query = state.query.trim().to_owned();
        if list.widened {
            ui.label(
                RichText::new("No matches here; showing all categories")
                    .small()
                    .color(p.section_label),
            );
        }
        if list.rows.is_empty() {
            let message = if self.custom_label.is_some() && !query.is_empty() {
                ""
            } else if query.is_empty() {
                "Nothing here yet"
            } else {
                "No matches"
            };
            if !message.is_empty() {
                ui.add_space(12.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new(message).color(p.text_weak));
                });
            }
        }
        if let Some(label) = self.custom_label
            && !query.is_empty()
            && !list
                .rows
                .iter()
                .any(|row| matches!(row, Row::Item { item, .. } if items[*item].id == query))
        {
            let text = format!("{label} {query}");
            if ui.selectable_label(false, text).clicked() {
                picked = Some(Picked::Custom(query));
            }
        }
        let has_thumbnails = items.iter().any(|item| item.icon.is_texture());
        let item_height = if has_thumbnails {
            THUMBNAIL_ROW_HEIGHT
        } else {
            ROW_HEIGHT
        };
        let row_height = |row: &Row| match row {
            Row::Header(_) => HEADER_HEIGHT,
            Row::Item { .. } => item_height,
        };
        let total: f32 = list.rows.iter().map(row_height).sum();
        let favorites = self
            .memory_key
            .map_or(&[][..], |key| memory.favorites_of(key));
        ScrollArea::vertical()
            .id_salt(ui.scope_id().with("rows"))
            .max_height(height)
            .auto_shrink([false, false])
            .scroll_source(ScrollSource::MOUSE_WHEEL | ScrollSource::SCROLL_BAR)
            .show_viewport(ui, |ui, viewport| {
                let width = ui.available_width();
                ui.set_height(total);
                let origin = ui.min_rect().min;
                let mut y = 0.0;
                for (position, row) in list.rows.iter().enumerate() {
                    let height = row_height(row);
                    let rect =
                        Rect::from_min_size(origin + Vec2::new(0.0, y), Vec2::new(width, height));
                    let visible = y + height >= viewport.min.y && y <= viewport.max.y;
                    y += height;
                    if state.reveal && position == state.highlight {
                        ui.scroll_to_rect(rect, None);
                        state.reveal = false;
                    }
                    if !visible {
                        continue;
                    }
                    match row {
                        Row::Header(title) => {
                            ui.painter().text(
                                rect.left_center() + Vec2::new(6.0, 2.0),
                                Align2::LEFT_CENTER,
                                title,
                                FontId::proportional(m.caption),
                                p.section_label,
                            );
                        }
                        Row::Item { section, item } => {
                            let item = &items[*item];
                            let favorite = favorites.contains(&item.id);
                            let id = ui.scope_id().with(("row", *section, &item.id));
                            if let Some(choice) = self.item_row(
                                ui,
                                rect,
                                id,
                                item,
                                position == state.highlight,
                                favorite,
                            ) {
                                picked = Some(choice);
                            }
                        }
                    }
                }
            });
        picked
    }

    /// Draw one row; returns what the user did with it.
    fn item_row(
        &self,
        ui: &mut Ui,
        rect: Rect,
        id: Id,
        item: &PickerItem<'_>,
        highlighted: bool,
        favorite: bool,
    ) -> Option<Picked> {
        let p = Palette::of(ui);
        let m = Metrics::of(ui);
        let sense = if self.drag_source.is_some() {
            Sense::click_and_drag()
        } else {
            Sense::click()
        };
        let response = ui.interact(rect, id, sense);
        let selected = item.id == self.selected_id;
        response.widget_info(|| WidgetInfo::selected(Role::Button, true, selected, &item.label));
        if let Some(source) = self.drag_source {
            response.dnd_set_drag_payload(PickerDrag {
                source: source.to_owned(),
                id: item.id.clone(),
            });
        }
        let hovered = response.hovered();
        let painter = ui.painter_at(rect);
        if selected || highlighted || hovered {
            let fill = if selected {
                p.selection_bg
            } else {
                p.card_bg_hover
            };
            painter.rect_filled(
                rect.shrink2(Vec2::new(0.0, 1.0)),
                CornerRadius::same(m.control_radius),
                fill,
            );
        }

        // ── Picture ─────────────────────────────────────────────────────────
        // Rows line up even when some have no thumbnail; the row height says
        // whether this list has them.
        let tall = rect.height() > ROW_HEIGHT;
        let text_x = rect.min.x + if tall { THUMBNAIL_SIZE + 12.0 } else { 30.0 };
        let picture = Rect::from_center_size(
            egui::pos2((rect.min.x + text_x) / 2.0 - 1.0, rect.center().y),
            Vec2::splat(if tall { THUMBNAIL_SIZE } else { 18.0 }),
        );
        if let Some(texture) = item.icon.resolved() {
            let tint = if item.sample {
                Color32::from_white_alpha(150)
            } else {
                Color32::WHITE
            };
            Image::new(SizedTexture::new(texture, picture.size()))
                .corner_radius(CornerRadius::same(5))
                .tint(tint)
                .paint_at(ui, picture);
        } else {
            let glyph = item.icon.glyph();
            painter.text(
                picture.center(),
                Align2::CENTER_CENTER,
                glyph.ch,
                glyph.font_id(if tall { 18.0 } else { 15.0 }),
                item.color.unwrap_or(p.text_weak),
            );
        }

        // ── Text ────────────────────────────────────────────────────────────
        let star_rect = Rect::from_center_size(
            rect.right_center() - Vec2::new(14.0, 0.0),
            Vec2::splat(20.0),
        );
        let mut trailing = rect.right() - 6.0;
        if self.memory_key.is_some() {
            trailing = star_rect.left() - 2.0;
        }
        if let Some(detail) = &item.detail {
            let galley = ui.fonts_mut(|fonts| {
                fonts.layout_no_wrap(detail.clone(), FontId::proportional(m.caption), p.text_weak)
            });
            let width = galley.size().x;
            painter.galley(
                egui::pos2(trailing - width, rect.center().y - galley.size().y / 2.0),
                galley,
                p.text_weak,
            );
            trailing -= width + 6.0;
        }
        if item.needs_setup {
            painter.circle_filled(egui::pos2(trailing - 4.0, rect.center().y), 3.0, p.neutral);
            trailing -= 12.0;
        }
        let mut job = egui::text::LayoutJob::simple_singleline(
            item.label.clone(),
            FontId::proportional(m.body),
            if selected { p.text } else { p.text_label },
        );
        job.wrap = egui::text::TextWrapping::truncate_at_width((trailing - text_x).max(10.0));
        let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
        painter.galley(
            egui::pos2(text_x, rect.center().y - galley.size().y / 2.0),
            galley,
            p.text,
        );

        // ── Favorite star ───────────────────────────────────────────────────
        if let Some(key) = self.memory_key
            && (favorite || hovered || highlighted)
        {
            let star = ui.interact(star_rect, id.with("star"), Sense::click());
            let name = if favorite {
                format!("Remove {} from favorites", item.label)
            } else {
                format!("Add {} to favorites", item.label)
            };
            star.widget_info(|| WidgetInfo::labeled(Role::Button, true, &name));
            let color = if favorite {
                p.caution
            } else if star.hovered() {
                p.text
            } else {
                p.hint_text
            };
            let glyph = Glyph::from(Icon::Star);
            painter.text(
                star_rect.center(),
                Align2::CENTER_CENTER,
                glyph.ch,
                glyph.font_id(13.0),
                color,
            );
            if star.clicked() {
                return Some(Picked::ToggleFavorite {
                    memory_key: key.to_owned(),
                    id: item.id.clone(),
                });
            }
        }
        let clicked = response.clicked();
        if let Some(tooltip) = &item.tooltip {
            response.on_hover_text(tooltip);
        }
        clicked.then(|| self.entry(&item.id))
    }
}

fn rail_row(ui: &mut Ui, icon: &Glyph, label: &str, count: usize, selected: bool) -> Response {
    let p = Palette::of(ui);
    let m = Metrics::of(ui);
    let size = Vec2::new(ui.available_width(), 24.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| WidgetInfo::selected(Role::Button, true, selected, label));
    let painter = ui.painter();
    let radius = CornerRadius::same(m.control_radius);
    if selected {
        painter.rect_filled(rect, radius, p.selection_bg);
    } else if response.hovered() {
        painter.rect_filled(rect, radius, p.card_bg_hover);
    }
    painter.text(
        rect.left_center() + Vec2::new(11.0, 0.0),
        Align2::CENTER_CENTER,
        icon.ch,
        icon.font_id(13.0),
        if selected { p.text } else { p.text_weak },
    );
    painter.text(
        rect.left_center() + Vec2::new(23.0, 0.0),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(m.caption),
        if selected { p.text } else { p.text_label },
    );
    painter.text(
        rect.right_center() - Vec2::new(6.0, 0.0),
        Align2::RIGHT_CENTER,
        count.to_string(),
        FontId::proportional(m.caption),
        p.text_weak,
    );
    response
}

/// The picture following the pointer during a drag.
fn paint_drag_ghost(ui: &Ui, item: &PickerItem<'_>) {
    let Some(pointer) = ui.ctx().pointer_latest_pos() else {
        return;
    };
    let p = Palette::of(ui);
    let texture = item.icon.resolved();
    egui::Area::new(Id::NULL.with("search_picker_drag_ghost"))
        .order(egui::Order::Tooltip)
        .interactable(false)
        .fixed_pos(pointer + Vec2::new(10.0, 10.0))
        .show(ui.ctx(), |ui| {
            Frame::popup(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    if let Some(texture) = texture {
                        ui.add(
                            Image::new(SizedTexture::new(texture, Vec2::splat(THUMBNAIL_SIZE)))
                                .corner_radius(CornerRadius::same(5)),
                        );
                    } else {
                        let color = item.color.unwrap_or(p.text_weak);
                        ui.label(item.icon.glyph().rich(18.0).color(color));
                    }
                    ui.label(RichText::new(&item.label).color(p.text));
                });
            });
        });
}

/// The glyph for a Lucide icon name; `?` when there's no icon of that name.
#[must_use]
pub fn lucide_glyph(name: &str) -> Glyph {
    Icon::try_from(name).map_or_else(|_| Glyph::new('?'), Glyph::from)
}

#[cfg(test)]
mod tests {
    use egui_kittest::{Harness, kittest::Queryable as _};
    use egui_mev_theme::{FontOptions, IconFont, Theme};

    use super::*;

    fn item(id: &str, label: &str, category: Option<usize>, keywords: &str) -> PickerItem<'static> {
        PickerItem {
            category,
            keywords: keywords.to_owned(),
            ..PickerItem::new(id, label, 'x')
        }
    }

    fn categories() -> [PickerCategory; 2] {
        [
            PickerCategory::new("System", 'a'),
            PickerCategory::new("Time", 'b'),
        ]
    }

    fn items() -> Vec<PickerItem<'static>> {
        vec![
            item("cpu", "CPU Usage", Some(0), "processor"),
            item("mem", "Memory Usage", Some(0), "ram"),
            item("clock", "Clock", Some(1), "time"),
        ]
    }

    fn ids(list: &RowList, items: &[PickerItem]) -> Vec<String> {
        list.rows
            .iter()
            .filter_map(|row| match row {
                Row::Item { item, .. } => Some(items[*item].id.clone()),
                Row::Header(_) => None,
            })
            .collect()
    }

    #[test]
    fn synonyms_and_categories_are_searchable() {
        let items = items();
        let list = build_rows(&items, &categories(), "ram", Rail::All, &[], &[]);
        assert_eq!(ids(&list, &items), ["mem"]);
        let list = build_rows(&items, &categories(), "system usage", Rail::All, &[], &[]);
        assert_eq!(ids(&list, &items), ["cpu", "mem"]);
    }

    #[test]
    fn a_name_prefix_ranks_before_other_matches() {
        let items = vec![
            item("other", "Top Process", None, "cpu busy"),
            item("cpu", "CPU Usage", None, ""),
        ];
        let list = build_rows(&items, &[], "cpu", Rail::All, &[], &[]);
        assert_eq!(ids(&list, &items), ["cpu", "other"]);
    }

    #[test]
    fn a_query_with_no_match_in_the_category_widens_to_all() {
        let items = items();
        let list = build_rows(&items, &categories(), "clock", Rail::Category(0), &[], &[]);
        assert!(list.widened);
        assert_eq!(ids(&list, &items), ["clock"]);
    }

    #[test]
    fn a_query_is_limited_to_the_chosen_category() {
        let items = items();
        let list = build_rows(&items, &categories(), "usage", Rail::Category(1), &[], &[]);
        assert!(list.widened, "nothing in Time should widen");
        let list = build_rows(&items, &categories(), "usage", Rail::Category(0), &[], &[]);
        assert!(!list.widened);
        assert_eq!(ids(&list, &items), ["cpu", "mem"]);
    }

    #[test]
    fn favorites_and_recents_lead_the_full_list() {
        let items = items();
        let list = build_rows(
            &items,
            &categories(),
            "",
            Rail::All,
            &[String::from("clock")],
            &[String::from("mem")],
        );
        assert_eq!(list.rows[0], Row::Header(String::from("Favorites")));
        assert_eq!(ids(&list, &items)[..2], ["clock", "mem"]);
    }

    #[test]
    fn unknown_remembered_ids_are_skipped() {
        let items = items();
        let list = build_rows(
            &items,
            &categories(),
            "",
            Rail::Favorites,
            &[String::from("gone"), String::from("cpu")],
            &[],
        );
        assert_eq!(ids(&list, &items), ["cpu"]);
    }

    #[test]
    fn the_highlight_steps_over_headers_and_stops_at_the_ends() {
        let rows = vec![
            Row::Header(String::from("A")),
            Row::Item {
                section: 0,
                item: 0,
            },
            Row::Header(String::from("B")),
            Row::Item {
                section: 1,
                item: 1,
            },
        ];
        assert_eq!(first_item_row(&rows), 1);
        assert_eq!(step_highlight(&rows, 1, true), 3);
        assert_eq!(step_highlight(&rows, 3, false), 1);
        assert_eq!(step_highlight(&rows, 3, true), 3);
        assert_eq!(step_highlight(&rows, 1, false), 1);
    }

    #[test]
    fn memory_toggles_favorites_and_orders_recents() {
        let mut memory = PickerMemory::default();
        memory.toggle_favorite("p", "a");
        assert_eq!(memory.favorites_of("p"), ["a"]);
        memory.toggle_favorite("p", "a");
        assert!(memory.favorites_of("p").is_empty());
        memory.remember("p", "a");
        memory.remember("p", "b");
        memory.remember("p", "a");
        assert_eq!(memory.recents_of("p"), ["a", "b"]);
        assert!(memory.recents_of("other").is_empty());
        for n in 0..MAX_RECENT_PICKS + 3 {
            memory.remember("p", &n.to_string());
        }
        assert_eq!(memory.recents_of("p").len(), MAX_RECENT_PICKS);
    }

    #[test]
    fn a_pick_says_how_to_update_the_memory() {
        let mut memory = PickerMemory::default();
        let remembered = Picked::Entry {
            id: "a".to_owned(),
            remember: Some("p".to_owned()),
        };
        assert_eq!(remembered.entry(), Some("a"));
        assert!(remembered.apply_to(&mut memory));
        assert_eq!(memory.recents_of("p"), ["a"]);

        let favorite = Picked::ToggleFavorite {
            memory_key: "p".to_owned(),
            id: "b".to_owned(),
        };
        assert_eq!(favorite.entry(), None);
        assert!(favorite.apply_to(&mut memory));
        assert_eq!(memory.favorites_of("p"), ["b"]);

        let before = memory.clone();
        let unremembered = Picked::Entry {
            id: "c".to_owned(),
            remember: None,
        };
        assert!(!unremembered.apply_to(&mut memory));
        assert!(!Picked::Custom("typed".to_owned()).apply_to(&mut memory));
        assert_eq!(memory, before);
    }

    #[test]
    fn lucide_names_become_glyphs() {
        assert_eq!(lucide_glyph("star"), Glyph::from(Icon::Star));
        assert_eq!(lucide_glyph("no-such-icon"), Glyph::new('?'));
    }

    #[test]
    fn textures_resolve_only_when_asked() {
        let calls = Rc::new(std::cell::Cell::new(0));
        let counter = Rc::clone(&calls);
        let icon = PickerIcon::Texture {
            resolve: Rc::new(move || {
                counter.set(counter.get() + 1);
                None
            }),
            fallback: Glyph::new('f'),
        };
        assert!(icon.is_texture());
        assert_eq!(calls.get(), 0);
        assert_eq!(icon.resolved(), None);
        assert_eq!(calls.get(), 1);
        assert_eq!(icon.glyph(), &Glyph::new('f'));
    }

    #[derive(Default)]
    struct App {
        memory: PickerMemory,
        picked: Vec<Picked>,
        built: u32,
    }

    fn harness<'a>() -> Harness<'a, App> {
        Harness::builder()
            .with_size([600.0, 600.0])
            .with_context_setup(|ctx| {
                egui_mev_test::install_with(
                    ctx,
                    Theme::default(),
                    &FontOptions {
                        icons: vec![IconFont::Lucide],
                        ..Default::default()
                    },
                );
            })
            .build_ui_state(
                |ui, app: &mut App| {
                    let categories = categories();
                    let picked = SearchPicker::new("picker", "Widget", "CPU Usage", 'x')
                        .selected_id("cpu")
                        .categories(&categories)
                        .memory_key("widgets")
                        .show(ui, &app.memory, || {
                            app.built += 1;
                            items()
                        });
                    if let Some(picked) = picked {
                        picked.apply_to(&mut app.memory);
                        app.picked.push(picked);
                    }
                },
                App::default(),
            )
    }

    #[test]
    fn items_are_built_only_while_open() {
        let mut harness = harness();
        harness.run();
        assert_eq!(harness.state().built, 0);
        harness.get_by_label("Widget").click();
        harness.run();
        assert!(harness.state().built > 0);
    }

    #[test]
    fn clicking_a_row_picks_it_remembers_it_and_closes() {
        let mut harness = harness();
        harness.run();
        harness.get_by_label("Widget").click();
        harness.run();
        harness.get_by_label("Clock").click();
        harness.run();
        assert_eq!(
            harness.state().picked,
            [Picked::Entry {
                id: "clock".to_owned(),
                remember: Some("widgets".to_owned()),
            }]
        );
        assert_eq!(harness.state().memory.recents_of("widgets"), ["clock"]);
        assert!(harness.query_by_label("Clock").is_none());
    }

    #[test]
    fn typing_then_return_picks_the_best_match() {
        let mut harness = harness();
        harness.run();
        harness.get_by_label("Widget").click();
        harness.run();
        harness.event(egui::Event::Text("ram".to_owned()));
        harness.run();
        harness.key_press(Key::Enter);
        harness.run();
        assert_eq!(harness.state().picked[0].entry(), Some("mem"));
    }

    #[test]
    fn the_star_favorites_without_closing_the_list() {
        let mut harness = harness();
        harness.run();
        harness.get_by_label("Widget").click();
        harness.run();
        // The highlighted row (the current entry) shows its star.
        harness.get_by_label("Add CPU Usage to favorites").click();
        harness.run();
        assert_eq!(harness.state().memory.favorites_of("widgets"), ["cpu"]);
        assert_eq!(
            harness.state().picked,
            [Picked::ToggleFavorite {
                memory_key: "widgets".to_owned(),
                id: "cpu".to_owned(),
            }]
        );
        assert!(harness.query_by_label("Clock").is_some());
    }

    #[test]
    fn the_rail_filters_to_a_category() {
        let mut harness = harness();
        harness.run();
        harness.get_by_label("Widget").click();
        harness.run();
        assert!(harness.query_by_label("Memory Usage").is_some());
        harness.get_by_label("Time").click();
        harness.run();
        assert!(harness.query_by_label("Memory Usage").is_none());
        assert!(harness.query_by_label("Clock").is_some());
    }

    #[test]
    fn a_custom_label_offers_the_typed_text() {
        let mut harness = Harness::builder()
            .with_size([500.0, 500.0])
            .with_context_setup(|ctx| {
                egui_mev_test::install_with(
                    ctx,
                    Theme::default(),
                    &FontOptions {
                        icons: vec![IconFont::Lucide],
                        ..Default::default()
                    },
                );
            })
            .build_ui_state(
                |ui, picked: &mut Option<Picked>| {
                    let choice = SearchPicker::new("picker", "Key", "A", 'x')
                        .custom_label("Use key")
                        .show(ui, &PickerMemory::default(), Vec::new);
                    if choice.is_some() {
                        *picked = choice;
                    }
                },
                None,
            );
        harness.run();
        harness.get_by_label("Key").click();
        harness.run();
        harness.event(egui::Event::Text("F13".to_owned()));
        harness.run();
        harness.get_by_label("Use key F13").click();
        harness.run();
        assert_eq!(harness.state().as_ref().unwrap().custom(), Some("F13"));
    }

    #[test]
    fn only_rows_in_view_resolve_their_textures() {
        // Far more rows than fit, each counting how often it is asked for a picture.
        let resolved = Rc::new(std::cell::RefCell::new(Vec::<usize>::new()));
        let log = Rc::clone(&resolved);
        let mut harness = Harness::builder()
            .with_size([500.0, 500.0])
            .with_context_setup(|ctx| {
                egui_mev_test::install_with(
                    ctx,
                    Theme::default(),
                    &FontOptions {
                        icons: vec![IconFont::Lucide],
                        ..Default::default()
                    },
                );
            })
            .build_ui(move |ui| {
                let log = Rc::clone(&log);
                let _ = SearchPicker::new("picker", "Big", "None", 'x').show(
                    ui,
                    &PickerMemory::default(),
                    move || {
                        (0..500)
                            .map(|index| {
                                let log = Rc::clone(&log);
                                PickerItem::new(
                                    index.to_string(),
                                    format!("Row {index}"),
                                    PickerIcon::Texture {
                                        resolve: Rc::new(move || {
                                            log.borrow_mut().push(index);
                                            None
                                        }),
                                        fallback: Glyph::new('x'),
                                    },
                                )
                            })
                            .collect()
                    },
                );
            });
        harness.run();
        harness.get_by_label("Big").click();
        harness.run();
        let highest = resolved.borrow().iter().copied().max().unwrap();
        assert!(highest < 50, "row {highest} is far out of view");
    }

    #[test]
    fn a_draggable_row_carries_its_id_and_source() {
        let mut harness = Harness::builder()
            .with_size([500.0, 500.0])
            .with_context_setup(|ctx| {
                egui_mev_test::install_with(
                    ctx,
                    Theme::default(),
                    &FontOptions {
                        icons: vec![IconFont::Lucide],
                        ..Default::default()
                    },
                );
            })
            .build_ui(|ui| {
                let _ = SearchPicker::new("picker", "Key", "A", 'x')
                    .draggable("content")
                    .show(ui, &PickerMemory::default(), items);
            });
        harness.run();
        harness.get_by_label("Key").click();
        harness.run();
        let from = harness.get_by_label("Clock").rect().center();
        harness.hover_at(from);
        egui_mev_test::settle(&mut harness);
        harness.drag_at(from);
        egui_mev_test::settle(&mut harness);
        harness.hover_at(from + Vec2::new(40.0, 40.0));
        egui_mev_test::settle(&mut harness);
        let payload =
            DragAndDrop::payload::<PickerDrag>(&harness.ctx).expect("a drag is under way");
        assert_eq!(payload.source, "content");
        assert_eq!(payload.id, "clock");
        harness.drop_at(from + Vec2::new(40.0, 40.0));
        egui_mev_test::settle(&mut harness);
    }

    /// A resolver can borrow what it needs for the frame, like a `&mut` cache.
    #[test]
    fn a_borrowed_resolver_runs_only_for_rows_in_view() {
        let resolved = Rc::new(std::cell::RefCell::new(Vec::<usize>::new()));
        let log = Rc::clone(&resolved);
        let mut harness = Harness::builder()
            .with_size([500.0, 500.0])
            .with_context_setup(|ctx| {
                egui_mev_test::install_with(
                    ctx,
                    Theme::default(),
                    &FontOptions {
                        icons: vec![IconFont::Lucide],
                        ..Default::default()
                    },
                );
            })
            .build_ui(move |ui| {
                let mut this_frame = Vec::new();
                let mut resolve = |index: usize| -> Option<TextureId> {
                    this_frame.push(index);
                    None
                };
                let resolve = std::cell::RefCell::new(&mut resolve);
                let resolve = &resolve;
                let _ = SearchPicker::new("picker", "Big", "None", 'x').show(
                    ui,
                    &PickerMemory::default(),
                    || {
                        (0..500)
                            .map(|index| {
                                PickerItem::new(
                                    index.to_string(),
                                    format!("Row {index}"),
                                    PickerIcon::Texture {
                                        resolve: Rc::new(move || (resolve.borrow_mut())(index)),
                                        fallback: Glyph::new('x'),
                                    },
                                )
                            })
                            .collect()
                    },
                );
                log.borrow_mut().append(&mut this_frame);
            });
        harness.run();
        assert!(resolved.borrow().is_empty());
        harness.get_by_label("Big").click();
        harness.run();
        let highest = resolved.borrow().iter().copied().max().unwrap();
        assert!(highest < 50, "row {highest} is far out of view");
    }
}
