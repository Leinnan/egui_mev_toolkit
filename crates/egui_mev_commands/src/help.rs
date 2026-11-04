//! A list of what the keys do, for a shortcut overlay or a settings page.

use std::borrow::Cow;

use egui::{Context, Frame, Id, Margin, Modal, RichText, ScrollArea, Ui};
use egui_mev_theme::{Metrics, Palette};
use egui_mev_widgets::{key_caps, secondary_button, settings_group};

use crate::keymap::Keymap;

/// Narrowest and widest the overlay gets (px).
const WIDTH: (f32, f32) = (300.0, 560.0);
/// Room for everything around the list (px).
const CHROME_HEIGHT: f32 = 160.0;

/// A titled group of rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HelpSection {
    /// The group's title.
    pub title: Cow<'static, str>,
    /// Its rows.
    pub rows: Vec<HelpRow>,
}

/// What a set of keys does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HelpRow {
    /// What happens.
    pub action: Cow<'static, str>,
    /// Every combination that does it, the preferred one first.
    pub keys: Vec<String>,
}

impl HelpSection {
    /// A group of every command in `keymap`, called what `label` says. Commands of
    /// the same name share one row, and `label` can skip one with `None`. Call it
    /// inside a frame: formatting a shortcut needs the fonts.
    #[must_use]
    pub fn from_keymap<C: Clone + PartialEq>(
        ctx: &Context,
        title: impl Into<Cow<'static, str>>,
        keymap: &Keymap<C>,
        label: impl Fn(&C) -> Option<Cow<'static, str>>,
    ) -> Self {
        let mut rows: Vec<HelpRow> = Vec::new();
        for binding in keymap.bindings() {
            let Some(action) = label(&binding.command) else {
                continue;
            };
            let keys = ctx.format_shortcut(&binding.shortcut);
            match rows.iter_mut().find(|row| row.action == action) {
                Some(row) => row.keys.push(keys),
                None => rows.push(HelpRow {
                    action,
                    keys: vec![keys],
                }),
            }
        }
        Self {
            title: title.into(),
            rows,
        }
    }

    /// A group of keys no keymap handles, as `(keys, action)` with the keys written
    /// out and separated by ", ": `("Ctrl+Enter, middle-click", "Open in a new tab")`.
    #[must_use]
    pub fn from_keys(
        title: impl Into<Cow<'static, str>>,
        rows: &[(&'static str, &'static str)],
    ) -> Self {
        Self {
            title: title.into(),
            rows: rows
                .iter()
                .map(|(keys, action)| HelpRow {
                    action: Cow::Borrowed(action),
                    keys: keys.split(", ").map(str::to_owned).collect(),
                })
                .collect(),
        }
    }
}

/// Every row as a settings row with key caps, so a settings page and the overlay
/// show the same list.
pub fn shortcut_rows(ui: &mut Ui, sections: &[HelpSection]) {
    for section in sections {
        settings_group(ui, &section.title, |group| {
            for row in &section.rows {
                // Drawn right to left, so the preferred combination ends up first.
                let keys: Vec<&str> = row.keys.iter().rev().map(String::as_str).collect();
                group.row(&row.action, None, |ui| key_caps(ui, &keys));
            }
        });
    }
}

/// Draws the keyboard shortcut overlay. Returns `true` when it should close.
pub fn help_ui(ctx: &Context, title: &str, close_label: &str, sections: &[HelpSection]) -> bool {
    let p = Palette::of_ctx(ctx);
    let m = Metrics::of_ctx(ctx);
    let frame = Frame::window(&ctx.global_style())
        .fill(p.window_bg)
        .inner_margin(Margin::same(m.card_padding + 6));
    let mut close = false;
    let modal = Modal::new(Id::unique("egui_mev_commands::help_ui"))
        .frame(frame)
        .show(ctx, |ui| {
            ui.set_width((ctx.content_rect().width() - 64.0).clamp(WIDTH.0, WIDTH.1));
            ui.label(RichText::new(title).size(m.subtitle).strong().color(p.text));
            ScrollArea::vertical()
                .max_height((ctx.content_rect().height() - CHROME_HEIGHT).max(m.row_height))
                .show(ui, |ui| shortcut_rows(ui, sections));
            ui.separator();
            if secondary_button(ui, close_label).clicked() {
                close = true;
            }
        });
    close || modal.should_close()
}

#[cfg(test)]
mod tests {
    use egui::{Key, Modifiers};
    use egui_kittest::{Harness, kittest::Queryable as _};

    use super::*;
    use crate::keymap::Binding;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Command {
        Refresh,
        Search,
        Hidden,
    }

    fn keymap() -> Keymap<Command> {
        Keymap::new([
            Binding::new(Modifiers::NONE, Key::F5, Command::Refresh),
            Binding::new(Modifiers::COMMAND, Key::R, Command::Refresh),
            Binding::new(Modifiers::COMMAND, Key::F, Command::Search),
            Binding::new(Modifiers::COMMAND, Key::H, Command::Hidden),
        ])
    }

    fn label(command: &Command) -> Option<Cow<'static, str>> {
        match command {
            Command::Refresh => Some("Refresh".into()),
            Command::Search => Some("Search".into()),
            Command::Hidden => None,
        }
    }

    #[test]
    fn a_command_with_two_shortcuts_is_one_row() {
        let ctx = Context::default();
        // Formatting a shortcut needs the fonts of a first frame.
        let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
        output.textures_delta.clear();
        let section = HelpSection::from_keymap(&ctx, "App", &keymap(), label);
        let f5 = ctx.format_shortcut(&egui::KeyboardShortcut::new(Modifiers::NONE, Key::F5));
        let r = ctx.format_shortcut(&egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::R));
        assert_eq!(section.rows.len(), 2);
        assert_eq!(section.rows[0].keys, vec![f5, r]);
        assert_eq!(section.rows[1].action, "Search");
    }

    #[test]
    fn written_out_keys_split_at_commas() {
        let section = HelpSection::from_keys("Files", &[("Enter, Ctrl+O", "Open")]);
        assert_eq!(section.rows[0].keys, vec!["Enter", "Ctrl+O"]);
    }

    #[test]
    fn close_button_and_escape_ask_to_close() {
        let mut harness = Harness::builder().with_size([700.0, 500.0]).build_ui_state(
            |ui, closed: &mut bool| {
                let sections = [HelpSection::from_keymap(ui.ctx(), "App", &keymap(), label)];
                *closed |= help_ui(ui.ctx(), "Keyboard shortcuts", "Close", &sections);
            },
            false,
        );
        harness.run();
        assert!(!harness.state());
        assert!(harness.query_by_label("F5").is_some(), "a key cap");
        assert!(harness.query_by_label("Ctrl+R").is_some(), "the second one");
        harness.get_by_label("Close").click();
        harness.run();
        assert!(*harness.state());

        *harness.state_mut() = false;
        harness.run();
        harness.key_press(egui::Key::Escape);
        harness.run();
        assert!(*harness.state());
    }
}
