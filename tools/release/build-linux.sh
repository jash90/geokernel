#!/usr/bin/env bash
# Compile and smoke-test Linux release packages against the same glibc baseline.
set -euo pipefail

rust_sysroot="$(rustup run 1.100.0 rustc --print sysroot)"
owner="$(id -u):$(id -g)"
bootstrap="$(mktemp -d)"
trap 'rm -rf -- "$bootstrap"' EXIT
curl --fail --silent --show-error --retry 2 --proto '=https' \
  https://archive.ubuntu.com/ubuntu/pool/main/c/ca-certificates/ca-certificates_20260816ubuntu0.24.04.2_all.deb \
  -o "$bootstrap/ca-certificates.deb"
printf '%s  %s\n' f9fbacd417b515450b831032cc39d18059e9874e67525679a43c16071dabf9a0 \
  "$bootstrap/ca-certificates.deb" | sha256sum --check --status
docker run --rm --platform linux/amd64 --workdir /source \
  --mount "type=bind,src=$PWD,dst=/source" \
  --mount "type=bind,src=$bootstrap,dst=/bootstrap,readonly" \
  --mount "type=bind,src=$rust_sysroot,dst=/opt/rust,readonly" \
  --env RELEASE_VERSION --env PRODUCT_NAME --env APP_IDENTIFIER --env LINUX_FORMATS \
  --env VARIANT --env GIAC_SOURCE_URL --env GIAC_SOURCE_SHA256 \
  --env GIAC_DEPENDENCY_SOURCE_URL --env GIAC_DEPENDENCY_SOURCE_SHA256 \
  --env "ARTIFACT_OWNER=$owner" \
  ubuntu@sha256:562111b88caa79c71391c0c4d3c2114a7d5c35915c20561fd893cb1e07929e6b \
  bash -euo pipefail -c '
    export DEBIAN_FRONTEND=noninteractive GEOKERNEL_RELEASE_VERSION="$RELEASE_VERSION"
    export PATH="/opt/rust/bin:$PATH"
    # The minimal Ubuntu image has no CA bundle for the HTTPS snapshot.
    dpkg-deb -x /bootstrap/ca-certificates.deb /tmp/bootstrap-ca
    cat /tmp/bootstrap-ca/usr/share/ca-certificates/mozilla/*.crt > /tmp/snapshot-ca.pem
    apt_snapshot=(--snapshot 20261125T000000Z -o Acquire::https::CaInfo=/tmp/snapshot-ca.pem)
    apt-get "${apt_snapshot[@]}" -o APT::Update::Error-Mode=any update -qq
    apt-get "${apt_snapshot[@]}" install -y -qq --no-install-recommends build-essential ca-certificates curl file git \
      desktop-file-utils libx11-6 libxkbcommon-dev libxcb-render0-dev libxcb-shape0-dev \
      libxcb-xfixes0-dev pkg-config python3 unzip
    git config --global --add safe.directory "$PWD"
    args=()
    if [ "$VARIANT" = gpl ]; then
      args+=(--features geo-kernel/giac)
    fi
    mkdir -p target
    cargo build --release --locked -p geo-app "${args[@]}" \
      --message-format=json-render-diagnostics > target/release-build.jsonl
    python3 tools/release/packaging_release.py package linux x86_64 "$VARIANT" target/release/geokernel-app dist \
      --build-messages target/release-build.jsonl
    chown -R "$ARTIFACT_OWNER" dist
  '
