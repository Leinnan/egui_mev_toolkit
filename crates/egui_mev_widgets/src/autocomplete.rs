//! A single-line text field with a completion popup, after
//! `egui_autocomplete` (JakeHandsome/egui_autocomplete).
//!
//! As there: the field's keys (Up/Down, Tab, Enter, Escape) are consumed
//! *before* the [`egui::TextEdit`] sees them while it has focus, the
//! selection lives in egui memory under the field's id, matched characters
//! are highlighted with a [`LayoutJob`], and accepting a choice replaces
//! only the word it completes and puts the cursor after it.
//!
//! Unlike there, the choices come from a *completer* callback given the text
//! and the cursor, which returns the byte range a choice replaces — so a
//! console can complete a command's third argument from a different list
//! than its first word. [`complete_from_list`] is the plain "fuzzy-filter
//! one list" completer. Up/Down with no choice selected are left to the
//! caller ([`AutoCompleteOutput::history`]), and Enter submits.
//!
//! | key    | popup open                                  | popup closed |
//! |--------|---------------------------------------------|--------------|
//! | Down   | select the next choice                      | `history`    |
//! | Up     | select the previous one (from the first: none) | `history` |
//! | Tab    | accept the selected choice, or the first    | —            |
//! | Enter  | accept the selected choice, else submit     | submit       |
//! | Escape | close the popup until the next edit         | `escape`     |
//!
//! A field that holds one value (an id, a path) sets [`single_value`]: an
//! accepted choice is the whole value (no space after it) and closes the
//! popup. [`open_on_focus`] shows the choices as soon as the field has
//! focus, before anything is typed (recent or likely picks first).
//!
//! [`single_value`]: AutoCompleteTextEdit::single_value
//! [`open_on_focus`]: AutoCompleteTextEdit::open_on_focus

use std::ops::Range;

use egui::{
    AsIdSalt, Color32, FontId, Id, IdSalt, Key, Modifiers, Popup, PopupCloseBehavior, RectAlign,
    Response, TextFormat, Ui, text::CCursor, text::CCursorRange, text::LayoutJob,
};
use egui_mev_theme::Palette;
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;

/// One completion choice.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Suggestion {
    /// What the choice inserts.
    pub text: String,
    /// Shown after the text, dimmed (usage, help).
    pub detail: String,
    /// The whole row dimmed (e.g. a cheat without devmode).
    pub dim: bool,
    /// Char indices of `text` to highlight.
    pub matched: Vec<usize>,
}

/// What the completer offers for the text at the cursor.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Completions {
    /// Byte range of the text a choice replaces.
    pub range: Range<usize>,
    /// The choices, best first.
    pub items: Vec<Suggestion>,
}

/// Up/Down pressed with no choice selected: the caller's history.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryStep {
    /// Up: an earlier line.
    Older,
    /// Down: a later line.
    Newer,
}

/// What happened to the field this frame.
pub struct AutoCompleteOutput {
    /// The text edit's response.
    pub response: Response,
    /// Enter with no choice selected: the text to run.
    pub submitted: bool,
    /// A choice was accepted.
    pub accepted: bool,
    /// Up or Down with no choice selected.
    pub history: Option<HistoryStep>,
    /// Escape with the popup closed.
    pub escape: bool,
    /// The popup is showing.
    pub popup_open: bool,
}

/// Selection state, kept in egui memory.
#[derive(Clone, Debug, Default)]
struct State {
    selected: Option<usize>,
    dismissed: bool,
    last_text: String,
    /// Where the popup was last frame: pressing on it takes the focus from
    /// the field, so the popup stays while the pointer is over it (else a
    /// click on a choice would close it before it lands).
    popup: Option<egui::Rect>,
}

type Completer<'a> = Box<dyn FnMut(&str, usize) -> Completions + 'a>;
type EditFn<'a> = Box<dyn FnOnce(egui::TextEdit<'_>) -> egui::TextEdit<'_> + 'a>;

