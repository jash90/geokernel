#!/usr/bin/env bash
# Prints the directory containing the four ABI directories for jniLibs.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
version=70507
name="giac-android-$version.aar"
url="https://repo.geogebra.net/releases/org/geogebra/giac-android/$version/$name"
sha=2e0fd53361e87f1db8d3acc353c370ea53a0688d9d0c7021a4c57c7758898433
dir="$here/build/$version/android"
mkdir -p "$dir"
tmp="$(mktemp -d "$dir/.extract.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT

verify() {
  if command -v shasum >/dev/null; then
    printf '%s  %s\n' "$sha" "$1" | shasum -a 256 -c --status
  else
    printf '%s  %s\n' "$sha" "$1" | sha256sum -c --status
  fi
}

if [ ! -f "$dir/$name" ] || ! verify "$dir/$name"; then
  curl --fail --silent --show-error --location --retry 2 "$url" -o "$tmp/$name"
  if ! verify "$tmp/$name"; then
    echo "SHA-256 mismatch for $name" >&2
    exit 1
  fi
  mv -f "$tmp/$name" "$dir/$name"
fi

for abi in arm64-v8a armeabi-v7a x86 x86_64; do
  mkdir -p "$dir/jniLibs/$abi"
  for library in libjavagiac.so libc++_shared.so; do
    unzip -p "$dir/$name" "jni/$abi/$library" > "$tmp/$library"
    mv -f "$tmp/$library" "$dir/jniLibs/$abi/$library"
  done
done
echo "$dir/jniLibs"
