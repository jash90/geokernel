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
    member_sha=d4c7f38357f06cc6a364d551ac3c375568b703c3abc4da76674e9642ce0316ec
    ;;
  Darwin:arm64)
    classifier=macosx-universal
    member=libjavagiac-arm64.jnilib
    sha=610da91a710e94c7cc9c1c3f21f0dc8ccd88faca692fbbcb04da82719e0816bf
    member_sha=a2ed7741e02e8569c9cc9354c9818d883e3d3175de8389c557930f13bd48c498
    ;;
  Darwin:x86_64)
    classifier=macosx-universal
    member=libjavagiac.jnilib
    sha=610da91a710e94c7cc9c1c3f21f0dc8ccd88faca692fbbcb04da82719e0816bf
    member_sha=b9abc2abb81c3dbc8397754e716483a61ace680106b15ccb66e19507fca10939
    ;;
  MINGW*:x86_64 | MSYS*:x86_64 | CYGWIN*:x86_64)
    classifier=windows-amd64
    member=javagiac64.dll
    sha=d577f5f4052b49ee7f4876d3aa6840c4795f612731478fe55d25d20eac2e8075
    member_sha=d54fbab833cc4a6bc07cb5e7f9c3a83cb5b08c33402198b3ca0495c77bdf0d71
    ;;
  *)
    echo "no Giac $version library is pinned for $(uname -s) $(uname -m)" >&2
    exit 2
    ;;
esac

verify() {
  if command -v shasum >/dev/null; then
    printf '%s  %s\n' "$1" "$2" | shasum -a 256 -c --status
  else
    printf '%s  %s\n' "$1" "$2" | sha256sum -c --status
  fi
}

dir="$here/build/$version/$classifier"
name="javagiac-$version-natives-$classifier.jar"
mkdir -p "$dir"
# A cached library is used only while it still has the pinned hash.
if [ ! -f "$dir/$member" ] \
  || ! verify "$member_sha" "$dir/$member"; then
  tmp="$(mktemp -d "$dir/.download.XXXXXX")"
  trap 'rm -rf "$tmp"' EXIT
  curl --fail --silent --show-error --location --retry 2 "$release/$version/$name" -o "$tmp/$name"
  verify "$sha" "$tmp/$name"
  unzip -p "$tmp/$name" "$member" > "$tmp/$member"
  verify "$member_sha" "$tmp/$member"
  mv -f "$tmp/$member" "$dir/$member"
fi
case "$(uname -s)" in
  MINGW* | MSYS* | CYGWIN*) cygpath -am "$dir/$member" ;;
  *) echo "$dir/$member" ;;
esac
