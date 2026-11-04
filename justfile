_list:
    @just --list

# Format, lints and crate boundaries: run before every commit.
check:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets --all-features -- -D warnings
    scripts/check-boundaries.sh

fix:
    cargo fmt --all
    cargo clippy --workspace --all-targets --all-features --fix --allow-dirty --allow-staged

test *ARGS:
    #!/usr/bin/env bash
    set -euo pipefail
    # Agents get the quiet profile, which prints only failures.
    profile="default"
    if [[ -n "${CLAUDECODE:-}" ]]; then profile="agent"; fi
    if command -v cargo-nextest >/dev/null; then
        cargo nextest run --workspace --all-features --profile "$profile" {{ARGS}}
    else
        cargo test --workspace --all-features {{ARGS}}
    fi

# Each crate alone with no default features, to catch feature assumptions.
check-alone:
    #!/usr/bin/env bash
    set -euo pipefail
    for manifest in crates/*/Cargo.toml; do
        crate="$(basename "$(dirname "$manifest")")"
        printf '== %s\n' "$crate"
        cargo check --package "$crate" --no-default-features
    done

# Crates meant to run on the web: everything except the native tools.
check-web:
    #!/usr/bin/env bash
    set -euo pipefail
    for manifest in crates/*/Cargo.toml; do
        crate="$(basename "$(dirname "$manifest")")"
        case "$crate" in egui_mev_test | egui_mev_icons_subset | egui_mev_host) continue ;; esac
        printf '== %s\n' "$crate"
        cargo check --target wasm32-unknown-unknown --package "$crate"
    done
    # Optional parts that must work there too.
    cargo check --target wasm32-unknown-unknown --package egui_mev_widgets --all-features
    cargo check --target wasm32-unknown-unknown --package egui_mev_app --features toml,json,chrome

# Windows code can't run here, so at least make sure it compiles.
check-windows:
    cargo check --workspace --all-features --target x86_64-pc-windows-msvc

# Everything CI runs.
ci: check check-alone check-web check-windows test

# A cargo config pointing a project at this checkout; run it from the project.
dev-config:
    @scripts/toolkit-dev-config.sh
