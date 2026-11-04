//! A value kept in a file: loaded with recovery from a corrupt file, saved
//! atomically. On the web there is no file system, so loading and saving return
//! errors there; [`crate::AppPaths`] has no paths to offer a web build anyway.

use std::{
    fs, io,
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
};

use egui_mev_core::atomic::{load_or_recover, write_atomic};
use serde::{Serialize, de::DeserializeOwned};

/// The format of a settings file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// TOML, for files people edit.
    #[cfg(feature = "toml")]
    Toml,
    /// Pretty-printed JSON.
    #[cfg(feature = "json")]
    Json,
}

impl Format {
    fn parse<T: DeserializeOwned>(self, bytes: &[u8]) -> Result<T, String> {
        match self {
            #[cfg(feature = "toml")]
            Self::Toml => std::str::from_utf8(bytes)
                .map_err(|error| error.to_string())
                .and_then(|text| toml::from_str(text).map_err(|error| error.to_string())),
            #[cfg(feature = "json")]
            Self::Json => serde_json::from_slice(bytes).map_err(|error| error.to_string()),
        }
    }

    fn serialize<T: Serialize>(self, value: &T) -> Result<Vec<u8>, String> {
        match self {
            #[cfg(feature = "toml")]
            Self::Toml => toml::to_string_pretty(value)
                .map(String::into_bytes)
                .map_err(|error| error.to_string()),
            #[cfg(feature = "json")]
            Self::Json => serde_json::to_vec_pretty(value).map_err(|error| error.to_string()),
        }
    }
}

/// A `T` and the file it lives in. It derefs to `T`, and writes only on
/// [`Settings::save`].
#[derive(Clone, Debug)]
pub struct Settings<T> {
    value: T,
    path: PathBuf,
    format: Format,
    set_aside: Option<PathBuf>,
}

impl<T: Serialize + DeserializeOwned + Default> Settings<T> {
    /// Startup load: a missing file gives the default, and an unparsable one is
    /// moved aside and also gives the default, so it can't stop the app from
    /// launching. Only a file that can't be read or moved is an error.
    pub fn load(path: impl Into<PathBuf>, format: Format) -> io::Result<Self> {
        let path = path.into();
        let recovered = load_or_recover(&path, |bytes| format.parse::<T>(bytes))?;
        Ok(Self {
            value: recovered.value,
            path,
            format,
            set_aside: recovered.set_aside,
        })
    }

    /// Replace the file with the current value, creating its folder if needed.
    /// A crash or a full disk leaves the previous file intact.
    pub fn save(&self) -> io::Result<()> {
        let bytes = self
            .format
            .serialize(&self.value)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        write_atomic(&self.path, &bytes)
    }
}

impl<T> Settings<T> {
    /// The file this value is saved to.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Where an unparsable file was moved during [`Settings::load`], so the app
    /// can tell the user.
    #[must_use]
    pub fn set_aside(&self) -> Option<&Path> {
        self.set_aside.as_deref()
    }

    /// The value, without its file.
    #[must_use]
    pub fn into_inner(self) -> T {
        self.value
    }
}

impl<T> Deref for Settings<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.value
    }
}

impl<T> DerefMut for Settings<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.value
    }
}

#[cfg(test)]
mod tests {
    use super::{Format, Settings};
    use serde::{Deserialize, Serialize};
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU32, Ordering},
    };

    #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
    struct Prefs {
        name: String,
        width: f32,
        recent: Vec<String>,
    }

    const FORMATS: &[Format] = &[
        #[cfg(feature = "toml")]
        Format::Toml,
        #[cfg(feature = "json")]
        Format::Json,
    ];

    fn test_dir() -> PathBuf {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "egui-mev-app-settings-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).expect("should create directory");
        dir
    }

    fn file_names(dir: &Path) -> Vec<String> {
        let mut names = fs::read_dir(dir)
            .expect("should read directory")
            .map(|entry| {
                entry
                    .expect("should read entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect::<Vec<_>>();
        names.sort();
        names
    }

    #[test]
    fn round_trips_in_every_format() {
        for &format in FORMATS {
            let path = test_dir().join("settings");
            let mut settings = Settings::<Prefs>::load(&path, format).expect("should load");
            settings.name = "ada".to_owned();
            settings.width = 12.5;
            settings.recent.push("a.png".to_owned());
            settings.save().expect("should save");

            let again = Settings::<Prefs>::load(&path, format).expect("should load");
            assert_eq!(*again, *settings, "{format:?}");
            assert_eq!(again.set_aside(), None);
        }
    }

    #[test]
    fn missing_file_gives_the_default_without_creating_it() {
        for &format in FORMATS {
            let path = test_dir().join("settings");
            let settings = Settings::<Prefs>::load(&path, format).expect("should load");
            assert_eq!(*settings, Prefs::default());
            assert_eq!(settings.set_aside(), None);
            assert_eq!(settings.path(), path);
            assert!(!path.exists());
        }
    }

    #[test]
    fn corrupt_file_is_set_aside_and_gives_the_default() {
        for &format in FORMATS {
            let dir = test_dir();
            let path = dir.join("settings");
            fs::write(&path, "}} not valid \u{0} in any format").expect("should seed");

            let settings = Settings::<Prefs>::load(&path, format).expect("should recover");

            assert_eq!(*settings, Prefs::default(), "{format:?}");
            let moved = settings.set_aside().expect("should say where it went");
            assert_eq!(
                fs::read_to_string(moved).expect("should keep the contents"),
                "}} not valid \u{0} in any format"
            );
            assert!(!path.exists());

            settings.save().expect("should save over the gap");
            assert_eq!(file_names(&dir), ["settings", "settings.corrupt"]);
        }
    }

    #[test]
    fn non_utf8_file_is_set_aside() {
        for &format in FORMATS {
            let path = test_dir().join("settings");
            fs::write(&path, [0xff, 0xfe, 0x00]).expect("should seed");
            let settings = Settings::<Prefs>::load(&path, format).expect("should recover");
            assert!(settings.set_aside().is_some(), "{format:?}");
        }
    }

    #[test]
    fn save_leaves_no_temp_file_and_creates_the_folder() {
        for &format in FORMATS {
            let dir = test_dir();
            let nested = dir.join("not").join("yet");
            let settings =
                Settings::<Prefs>::load(nested.join("settings"), format).expect("should load");
            settings.save().expect("should create");
            settings.save().expect("should replace");
            assert_eq!(file_names(&nested), ["settings"]);
        }
    }

    #[cfg(feature = "toml")]
    #[test]
    fn a_value_toml_cannot_hold_is_an_error_and_writes_nothing() {
        // TOML has no way to write a bare array as a document.
        let dir = test_dir();
        let path = dir.join("settings");
        let settings = Settings::<Vec<u32>>::load(&path, Format::Toml).expect("should load");
        settings.save().expect_err("should refuse");
        assert!(file_names(&dir).is_empty());
    }

    #[cfg(feature = "toml")]
    #[test]
    fn toml_files_are_toml() {
        let path = test_dir().join("settings.toml");
        let mut settings = Settings::<Prefs>::load(&path, Format::Toml).expect("should load");
        settings.name = "ada".to_owned();
        settings.save().expect("should save");
        let text = fs::read_to_string(&path).expect("should read");
        assert!(text.contains("name = \"ada\""), "{text}");
    }

    #[test]
    fn into_inner_returns_the_value() {
        let format = FORMATS[0];
        let path = test_dir().join("settings");
        let mut settings = Settings::<Prefs>::load(&path, format).expect("should load");
        settings.width = 3.0;
        assert_eq!(settings.into_inner().width, 3.0);
    }
}
