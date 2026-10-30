#!/usr/bin/env bash
# Sync the corpus (engine/corpus/projects.txt): real public Gradle and Maven builds, each pinned
# to a commit. For each project: a shallow fetch of that commit, then the project opened through
# snout-engine with the headless client (engine/lsp-driver), which waits for the sync, for jdtls
# to import the model and for a build of the whole workspace. One line per project, then a table.
#
#   JDK11=… JDK17=… JDK21=… JDK25=… scripts/corpus.sh [options] [name…]
#
#   --out <dir>        where everything goes (default: a new temporary folder)
#   --list <file>      another project list (default: engine/corpus/projects.txt)
#   --timeout <min>    the longest one project may take (default 50; needs `timeout`)
#   --keep             keep each project's sources and caches (default: only the logs, the
#                      model, the report and the result stay)
#
# This runs third-party builds: their Gradle scripts, plugins and Maven extensions. It is meant
# for CI (.github/workflows/corpus.yml), not for a developer's machine.
#
# The JDK variables name JDK homes: a project's JDK (the list's jdk column) is the engine's
# --java-home, and the others are offered with --jdk. Under --out: src/<name> (the sources),
# projects/<name>/ (the engine's cache, the logs and result.json), a Gradle user home, a Maven
# local repository and HOME for Maven, so nothing goes to ~/.gradle or ~/.m2. The results are
# results.md (a table) and results.json. The engine is engine/build/dist (or SNOUT_ENGINE_DIST);
# the client is built when missing (or SNOUT_LSP_DRIVER).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPORT="$ROOT/scripts/corpus/report.py"
out="" list="$ROOT/engine/corpus/projects.txt" limit=50 keep=false
names=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --out) out="$2"; shift ;;
    --list) list="$2"; shift ;;
    --timeout) limit="$2"; shift ;;
    --keep) keep=true ;;
    -h|--help) sed -n '2,24p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    -*) echo "Unknown option: $1" >&2; exit 2 ;;
    *) names+=("$1") ;;
  esac
  shift
done
selected="$(python3 -I "$REPORT" select "$list" ${names[@]+"${names[@]}"})"
[[ -n "$out" ]] || out="$(mktemp -d "${TMPDIR:-/tmp}/corpus.XXXXXX")"
mkdir -p "$out"
out="$(cd "$out" && pwd -P)"

DIST="${SNOUT_ENGINE_DIST:-$ROOT/engine/build/dist}"
[[ -x "$DIST/bin/snout-engine" && -x "$DIST/runtime/bin/java" ]] || { echo "No engine at $DIST: run ./snout.sh once, or set SNOUT_ENGINE_DIST" >&2; exit 1; }
DRIVER="${SNOUT_LSP_DRIVER:-$ROOT/engine/lsp-driver/build/install/lsp-driver/bin/lsp-driver}"
if [[ ! -x "$DRIVER" ]]; then
  (cd "$ROOT/engine" && ./gradlew --no-daemon --quiet :lsp-driver:installDist) >&2
fi
timer=()
if command -v timeout >/dev/null 2>&1; then timer=(timeout --kill-after=60 "${limit}m")
elif command -v gtimeout >/dev/null 2>&1; then timer=(gtimeout --kill-after=60 "${limit}m")
else echo "No timeout command: a project may run as long as it takes" >&2; fi

home="$out/home"
mkdir -p "$home" "$out/gradle" "$out/m2"
jdks=()
for level in 11 17 21 25; do
  var="JDK$level"; jdk="${!var:-}"
  [[ -d "$jdk/Contents/Home" ]] && jdk="$jdk/Contents/Home"
  [[ -n "$jdk" && -x "$jdk/bin/java" ]] && jdks+=("$level=$jdk")
done
[[ ${#jdks[@]} -gt 0 ]] || echo "No JDK11, JDK17, JDK21 or JDK25: the engine chooses among the JDKs it finds" >&2

echo "Output: $out"
results=()
while IFS=$'\t' read -r -u 3 name repo commit level tool shape; do
  dir="$out/projects/$name" src="$out/src/$name"
  rm -rf "$dir" "$src"; mkdir -p "$dir"
  start=$SECONDS hint=ran
  if ! { git init -q "$src" && git -C "$src" remote add origin "$repo" \
      && git -C "$src" -c protocol.version=2 fetch -q --depth 1 --no-tags origin "$commit" \
      && git -C "$src" -c advice.detachedHead=false checkout -q FETCH_HEAD; } >"$dir/fetch.log" 2>&1; then
    hint=fetch-failed
  else
    args=(--cache-dir "$dir/cache" --gradle-user-home "$out/gradle"
          --daemon-idle-minutes 1)
    for j in ${jdks[@]+"${jdks[@]}"}; do
      if [[ "${j%%=*}" == "$level" ]]; then args+=(--java-home "${j#*=}"); else args+=(--jdk "${j#*=}"); fi
    done
    python3 -I - "$dir/script.json" "$home" "$out/m2" "${args[@]}" <<'PY'
import json, sys
path, home, m2, args = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4:]
json.dump({"engineArgs": args,
           "env": {"HOME": home, "MAVEN_USER_HOME": home + "/.m2",
                   "MAVEN_OPTS": "-Duser.home=%s -Dmaven.repo.local=%s" % (home, m2)},
           "open": [], "checks": []}, open(path, "w"), indent=1)
PY
    status=0
    (cd "$dir" && JAVA_HOME="$DIST/runtime" ${timer[@]+"${timer[@]}"} "$DRIVER" --engine "$DIST/bin/snout-engine" \
      --project "$src" --script script.json --report report.json --stderr engine.log </dev/null >/dev/null 2>driver.log) || status=$?
    if [[ $status -eq 124 || $status -eq 137 ]] && [[ ${#timer[@]} -gt 0 ]]; then
      hint="timeout:$limit"
      # The engine stops itself once the client is gone; this stops what is left of it now.
      pkill -f -- "$src" 2>/dev/null || true
    fi
  fi
  model="$(ls "$dir"/cache/jdtls/*/model.json 2>/dev/null | head -1 || true)"
  [[ -n "$model" ]] && cp "$model" "$dir/model.json"
  python3 -I "$REPORT" result "$dir" "$hint" "$((SECONDS - start))" "$name" "$repo" "$commit" "$level" "$tool" "$shape"
  results+=("$dir/result.json")
  $keep || rm -rf "$src" "$dir/cache"
done 3<<<"$selected"

python3 -I "$REPORT" table "$out/results.md" "$out/results.json" "${results[@]}"
echo "Results: $out/results.md, $out/results.json"
