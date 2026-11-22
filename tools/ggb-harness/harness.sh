#!/usr/bin/env bash
# Builds and runs the GeoGebra harness.
#
#   GEOGEBRA_SRC=<path to a GeoGebra checkout> tools/ggb-harness/harness.sh build
#   GEOGEBRA_SRC=<path to a GeoGebra checkout> tools/ggb-harness/harness.sh golden [group...]
#   GEOGEBRA_SRC=<path to a GeoGebra checkout> tools/ggb-harness/harness.sh chain
#   GEOGEBRA_SRC=<path to a GeoGebra checkout> tools/ggb-harness/harness.sh ggb [sample step [zip step]]
#   GEOGEBRA_SRC=<path to a GeoGebra checkout> tools/ggb-harness/harness.sh saved
#   GEOGEBRA_SRC=<path to a GeoGebra checkout> tools/ggb-harness/harness.sh export-snapshot <directory>
#   GEOGEBRA_SRC=<path to a GeoGebra checkout> tools/ggb-harness/harness.sh reload <file.xml> <label>...
#   GEOGEBRA_SRC=<path to a GeoGebra checkout> tools/ggb-harness/harness.sh tpairs [type]
#   GEOGEBRA_SRC=<path to a GeoGebra checkout> tools/ggb-harness/harness.sh text-canvas
#   GEOGEBRA_SRC=<path to a GeoGebra checkout> tools/ggb-harness/harness.sh cas-smoke
#
# `build`  builds GeoGebra's common-jre with its own Gradle wrapper, records the
#          class path in build/classpath.txt and compiles the harness.
# `golden` regenerates golden/<group>/*.json from every cases/<group>.json;
#          `golden <group>...` regenerates only the named groups.
# `ggb`    writes the construction file GeoGebra produces for every case, to
#          golden/ggb/<group>/<case id>.xml (and .ggb for every zip step-th case);
#          the sample step keeps every n-th case of each group (default 1).
#          A partial run (sample step above 1) deletes nothing; a full run
#          prunes files it did not produce and prints each one.
# `saved`  writes construction files saved after the free points were moved,
#          with GeoGebra's own values at that moment, to golden/ggb/saved/
#          (<id>.xml or <id>.ggb, and <id>.snapshot), from saved/saved.json.
# `reload` loads a saved file into a fresh GeoGebra and prints the numbers it
#          computes for the given labels.
# `export-snapshot` loads every .ggb in a directory and writes a snapshot beside it.
# `tpairs` replays the upstream test methods of golden/t-pairs/scripts.jsonl and
#          writes what GeoGebra returns for every step to golden/t-pairs/verified.jsonl.
# `text-canvas` checks text pointer interaction and integer placement.
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

giac_version=70507

giac_platform() {
  case "$(uname -s):$(uname -m)" in
    Darwin:arm64)
      giac_classifier=macosx-universal
      giac_member=libjavagiac-arm64.jnilib
      giac_sha=610da91a710e94c7cc9c1c3f21f0dc8ccd88faca692fbbcb04da82719e0816bf
      ;;
    Linux:x86_64)
      giac_classifier=linux-amd64
      giac_member=libjavagiac64.so
      giac_sha=3e30a4cac3a26372a9ee98bb377e5d52d234f9fb98270f681ceb641b8144cd29
      ;;
    *) return 1 ;;
  esac
  giac_dir="$build/giac/$giac_version/$giac_classifier"
}

fetch_giac() (
  mkdir -p "$giac_dir"
  local name="javagiac-$giac_version-natives-$giac_classifier.jar"
  local jar="$giac_dir/$name"
  local tmp
  tmp="$(mktemp -d "$giac_dir/.download.XXXXXX")"
  trap 'rm -rf "$tmp"' EXIT
  if [ ! -f "$jar" ] || ! printf '%s  %s\n' "$giac_sha" "$jar" | shasum -a 256 -c --status; then
    local release=https://repo.geogebra.net/releases/fr/ujf-grenoble/javagiac
    curl --fail --silent --show-error --location --retry 2 \
      "$release/$giac_version/$name" -o "$tmp/$name"
    printf '%s  %s\n' "$giac_sha" "$tmp/$name" | shasum -a 256 -c
    mv -f "$tmp/$name" "$jar"
  fi
  unzip -p "$jar" "$giac_member" > "$tmp/$giac_member"
  mv -f "$tmp/$giac_member" "$giac_dir/$giac_member"
)

