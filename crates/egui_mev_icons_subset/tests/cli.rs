//! The built command: exit statuses and which stream says what.

use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
};

fn tool(args: &[&std::ffi::OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_egui-mev-icons-subset"))
        .args(args)
        .output()
        .expect("should run")
}

fn project(name: &str, source: &str) -> PathBuf {
    let root =
        std::env::temp_dir().join(format!("egui-mev-icons-cli-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("src")).expect("should create");
    fs::write(root.join("src/main.rs"), source).expect("should write");
    root
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn help_goes_to_stdout_and_succeeds() {
    let output = tool(&["--help".as_ref()]);
    assert_eq!(output.status.code(), Some(0));
    assert!(text(&output.stdout).contains("--scan <dir>..."));
    assert!(output.stderr.is_empty());
}

#[test]
fn a_usage_error_goes_to_stderr_with_status_2() {
    let output = tool(&["--scan".as_ref(), "a".as_ref()]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(text(&output.stderr).contains("`--out` is required"));
}

#[test]
fn write_then_check_then_stale_check() {
    let root = project("flow", "Icon::Crop;");
    let src = root.join("src");
    let out = root.join("subset.ttf");
    let args = [
        "--scan".as_ref(),
        src.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ];
    let mut check_args = args.to_vec();
    check_args.push("--check".as_ref());

    let output = tool(&args);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert!(text(&output.stdout).contains("wrote"));

    let output = tool(&check_args);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert!(text(&output.stdout).contains("up to date"));

    fs::write(src.join("main.rs"), "Icon::Crop; Icon::Anvil;").expect("should write");
    let before = fs::read(&out).expect("should read");
    let output = tool(&check_args);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(text(&output.stderr).contains("Icon::Anvil"));
    assert_eq!(
        fs::read(&out).expect("should read"),
        before,
        "check writes nothing"
    );

    fs::remove_dir_all(root).expect("should clean up");
}
