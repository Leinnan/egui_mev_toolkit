//! `egui-mev-icons-subset --scan <dir>... --out <file.ttf> [--check]`
//!
//! Cuts the Lucide font (about 900 KB, 1,600+ glyphs) down to the icons a
//! project's Rust sources name as `Icon::Name`, for `egui_mev_theme`'s
//! `IconFont::Subset`. With `--check` it writes nothing, and fails when the
//! committed subset is stale, for CI.
//!
//! It is pure Rust and needs no external tools: `subsetter` cuts the font, and
//! the `lucide-icons` crate supplies both the font and the name-to-codepoint map.
//! So a subset depends on that crate's version, and bumping it can make `--check`
//! fail until the subset is regenerated.

mod font;
mod scan;

use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use crate::{
    font::build_subset,
    scan::{kebab_case, used_icon_variants},
};

pub(crate) type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const USAGE: &str = "\
Cut the Lucide font down to the icons named in Rust sources.

Usage: egui-mev-icons-subset --scan <dir>... --out <file.ttf> [--check]

Options:
  --scan <dir>...    Folders to search for `Icon::Name`, recursively. Repeatable.
                     `target` and hidden folders are skipped.
  --out <file.ttf>   The subset font to write.
  --check            Write nothing; fail if <file.ttf> is missing or stale.
  -h, --help         Show this help.
";

