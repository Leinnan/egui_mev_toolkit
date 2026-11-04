//! Finding the Lucide icons a project's Rust sources name.

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use crate::Result;

/// Every `Icon::…` variant named under `dirs`, sorted. Build output and hidden
/// folders are skipped, so scanning a project root doesn't walk `target/`.
pub fn used_icon_variants(dirs: &[PathBuf]) -> Result<Vec<String>> {
    let mut variants = BTreeSet::new();
    let mut stack = dirs.to_vec();
    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir).map_err(|error| format!("{}: {error}", dir.display()))?;
        for entry in entries {
            let path = entry?.path();
            if path.is_dir() {
                if !is_skipped(&path) {
                    stack.push(path);
                }
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let text = fs::read_to_string(&path)
                    .map_err(|error| format!("{}: {error}", path.display()))?;
                variants.extend(scan_icon_variants(&text));
            }
        }
    }
    Ok(variants.into_iter().collect())
}

fn is_skipped(dir: &Path) -> bool {
    dir.file_name()
        .is_some_and(|name| name == "target" || name.as_encoded_bytes().first() == Some(&b'.'))
}

/// `Icon::Name` tokens in `text` that don't continue an identifier, so
/// `CursorIcon::None` and `CustomIcon::Hitbox` don't count, and whose name is
/// capitalized, so `winit::window::Icon::from_rgba` doesn't either.
fn scan_icon_variants(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut variants = Vec::new();
    let mut from = 0;
    while let Some(rel) = text[from..].find("Icon::") {
        let at = from + rel;
        let boundary = at == 0 || !(bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_');
        let name_start = at + "Icon::".len();
        let name_end = text[name_start..]
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .map_or(text.len(), |end| name_start + end);
        let capitalized = bytes.get(name_start).is_some_and(u8::is_ascii_uppercase);
        if boundary && capitalized && name_end > name_start {
            variants.push(text[name_start..name_end].to_owned());
        }
        from = name_start;
    }
    variants
}

/// `Icon::AArrowDown` → `a-arrow-down`, Lucide's kebab-case icon names. An
/// uppercase starts a new word after a lowercase or digit, or at the end
/// of an acronym (`…Ccw`); a digit starts one after any lowercase letter
/// except `x` (`Trash2` → `trash-2`, `Grid3x3` → `grid-3x3`).
pub fn kebab_case(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::with_capacity(name.len() + 4);
    for (i, &c) in chars.iter().enumerate() {
        let prev = i.checked_sub(1).map(|at| chars[at]);
        let next = chars.get(i + 1).copied();
        let dash = match c {
            'A'..='Z' => {
                let after_word = prev.is_some_and(|p| p.is_ascii_lowercase() || p.is_ascii_digit());
                let acronym_end = prev.is_some_and(|p| p.is_ascii_uppercase())
                    && next.is_some_and(|n| n.is_ascii_lowercase());
                i > 0 && (after_word || acronym_end)
            }
            '0'..='9' => prev.is_some_and(|p| p.is_ascii_lowercase() && p != 'x'),
            _ => false,
        };
        if dash {
            out.push('-');
        }
        out.extend(c.to_lowercase());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_variants_become_lucide_names() {
        for (rust, lucide) in [
            ("Anvil", "anvil"),
            ("AArrowDown", "a-arrow-down"),
            ("ArrowUpRight", "arrow-up-right"),
            ("RotateCcw", "rotate-ccw"),
            ("Grid3x3", "grid-3x3"),
            ("Trash2", "trash-2"),
            ("Layers2", "layers-2"),
            ("X", "x"),
        ] {
            assert_eq!(kebab_case(rust), lucide, "{rust}");
        }
    }

    #[test]
    fn only_icon_tokens_count() {
        let text = "Icon::Anvil; CursorIcon::None; lucide_icons::Icon::Crop; Icon::X";
        assert_eq!(scan_icon_variants(text), ["Anvil", "Crop", "X"]);
    }

    #[test]
    fn functions_on_other_icon_types_do_not_count() {
        let text = "winit::window::Icon::from_rgba(a); CustomIcon::Hitbox; Icon::Crop";
        assert_eq!(scan_icon_variants(text), ["Crop"]);
    }

    #[test]
    fn every_scanned_icon_exists_in_the_crate() {
        // The tool relies on this mapping.
        assert!(lucide_icons::Icon::try_from("a-arrow-down").is_ok());
        assert!(lucide_icons::Icon::try_from("not-an-icon").is_err());
    }

    fn test_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("egui-mev-icons-scan-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("should create directory");
        dir
    }

    #[test]
    fn scanning_merges_folders_and_skips_build_output() {
        let dir = test_dir("merge");
        fs::create_dir_all(dir.join("a/nested")).expect("should create");
        fs::create_dir_all(dir.join("b")).expect("should create");
        fs::create_dir_all(dir.join("a/target/debug")).expect("should create");
        fs::create_dir_all(dir.join("a/.hidden")).expect("should create");
        fs::write(dir.join("a/nested/one.rs"), "Icon::Crop Icon::X").expect("should write");
        fs::write(dir.join("b/two.rs"), "Icon::X Icon::Anvil").expect("should write");
        fs::write(dir.join("a/notes.md"), "Icon::Image").expect("should write");
        fs::write(dir.join("a/target/debug/gen.rs"), "Icon::Layers2").expect("should write");
        fs::write(dir.join("a/.hidden/x.rs"), "Icon::Layers2").expect("should write");

        let found = used_icon_variants(&[dir.join("a"), dir.join("b")]).expect("should scan");

        assert_eq!(found, ["Anvil", "Crop", "X"]);
        fs::remove_dir_all(dir).expect("should clean up");
    }

    #[test]
    fn a_missing_folder_is_named_in_the_error() {
        let dir = test_dir("missing").join("nope");
        let error = used_icon_variants(std::slice::from_ref(&dir)).expect_err("should fail");
        assert!(error.to_string().contains("nope"), "{error}");
    }
}
