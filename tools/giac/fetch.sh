#!/usr/bin/env bash
# Downloads the Giac library the reference loads (build 70507) for this
# platform, checks its SHA-256 and prints the path to set as GEO_GIAC_LIBRARY:
#
#   export GEO_GIAC_LIBRARY="$(tools/giac/fetch.sh)"
#
# Giac is GPL software; see docs/cas-giac.md and NOTICE before redistributing.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
version=70507
release=https://repo.geogebra.net/releases/fr/ujf-grenoble/javagiac

case "$(uname -s):$(uname -m)" in
  Linux:x86_64)
    classifier=linux-amd64
    member=libjavagiac64.so
    sha=3e30a4cac3a26372a9ee98bb377e5d52d234f9fb98270f681ceb641b8144cd29
    ;;
  Darwin:arm64)
    classifier=macosx-universal
    member=libjavagiac-arm64.jnilib
    sha=610da91a710e94c7cc9c1c3f21f0dc8ccd88faca692fbbcb04da82719e0816bf
    ;;
  MINGW*:x86_64 | MSYS*:x86_64 | CYGWIN*:x86_64)
    classifier=windows-amd64
    member=javagiac64.dll
    sha=d577f5f4052b49ee7f4876d3aa6840c4795f612731478fe55d25d20eac2e8075
    ;;
  *)
    echo "no Giac $version library is pinned for $(uname -s) $(uname -m)" >&2
    exit 2
    ;;
esac

dir="$here/build/$version/$classifier"
name="javagiac-$version-natives-$classifier.jar"
mkdir -p "$dir"
if [ ! -f "$dir/$member" ]; then
  tmp="$(mktemp -d "$dir/.download.XXXXXX")"
  trap 'rm -rf "$tmp"' EXIT
  curl --fail --silent --show-error --location --retry 2 "$release/$version/$name" -o "$tmp/$name"
  printf '%s  %s\n' "$sha" "$tmp/$name" | shasum -a 256 -c --status
  unzip -p "$tmp/$name" "$member" > "$tmp/$member"
  mv -f "$tmp/$member" "$dir/$member"
fi
echo "$dir/$member"
