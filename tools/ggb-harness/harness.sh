#!/usr/bin/env bash
# Builds and runs the GeoGebra harness.
#
#   GEOGEBRA_SRC=<path to a GeoGebra checkout> tools/ggb-harness/harness.sh build
#   GEOGEBRA_SRC=<path to a GeoGebra checkout> tools/ggb-harness/harness.sh golden [group...]
#   GEOGEBRA_SRC=<path to a GeoGebra checkout> tools/ggb-harness/harness.sh chain
#   GEOGEBRA_SRC=<path to a GeoGebra checkout> tools/ggb-harness/harness.sh ggb [sample step [zip step]]
#
# `build`  builds GeoGebra's common-jre with its own Gradle wrapper, records the
#          class path in build/classpath.txt and compiles the harness.
# `golden` regenerates golden/<group>/*.json from every cases/<group>.json;
#          `golden <group>...` regenerates only the named groups.
# `ggb`    writes the construction file GeoGebra produces for every case, to
#          golden/ggb/<group>/<case id>.xml (and .ggb for every zip step-th case);
#          the sample step keeps every n-th case of each group (default 1).
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
    if [ "$#" -gt 0 ]; then
      files=()
      for group in "$@"; do files+=("$here/cases/$group.json"); done
    else
      files=("$here"/cases/*.json)
    fi
    for file in "${files[@]}"; do
      java "${java_opts[@]}" -cp "$build/classes:$(cat "$build/classpath.txt")" \
        geokernel.harness.GoldenGenerator "$file" "$repo/golden" "$commit"
    done
    ;;
  chain)
    java "${java_opts[@]}" -cp "$build/classes:$(cat "$build/classpath.txt")" \
      geokernel.harness.ChainTiming "$@"
    ;;
  ggb)
    sample="${1:-1}"
    zipstep="${2:-50}"
    rm -rf "$repo/golden/ggb"
    for file in "$here"/cases/*.json; do
      java "${java_opts[@]}" -cp "$build/classes:$(cat "$build/classpath.txt")" \
        geokernel.harness.GgbExporter "$file" "$repo/golden/ggb" "$sample" "$zipstep"
    done
    ;;
  *)
    echo "usage: GEOGEBRA_SRC=<checkout> $0 build|golden|chain|ggb" >&2
    exit 2
    ;;
esac
