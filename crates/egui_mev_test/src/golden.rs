//! Golden hashes: compare many outputs against one small text file instead of
//! a folder of images, for outputs that should match bit for bit.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use egui_kittest::Harness;

/// FNV-1a 64-bit hash of an RGBA image's size and pixels.
#[must_use]
pub fn hash_rgba(width: u32, height: u32, rgba: &[u8]) -> u64 {
    let size = [width.to_le_bytes(), height.to_le_bytes()];
    size.iter()
        .flatten()
        .chain(rgba)
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, &byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
        })
}

/// The hash of what `harness` draws now.
///
/// # Panics
/// When rendering fails.
pub fn render_hash<S>(harness: &mut Harness<'_, S>) -> u64 {
    let image = harness
        .render()
        .unwrap_or_else(|err| panic!("rendering failed: {err}"));
    hash_rgba(image.width(), image.height(), image.as_raw())
}

/// A golden-hash file of `name hash` lines.
pub struct Golden {
    path: PathBuf,
    expected: BTreeMap<String, String>,
    actual: BTreeMap<String, String>,
}

impl Golden {
    /// Open `file`, relative to the calling crate's manifest dir. Pass
    /// `env!("CARGO_MANIFEST_DIR")` as `manifest_dir`.
    #[must_use]
    pub fn open(manifest_dir: &str, file: &str) -> Self {
        let path = Path::new(manifest_dir).join(file);
        let expected = std::fs::read_to_string(&path)
            .unwrap_or_default()
            .lines()
            .filter_map(|line| {
                let (name, hash) = line.rsplit_once(' ')?;
                Some((name.to_owned(), hash.to_owned()))
            })
            .collect();
        Self {
            path,
            expected,
            actual: BTreeMap::new(),
        }
    }

    /// Record the result for case `name`.
    ///
    /// # Panics
    /// When a case is recorded twice, or its name spans lines.
    pub fn record(&mut self, name: impl Into<String>, hash: u64) {
        let name = name.into();
        assert!(!name.contains('\n'), "case names are single-line");
        let previous = self.actual.insert(name.clone(), format!("{hash:016x}"));
        assert!(previous.is_none(), "duplicate golden case {name}");
    }

    /// Record a case that produced no output, such as a cancelled render.
    pub fn record_none(&mut self, name: impl Into<String>) {
        self.actual.insert(name.into(), "none".to_owned());
    }

    /// Compare against the file, or rewrite it under `UPDATE_GOLDEN=1`.
    ///
    /// # Panics
    /// When any case differs from, or is missing in, the golden file.
    pub fn check(self) {
        if std::env::var_os("UPDATE_GOLDEN").is_some() {
            self.write();
            return;
        }
        let problems = self.problems();
        assert!(
            problems.is_empty(),
            "{} golden case(s) changed in {}:\n  {}\n(rerun with UPDATE_GOLDEN=1 only if the change is intended)",
            problems.len(),
            self.path.display(),
            problems.join("\n  ")
        );
    }

    fn write(&self) {
        let mut text = String::new();
        for (name, hash) in &self.actual {
            let _ = writeln!(text, "{name} {hash}");
        }
        if let Err(err) = std::fs::write(&self.path, text) {
            panic!("writing {}: {err}", self.path.display());
        }
    }

    fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        for (name, hash) in &self.actual {
            match self.expected.get(name) {
                Some(expected) if expected == hash => {}
                Some(expected) => problems.push(format!("{name}: {expected} -> {hash}")),
                None => problems.push(format!("{name}: not in golden file")),
            }
        }
        for name in self
            .expected
            .keys()
            .filter(|name| !self.actual.contains_key(*name))
        {
            problems.push(format!("{name}: no longer produced"));
        }
        problems
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn golden(expected: &[(&str, &str)]) -> Golden {
        Golden {
            path: PathBuf::from("unused"),
            expected: expected
                .iter()
                .map(|(name, hash)| ((*name).to_owned(), (*hash).to_owned()))
                .collect(),
            actual: BTreeMap::new(),
        }
    }

    #[test]
    fn the_hash_covers_size_and_pixels() {
        let pixels = [1, 2, 3, 4, 5, 6, 7, 8];
        assert_eq!(hash_rgba(2, 1, &pixels), hash_rgba(2, 1, &pixels));
        assert_ne!(hash_rgba(2, 1, &pixels), hash_rgba(1, 2, &pixels));
        assert_ne!(hash_rgba(2, 1, &pixels), hash_rgba(2, 1, &[0; 8]));
    }

    #[test]
    fn matching_cases_have_no_problems() {
        let mut golden = golden(&[("a", "000000000000000f"), ("b", "none")]);
        golden.record("a", 15);
        golden.record_none("b");
        assert_eq!(golden.problems(), Vec::<String>::new());
    }

    #[test]
    fn changed_new_and_missing_cases_are_all_named() {
        let mut golden = golden(&[("changed", "000000000000000f"), ("gone", "none")]);
        golden.record("changed", 16);
        golden.record("new", 1);
        assert_eq!(
            golden.problems(),
            [
                "changed: 000000000000000f -> 0000000000000010",
                "new: not in golden file",
                "gone: no longer produced",
            ]
        );
    }

    #[test]
    fn names_may_contain_spaces() {
        let dir = std::env::temp_dir().join(format!("egui_mev_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("names.txt");
        std::fs::write(&file, "two words 000000000000000f\n").unwrap();
        let mut golden = Golden::open(dir.to_str().unwrap(), "names.txt");
        golden.record("two words", 15);
        golden.check();
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    #[should_panic(expected = "duplicate golden case")]
    fn recording_a_case_twice_is_a_bug_in_the_test() {
        let mut golden = golden(&[]);
        golden.record("a", 1);
        golden.record("a", 2);
    }
}
