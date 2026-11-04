//! Serialization and deserialization utilities for configuration files.

use std::{fs, path::Path};

use bevy_reflect::{TypeRegistry, serde::TypedReflectSerializer};
use egui_mev_core::atomic::write_atomic;

pub trait ConfigsSerializerExt {
    /// Reads the config at `path`, or `None` when it is missing or can't be parsed.
    fn try_load_config<T: bevy_reflect::Reflect + serde::de::DeserializeOwned + Default>(
        &self,
        path: &Path,
    ) -> Option<T>;

    /// Replaces the config at `path` atomically, creating missing parent directories.
    fn try_save_config<T: bevy_reflect::Reflect + serde::Serialize>(
        &self,
        config: &T,
        path: &Path,
    ) -> Result<(), std::io::Error>;
}

/// Parses config file contents in the format this build uses.
pub(crate) fn parse_config<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, String> {
    let file = std::str::from_utf8(bytes).map_err(|error| error.to_string())?;
    #[cfg(feature = "configs_json")]
    let config = serde_json::from_str(file).map_err(|error| error.to_string());
    #[cfg(not(feature = "configs_json"))]
    let config = toml::from_str(file).map_err(|error| error.to_string());
    config
}

impl ConfigsSerializerExt for TypeRegistry {
    fn try_load_config<T: bevy_reflect::Reflect + serde::de::DeserializeOwned + Default>(
        &self,
        path: &Path,
    ) -> Option<T> {
        parse_config(&fs::read(path).ok()?).ok()
    }

    fn try_save_config<T: bevy_reflect::Reflect + serde::Serialize>(
        &self,
        config: &T,
        path: &Path,
    ) -> Result<(), std::io::Error> {
        let serializer = TypedReflectSerializer::new(config, self);
        #[cfg(feature = "configs_json")]
        let value = serde_json::to_string_pretty(&serializer).map_err(std::io::Error::other)?;
        #[cfg(not(feature = "configs_json"))]
        let value = toml::ser::to_string_pretty(&serializer).map_err(std::io::Error::other)?;
        // App config directories may not exist yet, or may have been removed.
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        write_atomic(path, value.as_bytes())
    }
}