enable_giac() {
  if ! giac_platform; then
    echo 'the Giac oracle requires macOS arm64 or Linux x86_64' >&2
    exit 2
  fi
  if [ ! -f "$giac_dir/$giac_member" ]; then
    echo 'run harness.sh build before enabling the Giac oracle' >&2
    exit 2
  fi
  java_opts+=("-DggbHarness.giac=$giac_dir/$giac_member")
}

if [ "${GGB_HARNESS_CAS:-0}" = 1 ] || [ "$cmd" = cas-smoke ]; then
  if [ "$cmd" != build ]; then enable_giac; fi
fi

case "$cmd" in
  build)
    mkdir -p "$build/classes"
    if giac_platform; then fetch_giac; fi
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
      for group in "$@"; do
        files+=("$here/cases/$group.json")
        reference="$here/cases/reference/$group.json"
        if [ -f "$reference" ]; then files+=("$reference"); fi
      done
    else
      shopt -s nullglob
      files=("$here"/cases/*.json "$here"/cases/reference/*.json)
    fi
    for file in "${files[@]}"; do
      (
        run_golden() {
          java "${java_opts[@]}" -cp "$build/classes:$(cat "$build/classpath.txt")" \
            geokernel.harness.GoldenGenerator "$file" "$repo/golden" "$commit" "$@"
        }
        if [ "${GGB_HARNESS_CAS:-0}" != 1 ]; then
          case "${file##*/}" in
            calculus.json)
              run_golden '!degree'
              enable_giac
              run_golden degree
              exit
              ;;
            cas.json|locusequation.json|envelope.json) enable_giac ;;
          esac
        fi
        run_golden
      )
    done
    ;;
  chain)
    java "${java_opts[@]}" -cp "$build/classes:$(cat "$build/classpath.txt")" \
      geokernel.harness.ChainTiming "$@"
    ;;
  ggb)
    sample="${1:-1}"
    zipstep="${2:-50}"
    for file in "$here"/cases/*.json; do
      (
        if [ "${file##*/}" = cas.json ] && [ "${GGB_HARNESS_CAS:-0}" != 1 ]; then
          enable_giac
        fi
        java "${java_opts[@]}" -cp "$build/classes:$(cat "$build/classpath.txt")" \
          geokernel.harness.GgbExporter "$file" "$repo/golden/ggb" "$sample" "$zipstep"
      )
    done
    ;;
  saved)
    java "${java_opts[@]}" -cp "$build/classes:$(cat "$build/classpath.txt")" \
      geokernel.harness.SavedExporter "$here/saved/saved.json" "$repo/golden/ggb/saved" "$@"
    ;;
  tpairs)
    commit="$(git -C "$ggb" rev-parse HEAD)"
    java "${java_opts[@]}" -cp "$build/classes:$(cat "$build/classpath.txt")" \
      geokernel.harness.TPairs "$repo/golden/t-pairs/scripts.jsonl" \
      "$repo/golden/t-pairs/verified.jsonl" "$commit" "$@"
    ;;
  export-snapshot)
    exec java -Xmx512m -Djava.awt.headless=true -cp "$build/classes:$(cat "$build/classpath.txt")" \
      geokernel.harness.ExportSnapshot "$@"
    ;;
  reload)
    java "${java_opts[@]}" -cp "$build/classes:$(cat "$build/classpath.txt")" \
      geokernel.harness.Reload "$@"
    ;;
  text-canvas)
    java "${java_opts[@]}" -cp "$build/classes:$(cat "$build/classpath.txt")" \
      geokernel.harness.TextCanvas
    ;;
  cas-smoke)
    java "${java_opts[@]}" -cp "$build/classes:$(cat "$build/classpath.txt")" \
      geokernel.harness.GiacOracle
    ;;
  *)
    echo "usage: GEOGEBRA_SRC=<checkout> $0 build|golden|chain|ggb|saved|export-snapshot|reload|tpairs|text-canvas|cas-smoke" >&2
    exit 2
    ;;
esac
