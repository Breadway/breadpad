#!/usr/bin/env bash
# Builds (or reuses, via docker's own layer cache) the pinned Arch CI image
# from ci/Containerfile, then runs the given cargo command inside it against
# this repo checkout.
#
# Cargo's registry/git caches and CARGO_TARGET_DIR are persisted in named
# docker volumes so they survive across runs even though the repo checkout
# itself (a fresh --depth 1 clone per workflow run) does not.
#
# Usage: ci/build.sh cargo build --release --locked
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

docker build -t breadpad-ci:archlinux -f ci/Containerfile ci

docker run --rm \
    -v "${ROOT}:/workspace" \
    -v breadpad-cargo-registry:/root/.cargo/registry \
    -v breadpad-cargo-git:/root/.cargo/git \
    -v breadpad-cargo-target:/cargo-target \
    -w /workspace \
    -e CARGO_TARGET_DIR=/cargo-target \
    breadpad-ci:archlinux \
    bash -c '
        set -euo pipefail
        "$@"
        mkdir -p /workspace/target
        cp -a /cargo-target/. /workspace/target/
    ' bash "$@"
