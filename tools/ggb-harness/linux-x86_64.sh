#!/usr/bin/env bash
# Runs the harness in a Linux x86_64 container.
#
#   GEOGEBRA_SRC=<path to a GeoGebra checkout> tools/ggb-harness/linux-x86_64.sh golden [group...]
#
# Every command of harness.sh except `build` can follow. The harness is built
# on this system first (`harness.sh build`); the container only runs it, as
# Java class files and jars are the same on every system.
#
# The golden data is recorded on Linux x86_64. HotSpot's intrinsics for the
# math functions give other last bits on other systems (Math.exp(1) is
# 2.718281828459045 there and 2.7182818284590455 on macOS arm64), so a
# recording made elsewhere moves sample values by one unit in the last
# place. After a run, check with `git diff` that the files whose outcome does
# not change are byte-identical. The bounds of a text depend on the fonts of
# the system; they are not recorded this way.
#
# Needs a container runtime that runs x86_64 images. GGB_HARNESS_RUNTIME
# names it (default `docker`) and GGB_HARNESS_IMAGE the image (default
# `eclipse-temurin:17-jdk`: the JDK 17 the harness needs and nothing more).
# GRADLE_USER_HOME is Gradle's directory with the dependency jars (default
# `~/.gradle`).

set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
case "${1:-}" in
  ""|build|fetch-giac)
    echo "usage: GEOGEBRA_SRC=<checkout> $0 <a harness.sh command except build>" >&2
    exit 2
    ;;
esac
if [ ! -f "$here/build/classpath.txt" ]; then
  echo 'run harness.sh build first' >&2
  exit 2
fi
: "${GEOGEBRA_SRC:?set GEOGEBRA_SRC to the root of a GeoGebra checkout}"
ggb="$(cd "$GEOGEBRA_SRC" && pwd -P)"
gradle="$(cd "${GRADLE_USER_HOME:-$HOME/.gradle}" && pwd -P)"
runtime="${GGB_HARNESS_RUNTIME:-docker}"
image="${GGB_HARNESS_IMAGE:-eclipse-temurin:17-jdk}"

# The Giac library of the container's platform is fetched and checked here.
GGB_HARNESS_GIAC_PLATFORM=Linux:x86_64 GEOGEBRA_SRC="$ggb" "$here/harness.sh" fetch-giac > /dev/null

exec "$runtime" run --rm --platform linux/amd64 \
  -v "$ggb:/geogebra:ro" -v "$gradle:/gradle:ro" -v "$repo:/geokernel" -w /geokernel \
  -e GEOGEBRA_SRC=/geogebra \
  -e "GEOGEBRA_REV=$(git -C "$ggb" rev-parse HEAD)" \
  -e "GGB_HARNESS_CP_SED=s|$ggb|/geogebra|g; s|$gradle|/gradle|g" \
  -e "GGB_HARNESS_JAVA_OPTS=${GGB_HARNESS_JAVA_OPTS:--Djava.awt.headless=true}" \
  ${GGB_HARNESS_TOPICS:+-e GGB_HARNESS_TOPICS} \
  ${GGB_HARNESS_CAS:+-e GGB_HARNESS_CAS} \
  "$image" bash tools/ggb-harness/harness.sh "$@"
