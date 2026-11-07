//! Native dialogs behind a trait, so tests can script them, and
//! backend-independent three-button results.
//!
//! RFD's legacy Windows backend returns Yes/No even for custom labels, and it
//! only shows custom labels with rfd's `common-controls-v6` feature, which also
//! needs the app to embed a manifest. The app enables that itself; either way
//! [`Choice`] comes out the same.
//!
//! ```ignore
//! fn confirm_close(dialogs: &dyn Dialogs) -> Choice {
//!     dialogs.ask(MessageLevel::Warning, "Unsaved", "Save first?", ["Save", "Discard", "Cancel"])
//! }
//! ```

use std::{cell::RefCell, collections::VecDeque, path::PathBuf};

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
pub use rfd::{FileDialog, MessageDialogResult, MessageLevel};
use rfd::{MessageButtons, MessageDialog};

/// Which of a prompt's three buttons was chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    /// The first label.
    First,
    /// The second label.
    Second,
    /// The third label, or the dialog was dismissed.
    Cancel,
}

fn normalize(answer: &MessageDialogResult, labels: [&str; 3]) -> Choice {
    match answer {
        MessageDialogResult::Yes => Choice::First,
        MessageDialogResult::No => Choice::Second,
        MessageDialogResult::Custom(label) if label == labels[0] => Choice::First,
        MessageDialogResult::Custom(label) if label == labels[1] => Choice::Second,
        _ => Choice::Cancel,
    }
}

/// The dialogs an app shows. [`NativeDialogs`] shows real ones;
/// [`ScriptedDialogs`] answers from a script.
pub trait Dialogs {
    /// A file dialog to configure and pass to [`Dialogs::save_file`].
    fn file(&self) -> FileDialog {
        FileDialog::new()
    }

    /// A prompt with three buttons. `labels[2]` is the cancel button.
    fn ask(&self, level: MessageLevel, title: &str, description: &str, labels: [&str; 3])
    -> Choice;

    /// A save picker. `None` if the user cancelled.
    fn save_file(&self, dialog: FileDialog) -> Option<PathBuf>;

    /// An open picker for one file. `None` if the user cancelled.
    fn pick_file(&self, dialog: FileDialog) -> Option<PathBuf> {
        dialog.pick_file()
    }

    /// A picker for one folder. `None` if the user cancelled.
    fn pick_folder(&self, dialog: FileDialog) -> Option<PathBuf> {
        dialog.pick_folder()
    }

    /// An error message with an OK button.
    fn error(&self, title: &str, detail: &str);
}

/// Real dialogs, on top of the window they were attached to.
#[derive(Clone, Debug, Default)]
pub struct NativeDialogs {
    file: FileDialog,
    message: MessageDialog,
}

impl NativeDialogs {
    /// Dialogs with no parent window.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Dialogs parented to `parent`, which can be eframe's `CreationContext`.
    /// RFD copies the window handles, so the window has to outlive these, and
    /// they are shown synchronously on its UI thread.
    #[must_use]
    pub fn attach<W: HasWindowHandle + HasDisplayHandle + ?Sized>(parent: &W) -> Self {
        Self {
            file: FileDialog::new().set_parent(parent),
            message: MessageDialog::new().set_parent(parent),
        }
    }

    /// A message dialog to configure and show yourself.
    #[must_use]
    pub fn message(&self) -> MessageDialog {
        self.message.clone()
    }
}

impl Dialogs for NativeDialogs {
    fn file(&self) -> FileDialog {
        self.file.clone()
    }

    fn ask(
        &self,
        level: MessageLevel,
        title: &str,
        description: &str,
        labels: [&str; 3],
    ) -> Choice {
        let answer = self
            .message()
            .set_level(level)
            .set_title(title)
            .set_description(description)
            .set_buttons(MessageButtons::YesNoCancelCustom(
                labels[0].to_owned(),
                labels[1].to_owned(),
                labels[2].to_owned(),
            ))
            .show();
        normalize(&answer, labels)
    }

    fn save_file(&self, dialog: FileDialog) -> Option<PathBuf> {
        dialog.save_file()
    }

    fn error(&self, title: &str, detail: &str) {
        self.message()
            .set_level(MessageLevel::Error)
            .set_title(title)
            .set_description(detail)
            .set_buttons(MessageButtons::Ok)
            .show();
    }
}

/// A test double that answers from a script and records what it was asked. A
/// prompt or picker with no scripted answer panics, so a test can't pass
/// through a dialog it didn't expect. Results go through the same normalization
/// as [`NativeDialogs`].
#[derive(Debug, Default)]
pub struct ScriptedDialogs {
    answers: RefCell<VecDeque<MessageDialogResult>>,
    paths: RefCell<VecDeque<Option<PathBuf>>>,
    prompts: RefCell<Vec<String>>,
    errors: RefCell<Vec<(String, String)>>,
}

impl ScriptedDialogs {
    /// A double with nothing scripted.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Queue answers for the next prompts.
    pub fn respond(&self, answers: impl IntoIterator<Item = MessageDialogResult>) {
        self.answers.borrow_mut().extend(answers);
    }

    /// Queue the result of the next save picker.
    pub fn save_to(&self, path: Option<PathBuf>) {
        self.paths.borrow_mut().push_back(path);
    }

    /// Queue the result of the next file or folder picker. Pickers of every
    /// kind answer from one queue, in the order they're shown.
    pub fn pick(&self, path: Option<PathBuf>) {
        self.paths.borrow_mut().push_back(path);
    }

