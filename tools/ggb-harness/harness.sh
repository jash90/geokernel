#!/usr/bin/env bash
# Builds and runs the GeoGebra harness.
#
#   GEOGEBRA_SRC=<path to a GeoGebra checkout> tools/ggb-harness/harness.sh build
#   GEOGEBRA_SRC=<path to a GeoGebra checkout> tools/ggb-harness/harness.sh golden
#   GEOGEBRA_SRC=<path to a GeoGebra checkout> tools/ggb-harness/harness.sh chain
#
# `build`  builds GeoGebra's common-jre with its own Gradle wrapper, records the
#          class path in build/classpath.txt and compiles the harness.
# `golden` regenerates golden/geometry/*.json from cases/geometry.json.
# `chain`  times the bench() chain (see README.md); extra arguments are passed on.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
build="$here/build"
cmd="${1:-}"
shift || true

: "${GEOGEBRA_SRC:?set GEOGEBRA_SRC to the root of a GeoGebra checkout}"
ggb="$(cd "$GEOGEBRA_SRC" && pwd)"
java_opts=(-Xms4g -Xmx4g -XX:+UseG1GC)

case "$cmd" in
  build)
    mkdir -p "$build/classes"
    "$ggb/gradlew" -p "$ggb/source/shared" --console=plain -q \
      -I "$here/gradle/classpath.init.gradle" \
      "-DggbHarness.classpathFile=$build/classpath.txt" \
      :common-jre:ggbHarnessClasspath
    rm -rf "$build/classes"
    mkdir -p "$build/classes"
    javac --release 17 -encoding UTF-8 -Xlint:all,-processing \
      -cp "$(cat "$build/classpath.txt")" -d "$build/classes" \
      $(find "$here/src/main/java" -name '*.java' | sort)
    echo "built into $build/classes"
    ;;
  golden)
    commit="$(git -C "$ggb" rev-parse HEAD)"
    java "${java_opts[@]}" -cp "$build/classes:$(cat "$build/classpath.txt")" \
      geokernel.harness.GoldenGenerator "$here/cases/geometry.json" "$repo/golden" "$commit"
    ;;
  chain)
    java "${java_opts[@]}" -cp "$build/classes:$(cat "$build/classpath.txt")" \
      geokernel.harness.ChainTiming "$@"
    ;;
  *)
    echo "usage: GEOGEBRA_SRC=<checkout> $0 build|golden|chain" >&2
    exit 2
    ;;
esac
