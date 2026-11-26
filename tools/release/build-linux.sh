#!/usr/bin/env bash
# Compile and smoke-test Linux release packages against the same glibc baseline.
set -euo pipefail

rust_sysroot="$(rustup run 1.100.0 rustc --print sysroot)"
owner="$(id -u):$(id -g)"
docker run --rm --platform linux/amd64 --workdir /source \
  --mount "type=bind,src=$PWD,dst=/source" \
  --mount "type=bind,src=$rust_sysroot,dst=/opt/rust,readonly" \
  --env RELEASE_VERSION --env PRODUCT_NAME --env APP_IDENTIFIER --env LINUX_FORMATS \
  --env VARIANT --env GIAC_SOURCE_URL --env GIAC_SOURCE_SHA256 \
  --env GIAC_DEPENDENCY_SOURCE_URL --env GIAC_DEPENDENCY_SOURCE_SHA256 \
  --env "ARTIFACT_OWNER=$owner" \
  ubuntu@sha256:562111b88caa79c71391c0c4d3c2114a7d5c35915c20561fd893cb1e07929e6b \
  bash -euo pipefail -c '
    export DEBIAN_FRONTEND=noninteractive GEOKERNEL_RELEASE_VERSION="$RELEASE_VERSION"
    export PATH="/opt/rust/bin:$PATH"
    apt-get update -qq
    apt-get install -y -qq --no-install-recommends build-essential ca-certificates curl file git \
      desktop-file-utils libx11-6 libxkbcommon-dev libxcb-render0-dev libxcb-shape0-dev \
      libxcb-xfixes0-dev pkg-config python3 unzip
    git config --global --add safe.directory "$PWD"
    args=()
    if [ "$VARIANT" = gpl ]; then
      args+=(--features geo-kernel/giac)
    fi
    cargo build --release --locked -p geo-app "${args[@]}"
    python3 tools/release/packaging_release.py package linux x86_64 "$VARIANT" target/release/geokernel-app dist
    chown -R "$ARTIFACT_OWNER" dist
  '