    /// The titles of the prompts asked so far.
    #[must_use]
    pub fn prompts(&self) -> Vec<String> {
        self.prompts.borrow().clone()
    }

    /// The `(title, detail)` of the errors shown so far.
    #[must_use]
    pub fn errors(&self) -> Vec<(String, String)> {
        self.errors.borrow().clone()
    }
}

impl Dialogs for ScriptedDialogs {
    fn ask(
        &self,
        _level: MessageLevel,
        title: &str,
        _description: &str,
        labels: [&str; 3],
    ) -> Choice {
        self.prompts.borrow_mut().push(title.to_owned());
        let answer = self
            .answers
            .borrow_mut()
            .pop_front()
            .unwrap_or_else(|| panic!("unexpected native prompt: {title}"));
        normalize(&answer, labels)
    }

    fn save_file(&self, _dialog: FileDialog) -> Option<PathBuf> {
        self.paths
            .borrow_mut()
            .pop_front()
            .expect("unexpected save picker")
    }

    fn pick_file(&self, _dialog: FileDialog) -> Option<PathBuf> {
        self.paths
            .borrow_mut()
            .pop_front()
            .expect("unexpected file picker")
    }

    fn pick_folder(&self, _dialog: FileDialog) -> Option<PathBuf> {
        self.paths
            .borrow_mut()
            .pop_front()
            .expect("unexpected folder picker")
    }

    fn error(&self, title: &str, detail: &str) {
        self.errors
            .borrow_mut()
            .push((title.to_owned(), detail.to_owned()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_prompt_labels_accept_standard_and_custom_results() {
        for labels in [
            ["Save", "Don't Save", "Cancel"],
            ["Review Changes…", "Discard Changes", "Cancel"],
            ["Expand Canvas", "Keep Canvas Size", "Cancel"],
        ] {
            for (answer, expected) in [
                (MessageDialogResult::Yes, Choice::First),
                (MessageDialogResult::No, Choice::Second),
                (MessageDialogResult::Cancel, Choice::Cancel),
                (MessageDialogResult::Ok, Choice::Cancel),
                (MessageDialogResult::Custom(labels[0].into()), Choice::First),
                (
                    MessageDialogResult::Custom(labels[1].into()),
                    Choice::Second,
                ),
                (
                    MessageDialogResult::Custom(labels[2].into()),
                    Choice::Cancel,
                ),
                (
                    MessageDialogResult::Custom("unexpected".into()),
                    Choice::Cancel,
                ),
            ] {
                assert_eq!(normalize(&answer, labels), expected);
            }
        }
    }

    /// Code under test only sees the trait.
    fn export(dialogs: &dyn Dialogs) -> Option<PathBuf> {
        match dialogs.ask(
            MessageLevel::Warning,
            "Replace?",
            "It exists.",
            ["Replace", "Rename", "Cancel"],
        ) {
            Choice::First => {
                let path = dialogs.save_file(dialogs.file().set_file_name("out.png"));
                if path.is_none() {
                    dialogs.error("Export failed", "no file chosen");
                }
                path
            }
            Choice::Second | Choice::Cancel => None,
        }
    }

    #[test]
    fn the_double_scripts_answers_and_records_calls() {
        let dialogs = ScriptedDialogs::new();
        dialogs.respond([MessageDialogResult::Custom("Replace".into())]);
        dialogs.save_to(Some(PathBuf::from("/tmp/out.png")));
        assert_eq!(export(&dialogs), Some(PathBuf::from("/tmp/out.png")));
        assert_eq!(dialogs.prompts(), ["Replace?"]);
        assert!(dialogs.errors().is_empty());
    }

    #[test]
    fn the_double_answers_pickers_in_order() {
        let dialogs = ScriptedDialogs::new();
        dialogs.pick(Some(PathBuf::from("/music/cover.jpg")));
        dialogs.pick(None);
        assert_eq!(
            dialogs.pick_file(dialogs.file()),
            Some(PathBuf::from("/music/cover.jpg"))
        );
        assert_eq!(dialogs.pick_folder(dialogs.file()), None);
    }

    #[test]
    #[should_panic(expected = "unexpected folder picker")]
    fn an_unscripted_picker_fails_the_test() {
        ScriptedDialogs::new().pick_folder(FileDialog::new());
    }

    #[test]
    fn the_double_records_errors_and_answers_in_order() {
        let dialogs = ScriptedDialogs::new();
        dialogs.respond([MessageDialogResult::Yes, MessageDialogResult::No]);
        dialogs.save_to(None);
        assert_eq!(export(&dialogs), None);
        assert_eq!(
            dialogs.errors(),
            [("Export failed".to_owned(), "no file chosen".to_owned())]
        );
        assert_eq!(export(&dialogs), None, "the second answer declines");
        assert_eq!(dialogs.prompts().len(), 2);
    }

    #[test]
    #[should_panic(expected = "unexpected native prompt")]
    fn an_unscripted_prompt_panics() {
        let dialogs = ScriptedDialogs::new();
        let _ = export(&dialogs);
    }

    #[test]
    #[should_panic(expected = "unexpected save picker")]
    fn an_unscripted_save_picker_panics() {
        let dialogs = ScriptedDialogs::new();
        dialogs.respond([MessageDialogResult::Yes]);
        let _ = export(&dialogs);
    }

    /// Compile-time proof that the eframe context can parent dialogs.
    #[cfg(feature = "eframe")]
    #[expect(dead_code, reason = "only has to compile")]
    fn attach_takes_the_creation_context(cc: &eframe::CreationContext<'_>) -> NativeDialogs {
        NativeDialogs::attach(cc)
    }
}
