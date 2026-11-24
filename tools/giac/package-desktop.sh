#!/usr/bin/env bash
# Stage one already-built desktop variant and print the resulting archive.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$here/../.."
variant="${1:?usage: package-desktop.sh <variant> <binaries-dir>}"
binaries="${2:?missing binaries directory}"
case "$variant" in
  geokernel | geokernel-giac) ;;
  *) echo "invalid variant: $variant" >&2; exit 2 ;;
esac

suffix=''
case "$(uname -s)" in
  Darwin) os=macos ;;
  Linux) os=linux ;;
  MINGW* | MSYS* | CYGWIN*) os=windows; suffix=.exe ;;
  *) echo 'unsupported packaging host' >&2; exit 2 ;;
esac
platform="$os-$(uname -m)"
out=target/dist
mkdir -p "$out"
tmp="$(mktemp -d "$out/.stage.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT
root="$tmp/$variant"
mkdir -p "$root"
cp NOTICE LICENSE "$root/"
cp "$binaries/geokernel$suffix" "$root/"
printf 'geokernel %s\nPlatform %s\n' "$(git rev-parse HEAD)" "$platform" > "$root/REVISION"

app_dir="$root"
if [ "$os" = macos ]; then
  contents="$root/geokernel.app/Contents"
  app_dir="$contents/MacOS"
  mkdir -p "$app_dir"
  cat > "$contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key><string>geokernel-app</string>
  <key>CFBundleIdentifier</key><string>org.geokernel.app</string>
  <key>CFBundleName</key><string>geokernel</string>
  <key>CFBundlePackageType</key><string>APPL</string>
</dict>
</plist>
PLIST
fi
cp "$binaries/geokernel-app$suffix" "$app_dir/"

if [ "$variant" = geokernel-giac ]; then
  library="$("$here/fetch.sh")"
  cp "$library" "$root/"
  if [ "$os" = macos ]; then
    mkdir "$contents/Frameworks"
    cp "$library" "$contents/Frameworks/"
    ln -s "../Frameworks/$(basename "$library")" "$app_dir/$(basename "$library")"
  fi
  printf 'Giac javagiac 70507 (%s)\n' "$(basename "$library")" >> "$root/REVISION"
  cp "$here/source-offer.txt" "$root/SOURCE-OFFER.txt"
  git archive --format=tar.gz --prefix=geokernel-source/ HEAD > "$root/geokernel-source.tar.gz"
  gpl_sha=3972dc9744f6499f0f9b2dbf76696f2ae7ad8af9b23dde66d6af86c9dfb36986
  curl --fail --silent --show-error --location --retry 2 \
    https://www.gnu.org/licenses/gpl-3.0.txt -o "$root/GPL-3.0.txt"
  if command -v shasum >/dev/null; then
    printf '%s  %s\n' "$gpl_sha" "$root/GPL-3.0.txt" | shasum -a 256 -c --status
  else
    printf '%s  %s\n' "$gpl_sha" "$root/GPL-3.0.txt" | sha256sum -c --status
  fi
fi

archive="$variant-$platform.tar.gz"
tar -C "$tmp" -czf "$tmp/$archive" "$variant"
mv -f "$tmp/$archive" "$out/$archive"
echo "$out/$archive"