/// The field. Build it each frame and call [`Self::show`].
pub struct AutoCompleteTextEdit<'a> {
    text: &'a mut String,
    completer: Completer<'a>,
    id_salt: IdSalt,
    max_suggestions: usize,
    highlight: bool,
    single_value: bool,
    open_on_focus: bool,
    text_edit: Option<EditFn<'a>>,
}

impl<'a> AutoCompleteTextEdit<'a> {
    /// `completer(text, cursor)` gets the cursor as a byte offset.
    pub fn new(
        text: &'a mut String,
        completer: impl FnMut(&str, usize) -> Completions + 'a,
    ) -> Self {
        Self {
            text,
            completer: Box::new(completer),
            id_salt: IdSalt::new("autocomplete"),
            max_suggestions: 8,
            highlight: true,
            single_value: false,
            open_on_focus: false,
            text_edit: None,
        }
    }

    /// The inner [`egui::TextEdit`]'s id for a field shown in `ui` with this
    /// salt (to request focus, or read its cursor before showing it).
    #[must_use]
    pub fn edit_id(ui: &Ui, salt: impl AsIdSalt) -> Id {
        ui.scope_id().with_salt(IdSalt::new(salt))
    }

    /// Whether the field with this salt is in use: focused, or its popup
    /// under the pointer (a completer can skip its work otherwise).
    #[must_use]
    pub fn is_active(ui: &Ui, salt: impl AsIdSalt) -> bool {
        let edit_id = Self::edit_id(ui, salt);
        let ctx = ui.ctx();
        ctx.memory(|m| m.has_focus(edit_id))
            || ctx
                .data(|d| d.get_temp::<State>(edit_id.with("autocomplete_state")))
                .and_then(|state| state.popup)
                .is_some_and(|rect| pointer_in(ctx, rect))
    }

    /// Tells this field apart from others in the same scope.
    #[must_use]
    pub fn id_salt(mut self, salt: impl AsIdSalt) -> Self {
        self.id_salt = IdSalt::new(salt);
        self
    }

    /// Rows shown at most (default 8).
    #[must_use]
    pub fn max_suggestions(mut self, max: usize) -> Self {
        self.max_suggestions = max.max(1);
        self
    }

    /// Highlight the matched characters (default on).
    #[must_use]
    pub const fn highlight_matches(mut self, highlight: bool) -> Self {
        self.highlight = highlight;
        self
    }

    /// The field holds one value: an accepted choice gets no space after it
    /// and closes the popup.
    #[must_use]
    pub const fn single_value(mut self, single: bool) -> Self {
        self.single_value = single;
        self
    }

    /// Show the choices on focus, before anything is typed.
    #[must_use]
    pub const fn open_on_focus(mut self, open: bool) -> Self {
        self.open_on_focus = open;
        self
    }

