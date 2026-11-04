//! A searchable list of actions, opened over the app, that hands back the one
//! chosen.

use std::borrow::Cow;

use egui::{
    Atom, Button, Context, Frame, Id, Key, Margin, Modal, Modifiers, RichText, ScrollArea, Vec2,
};
use egui_mev_theme::{Metrics, Palette};
use egui_mev_widgets::search_field;
use fuzzy_matcher::{FuzzyMatcher as _, skim::SkimMatcherV2};

/// Narrowest and widest the palette gets (px).
const WIDTH: (f32, f32) = (300.0, 460.0);
/// Room for everything around the list (px).
const CHROME_HEIGHT: f32 = 180.0;

/// One action on offer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaletteItem<A> {
    /// What is handed back when this is chosen.
    pub action: A,
    /// Its name, which the search reads.
    pub name: Cow<'static, str>,
    /// Keyboard shortcut shown next to the name.
    pub shortcut: Option<String>,
}

impl<A> PaletteItem<A> {
    /// `action`, called `name`.
    #[must_use]
    pub fn new(name: impl Into<Cow<'static, str>>, action: A) -> Self {
        Self {
            action,
            name: name.into(),
            shortcut: None,
        }
    }

    /// With a keyboard shortcut to show, such as from `Keymap::shortcut_text`.
    #[must_use]
    pub fn shortcut(mut self, shortcut: Option<String>) -> Self {
        self.shortcut = shortcut;
        self
    }
}

/// The palette's words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaletteLabels {
    /// The heading.
    pub title: Cow<'static, str>,
    /// Inside the empty search field.
    pub search_hint: Cow<'static, str>,
    /// Shown when nothing matches.
    pub no_matches: Cow<'static, str>,
    /// The keys, at the bottom.
    pub help: Cow<'static, str>,
}

impl Default for PaletteLabels {
    fn default() -> Self {
        Self {
            title: "Commands".into(),
            search_hint: "Find a command…".into(),
            no_matches: "No matching commands.".into(),
            help: "Up/Down to choose · Enter to run · Esc to close".into(),
        }
    }
}

/// What the palette did this frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PaletteResponse<A> {
    /// Still open.
    Open,
    /// The action to run. Stop showing the palette, first if the action opens a
    /// window of its own.
    Chosen(A),
    /// Dismissed with Escape or a click outside. Stop showing the palette.
    Closed,
}

/// A command palette over any kind of action. Show it every frame while open.
#[derive(Clone, Debug)]
pub struct CommandPalette<A> {
    items: Vec<PaletteItem<A>>,
    labels: PaletteLabels,
    query: String,
    selected: usize,
    focus_requested: bool,
}

impl<A> Default for CommandPalette<A> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            labels: PaletteLabels::default(),
            query: String::new(),
            selected: 0,
            focus_requested: false,
        }
    }
}

impl<A> CommandPalette<A> {
    /// A palette offering `items`, in the order to show them before any search.
    #[must_use]
    pub fn new(items: Vec<PaletteItem<A>>) -> Self {
        Self {
            items,
            ..Self::default()
        }
    }

    /// With other words than the English defaults.
    #[must_use]
    pub fn labels(mut self, labels: PaletteLabels) -> Self {
        self.labels = labels;
        self
    }

    /// Open again with `items`: an empty search, the first item selected and focus
    /// in the search field.
    pub fn reopen(&mut self, items: Vec<PaletteItem<A>>) {
        self.items = items;
        self.query.clear();
        self.selected = 0;
        self.focus_requested = false;
    }

    /// The indices of the items matching `query`, the best match first. Equally
    /// good matches keep their order, and an empty query matches everything.
    fn ranked(&self, query: &str) -> Vec<usize> {
        let query = query.trim();
        if query.is_empty() {
            return (0..self.items.len()).collect();
        }
        let matcher = SkimMatcherV2::default().ignore_case();
        let mut matches: Vec<(i64, usize)> = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| {
                matcher
                    .fuzzy_match(&item.name, query)
                    .map(|score| (score, index))
            })
            .collect();
        matches.sort_by_key(|&(score, _)| std::cmp::Reverse(score));
        matches.into_iter().map(|(_, index)| index).collect()
    }
}

