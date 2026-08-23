#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only

set -Eeuo pipefail

REPO_URL="${REPO_URL:-https://github.com/Tihulu/tihulu-cosmic-system-monitor.git}"
BRANCH="${BRANCH:-main}"
PREFIX="${PREFIX:-/usr}"
KEEP_BUILD_DIR="${KEEP_BUILD_DIR:-0}"

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
        pkgconf
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

ensure_just() {
    if need_cmd just; then
        return
    fi

    log "Installing just"
    cargo install just
}

main() {
    install_apt_deps
    ensure_rust
    ensure_just

    BUILD_DIR="$(mktemp -d -t tihulu-cosmic-system-monitor.XXXXXX)"

    if [ "$KEEP_BUILD_DIR" != "1" ]; then
        trap 'rm -rf "$BUILD_DIR"' EXIT
    else
        log "Keeping build directory: $BUILD_DIR"
    fi

    log "Cloning $REPO_URL"
    git clone --depth 1 --branch "$BRANCH" "$REPO_URL" "$BUILD_DIR"
    cd "$BUILD_DIR"

    log "Checking project"
    cargo check
    cargo test --all-targets

    local just_bin
    just_bin="$(command -v just)"

    log "Building release binary"
    "$just_bin" build-release

    log "Installing to $PREFIX"
    sudo env "prefix=$PREFIX" "$just_bin" install

    log "Installed Tihulu System Monitor"
    printf 'Add “Tihulu System Monitor” in Settings → Desktop → Panel, or restart COSMIC Shell/log out and in if it is not listed yet.\n'

    if ! need_cmd nvidia-smi; then
        warn "nvidia-smi was not found. NVIDIA GPU/VRAM metrics need the proprietary NVIDIA driver tools; AMD has a sysfs fallback."
    fi
}

main "$@"
