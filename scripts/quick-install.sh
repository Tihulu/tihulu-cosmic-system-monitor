#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only

set -Eeuo pipefail

REPO="${REPO:-Tihulu/tihulu-cosmic-system-monitor}"
REF="${REF:-4937542780791a867fae0cc83c5a92411593560c}"
PREFIX="${PREFIX:-/usr}"
KEEP_BUILD_DIR="${KEEP_BUILD_DIR:-0}"
APP_ID="io.github.tihulu.SystemMonitor"
BIN_NAME="tihulu-cosmic-system-monitor"

if [ -f "$HOME/.cargo/env" ]; then
    # shellcheck disable=SC1090
    source "$HOME/.cargo/env"
fi

log() { printf '\n==> %s\n' "$*"; }
warn() { printf '\nWARN: %s\n' "$*" >&2; }
need_cmd() { command -v "$1" >/dev/null 2>&1; }

install_apt_deps() {
    if ! need_cmd apt-get; then
        warn "apt-get not found; install the COSMIC/libcosmic build dependencies manually."
        return
    fi

    log "Installing build dependencies"
    sudo apt-get update
    sudo apt-get install -y \
        build-essential \
        cmake \
        curl \
        git \
        libegl1-mesa-dev \
        libexpat1-dev \
        libfontconfig-dev \
        libfreetype-dev \
        libwayland-dev \
        libxkbcommon-dev \
        pkgconf \
        tar
}

rust_is_new_enough() {
    need_cmd rustc || return 1
    local version major minor
    version="$(rustc --version | awk '{print $2}')"
    major="${version%%.*}"
    version="${version#*.}"
    minor="${version%%.*}"
    [ "$major" -gt 1 ] || { [ "$major" -eq 1 ] && [ "$minor" -ge 85 ]; }
}

ensure_rust() {
    if need_cmd cargo && rust_is_new_enough; then
        return
    fi

    if need_cmd rustup; then
        log "Updating Rust stable toolchain"
        rustup toolchain install stable
        rustup default stable
    else
        log "Installing current Rust with rustup"
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    fi

    # shellcheck disable=SC1090
    source "$HOME/.cargo/env"
}

main() {
    install_apt_deps
    ensure_rust

    local build_dir archive_url
    build_dir="$(mktemp -d -t tihulu-cosmic-system-monitor.XXXXXX)"
    archive_url="https://github.com/${REPO}/archive/${REF}.tar.gz"

    if [ "$KEEP_BUILD_DIR" != "1" ]; then
        trap 'rm -rf "$build_dir"' EXIT
    else
        log "Keeping build directory: $build_dir"
    fi

    log "Downloading verified source: $REF"
    curl -fsSL "$archive_url" | tar -xz -C "$build_dir" --strip-components=1
    cd "$build_dir"

    log "Running tests"
    cargo test --all-targets

    log "Building release binary"
    cargo build --release

    log "Installing to $PREFIX"
    sudo install -Dm0755 "target/release/$BIN_NAME" "$PREFIX/bin/$BIN_NAME"
    sudo install -Dm0644 resources/app.desktop "$PREFIX/share/applications/$APP_ID.desktop"
    sudo install -Dm0644 resources/app.metainfo.xml "$PREFIX/share/metainfo/$APP_ID.metainfo.xml"
    sudo install -Dm0644 resources/icon.svg "$PREFIX/share/icons/hicolor/scalable/apps/$APP_ID.svg"

    if need_cmd update-desktop-database; then
        sudo update-desktop-database "$PREFIX/share/applications" >/dev/null 2>&1 || true
    fi

    log "Tihulu System Monitor installed"
    printf 'Add “Tihulu System Monitor” in Settings → Desktop → Panel. If an older copy is running, remove/re-add the applet or log out and back in.\n'

    if ! need_cmd nvidia-smi; then
        warn "nvidia-smi was not found. NVIDIA GPU/VRAM metrics require the NVIDIA driver tools; CPU/RAM/network and DRM fallback metrics will still work."
    fi
}

main "$@"
