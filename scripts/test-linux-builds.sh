#!/usr/bin/env bash
#
# Build and smoke-test the Linux release targets in Docker, the way release.yaml does.
#
# Each target is built in a rust container, then the resulting binary is run in a
# minimal runtime image. The musl binary runs on Alpine, which has no glibc, so a
# successful `rona --version` there proves the static link actually worked.
#
#   ./scripts/test-linux-builds.sh              # the two shipped targets
#   ./scripts/test-linux-builds.sh musl         # x86_64-unknown-linux-musl
#   ./scripts/test-linux-builds.sh arm64        # aarch64-unknown-linux-gnu
#   ./scripts/test-linux-builds.sh musl-arm64   # aarch64-unknown-linux-musl, not shipped
#
# On Apple Silicon the linux/amd64 targets run under emulation and are slow.
# musl-arm64 answers the same "does it build against musl" question natively.

set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
RUST_IMAGE="rust:slim-bookworm"
CARGO_VOLUME="rona-cargo-registry"

if ! docker info &>/dev/null; then
    echo "ERROR: the Docker daemon is not reachable" >&2
    exit 1
fi

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$REPO_ROOT/Cargo.toml" | head -n1)"

# name | docker platform | rust target | runtime image | needs musl-tools
target_spec() {
    case "$1" in
        musl)       echo "linux/amd64 x86_64-unknown-linux-musl  alpine:3             yes" ;;
        arm64)      echo "linux/arm64 aarch64-unknown-linux-gnu  debian:bookworm-slim no"  ;;
        musl-arm64) echo "linux/arm64 aarch64-unknown-linux-musl alpine:3             yes" ;;
        *)          return 1 ;;
    esac
}

run_target() {
    local name="$1" spec platform target runtime needs_musl

    if ! spec="$(target_spec "$name")"; then
        echo "ERROR: unknown target '$name'" >&2
        echo "Valid targets: musl, arm64, musl-arm64" >&2
        exit 1
    fi
    read -r platform target runtime needs_musl <<<"$spec"

    echo
    echo "=============================================================="
    echo "  $name  ($target on $platform)"
    echo "=============================================================="

    local install_musl=""
    if [[ "$needs_musl" == "yes" ]]; then
        install_musl="apt-get update -qq && apt-get install -y -qq musl-tools >/dev/null &&"
    fi

    echo "--- Building"
    docker run --rm --platform "$platform" \
        -v "$REPO_ROOT:/work" -w /work \
        -v "$CARGO_VOLUME:/usr/local/cargo/registry" \
        -e CARGO_TARGET_DIR=/work/target/docker \
        -e CARGO_TERM_COLOR=always \
        "$RUST_IMAGE" \
        bash -c "set -e; $install_musl rustup target add $target >/dev/null; cargo build --locked --release --target $target"

    local binary="$REPO_ROOT/target/docker/$target/release/rona"
    if [[ ! -x "$binary" ]]; then
        echo "ERROR: no binary produced at $binary" >&2
        exit 1
    fi

    echo "--- Running on $runtime"
    local output
    output="$(docker run --rm --platform "$platform" \
        -v "$binary:/rona:ro" \
        "$runtime" /rona --version)"

    echo "    $output"
    if [[ "$output" != *"$VERSION"* ]]; then
        echo "ERROR: expected version $VERSION in '$output'" >&2
        exit 1
    fi
    echo "PASS: $name builds, links, and runs on $runtime"
}

targets=("$@")
if [[ ${#targets[@]} -eq 0 ]]; then
    targets=(musl arm64)
fi

for t in "${targets[@]}"; do
    run_target "$t"
done

echo
echo "All requested targets passed."