impl<A: Clone> CommandPalette<A> {
    /// Draw the palette. Up, Down and Enter belong to it while it is open.
    pub fn show(&mut self, ctx: &Context) -> PaletteResponse<A> {
        let consume = |key| ctx.input_mut(|input| input.consume_key(Modifiers::NONE, key));
        let down = consume(Key::ArrowDown);
        let up = consume(Key::ArrowUp);
        let enter = consume(Key::Enter);

        let p = Palette::of_ctx(ctx);
        let m = Metrics::of_ctx(ctx);
        let frame = Frame::window(&ctx.global_style())
            .fill(p.window_bg)
            .inner_margin(Margin::same(m.card_padding + 6));
        let mut chosen = None;
        let modal = Modal::new(Id::unique("egui_mev_commands::CommandPalette"))
            .frame(frame)
            .show(ctx, |ui| {
                ui.set_width((ctx.content_rect().width() - 64.0).clamp(WIDTH.0, WIDTH.1));
                ui.label(
                    RichText::new(self.labels.title.as_ref())
                        .size(m.subtitle)
                        .strong()
                        .color(p.text),
                );
                ui.add_space(m.card_gap);
                let hint = self.labels.search_hint.clone();
                let edit = search_field(ui, &mut self.query, &hint);
                if edit.changed() {
                    self.selected = 0;
                }
                if !self.focus_requested {
                    edit.request_focus();
                    self.focus_requested = true;
                }
                ui.separator();
                let ranked = self.ranked(&self.query);
                if ranked.is_empty() {
                    ui.label(RichText::new(self.labels.no_matches.as_ref()).color(p.text_weak));
                } else {
                    if down {
                        self.selected = (self.selected + 1).min(ranked.len() - 1);
                    }
                    if up {
                        self.selected = self.selected.saturating_sub(1);
                    }
                    self.selected = self.selected.min(ranked.len() - 1);
                    ScrollArea::vertical()
                        .max_height((ctx.content_rect().height() - CHROME_HEIGHT).max(m.row_height))
                        .show(ui, |ui| {
                            for (row, &index) in ranked.iter().enumerate() {
                                let item = &self.items[index];
                                // egui's selected text is the accent on an accent fill.
                                let selected = row == self.selected;
                                let name = RichText::new(item.name.as_ref()).color(p.text);
                                let button = Button::selectable(selected, name)
                                    .min_size(Vec2::new(ui.available_width(), m.row_height));
                                let button = match &item.shortcut {
                                    Some(shortcut) if selected => button
                                        .right_text(RichText::new(shortcut.as_str()).color(p.text)),
                                    Some(shortcut) => button.shortcut_text(shortcut.as_str()),
                                    // Keeps the name on the left, like the others.
                                    None => button.right_text(Atom::grow()),
                                };
                                let response = ui.add(button);
                                if response.clicked() {
                                    chosen = Some(item.action.clone());
                                }
                                if row == self.selected && (up || down) {
                                    response.scroll_to_me(None);
                                }
                            }
                        });
                    if enter {
                        chosen = Some(self.items[ranked[self.selected]].action.clone());
                    }
                }
                ui.separator();
                ui.label(
                    RichText::new(self.labels.help.as_ref())
                        .size(m.caption)
                        .color(p.text_weak),
                );
            });
        match chosen {
            Some(action) => PaletteResponse::Chosen(action),
            None if modal.should_close() => PaletteResponse::Closed,
            None => PaletteResponse::Open,
        }
    }
}

#[cfg(test)]
mod tests {
    use egui_kittest::{Harness, kittest::Queryable as _};

    use super::*;

