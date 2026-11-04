//! Where the app keeps its files, named after an ID the app supplies.

use std::{
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
};

/// How an app names its folders.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppId<'a> {
    /// One name, used as the folder on every system, such as `io.example.app`.
    Path(&'a str),
    /// The three parts [`directories`] maps to each system's convention. Apps
    /// that already use them keep their folders by passing the same values.
    Parts {
        /// Reverse domain name's top level, such as `io`.
        qualifier: &'a str,
        /// The organization, such as `example`.
        organization: &'a str,
        /// The application, such as `app`.
        application: &'a str,
    },
}

impl<'a> From<&'a str> for AppId<'a> {
    fn from(id: &'a str) -> Self {
        Self::Path(id)
    }
}

impl<'a> From<(&'a str, &'a str, &'a str)> for AppId<'a> {
    fn from((qualifier, organization, application): (&'a str, &'a str, &'a str)) -> Self {
        Self::Parts {
            qualifier,
            organization,
            application,
        }
    }
}

/// The app's folders. None of them is created until [`AppPaths::ensure_exists`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppPaths {
    /// User settings.
    pub config_dir: PathBuf,
    /// App data worth keeping and, on systems that have it, roaming.
    pub data_dir: PathBuf,
    /// App data that stays on this machine.
    pub data_local_dir: PathBuf,
    /// Anything that can be rebuilt.
    pub cache_dir: PathBuf,
}

impl AppPaths {
    /// The folders for `id`, unless the env var `env_override` names a folder
    /// to keep everything under instead. `None` when the system has no home
    /// folder to resolve against, and always on the web.
    pub fn discover<'a>(id: impl Into<AppId<'a>>, env_override: Option<&str>) -> Option<Self> {
        Self::resolve(id, env_override.and_then(std::env::var_os))
    }

    /// [`Self::discover`] with the override's value already read. An empty
    /// value counts as unset.
    pub fn resolve<'a>(id: impl Into<AppId<'a>>, root: Option<OsString>) -> Option<Self> {
        if cfg!(target_arch = "wasm32") {
            return None;
        }
        match root.filter(|root| !root.is_empty()) {
            Some(root) => Some(Self::under_root(root)),
            None => system_dirs(id.into()),
        }
    }

    /// Everything under one folder, with settings directly in it. The cache
    /// has its own subfolder so that clearing it can't take the settings along.
    #[must_use]
    pub fn under_root(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        Self {
            data_dir: root.join("data"),
            data_local_dir: root.join("data"),
            cache_dir: root.join("cache"),
            config_dir: root,
        }
    }

    /// `name` in the settings folder.
    #[must_use]
    pub fn config_file(&self, name: impl AsRef<Path>) -> PathBuf {
        self.config_dir.join(name)
    }

    /// Create every folder that doesn't exist yet.
    pub fn ensure_exists(&self) -> io::Result<()> {
        for dir in [
            &self.config_dir,
            &self.data_dir,
            &self.data_local_dir,
            &self.cache_dir,
        ] {
            fs::create_dir_all(dir)?;
        }
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn system_dirs(id: AppId<'_>) -> Option<AppPaths> {
    let dirs = match id {
        AppId::Path(id) => directories::ProjectDirs::from_path(PathBuf::from(id)),
        AppId::Parts {
            qualifier,
            organization,
            application,
        } => directories::ProjectDirs::from(qualifier, organization, application),
    }?;
    Some(AppPaths {
        config_dir: dirs.config_dir().to_path_buf(),
        data_dir: dirs.data_dir().to_path_buf(),
        data_local_dir: dirs.data_local_dir().to_path_buf(),
        cache_dir: dirs.cache_dir().to_path_buf(),
    })
}

#[cfg(target_arch = "wasm32")]
fn system_dirs(_id: AppId<'_>) -> Option<AppPaths> {
    None
}

#[cfg(test)]
mod tests {
    use super::AppPaths;
    use std::{ffi::OsString, fs, path::Path};

    const ID: &str = "io.example.egui_mev_app_tests";

    #[test]
    fn override_value_replaces_the_system_folders() {
        let paths = AppPaths::resolve(ID, Some(OsString::from("/tmp/somewhere"))).expect("paths");
        assert_eq!(paths.config_dir, Path::new("/tmp/somewhere"));
        assert_eq!(paths.data_dir, Path::new("/tmp/somewhere/data"));
        assert_eq!(paths.cache_dir, Path::new("/tmp/somewhere/cache"));
        assert_eq!(
            paths.config_file("config.toml"),
            Path::new("/tmp/somewhere/config.toml")
        );
    }

    #[test]
    fn empty_override_counts_as_unset() {
        assert_eq!(
            AppPaths::resolve(ID, Some(OsString::new())),
            AppPaths::resolve(ID, None)
        );
    }

    #[test]
    fn every_folder_is_named_after_the_id() {
        let paths = AppPaths::resolve(ID, None).expect("home folder");
        for dir in [
            &paths.config_dir,
            &paths.data_dir,
            &paths.data_local_dir,
            &paths.cache_dir,
        ] {
            assert!(
                dir.components().any(|part| part.as_os_str() == ID),
                "{}",
                dir.display()
            );
        }
    }

    #[test]
    fn three_part_ids_use_the_application_name() {
        let paths = AppPaths::resolve(("io", "example", "mevtests"), None).expect("home folder");
        assert!(
            paths.config_dir.to_string_lossy().contains("mevtests"),
            "{}",
            paths.config_dir.display()
        );
    }

    /// Cargo sets this variable for every test process, so nothing has to mutate
    /// the environment, which other test threads may be reading.
    #[test]
    fn discover_reads_the_named_variable() {
        let value = std::env::var_os("CARGO_MANIFEST_DIR").expect("set by cargo");
        let paths = AppPaths::discover(ID, Some("CARGO_MANIFEST_DIR")).expect("paths");
        assert_eq!(paths.config_dir, Path::new(&value));
    }

    #[test]
    fn discover_without_the_variable_uses_the_system_folders() {
        assert_eq!(
            AppPaths::discover(ID, Some("EGUI_MEV_APP_TEST_PATHS_NEVER_SET")),
            AppPaths::resolve(ID, None)
        );
        assert_eq!(AppPaths::discover(ID, None), AppPaths::resolve(ID, None));
    }

    #[test]
    fn ensure_exists_creates_every_folder() {
        let root = std::env::temp_dir().join(format!("egui-mev-app-paths-{}", std::process::id()));
        let paths = AppPaths::under_root(&root);
        paths.ensure_exists().expect("should create");
        assert!(paths.config_dir.is_dir());
        assert!(paths.data_dir.is_dir());
        assert!(paths.cache_dir.is_dir());
        fs::remove_dir_all(root).expect("should clean up");
    }
}
