//! Keyboard shortcuts as text: what a person types, what settings files save and
//! what menus show.

use std::fmt;

use egui::{Context, Key, KeyboardShortcut, ModifierNames, Modifiers};

/// A keyboard shortcut with its text forms.
///
/// - [`Self::parse`] reads what a person wrote: names in any case, and `Ctrl`
///   means the platform's command key.
/// - [`Self::encode`] and [`Self::decode`] are the saved form, where `Cmd` is the
///   command key and `Ctrl` is the Control key itself.
/// - [`Self::display`] is how menus write it on this platform.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Shortcut(pub KeyboardShortcut);

/// Why [`Shortcut::parse`] refused a text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseShortcutError {
    /// It has an empty part, as in `Ctrl+`.
    Malformed(String),
    /// It names more than one key.
    TwoKeys(String),
    /// A part is neither a modifier nor a key: the part, then the whole text.
    UnknownKey(String, String),
    /// It has modifiers only.
    NoKey(String),
    /// A plain key that isn't a function key, which would take the key away from
    /// anyone typing.
    NeedsModifier(String),
}

impl fmt::Display for ParseShortcutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(text) => write!(f, "\"{text}\" is not a shortcut"),
            Self::TwoKeys(text) => write!(f, "\"{text}\" names more than one key"),
            Self::UnknownKey(part, text) => write!(f, "unknown key \"{part}\" in \"{text}\""),
            Self::NoKey(text) => write!(f, "\"{text}\" has no key"),
            Self::NeedsModifier(text) => {
                write!(
                    f,
                    "\"{text}\" needs Ctrl or Alt, so typing a name still works"
                )
            }
        }
    }
}

impl std::error::Error for ParseShortcutError {}

/// `text` split at `+`, where "+" itself can be the last part: "Cmd++".
fn split_parts(text: &str) -> Vec<&str> {
    if text == "+" {
        return vec!["+"];
    }
    match text.strip_suffix("++") {
        Some(rest) => {
            let mut parts: Vec<&str> = rest.split('+').collect();
            parts.push("+");
            parts
        }
        None => text.split('+').collect(),
    }
}

/// The key called `name`, in any case.
fn key_named(name: &str) -> Option<Key> {
    Key::from_name(name).or_else(|| {
        Key::ALL
            .iter()
            .copied()
            .find(|key| key.name().eq_ignore_ascii_case(name))
    })
}

/// F1 to F35, which nobody needs for typing.
fn is_function_key(key: Key) -> bool {
    let name = key.name();
    name.strip_prefix('F')
        .is_some_and(|number| !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()))
}

impl Shortcut {
    /// A shortcut of `modifiers` and `key`.
    #[must_use]
    pub const fn new(modifiers: Modifiers, key: Key) -> Self {
        Self(KeyboardShortcut::new(modifiers, key))
    }

    /// Reads `Ctrl+Alt+M`, `Shift+F5` and the like. `Ctrl` means Cmd on macOS.
    ///
    /// # Errors
    /// When the text isn't a shortcut, or is a plain key other than a function
    /// key, which would stop that key from typing.
    pub fn parse(text: &str) -> Result<Self, ParseShortcutError> {
        let mut modifiers = Modifiers::NONE;
        let mut key = None;
        for part in split_parts(text).into_iter().map(str::trim) {
            match part.to_ascii_lowercase().as_str() {
                "ctrl" | "cmd" | "command" | "mod" => modifiers |= Modifiers::COMMAND,
                "alt" | "option" => modifiers |= Modifiers::ALT,
                "shift" => modifiers |= Modifiers::SHIFT,
                "" => return Err(ParseShortcutError::Malformed(text.to_owned())),
                _ => {
                    if key.is_some() {
                        return Err(ParseShortcutError::TwoKeys(text.to_owned()));
                    }
                    key = Some(key_named(part).ok_or_else(|| {
                        ParseShortcutError::UnknownKey(part.to_owned(), text.to_owned())
                    })?);
                }
            }
        }
        let key = key.ok_or_else(|| ParseShortcutError::NoKey(text.to_owned()))?;
        if modifiers.is_none() && !is_function_key(key) {
            return Err(ParseShortcutError::NeedsModifier(text.to_owned()));
        }
        Ok(Self::new(modifiers, key))
    }

    /// As saved: modifier names then the key, joined by `+` ("Cmd+Shift+Z").
    #[must_use]
    pub fn encode(self) -> String {
        let m = self.0.modifiers;
        let mut parts = Vec::new();
        if m.command || m.mac_cmd {
            parts.push("Cmd");
        }
        // Off macOS a raw Ctrl press reports both `ctrl` and `command`: one key.
        let ctrl_is_command = m.command && !cfg!(target_os = "macos");
        if m.ctrl && !ctrl_is_command {
            parts.push("Ctrl");
        }
        if m.alt {
            parts.push("Alt");
        }
        if m.shift {
            parts.push("Shift");
        }
        parts.push(self.0.logical_key.name());
        parts.join("+")
    }

    /// Reads a shortcut saved by [`Self::encode`].
    #[must_use]
    pub fn decode(text: &str) -> Option<Self> {
        let mut parts = split_parts(text);
        let key = Key::from_name(parts.pop()?)?;
        let mut modifiers = Modifiers::NONE;
        for part in parts {
            modifiers = modifiers.plus(match part {
                "Cmd" => Modifiers::COMMAND,
                "Ctrl" => Modifiers::CTRL,
                "Alt" => Modifiers::ALT,
                "Shift" => Modifiers::SHIFT,
                _ => return None,
            });
        }
        Some(Self::new(modifiers, key))
    }

