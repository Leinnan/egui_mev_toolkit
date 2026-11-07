//! Walking through a task the way a user would, counting what it took.

use egui::{Event, Key, Modifiers};
use egui_kittest::{Harness, kittest::Queryable as _};

type AfterFrames<'a, S> = Box<dyn FnMut(&mut Harness<'a, S>) -> bool + 'a>;

/// One task in progress: the UI under test and what it took so far. Tests can
/// then assert how many clicks a task needs, not only that it can be done.
pub struct Walk<'a, S = ()> {
    /// The UI under test.
    pub harness: Harness<'a, S>,
    /// Clicks and key presses.
    pub steps: u32,
    /// Things the user had to type.
    pub typed: Vec<String>,
    after_frames: Option<AfterFrames<'a, S>>,
}

impl<'a, S> Walk<'a, S> {
    /// Start a walk over `harness`, and let it settle.
    #[must_use]
    pub fn new(harness: Harness<'a, S>) -> Self {
        let mut walk = Self {
            harness,
            steps: 0,
            typed: Vec::new(),
            after_frames: None,
        };
        walk.settle();
        walk
    }

    /// Call `apply` after frames run, the way an app applies what its UI queued.
    /// It returns whether it changed anything, which runs the frames again.
    #[must_use]
    pub fn after_frames(mut self, apply: impl FnMut(&mut Harness<'a, S>) -> bool + 'a) -> Self {
        self.after_frames = Some(Box::new(apply));
        self.settle();
        self
    }

    /// Run frames until the UI is idle and nothing is left to apply. A UI
    /// that keeps repainting, such as one showing a spinner, never goes
    /// idle; it gets the harness's `max_steps` frames instead of a panic.
    pub fn settle(&mut self) {
        self.harness.run_ok();
        if let Some(apply) = &mut self.after_frames {
            while apply(&mut self.harness) {
                self.harness.run_ok();
            }
        }
    }

    /// Click what is labelled `label`, as one step. The last match is the
    /// frontmost, because an open list sits above the form under it.
    ///
    /// # Panics
    /// When nothing is labelled `label`.
    pub fn click(&mut self, label: &str) {
        let nodes: Vec<_> = self.harness.query_all_by_label(label).collect();
        nodes
            .last()
            .unwrap_or_else(|| panic!("nothing is labelled {label:?}"))
            .click();
        self.steps += 1;
        self.settle();
    }

    /// Click what has `part` in its label, as one step: for labels that carry
    /// a count or a key hint, such as "Save (3)". The last match is the
    /// frontmost.
    ///
    /// # Panics
    /// When no label contains `part`.
    pub fn click_containing(&mut self, part: &str) {
        let nodes: Vec<_> = self.harness.query_all_by_label_contains(part).collect();
        nodes
            .last()
            .unwrap_or_else(|| panic!("no label contains {part:?}"))
            .click();
        self.steps += 1;
        self.settle();
    }

    /// Type into whatever has keyboard focus.
    pub fn type_text(&mut self, text: &str) {
        self.harness
            .input_mut()
            .events
            .push(Event::Text(text.to_owned()));
        self.typed.push(text.to_owned());
        self.settle();
    }

    /// Press `key`, as one step.
    pub fn press(&mut self, key: Key) {
        self.press_with(Modifiers::NONE, key);
    }

    /// Press `key` with `modifiers` held, such as Cmd+S, as one step.
    pub fn press_with(&mut self, modifiers: Modifiers, key: Key) {
        self.harness.key_press_modifiers(modifiers, key);
        self.steps += 1;
        self.settle();
    }
}

#[cfg(test)]
mod tests {
    use egui_mev_theme::Theme;

    use super::*;

    fn counter<'a>() -> Harness<'a, (u32, String)> {
        Harness::builder()
            .with_size([300.0, 200.0])
            .with_context_setup(|ctx| crate::install(ctx, Theme::default()))
            .build_ui_state(
                |ui, (clicks, text): &mut (u32, String)| {
                    if ui.button("Add").clicked() {
                        *clicks += 1;
                    }
                    ui.add(egui::TextEdit::singleline(text))
                        .accessible_name("Name");
                    if ui.input(|i| i.key_pressed(Key::Enter)) {
                        *clicks += 10;
                    }
                },
                (0, String::new()),
            )
    }

    #[test]
    fn counts_clicks_and_presses_but_not_typing() {
        let mut walk = Walk::new(counter());
        walk.click("Add");
        walk.press(Key::Enter);
        assert_eq!(walk.harness.state().0, 11);
        assert_eq!(walk.steps, 2);
        assert!(walk.typed.is_empty());
    }

    #[test]
    fn typing_goes_to_the_focused_field_and_is_recorded() {
        let mut walk = Walk::new(counter());
        walk.harness.get_by_role(egui::Role::TextInput).focus();
        walk.settle();
        walk.type_text("hello");
        assert_eq!(walk.harness.state().1, "hello");
        assert_eq!(walk.typed, ["hello"]);
        assert_eq!(walk.steps, 0);
    }

    #[test]
    fn applies_queued_work_until_there_is_none() {
        let mut left = 3;
        let mut walk = Walk::new(counter()).after_frames(move |harness| {
            if left == 0 {
                return false;
            }
            left -= 1;
            harness.state_mut().0 += 1;
            true
        });
        walk.settle();
        assert_eq!(walk.harness.state().0, 3);
    }

    #[test]
    #[should_panic(expected = "nothing is labelled")]
    fn clicking_what_is_not_there_says_so() {
        Walk::new(counter()).click("Missing");
    }

    #[test]
    fn a_ui_that_keeps_repainting_still_settles() {
        let harness = Harness::builder()
            .with_size([300.0, 200.0])
            .with_context_setup(|ctx| crate::install(ctx, Theme::default()))
            .build_ui_state(
                |ui, saved: &mut bool| {
                    ui.spinner();
                    if ui.button("Save (1)").clicked() {
                        *saved = true;
                    }
                },
                false,
            );
        let mut walk = Walk::new(harness);
        walk.click_containing("Save");
        assert!(*walk.harness.state());
        walk.press_with(Modifiers::COMMAND, Key::S);
        assert_eq!(walk.steps, 2);
    }
}