fn main() -> ExitCode {
    let options = match parse_args(std::env::args_os().skip(1)) {
        Ok(Parsed::Help) => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Ok(Parsed::Run(options)) => options,
        Err(error) => {
            eprintln!("error: {error}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match run(&options) {
        Ok(report) => {
            println!("{report}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Options {
    scan: Vec<PathBuf>,
    out: PathBuf,
    check: bool,
}

#[derive(Debug, PartialEq, Eq)]
enum Parsed {
    Run(Options),
    Help,
}

fn is_flag(arg: &OsString) -> bool {
    arg.as_encoded_bytes().starts_with(b"--")
}

/// Values of `--scan` run up to the next `--flag`. A folder that really starts
/// with two dashes can be written `./--name`.
fn parse_args(args: impl IntoIterator<Item = OsString>) -> std::result::Result<Parsed, String> {
    let mut args = args.into_iter().peekable();
    let mut scan = Vec::new();
    let mut out = None;
    let mut check = false;
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("-h" | "--help") => return Ok(Parsed::Help),
            Some("--check") => check = true,
            Some("--scan") => {
                let before = scan.len();
                while let Some(value) = args.next_if(|value| !is_flag(value)) {
                    scan.push(PathBuf::from(value));
                }
                if scan.len() == before {
                    return Err("`--scan` needs at least one folder".to_owned());
                }
            }
            Some("--out") => {
                let value = args
                    .next_if(|value| !is_flag(value))
                    .ok_or("`--out` needs a file")?;
                if out.replace(PathBuf::from(value)).is_some() {
                    return Err("`--out` was given twice".to_owned());
                }
            }
            _ => return Err(format!("unexpected argument `{}`", arg.display())),
        }
    }
    if scan.is_empty() {
        return Err("`--scan` is required".to_owned());
    }
    let out = out.ok_or("`--out` is required")?;
    Ok(Parsed::Run(Options { scan, out, check }))
}

/// Regenerate (or with `check`, verify) the subset, and say what happened.
fn run(options: &Options) -> Result<String> {
    let variants = used_icon_variants(&options.scan)?;
    if variants.is_empty() {
        let folders = options
            .scan
            .iter()
            .map(|dir| dir.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!("no `Icon::…` usages found under {folders}").into());
    }
    let mut chars = Vec::with_capacity(variants.len());
    for variant in &variants {
        let kebab = kebab_case(variant);
        let icon = lucide_icons::Icon::try_from(kebab.as_str()).map_err(|error| {
            format!("lucide-icons has no icon `{kebab}` (from `Icon::{variant}`): {error}")
        })?;
        chars.push(char::from(icon));
    }

    let full = lucide_icons::LUCIDE_FONT_BYTES;
    let subset = build_subset(full, &chars)?;

    if options.check {
        return check(&options.out, &subset, &variants, &chars);
    }

    if let Some(parent) = options.out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&options.out, &subset)
        .map_err(|error| format!("{}: {error}", options.out.display()))?;
    Ok(format!(
        "wrote {} ({} icons, {} → {} bytes)",
        options.out.display(),
        variants.len(),
        full.len(),
        subset.len()
    ))
}

fn check(path: &Path, subset: &[u8], variants: &[String], chars: &[char]) -> Result<String> {
    let bytes = fs::read(path).map_err(|error| {
        format!(
            "{}: {error}; run this command without `--check` to create it",
            path.display()
        )
    })?;
    // Diagnose the common case (an icon added to the sources) with names.
    if let Ok(face) = ttf_parser::Face::parse(&bytes, 0) {
        let mut missing = Vec::new();
        for (variant, ch) in variants.iter().zip(chars) {
            if face.glyph_index(*ch).is_none() {
                missing.push(format!("Icon::{variant} (U+{:04X})", u32::from(*ch)));
            }
        }
        if !missing.is_empty() {
            return Err(format!(
                "{} is missing {} icon(s): {}. Run this command without `--check`.",
                path.display(),
                missing.len(),
                missing.join(", ")
            )
            .into());
        }
    }
    // Byte equality also catches a stale subset whose metrics or tables no
    // longer match what this tool builds.
    if bytes != subset {
        return Err(format!(
            "{} is out of date ({} bytes on disk, {} expected); run this command without `--check`",
            path.display(),
            bytes.len(),
            subset.len()
        )
        .into());
    }
    Ok(format!(
        "{} is up to date ({} icons)",
        path.display(),
        variants.len()
    ))
}

#[cfg(test)]
mod tests {
    use super::{Options, Parsed, parse_args, run};
    use std::{ffi::OsString, fs, path::PathBuf};

    fn parse(args: &[&str]) -> Result<Parsed, String> {
        parse_args(args.iter().map(OsString::from))
    }

    fn options(scan: &[&str], out: &str, check: bool) -> Parsed {
        Parsed::Run(Options {
            scan: scan.iter().map(PathBuf::from).collect(),
            out: PathBuf::from(out),
            check,
        })
    }

    #[test]
    fn scan_takes_several_folders_and_can_repeat() {
        assert_eq!(
            parse(&[
                "--scan", "a", "b", "--out", "o.ttf", "--scan", "c", "--check"
            ]),
            Ok(options(&["a", "b", "c"], "o.ttf", true))
        );
        assert_eq!(
            parse(&["--out", "o.ttf", "--scan", "-odd"]),
            Ok(options(&["-odd"], "o.ttf", false))
        );
    }

    #[test]
    fn help_wins_over_missing_arguments() {
        assert_eq!(parse(&["--help"]), Ok(Parsed::Help));
        assert_eq!(parse(&["--scan", "a", "--help"]), Ok(Parsed::Help));
    }

    #[test]
    fn bad_grammar_says_what_is_wrong() {
        for (args, message) in [
            (&["--out", "o.ttf"][..], "`--scan` is required"),
            (&["--scan", "a"][..], "`--out` is required"),
            (&["--scan", "--out", "o.ttf"][..], "`--scan` needs at least"),
            (&["--scan", "a", "--out"][..], "`--out` needs a file"),
            (
                &["--scan", "a", "--out", "--check"][..],
                "`--out` needs a file",
            ),
            (
                &["--scan", "a", "--out", "x", "--out", "y"][..],
                "`--out` was given twice",
            ),
            (&["--frobnicate"][..], "unexpected argument `--frobnicate`"),
            (&["stray"][..], "unexpected argument `stray`"),
        ] {
            let error = parse(args).expect_err("should fail");
            assert!(error.contains(message), "{args:?}: {error}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn folders_need_not_be_utf8() {
        use std::os::unix::ffi::OsStringExt;

        let dir = OsString::from_vec(vec![b'a', 0xff]);
        let args = [
            OsString::from("--scan"),
            dir.clone(),
            OsString::from("--out"),
            OsString::from("o.ttf"),
        ];
        let Ok(Parsed::Run(options)) = parse_args(args) else {
            panic!("should parse");
        };
        assert_eq!(options.scan, [PathBuf::from(dir)]);
    }

    fn project(name: &str, source: &str) -> (PathBuf, Options) {
        let root =
            std::env::temp_dir().join(format!("egui-mev-icons-run-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src")).expect("should create");
        fs::write(root.join("src/main.rs"), source).expect("should write");
        let options = Options {
            scan: vec![root.join("src")],
            out: root.join("assets/fonts/lucide-subset.ttf"),
            check: false,
        };
        (root, options)
    }

    fn checking(options: &Options) -> Options {
        Options {
            scan: options.scan.clone(),
            out: options.out.clone(),
            check: true,
        }
    }

    #[test]
    fn writes_the_subset_then_check_passes() {
        let (root, options) = project("fresh", "Icon::Crop; Icon::X;");

        let report = run(&options).expect("should write");
        assert!(report.contains("2 icons"), "{report}");
        assert!(options.out.is_file());

        let report = run(&checking(&options)).expect("should be current");
        assert!(report.contains("up to date"), "{report}");
        fs::remove_dir_all(root).expect("should clean up");
    }

    #[test]
    fn check_writes_nothing_and_fails_when_the_file_is_missing() {
        let (root, options) = project("missing", "Icon::Crop;");

        let error = run(&checking(&options)).expect_err("should fail");

        assert!(error.to_string().contains("without `--check`"), "{error}");
        assert!(!options.out.exists());
        fs::remove_dir_all(root).expect("should clean up");
    }

    #[test]
    fn check_names_an_icon_added_to_the_sources() {
        let (root, options) = project("added", "Icon::Crop;");
        run(&options).expect("should write");
        fs::write(root.join("src/main.rs"), "Icon::Crop; Icon::Anvil;").expect("should write");

        let error = run(&checking(&options)).expect_err("should fail");

        assert!(error.to_string().contains("Icon::Anvil (U+"), "{error}");
        fs::remove_dir_all(root).expect("should clean up");
    }

    #[test]
    fn check_catches_an_icon_removed_from_the_sources() {
        let (root, options) = project("removed", "Icon::Crop; Icon::X;");
        run(&options).expect("should write");
        fs::write(root.join("src/main.rs"), "Icon::Crop;").expect("should write");

        let error = run(&checking(&options)).expect_err("should fail");

        assert!(error.to_string().contains("out of date"), "{error}");
        fs::remove_dir_all(root).expect("should clean up");
    }

    #[test]
    fn check_catches_a_file_that_is_not_a_font() {
        let (root, options) = project("garbage", "Icon::Crop;");
        run(&options).expect("should write");
        fs::write(&options.out, "not a font").expect("should write");

        let error = run(&checking(&options)).expect_err("should fail");

        assert!(error.to_string().contains("out of date"), "{error}");
        fs::remove_dir_all(root).expect("should clean up");
    }

    #[test]
    fn an_unknown_icon_is_an_error_naming_it() {
        let (root, options) = project("unknown", "Icon::NotARealIcon;");

        let error = run(&options).expect_err("should fail");

        assert!(error.to_string().contains("Icon::NotARealIcon"), "{error}");
        assert!(!options.out.exists());
        fs::remove_dir_all(root).expect("should clean up");
    }

    #[test]
    fn sources_with_no_icons_are_an_error() {
        let (root, options) = project("none", "fn main() {}");

        let error = run(&options).expect_err("should fail");

        assert!(error.to_string().contains("no `Icon::…` usages"), "{error}");
        fs::remove_dir_all(root).expect("should clean up");
    }
}