    fn items() -> Vec<PaletteItem<&'static str>> {
        vec![
            PaletteItem::new("Go Up", "up"),
            PaletteItem::new("Open in new tab", "tab"),
            PaletteItem::new("Open in new window", "window"),
            PaletteItem::new("Add to favorites", "favorite"),
            PaletteItem::new("Save search", "save"),
        ]
    }

    fn names(palette: &CommandPalette<&'static str>, query: &str) -> Vec<&'static str> {
        palette
            .ranked(query)
            .into_iter()
            .map(|index| palette.items[index].action)
            .collect()
    }

    #[test]
    fn an_empty_search_keeps_the_table_order() {
        let palette = CommandPalette::new(items());
        let all = vec!["up", "tab", "window", "favorite", "save"];
        assert_eq!(names(&palette, ""), all);
        assert_eq!(names(&palette, "  "), all);
    }

    #[test]
    fn matches_letters_in_order_with_gaps() {
        let palette = CommandPalette::new(items());
        // Not a substring of anything, which the old filter needed.
        assert_eq!(names(&palette, "onw"), vec!["window", "tab"]);
        assert_eq!(names(&palette, "zzz"), Vec::<&str>::new());
    }

    #[test]
    fn the_best_match_comes_first_whatever_the_table_order() {
        let palette = CommandPalette::new(vec![
            PaletteItem::new("Discard all changes", "discard"),
            PaletteItem::new("Search", "search"),
            PaletteItem::new("Save", "save"),
        ]);
        assert_eq!(names(&palette, "sa")[0], "save");
        assert_eq!(names(&palette, "SEA")[0], "search", "any case");
    }

    #[test]
    fn equal_scores_keep_the_table_order() {
        let palette = CommandPalette::new(vec![
            PaletteItem::new("Save", 1),
            PaletteItem::new("Save", 2),
            PaletteItem::new("Save", 3),
        ]);
        assert_eq!(palette.ranked("save"), vec![0, 1, 2]);
    }

    struct State {
        palette: CommandPalette<&'static str>,
        closed: bool,
        ran: Vec<&'static str>,
    }

    fn harness() -> Harness<'static, State> {
        let mut harness = Harness::builder().with_size([600.0, 500.0]).build_ui_state(
            |ui, state: &mut State| match state.palette.show(ui.ctx()) {
                PaletteResponse::Open => {}
                PaletteResponse::Chosen(action) => state.ran.push(action),
                PaletteResponse::Closed => state.closed = true,
            },
            State {
                palette: CommandPalette::new(items()),
                closed: false,
                ran: Vec::new(),
            },
        );
        harness.run();
        harness
    }

    #[test]
    fn enter_runs_the_first_item_at_the_start() {
        let mut harness = harness();
        harness.key_press(Key::Enter);
        harness.run();
        assert_eq!(harness.state().ran, vec!["up"]);
    }

    #[test]
    fn arrow_keys_move_and_stop_at_the_ends() {
        let mut harness = harness();
        harness.key_press(Key::ArrowDown);
        harness.run();
        harness.key_press(Key::ArrowDown);
        harness.run();
        harness.key_press(Key::ArrowUp);
        harness.run();
        assert_eq!(harness.state().palette.selected, 1);
        harness.key_press(Key::Enter);
        harness.run();
        assert_eq!(harness.state().ran, vec!["tab"]);

        for _ in 0..4 {
            harness.key_press(Key::ArrowUp);
            harness.run();
        }
        assert_eq!(harness.state().palette.selected, 0);
        for _ in 0..9 {
            harness.key_press(Key::ArrowDown);
            harness.run();
        }
        assert_eq!(harness.state().palette.selected, 4);
    }

    #[test]
    fn typing_goes_to_the_search_and_restarts_the_selection() {
        let mut harness = harness();
        harness.key_press(Key::ArrowDown);
        harness.run();
        assert_eq!(harness.state().palette.selected, 1);
        harness.event(egui::Event::Text("fav".to_owned()));
        harness.run();
        assert_eq!(harness.state().palette.query, "fav");
        assert_eq!(harness.state().palette.selected, 0);
        harness.key_press(Key::Enter);
        harness.run();
        assert_eq!(harness.state().ran, vec!["favorite"]);
    }

    #[test]
    fn enter_with_no_match_runs_nothing() {
        let mut harness = harness();
        harness.event(egui::Event::Text("zzz".to_owned()));
        harness.run();
        assert!(harness.query_by_label("No matching commands.").is_some());
        harness.key_press(Key::Enter);
        harness.run();
        assert_eq!(harness.state().ran, Vec::<&str>::new());
    }

    #[test]
    fn clicking_an_item_chooses_it() {
        let mut harness = harness();
        harness.get_by_label("Save search").click();
        harness.run();
        assert_eq!(harness.state().ran, vec!["save"]);
    }

    #[test]
    fn escape_closes() {
        let mut harness = harness();
        harness.key_press(Key::Escape);
        harness.run();
        assert!(harness.state().closed);
        assert_eq!(harness.state().ran, Vec::<&str>::new());
    }

    #[test]
    fn the_shortcut_is_part_of_the_row() {
        let palette = CommandPalette::new(vec![
            PaletteItem::new("Save", 1).shortcut(Some("Ctrl+S".to_owned())),
        ]);
        let mut harness = Harness::builder().with_size([600.0, 400.0]).build_ui_state(
            |ui, palette: &mut CommandPalette<i32>| {
                palette.show(ui.ctx());
            },
            palette,
        );
        harness.run();
        assert!(harness.query_by_label_contains("Ctrl+S").is_some());
    }

    #[test]
    fn reopening_starts_over() {
        let mut harness = harness();
        harness.key_press(Key::ArrowDown);
        harness.event(egui::Event::Text("o".to_owned()));
        harness.run();
        harness.state_mut().palette.reopen(items());
        let palette = &harness.state().palette;
        assert_eq!((palette.query.as_str(), palette.selected), ("", 0));
        assert!(!palette.focus_requested);
    }
}
