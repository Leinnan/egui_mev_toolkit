//! wgpu presets: the one backend each system uses, and a present-mode override
//! for diagnosing frame pacing on a user's machine.

use eframe::egui_wgpu::{SurfaceConfig, WgpuSetup, WgpuSetupCreateNew};
use wgpu::{Backends, PresentMode};

/// The backends this crate's manifest compiles in for the current system.
#[must_use]
pub fn os_backends() -> Backends {
    if cfg!(target_os = "macos") {
        Backends::METAL
    } else if cfg!(windows) {
        Backends::DX12
    } else {
        Backends::VULKAN | Backends::GL
    }
}

/// A wgpu setup limited to [`os_backends`].
#[must_use]
pub fn os_setup() -> WgpuSetup {
    let mut setup = WgpuSetupCreateNew::without_display_handle();
    setup.instance_descriptor.backends = os_backends();
    WgpuSetup::CreateNew(setup)
}

/// Select wgpu on `options`, limited to [`os_backends`] and presenting with
/// `present_mode`, which [`present_mode`] can read from the command line and
/// the environment.
pub fn apply(options: &mut eframe::NativeOptions, present_mode: PresentMode) {
    options.renderer = eframe::Renderer::Wgpu;
    options.wgpu_options.wgpu_setup = os_setup();
    options.wgpu_options.surface = SurfaceConfig {
        present_mode,
        ..SurfaceConfig::HIGH_THROUGHPUT
    };
}

/// The present mode to use: the `--present-mode <mode>` argument, then the env
/// var the app names, then `AutoVsync`. See [`parse_present_mode`] for the names.
#[must_use]
pub fn present_mode(env_var: &str) -> PresentMode {
    let env_value = std::env::var(env_var).ok();
    let mode = present_mode_from(std::env::args(), env_value.as_deref());
    if mode != PresentMode::AutoVsync {
        log::info!("Using present mode: {mode:?}");
    }
    mode
}

/// [`present_mode`] with its inputs passed in.
#[must_use]
pub fn present_mode_from(
    args: impl IntoIterator<Item = String>,
    env_value: Option<&str>,
) -> PresentMode {
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        if arg == "--present-mode"
            && let Some(value) = args.next()
        {
            return present_mode_or_default(&value);
        }
    }
    env_value.map_or(PresentMode::AutoVsync, present_mode_or_default)
}

fn present_mode_or_default(value: &str) -> PresentMode {
    parse_present_mode(value).unwrap_or_else(|| {
        log::warn!("Unknown present mode '{value}', using AutoVsync");
        PresentMode::AutoVsync
    })
}

/// `fifo`, `mailbox`, `immediate`, `auto-vsync` or `auto-no-vsync`, in any case.
#[must_use]
pub fn parse_present_mode(value: &str) -> Option<PresentMode> {
    match value.to_lowercase().as_str() {
        "fifo" => Some(PresentMode::Fifo),
        "mailbox" => Some(PresentMode::Mailbox),
        "immediate" => Some(PresentMode::Immediate),
        "auto-vsync" => Some(PresentMode::AutoVsync),
        "auto-no-vsync" => Some(PresentMode::AutoNoVsync),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{apply, os_backends, parse_present_mode, present_mode, present_mode_from};
    use eframe::egui_wgpu::WgpuSetup;
    use wgpu::{Backends, PresentMode};

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn names_parse_in_any_case() {
        for (name, mode) in [
            ("fifo", PresentMode::Fifo),
            ("MAILBOX", PresentMode::Mailbox),
            ("Immediate", PresentMode::Immediate),
            ("auto-vsync", PresentMode::AutoVsync),
            ("auto-no-vsync", PresentMode::AutoNoVsync),
        ] {
            assert_eq!(parse_present_mode(name), Some(mode), "{name}");
        }
        assert_eq!(parse_present_mode("warp"), None);
    }

    #[test]
    fn the_argument_wins_over_the_variable() {
        let list = args(&["app", "--present-mode", "immediate"]);
        assert_eq!(
            present_mode_from(list, Some("mailbox")),
            PresentMode::Immediate
        );
    }

    #[test]
    fn the_variable_applies_without_the_argument() {
        assert_eq!(
            present_mode_from(args(&["app"]), Some("mailbox")),
            PresentMode::Mailbox
        );
    }

    #[test]
    fn nothing_or_unknown_gives_auto_vsync() {
        assert_eq!(
            present_mode_from(args(&["app"]), None),
            PresentMode::AutoVsync
        );
        assert_eq!(
            present_mode_from(args(&["app", "--present-mode", "warp"]), Some("mailbox")),
            PresentMode::AutoVsync
        );
        assert_eq!(
            present_mode_from(args(&["app"]), Some("warp")),
            PresentMode::AutoVsync
        );
    }

    #[test]
    fn a_trailing_flag_without_a_value_falls_through_to_the_variable() {
        assert_eq!(
            present_mode_from(args(&["app", "--present-mode"]), Some("fifo")),
            PresentMode::Fifo
        );
    }

    #[test]
    fn an_unset_variable_gives_auto_vsync() {
        assert_eq!(
            present_mode("EGUI_MEV_APP_TEST_PRESENT_MODE_NEVER_SET"),
            PresentMode::AutoVsync
        );
    }

    #[test]
    fn apply_sets_renderer_backends_and_present_mode() {
        let mut options = eframe::NativeOptions::default();
        apply(&mut options, PresentMode::Mailbox);
        assert!(matches!(options.renderer, eframe::Renderer::Wgpu));
        assert_eq!(
            options.wgpu_options.surface.present_mode,
            PresentMode::Mailbox
        );
        let WgpuSetup::CreateNew(setup) = &options.wgpu_options.wgpu_setup else {
            panic!("should create a new instance");
        };
        assert_eq!(setup.instance_descriptor.backends, os_backends());
    }

    #[test]
    fn the_backends_are_the_ones_the_manifest_compiles_in() {
        let backends = os_backends();
        if cfg!(target_os = "macos") {
            assert_eq!(backends, Backends::METAL);
        } else if cfg!(windows) {
            assert_eq!(backends, Backends::DX12);
        } else {
            assert_eq!(backends, Backends::VULKAN | Backends::GL);
        }
    }
}
