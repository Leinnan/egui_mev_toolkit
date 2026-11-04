#!/usr/bin/env bash
# The rules of docs/crate-rules.md that a dependency tree can prove. Every
# `cargo tree` failure stops the script, so a failed fetch can never look clean.
set -euo pipefail

failed=0

# tree CRATE [cargo tree flags...]: the crate's dependency names, one per line.
tree() {
    local package="$1"
    shift
    cargo tree --package "$package" --edges normal --prefix none --format '{p}' "$@" |
        awk '{print $1}' | sort -u
}

# forbid LABEL NAMES_REGEX LIST
forbid() {
    local found
    found="$(printf '%s\n' "$3" | grep -E "^($2)$" || true)"
    if [[ -n "$found" ]]; then
        printf 'boundary violation in %s:\n%s\n' "$1" "$found" >&2
        failed=1
    fi
}

# control LABEL NAME LIST: the checks above see what they look for.
control() {
    if ! printf '%s\n' "$3" | grep -qx "$2"; then
        printf 'control failed: %s not found in %s\n' "$2" "$1" >&2
        failed=1
    fi
}

renderer='eframe|wgpu|glow|winit|egui-wgpu|egui_glow|egui-winit'
any_bevy='bevy|bevy_[a-z_]+'
# What `bevy_reflect` brings along, with no ECS, rendering or app.
reflect_only='bevy_reflect|bevy_reflect_derive|bevy_macro_utils|bevy_platform|bevy_ptr|bevy_utils'

# `*_core` crates have no egui at all, in tests either.
for package in egui_mev_core; do
    list="$(cargo tree --package "$package" --edges normal,dev --prefix none --format '{p}' | awk '{print $1}' | sort -u)"
    forbid "$package" 'egui|ecolor|emath|epaint|eframe' "$list"
    if grep -rnE '^\s*(pub\s+)?use\s+egui(::|;)' "crates/$package/src" >&2; then
        printf 'boundary violation: egui import in crates/%s/src\n' "$package" >&2
        failed=1
    fi
done

# Snapshot images stay out of packages.
for manifest in crates/*/Cargo.toml; do
    if ! grep -qx 'exclude.workspace = true' "$manifest"; then
        printf 'packaging: %s must inherit exclude from the workspace\n' "$manifest" >&2
        failed=1
    fi
done

# No renderer, whichever features are on and on every target.
for package in egui_mev_core egui_mev_theme egui_mev_widgets egui_mev_data egui_mev_graphics egui_mev_commands egui_mev_tasks egui_mev_inspect; do
    forbid "$package (all features)" "$renderer" "$(tree "$package" --all-features --target all)"
done

# The app shell links a renderer only when asked for one.
forbid "egui_mev_app (default features)" "$renderer" "$(tree egui_mev_app --target all)"
control "egui_mev_app with its wgpu feature" wgpu "$(tree egui_mev_app --features wgpu)"

# Theme OS calls exist only behind `native`.
for target in aarch64-apple-darwin x86_64-pc-windows-msvc; do
    forbid "egui_mev_theme (default features, $target)" 'objc2-app-kit|windows-sys|raw-window-handle' \
        "$(tree egui_mev_theme --target "$target")"
done
control "egui_mev_theme with native on macOS" objc2-app-kit \
    "$(tree egui_mev_theme --features native --target aarch64-apple-darwin)"
control "egui_mev_theme with native on Windows" windows-sys \
    "$(tree egui_mev_theme --features native --target x86_64-pc-windows-msvc)"

# Chrome links no renderer of its own.
forbid "egui_mev_app (chrome)" "$renderer" "$(tree egui_mev_app --features chrome --target all)"

# The host is raw winit and wgpu, and eframe would defeat its purpose. glow is
# allowed: wgpu's gles backend uses it.
forbid "egui_mev_host" 'eframe|glutin|egui_glow' "$(tree egui_mev_host --all-features --target all)"
control egui_mev_host winit "$(tree egui_mev_host)"
control egui_mev_host egui-wgpu "$(tree egui_mev_host)"

# The native menu exists only on macOS: Linux must not pull in muda's GTK.
linux="$(tree egui_mev_commands --all-features --target x86_64-unknown-linux-gnu)"
forbid "egui_mev_commands (all features, Linux)" 'muda|gtk|arboard' "$linux"
control "egui_mev_commands with native_menu on macOS" muda \
    "$(tree egui_mev_commands --features native_menu --target aarch64-apple-darwin)"
forbid "egui_mev_commands (default features, macOS)" 'muda|arboard' \
    "$(tree egui_mev_commands --target aarch64-apple-darwin)"

# No Bevy anywhere, except `bevy_reflect` in the inspector.
for package in egui_mev_core egui_mev_theme egui_mev_widgets egui_mev_data egui_mev_graphics egui_mev_commands egui_mev_tasks egui_mev_app egui_mev_host egui_mev_test egui_mev_icons_subset; do
    forbid "$package (all features)" "$any_bevy" "$(tree "$package" --all-features)"
done
inspect="$(tree egui_mev_inspect --all-features)"
forbid "egui_mev_inspect (all features)" "$any_bevy" \
    "$(printf '%s\n' "$inspect" | grep -vE "^($reflect_only)$" || true)"
control egui_mev_inspect bevy_reflect "$inspect"

control egui_mev_theme egui "$(tree egui_mev_theme)"

if ((failed != 0)); then
    exit 1
fi
printf 'crate boundaries verified\n'