    /// Adjusts the inner [`egui::TextEdit`] (font, hint text, width…).
    #[must_use]
    pub fn text_edit(
        mut self,
        edit: impl FnOnce(egui::TextEdit<'_>) -> egui::TextEdit<'_> + 'a,
    ) -> Self {
        self.text_edit = Some(Box::new(edit));
        self
    }

    /// Show the field and, when there are choices, its popup.
    pub fn show(mut self, ui: &mut Ui) -> AutoCompleteOutput {
        let edit_id = ui.scope_id().with_salt(self.id_salt);
        let state_id = edit_id.with("autocomplete_state");
        let ctx = ui.ctx().clone();
        let mut state: State = ctx.data(|d| d.get_temp(state_id)).unwrap_or_default();
        if state.last_text != *self.text {
            state.selected = None;
            state.dismissed = false;
        }
        let on_popup = state.popup.is_some_and(|rect| pointer_in(&ctx, rect));
        let focused = ctx.memory(|m| m.has_focus(edit_id));
        let cursor = cursor_byte(&ctx, edit_id, self.text);
        let mut completions = (self.completer)(self.text, cursor);
        let max = self.max_suggestions;
        let on_focus = self.open_on_focus;
        let shown =
            |completions: &Completions, text: &str| visible(completions, text, on_focus).min(max);
        let mut count = shown(&completions, self.text);
        let mut open = (focused || on_popup) && !state.dismissed && count > 0;
        if state.selected.is_some_and(|i| i >= count) {
            state.selected = None;
        }

        let (mut submitted, mut accepted, mut escape) = (false, false, false);
        let mut history = None;
        let mut accept: Option<usize> = None;
        if focused {
            ctx.input_mut(|i| {
                if i.consume_key(Modifiers::NONE, Key::ArrowDown) {
                    match (open, state.selected) {
                        (true, None) => state.selected = Some(0),
                        (true, Some(s)) if s + 1 < count => state.selected = Some(s + 1),
                        (true, Some(_)) => {}
                        (false, _) => history = Some(HistoryStep::Newer),
                    }
                }
                if i.consume_key(Modifiers::NONE, Key::ArrowUp) {
                    match (open, state.selected) {
                        (true, Some(0)) => state.selected = None,
                        (true, Some(s)) => state.selected = Some(s - 1),
                        (_, None) => history = Some(HistoryStep::Older),
                        (false, Some(_)) => state.selected = None,
                    }
                }
                if i.consume_key(Modifiers::NONE, Key::Tab) && open {
                    accept = Some(state.selected.unwrap_or(0));
                }
                if i.consume_key(Modifiers::NONE, Key::Enter) {
                    match (open, state.selected) {
                        (true, Some(s)) => accept = Some(s),
                        _ => submitted = true,
                    }
                }
                if i.consume_key(Modifiers::NONE, Key::Escape) {
                    if open {
                        state.dismissed = true;
                        state.selected = None;
                    } else {
                        escape = true;
                    }
                }
            });
        }
        if let Some(index) = accept {
            self.accept(&ctx, edit_id, &completions, index);
            state.selected = None;
            accepted = true;
        }
        if history.is_some() {
            // Recalling a line is not typing: no popup until the next edit.
            state.dismissed = true;
        }

        let before = self.text.clone();
        let mut edit = egui::TextEdit::singleline(self.text)
            .id(edit_id)
            .desired_width(f32::INFINITY);
        if let Some(adjust) = self.text_edit.take() {
            edit = adjust(edit);
        }
        let response = ui.add(edit);
        if response.has_focus() {
            // The field keeps Escape, Tab and the arrows: egui would
            // otherwise drop or move its focus on them before we see them.
            ctx.memory_mut(|m| {
                m.set_focus_lock_filter(
                    edit_id,
                    egui::EventFilter {
                        tab: true,
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        escape: true,
                    },
                )
            });
        }
        if *self.text != before || accepted {
            let cursor = cursor_byte(&ctx, edit_id, self.text);
            completions = (self.completer)(self.text, cursor);
            count = shown(&completions, self.text);
            if *self.text != before {
                state.selected = None;
                state.dismissed = false;
            }
        }
        open = (response.has_focus() || on_popup) && !state.dismissed && count > 0;

        let mut clicked = None;
        state.popup = None;
        if open {
            let selected = state.selected;
            let highlight = self.highlight;
            let palette = Palette::of(ui);
            let total = completions.items.len();
            let shown = Popup::from_response(&response)
                .id(edit_id.with("autocomplete_popup"))
                .open(true)
                .align(RectAlign::BOTTOM_START)
                .width(response.rect.width())
                .close_behavior(PopupCloseBehavior::IgnoreClicks)
                .show(|ui| {
                    let font =
                        FontId::monospace(ui.style().text_styles[&egui::TextStyle::Body].size);
                    for (index, item) in completions.items.iter().take(count).enumerate() {
                        let job = suggestion_job(item, highlight, font.clone(), &palette);
                        let row = ui.selectable_label(selected == Some(index), job);
                        if row.clicked() {
                            clicked = Some(index);
                        }
                    }
                    if total > count {
                        ui.label(
                            egui::RichText::new(format!("… {} more", total - count))
                                .color(palette.hint_text)
                                .small(),
                        );
                    }
                });
            state.popup = shown.map(|inner| inner.response.rect);
        }
        if let Some(index) = clicked {
            self.accept(&ctx, edit_id, &completions, index);
            state.selected = None;
            accepted = true;
        }
        state.last_text = self.text.clone();
        if accepted {
            // The text changed on purpose; keep the popup for the next word
            // (a single value is complete).
            state.dismissed = self.single_value;
        }
        ctx.data_mut(|d| d.insert_temp(state_id, state));
        AutoCompleteOutput {
            response,
            submitted,
            accepted,
            history,
            escape,
            popup_open: open,
        }
    }

    /// Replaces the choice's range with it (and a space) and puts the
    /// cursor after it.
    fn accept(
        &mut self,
        ctx: &egui::Context,
        edit_id: Id,
        completions: &Completions,
        index: usize,
    ) {
        let Some(item) = completions.items.get(index) else {
            return;
        };
        let range = completions.range.start.min(self.text.len())
            ..completions.range.end.min(self.text.len());
        self.text.replace_range(range.clone(), &item.text);
        let mut end = range.start + item.text.len();
        if !self.single_value {
            if !self.text[end..].starts_with(char::is_whitespace) {
                self.text.insert(end, ' ');
            }
            end += 1;
        }
        let char_index = self.text[..end].chars().count();
        let mut edit_state = egui::text_edit::TextEditState::load(ctx, edit_id).unwrap_or_default();
        edit_state
            .cursor
            .set_char_range(Some(CCursorRange::one(CCursor::new(char_index))));
        edit_state.store(ctx, edit_id);
        ctx.memory_mut(|m| m.request_focus(edit_id));
    }
}

/// Whether the pointer is over `rect` (or pressed there).
fn pointer_in(ctx: &egui::Context, rect: egui::Rect) -> bool {
    ctx.input(|i| {
        i.pointer
            .hover_pos()
            .or(i.pointer.interact_pos())
            .is_some_and(|p| rect.contains(p))
    })
}

/// How many choices are worth showing: none on an empty field (unless the
/// field opens on focus), or when the only one is exactly what is typed
/// already.
fn visible(completions: &Completions, text: &str, on_focus: bool) -> usize {
    if text.trim().is_empty() && !on_focus {
        // Nothing typed yet: no popup (as egui_autocomplete).
        return 0;
    }
    match completions.items.as_slice() {
        [] => 0,
        [only]
            if text
                .get(completions.range.clone())
                .is_some_and(|typed| typed == only.text) =>
        {
            0
        }
        items => items.len(),
    }
}

/// The cursor's byte offset in a text field (the end when the field has
/// no state yet).
#[must_use]
pub fn cursor_byte(ctx: &egui::Context, edit_id: Id, text: &str) -> usize {
    let chars = egui::text_edit::TextEditState::load(ctx, edit_id)
        .and_then(|state| state.cursor.char_range())
        .map(|range| range.primary.index.0);
    match chars {
        Some(chars) => text
            .char_indices()
            .nth(chars)
            .map_or(text.len(), |(byte, _)| byte),
        None => text.len(),
    }
}

/// The row: the text with matched characters in the accent color, then the
/// detail, dimmed.
#[must_use]
pub fn suggestion_job(
    item: &Suggestion,
    highlight: bool,
    font: FontId,
    palette: &Palette,
) -> LayoutJob {
    let base = if item.dim {
        palette.text_weak
    } else {
        palette.text
    };
    let mut job = LayoutJob::default();
    let format = |color: Color32| TextFormat::simple(font.clone(), color);
    let mut run = String::new();
    let mut run_hit = false;
    for (index, c) in item.text.chars().enumerate() {
        let hit = highlight && item.matched.contains(&index);
        if hit != run_hit && !run.is_empty() {
            let color = if run_hit { palette.accent } else { base };
            job.append(&std::mem::take(&mut run), 0.0, format(color));
        }
        run_hit = hit;
        run.push(c);
    }
    if !run.is_empty() {
        let color = if run_hit { palette.accent } else { base };
        job.append(&run, 0.0, format(color));
    }
    if !item.detail.is_empty() {
        job.append(&item.detail, 12.0, format(palette.hint_text));
    }
    job
}

/// A completer over one list: the whole text is the query, ranked by skim
/// score (best first).
#[must_use]
pub fn complete_from_list<S: AsRef<str>>(items: &[S], text: &str) -> Completions {
    let matcher = SkimMatcherV2::default().ignore_case();
    let mut scored: Vec<(i64, Suggestion)> = items
        .iter()
        .filter_map(|item| {
            let item = item.as_ref();
            let (score, matched) = if text.is_empty() {
                (0, Vec::new())
            } else {
                matcher.fuzzy_indices(item, text)?
            };
            Some((
                score,
                Suggestion {
                    text: item.to_owned(),
                    matched,
                    ..Default::default()
                },
            ))
        })
        .collect();
    scored.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    Completions {
        range: 0..text.len(),
        items: scored.into_iter().map(|(_, s)| s).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, RawInput};

    const WORDS: &[&str] = &["addItem", "addGold", "godmode", "heal"];

    /// One field over `text` (its last word completed from [`WORDS`]).
    struct Harness {
        ctx: egui::Context,
        text: String,
        last: Option<AutoCompleteOutput>,
        single: bool,
    }

    impl Harness {
        fn new(text: &str) -> Self {
            Self::with(text, false)
        }

        fn with(text: &str, single: bool) -> Self {
            let mut harness = Self {
                ctx: crate::test_support::themed(egui::Theme::Dark),
                text: text.to_owned(),
                last: None,
                single,
            };
            // Focus the field, and let it settle.
            harness.frame(Vec::new());
            let id = harness.edit_id();
            harness.ctx.memory_mut(|m| m.request_focus(id));
            harness.frame(Vec::new());
            harness.frame(Vec::new());
            harness
        }

        fn edit_id(&self) -> Id {
            self.last.as_ref().unwrap().response.id
        }

        fn frame(&mut self, events: Vec<Event>) -> &AutoCompleteOutput {
            let input = RawInput {
                events,
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 600.0),
                )),
                ..Default::default()
            };
            let text = &mut self.text;
            let single = self.single;
            let mut out = None;
            self.ctx
                .run_ui(input, |ui| {
                    ui.push_id("autocomplete_test_area", |ui| {
                        let field = AutoCompleteTextEdit::new(text, |text, cursor| {
                            let start = text[..cursor]
                                .rfind(char::is_whitespace)
                                .map_or(0, |i| i + 1);
                            let mut c = complete_from_list(WORDS, &text[start..cursor]);
                            c.range = start..cursor;
                            c
                        })
                        .id_salt("field")
                        .single_value(single)
                        .open_on_focus(single);
                        out = Some(field.show(ui));
                    });
                })
                .textures_delta
                .clear();
            self.last = out;
            self.last.as_ref().unwrap()
        }

        fn key(&mut self, key: Key, modifiers: Modifiers) -> &AutoCompleteOutput {
            self.frame(vec![
                Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers,
                },
                Event::Key {
                    key,
                    physical_key: None,
                    pressed: false,
                    repeat: false,
                    modifiers,
                },
            ])
        }

        fn type_text(&mut self, text: &str) -> &AutoCompleteOutput {
            self.frame(vec![Event::Text(text.to_owned())])
        }
    }

    #[test]
    fn typing_opens_the_popup_and_tab_accepts_the_best() {
        let mut h = Harness::new("");
        assert!(!h.last.as_ref().unwrap().popup_open);
        assert!(h.type_text("addI").popup_open);
        assert!(h.key(Key::Tab, Modifiers::NONE).accepted);
        assert_eq!(h.text, "addItem ");
        // The cursor sits after the space: typing continues there.
        h.frame(Vec::new());
        h.type_text("x");
        assert_eq!(h.text, "addItem x");
    }

    #[test]
    fn down_selects_and_enter_accepts_then_submits() {
        let mut h = Harness::new("");
        h.type_text("add");
        h.key(Key::ArrowDown, Modifiers::NONE);
        h.key(Key::ArrowDown, Modifiers::NONE);
        let out = h.key(Key::Enter, Modifiers::NONE);
        assert!(out.accepted && !out.submitted);
        assert!(h.text == "addGold " || h.text == "addItem ", "{}", h.text);
        h.frame(Vec::new());
        assert!(h.key(Key::Enter, Modifiers::NONE).submitted);
    }

    #[test]
    fn up_from_the_first_choice_leaves_the_list_and_then_is_history() {
        let mut h = Harness::new("");
        h.type_text("add");
        h.key(Key::ArrowDown, Modifiers::NONE);
        assert_eq!(h.key(Key::ArrowUp, Modifiers::NONE).history, None);
        assert_eq!(
            h.key(Key::ArrowUp, Modifiers::NONE).history,
            Some(HistoryStep::Older)
        );
        // Recalling history hides the popup.
        assert!(!h.frame(Vec::new()).popup_open);
    }

    #[test]
    fn escape_closes_the_popup_then_reports() {
        let mut h = Harness::new("");
        h.type_text("god");
        let out = h.key(Key::Escape, Modifiers::NONE);
        assert!(!out.escape && !out.popup_open);
        assert_eq!(h.text, "god");
        assert!(h.key(Key::Escape, Modifiers::NONE).escape);
        // Typing brings it back.
        assert!(h.type_text("m").popup_open);
    }

    #[test]
    fn accepting_mid_line_replaces_only_that_word() {
        let mut h = Harness::new("hea 5");
        // Cursor after "hea".
        let mut state = egui::text_edit::TextEditState::load(&h.ctx, h.edit_id()).unwrap();
        state
            .cursor
            .set_char_range(Some(CCursorRange::one(CCursor::new(3))));
        state.store(&h.ctx, h.edit_id());
        h.frame(Vec::new());
        assert!(h.key(Key::Tab, Modifiers::NONE).accepted);
        assert_eq!(h.text, "heal 5");
    }

    #[test]
    fn a_single_value_field_opens_on_focus_and_closes_on_accept() {
        let mut h = Harness::with("", true);
        assert!(h.frame(Vec::new()).popup_open, "open before typing");
        h.type_text("go");
        assert!(h.key(Key::Tab, Modifiers::NONE).accepted);
        assert_eq!(h.text, "godmode");
        assert!(!h.frame(Vec::new()).popup_open);
    }

    #[test]
    fn clicking_a_choice_accepts_it() {
        let mut h = Harness::with("", true);
        h.type_text("hea");
        h.frame(Vec::new());
        let state: State = h
            .ctx
            .data(|d| d.get_temp(h.edit_id().with("autocomplete_state")))
            .unwrap();
        let popup = state.popup.expect("the popup is shown");
        // The first row: just inside the popup's top.
        let at = popup.min + egui::vec2(12.0, 12.0);
        let button = |pressed| Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        h.frame(vec![Event::PointerMoved(at)]);
        h.frame(vec![button(true)]);
        let out = h.frame(vec![button(false)]);
        assert!(out.accepted, "the click lands");
        assert_eq!(h.text, "heal");
    }

    #[test]
    fn an_exact_lone_match_shows_nothing() {
        let mut h = Harness::new("");
        assert!(!h.type_text("heal").popup_open);
    }

    #[test]
    fn highlight_runs_follow_the_matches() {
        let item = Suggestion {
            text: "addItem".into(),
            matched: vec![0, 1, 3],
            ..Default::default()
        };
        let palette = egui_mev_theme::presets::GRAPHITE_DARK;
        let job = suggestion_job(&item, true, FontId::monospace(12.0), &palette);
        let runs: Vec<&str> = job
            .sections
            .iter()
            .map(|s| &job.text[s.byte_range.start.0..s.byte_range.end.0])
            .collect();
        assert_eq!(runs, vec!["ad", "d", "I", "tem"]);
        assert_eq!(job.sections[0].format.color, palette.accent);
        assert_eq!(job.sections[1].format.color, palette.text);
    }

    #[test]
    fn list_completer_ranks_fuzzily() {
        let c = complete_from_list(WORDS, "agd");
        assert_eq!(c.items[0].text, "addGold");
        assert_eq!(c.range, 0..3);
        assert_eq!(complete_from_list(WORDS, "").items.len(), 4);
    }
}
