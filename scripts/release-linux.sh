#!/bin/sh
# Builds dist/Kiri-linux-x64.tar.gz, which holds Kiri/kiri for the updater and for users to
# extract, and with --upload adds it to the GitHub release v<version> that scripts/release.sh
# --publish created. Off Linux it runs itself in an x86_64 Ubuntu 22.04 container: a binary
# needs the glibc it was built against or older, so it runs on 22.04 and later.
# --setup installs the build's packages and toolchain first; the container and CI run as root.
# See docs/operations/release.md.
set -eu
cd "$(dirname "$0")/.."

VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
TAR=dist/Kiri-linux-x64.tar.gz
# release.yml's RUSTUP_TOOLCHAIN.
TOOLCHAIN=${RUSTUP_TOOLCHAIN:-1.99.0}
PACKAGES="build-essential pkg-config curl ca-certificates libfontconfig1-dev libfreetype-dev libxcb1-dev libxkbcommon-x11-dev"
CARGO_BIN=${CARGO_HOME:-$HOME/.cargo}/bin

UPLOAD= SETUP=
for arg in "$@"; do
    case $arg in
        --upload) UPLOAD=1 ;;
        --setup) SETUP=1 ;;
        *) echo "usage: $0 [--setup] [--upload]" >&2; exit 2 ;;
    esac
done

if [ -n "$UPLOAD" ]; then
    git diff --quiet HEAD || { echo "Commit first: the asset is built from HEAD." >&2; exit 1; }
    gh release view "v$VERSION" --json tagName > /dev/null || { echo "No release v$VERSION: scripts/release.sh --publish creates it." >&2; exit 1; }
fi

if [ "$(uname -s)" != Linux ]; then
    # One volume keeps the toolchain, the registry and the build between runs, away from ./target.
    docker run --rm --platform linux/amd64 -v "$PWD:/src" -w /src -v kiri-linux-x64:/cache \
        -e CARGO_HOME=/cache/cargo -e RUSTUP_HOME=/cache/rustup -e CARGO_TARGET_DIR=/cache/target \
        ubuntu:22.04 scripts/release-linux.sh --setup
else
    [ "$(uname -m)" = x86_64 ] || { echo "This is a $(uname -m) machine, and the asset is x64." >&2; exit 1; }
    if [ -n "$SETUP" ]; then
        export DEBIAN_FRONTEND=noninteractive
        apt-get update -qq
        # shellcheck disable=SC2086
        apt-get install -y -qq --no-install-recommends $PACKAGES > /dev/null
        [ -x "$CARGO_BIN/rustup" ] || curl -sSf https://sh.rustup.rs | sh -s -- -y -q --profile minimal --default-toolchain none --no-modify-path
        "$CARGO_BIN/rustup" toolchain install "$TOOLCHAIN" --profile minimal
    fi
    export PATH="$CARGO_BIN:$PATH"
    cargo +"$TOOLCHAIN" build --release --locked
    STAGE=$(mktemp -d)
    mkdir "$STAGE/Kiri"
    cp "${CARGO_TARGET_DIR:-target}/release/kiri" "$STAGE/Kiri/kiri"
    mkdir -p dist
    tar -czf "$TAR" --owner=0 --group=0 -C "$STAGE" Kiri
    rm -rf "$STAGE"
    echo "Built $TAR ($VERSION)"
fi

[ -n "$UPLOAD" ] || exit 0
gh release upload "v$VERSION" "$TAR" --clobber
