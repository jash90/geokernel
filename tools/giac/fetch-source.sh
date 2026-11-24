#!/usr/bin/env bash
# Verify the publisher's javagiac 70507 source archive and print its path.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
revision=19a710aea9d528afa272ea582f3283f242e17af3
sha=a7f63cf3cd19040b81be7e35ccf0a396d2b2d9629a6db902c47529cf95ef6e36
if [[ "${GIAC_SOURCE_URL:-}" != https://* ]] ||
  [[ "${GIAC_SOURCE_SHA256:-}" != "$sha" ]]; then
  echo 'The GPL archive requires an HTTPS URL and the pinned javagiac 70507 source SHA-256.' >&2
  exit 2
fi

verify() {
  if command -v shasum >/dev/null; then
    printf '%s  %s\n' "$sha" "$1" | shasum -a 256 -c --status
  else
    printf '%s  %s\n' "$sha" "$1" | sha256sum -c --status
  fi
}

dir="$here/build/70507"
archive="$dir/giac-70507-source.tar.gz"
mkdir -p "$dir"
tmp="$(mktemp -d "$dir/.source.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT
source_archive="$archive"
if [ ! -f "$archive" ] || ! verify "$archive"; then
  source_archive="$tmp/source.tar.gz"
  curl --fail --silent --show-error --location --retry 2 \
    --proto '=https' --proto-redir '=https' -- "$GIAC_SOURCE_URL" > "$source_archive"
  if ! verify "$source_archive"; then
    echo 'Giac source does not match the pinned 70507 archive SHA-256.' >&2
    exit 2
  fi
fi

# This mirror keeps the regression checks under src/test.
tar -tzf "$source_archive" > "$tmp/entries"
for entry in / src/ src/giac/cpp/gen.cc src/giac/headers/gen.h \
  src/jni/cpp/giac_wrap.cxx src/test/regression CMakeLists.txt build.gradle \
  deploy.gradle Jenkinsfile; do
  path="giac-$revision/${entry#/}"
  if ! grep -Fxq "$path" "$tmp/entries"; then
    echo "Giac source archive is missing $path." >&2
    exit 2
  fi
done
if [ "$source_archive" != "$archive" ]; then
  mv -f "$source_archive" "$archive"
fi
echo "$archive"
