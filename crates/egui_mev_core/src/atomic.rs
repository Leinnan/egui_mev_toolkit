//! Crash-safe replacement of files, and setting aside unreadable ones.

use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
};

/// Replace `path` with `contents` via a sibling temporary file and a rename, so
/// a crash or a full disk never leaves a truncated file behind.
///
/// The temporary file takes the existing file's permissions before any content
/// is written (config files can hold API keys), and a symlinked `path` keeps its
/// link: the file it points at is the one replaced.
pub fn write_atomic(path: &Path, contents: &[u8]) -> io::Result<()> {
    let target = fs::canonicalize(path).unwrap_or_else(|_| path.to_owned());
    let tmp = temp_path(&target);
    let result = write_and_rename(&target, &tmp, contents);
    if result.is_err() {
        // Best effort: the temporary file is ours and incomplete.
        let _ = fs::remove_file(&tmp);
    }
    result
}

fn write_and_rename(target: &Path, tmp: &Path, contents: &[u8]) -> io::Result<()> {
    let mut file = File::create(tmp)?;
    if let Ok(metadata) = fs::metadata(target) {
        file.set_permissions(metadata.permissions())?;
    }
    file.write_all(contents)?;
    file.sync_all()?;
    drop(file);
    fs::rename(tmp, target)?;
    // Best effort: makes the rename itself durable; directories can't be
    // opened this way on Windows.
    if let Some(parent) = target.parent()
        && let Ok(directory) = File::open(parent)
    {
        let _ = directory.sync_all();
    }
    Ok(())
}

/// A hidden sibling of `path`, ending in `.tmp` so directory listings can skip it.
fn temp_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map_or_else(|| "config".into(), |name| name.to_string_lossy());
    path.with_file_name(format!(".{name}.{}.tmp", std::process::id()))
}

/// Move an unreadable `path` to `<path>.corrupt` (or `<path>.corrupt.N` when
/// that is taken) so its contents survive, and return where it went.
pub fn set_aside_corrupt(path: &Path) -> io::Result<PathBuf> {
    let base = path.as_os_str().to_string_lossy();
    let free = std::iter::once(format!("{base}.corrupt"))
        .chain((1_u32..).map(|n| format!("{base}.corrupt.{n}")))
        .map(PathBuf::from)
        .find(|candidate| !candidate.exists())
        .ok_or_else(|| io::Error::other("no free name for the corrupt file"))?;
    fs::rename(path, &free)?;
    Ok(free)
}

/// What [`load_or_recover`] found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recovered<T> {
    /// The parsed file, or the default when it was missing or unreadable.
    pub value: T,
    /// Where an unparsable file was moved, so the app can tell the user.
    pub set_aside: Option<PathBuf>,
}

/// Startup load: a missing file gives the default, and an unparsable one is
/// set aside and also gives the default, instead of stopping the app from
/// launching. Only a file that can't be read or moved is an error.
pub fn load_or_recover<T: Default, E: std::fmt::Display>(
    path: &Path,
    parse: impl FnOnce(&[u8]) -> Result<T, E>,
) -> io::Result<Recovered<T>> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(Recovered {
                value: T::default(),
                set_aside: None,
            });
        }
        Err(error) => return Err(error),
    };
    match parse(&bytes) {
        Ok(value) => Ok(Recovered {
            value,
            set_aside: None,
        }),
        Err(_) => Ok(Recovered {
            value: T::default(),
            set_aside: Some(set_aside_corrupt(path)?),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::{load_or_recover, set_aside_corrupt, write_atomic};
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU32, Ordering},
    };

    fn test_dir() -> PathBuf {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "egui-mev-atomic-tests-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).expect("should create directory");
        dir
    }

    fn file_names(dir: &std::path::Path) -> Vec<String> {
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
    fn write_atomic_creates_and_replaces_without_leaving_a_temp_file() {
        let dir = test_dir();
        let path = dir.join("config.toml");

        write_atomic(&path, b"first").expect("should create");
        write_atomic(&path, b"second").expect("should replace");

        assert_eq!(fs::read(&path).expect("should read"), b"second");
        assert_eq!(file_names(&dir), ["config.toml"]);
    }

    #[test]
    fn failed_write_leaves_the_original_intact_and_no_temp_file() {
        let dir = test_dir();
        let path = dir.join("config.toml");
        fs::write(&path, "original").expect("should seed");
        // A directory at the temp name makes creating the temp file fail.
        let blocker = super::temp_path(&path.canonicalize().expect("should resolve"));
        fs::create_dir(&blocker).expect("should block");

        assert!(write_atomic(&path, b"new").is_err());

        assert_eq!(fs::read(&path).expect("should read"), b"original");
        fs::remove_dir(&blocker).expect("should unblock");
        assert_eq!(file_names(&dir), ["config.toml"]);
    }

    #[cfg(unix)]
    #[test]
    fn write_atomic_keeps_permissions_and_symlinks() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let dir = test_dir();
        let real = dir.join("real.toml");
        let link = dir.join("link.toml");
        fs::write(&real, "old").expect("should seed");
        fs::set_permissions(&real, fs::Permissions::from_mode(0o600)).expect("should chmod");
        symlink(&real, &link).expect("should link");

        write_atomic(&link, b"new").expect("should write");

        assert!(
            fs::symlink_metadata(&link)
                .expect("should stat link")
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read(&real).expect("should read"), b"new");
        let mode = fs::metadata(&real)
            .expect("should stat")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn set_aside_corrupt_never_overwrites_an_earlier_copy() {
        let dir = test_dir();
        let path = dir.join("config.toml");

        fs::write(&path, "bad one").expect("should seed");
        let first = set_aside_corrupt(&path).expect("should move");
        fs::write(&path, "bad two").expect("should seed");
        let second = set_aside_corrupt(&path).expect("should move");

        assert_eq!(first, dir.join("config.toml.corrupt"));
        assert_eq!(second, dir.join("config.toml.corrupt.1"));
        assert_eq!(fs::read(&first).expect("should read"), b"bad one");
        assert_eq!(fs::read(&second).expect("should read"), b"bad two");
        assert!(!path.exists());
    }

    fn parse_number(bytes: &[u8]) -> Result<u32, String> {
        std::str::from_utf8(bytes)
            .map_err(|error| error.to_string())?
            .parse()
            .map_err(|error: std::num::ParseIntError| error.to_string())
    }

    #[test]
    fn load_or_recover_defaults_a_missing_file_without_creating_it() {
        let dir = test_dir();
        let path = dir.join("settings");

        let loaded = load_or_recover(&path, parse_number).expect("should load");

        assert_eq!(loaded.value, 0);
        assert_eq!(loaded.set_aside, None);
        assert!(!path.exists());
    }

    #[test]
    fn load_or_recover_reads_a_good_file() {
        let dir = test_dir();
        let path = dir.join("settings");
        fs::write(&path, "7").expect("should seed");

        let loaded = load_or_recover(&path, parse_number).expect("should load");

        assert_eq!(loaded.value, 7);
        assert_eq!(loaded.set_aside, None);
        assert_eq!(fs::read(&path).expect("should stay"), b"7");
    }

    #[test]
    fn load_or_recover_sets_a_corrupt_file_aside_and_keeps_its_contents() {
        let dir = test_dir();
        let path = dir.join("settings");
        fs::write(&path, "not a number").expect("should seed");

        let loaded = load_or_recover(&path, parse_number).expect("should recover");

        assert_eq!(loaded.value, 0);
        let moved = loaded.set_aside.expect("should say where it went");
        assert_eq!(fs::read(&moved).expect("should read"), b"not a number");
        assert!(!path.exists());
    }
}