    /// As menus show it on this platform ("⇧⌘Z", "Ctrl+Shift+Z"). Inside a frame
    /// prefer [`Self::format`], which follows the context's operating system.
    #[must_use]
    pub fn display(self) -> String {
        let mac = cfg!(target_os = "macos");
        let names = if mac {
            ModifierNames::SYMBOLS
        } else {
            ModifierNames::NAMES
        };
        self.0.format(&names, mac)
    }

    /// As `ctx` writes it.
    #[must_use]
    pub fn format(self, ctx: &Context) -> String {
        ctx.format_shortcut(&self.0)
    }
}

impl From<KeyboardShortcut> for Shortcut {
    fn from(shortcut: KeyboardShortcut) -> Self {
        Self(shortcut)
    }
}

impl From<Shortcut> for KeyboardShortcut {
    fn from(shortcut: Shortcut) -> Self {
        shortcut.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts(shortcut: Shortcut) -> (Modifiers, Key) {
        (shortcut.0.modifiers, shortcut.0.logical_key)
    }

    #[test]
    fn parses_shortcuts() {
        assert_eq!(
            parts(Shortcut::parse("Ctrl+Alt+M").expect("valid")),
            (Modifiers::COMMAND | Modifiers::ALT, Key::M)
        );
        assert_eq!(parts(Shortcut::parse("F6").expect("valid")).1, Key::F6);
        assert_eq!(parts(Shortcut::parse("F30").expect("valid")).1, Key::F30);
        assert_eq!(
            parts(Shortcut::parse("cmd + shift + z").expect("valid")),
            (Modifiers::COMMAND | Modifiers::SHIFT, Key::Z)
        );
        assert_eq!(
            parts(Shortcut::parse("Cmd++").expect("valid")),
            (Modifiers::COMMAND, Key::Plus)
        );
        assert_eq!(
            parts(Shortcut::parse("ctrl+left").expect("valid")),
            (Modifiers::COMMAND, Key::ArrowLeft)
        );
        assert_eq!(
            parts(Shortcut::parse("Alt+ArrowLeft").expect("valid")),
            (Modifiers::ALT, Key::ArrowLeft)
        );
    }

    #[test]
    fn refuses_what_is_not_a_shortcut() {
        assert!(matches!(
            Shortcut::parse("M"),
            Err(ParseShortcutError::NeedsModifier(_))
        ));
        assert!(matches!(
            Shortcut::parse("Ctrl+Nope"),
            Err(ParseShortcutError::UnknownKey(..))
        ));
        assert!(matches!(
            Shortcut::parse("Ctrl+A+B"),
            Err(ParseShortcutError::TwoKeys(_))
        ));
        assert!(matches!(
            Shortcut::parse("Ctrl+"),
            Err(ParseShortcutError::Malformed(_))
        ));
        assert!(matches!(
            Shortcut::parse("Ctrl+Shift"),
            Err(ParseShortcutError::NoKey(_))
        ));
        let message = Shortcut::parse("Ctrl+Nope").unwrap_err().to_string();
        assert!(message.contains("Nope"), "{message}");
    }

    #[test]
    fn shortcuts_round_trip_through_text() {
        for shortcut in [
            Shortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::Z),
            Shortcut::new(Modifiers::NONE, Key::Tab),
            Shortcut::new(Modifiers::ALT.plus(Modifiers::COMMAND), Key::ArrowUp),
            Shortcut::new(Modifiers::COMMAND, Key::Plus),
            Shortcut::new(Modifiers::NONE, Key::OpenBracket),
            Shortcut::new(Modifiers::CTRL, Key::Tab),
        ] {
            let text = shortcut.encode();
            let back = Shortcut::decode(&text).expect("decodes");
            assert_eq!(parts(back), parts(shortcut), "{text}");
        }
        assert!(Shortcut::decode("Hyper+Z").is_none());
        assert!(Shortcut::decode("Cmd+NotAKey").is_none());
        assert!(Shortcut::decode("").is_none());
    }

    /// What was typed reads the same once it has been saved and read back.
    #[test]
    fn parsed_text_round_trips_through_saved_text() {
        for text in ["Ctrl+Alt+M", "Shift+F5", "Cmd+Shift+Z", "Ctrl+Comma"] {
            let parsed = Shortcut::parse(text).expect("valid");
            assert_eq!(Shortcut::decode(&parsed.encode()), Some(parsed), "{text}");
            assert_eq!(Shortcut::parse(&parsed.encode()), Ok(parsed), "{text}");
        }
    }

    /// Settings saved when the plus key's name was "+".
    #[test]
    fn decodes_the_old_spelling_of_plus() {
        assert_eq!(
            Shortcut::decode("Cmd++"),
            Some(Shortcut::new(Modifiers::COMMAND, Key::Plus))
        );
        assert_eq!(
            Shortcut::decode("+"),
            Some(Shortcut::new(Modifiers::NONE, Key::Plus))
        );
    }

    #[test]
    fn saved_ctrl_is_the_control_key_but_typed_ctrl_is_the_command_key() {
        assert_eq!(
            parts(Shortcut::decode("Ctrl+S").unwrap()).0,
            Modifiers::CTRL
        );
        assert_eq!(
            parts(Shortcut::parse("Ctrl+S").unwrap()).0,
            Modifiers::COMMAND
        );
    }

    #[test]
    fn display_writes_the_platform_form() {
        let text = Shortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::Z).display();
        if cfg!(target_os = "macos") {
            assert_eq!(text, "⇧⌘Z");
        } else {
            assert_eq!(text, "Ctrl+Shift+Z");
        }
    }

    #[test]
    fn converts_to_and_from_the_egui_type() {
        let plain = KeyboardShortcut::new(Modifiers::ALT, Key::C);
        let shortcut = Shortcut::from(plain);
        assert_eq!(KeyboardShortcut::from(shortcut), plain);
    }
}
