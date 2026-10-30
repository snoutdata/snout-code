#!/usr/bin/env bash
# Time opening a project: a cold open (no SnoutCode caches) and warm opens (the project
# reopened, its model, Scala classes and jdtls workspace cached), phase by phase.
#
#   scripts/bench-open.sh <project folder> [options]
#
#   --open <file>        a Java file to open, relative to the project (default: the first
#                        .java file found under a src/main/java)
#   --warm <n>           warm opens after the cold one (default 2)
#   --no-cold            only warm opens, on the caches of an earlier run (needs --out)
#   --java-home <dir>    the JDK for the builds' Gradle (default: as the engine chooses)
#   --gradle-user-home <dir>  another Gradle user home (default: Gradle's own, ~/.gradle)
#   --out <dir>          where the caches, logs and reports go (default: a new temporary folder)
#   --app <binary>       time the real app instead (target/release/snout-code), with HOME
#                        pointed at a folder under --out, so your own settings and caches are
#                        never read or written
#
# The engine is engine/build/dist (or SNOUT_ENGINE_DIST); the headless client is
# engine/lsp-driver, built here when missing (with ./gradlew --no-daemon, which needs a JDK 21
# in JAVA_HOME). Everything the open writes goes under --out: the model, Gradle's build
# directories and project caches, the jdtls workspace and the Gradle daemons' registry (the
# engine stops its daemons when it ends). Gradle's own caches stay where they are, so "cold" is
# cold for SnoutCode, with the dependencies already downloaded. Included builds keep their
# own .gradle folder: point the script at a copy of a project you must not write to.
#
# The phases, in seconds from the engine's start (from the app's start with --app):
#   model     the build model is there: synced, or taken from the cache
#   jdtls     jdtls is ready: completion, navigation and search work
#   diag      the first diagnostics of the open file
#   ready     every module is ready (the Scala bridge's classes read by jdtls)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
project="" open="" warm=2 cold=true java_home="" gradle_home="" out="" app=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --open) open="$2"; shift ;;
    --warm) warm="$2"; shift ;;
    --no-cold) cold=false ;;
    --java-home) java_home="$2"; shift ;;
    --gradle-user-home) gradle_home="$2"; shift ;;
    --out) out="$2"; shift ;;
    --app) app="$2"; shift ;;
    -h|--help) sed -n '2,30p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    -*) echo "Unknown option: $1" >&2; exit 2 ;;
    *) project="$1" ;;
  esac
  shift
done
[[ -n "$project" && -d "$project" ]] || { echo "usage: scripts/bench-open.sh <project folder> [options] (--help)" >&2; exit 2; }
project="$(cd "$project" && pwd -P)"
if [[ -z "$out" ]]; then
  $cold || { echo "--no-cold needs the --out of an earlier run" >&2; exit 2; }
  out="$(mktemp -d "${TMPDIR:-/tmp}/bench-open.XXXXXX")"
fi
mkdir -p "$out"
out="$(cd "$out" && pwd -P)"
if [[ -z "$open" ]]; then
  open="$(cd "$project" && find . -path '*/src/main/java/*.java' -not -path '*/build/*' -not -path '*/node_modules/*' 2>/dev/null | sort | head -1)"
  open="${open#./}"
fi

DIST="${SNOUT_ENGINE_DIST:-$ROOT/engine/build/dist}"
[[ -x "$DIST/bin/snout-engine" && -x "$DIST/runtime/bin/java" ]] || { echo "No engine at $DIST: run ./snout.sh once, or set SNOUT_ENGINE_DIST" >&2; exit 1; }
DRIVER="$ROOT/engine/lsp-driver/build/install/lsp-driver/bin/lsp-driver"
if [[ -z "$app" && ! -x "$DRIVER" ]]; then
  echo "Building the headless client (engine/lsp-driver)…" >&2
  (cd "$ROOT/engine" && ./gradlew --no-daemon --quiet :lsp-driver:installDist) >&2
fi

# The engine with each line of its log stamped with the seconds since it started.
cat >"$out/engine.sh" <<EOF
#!/usr/bin/env bash
exec "$DIST/bin/snout-engine" "\$@" 2> >(perl -MTime::HiRes=time -e '\$|=1; \$t0=time; while (<STDIN>) { printf STDERR "[%8.2f] %s", time-\$t0, \$_ }')
EOF
chmod +x "$out/engine.sh"

# The seconds of the first log line matching a pattern, or "-".
at() { local t; t="$(grep -m1 -E "$2" "$1" 2>/dev/null | sed -E 's/^\[ *([0-9.]+)\].*/\1/')"; echo "${t:--}"; }

engine_run() {
  local name="$1" log="$out/$1.log" report="$out/$1.json" script="$out/$1.script.json"
  local args=(--cache-dir "$out/cache")
  [[ -n "$java_home" ]] && args+=(--java-home "$java_home")
  [[ -n "$gradle_home" ]] && args+=(--gradle-user-home "$gradle_home")
  python3 -I - "$script" "$open" "${args[@]}" <<'PY'
import json, sys
path, open_file, args = sys.argv[1], sys.argv[2], sys.argv[3:]
json.dump({"engineArgs": args, "open": [open_file] if open_file else [], "checks": []}, open(path, "w"), indent=1)
PY
  rm -f "$log"
  JAVA_HOME="$DIST/runtime" "$DRIVER" --engine "$out/engine.sh" --project "$project" --script "$script" \
    --report "$report" --stderr "$log" >/dev/null 2>"$out/$name.driver.log" || {
      echo "$name: the run failed (see $out/$name.driver.log and $log)" >&2; return 1; }
  python3 -I - "$name" "$report" "$(at "$log" 'using its model|snout-engine: synced [0-9]+ of')" <<'PY'
import json, sys
name, report, model = sys.argv[1], sys.argv[2], sys.argv[3]
r = json.load(open(report))
s = lambda ms: "-" if ms is None or ms < 0 else "%.1f" % (ms / 1000)
diag = [v for v in (r.get("firstDiagnosticsMillis") or {}).values() if v is not None]
print("%-6s model %6s   jdtls %6s   diag %6s   ready %6s   errors %s" % (
    name, model, s(r.get("serviceReadyMillis")), s(min(diag)) if diag else "-",
    s(r.get("projectReadyMillis")), r.get("diagnostics", {}).get("errors")))
PY
}

app_run() {
  local name="$1" log="$out/$1.log" home="$out/home"
  mkdir -p "$home"
  if [[ -n "$java_home" ]]; then
    # Where the app looks for JDKs: JAVA_HOME (Linux), the user's JDK folder (macOS).
    local bundle="$java_home"; [[ "$bundle" == */Contents/Home ]] && bundle="${bundle%/Contents/Home}"
    mkdir -p "$home/Library/Java/JavaVirtualMachines"
    [[ -d "$bundle/Contents/Home" ]] && ln -sfn "$bundle" "$home/Library/Java/JavaVirtualMachines/bench.jdk"
  fi
  local config="$home/Library/Application Support/SnoutCode"
  [[ "$(uname -s)" == Darwin ]] || config="$home/.config/SnoutCode"
  if [[ ! -f "$config/settings.json" ]]; then
    mkdir -p "$config"
    # The project open, with the file in a tab, as a reopened project restores it.
    python3 -I - "$config/settings.json" "$project" "${open:+$project/$open}" <<'PY'
import json, sys
path, project, file = sys.argv[1], sys.argv[2], sys.argv[3]
tabs = [{"panel_name": "Editor", "children": [], "info": {"panel": {"path": file, "line": 0, "column": 0, "scroll": 0}}}] if file else []
center = {"panel_name": "StackPanel", "info": {"stack": {"sizes": [100.0], "axis": 0}},
          "children": [{"panel_name": "TabPanel", "children": tabs, "info": {"tabs": {"active_index": 0}}}]}
side = {"panel_name": "TabPanel", "children": [{"panel_name": "Side", "children": [], "info": {"panel": None}}], "info": {"tabs": {"active_index": 0}}}
json.dump({"repos": [project], "current": project, "activity": "Project",
           "dock": {"version": 2, "center": center, "left_dock": {"panel": side, "placement": "left", "size": 320.0, "open": True}}},
          open(path, "w"), indent=1)
PY
  fi
  env HOME="$home" SNOUT_STARTUP_TRACE=1 SNOUT_NO_FETCH=1 SNOUT_ENGINE_DIST="$DIST" \
    ${gradle_home:+"GRADLE_USER_HOME=$gradle_home"} ${java_home:+"JAVA_HOME=$java_home"} \
    "$app" "$project" >"$log" 2>&1 &
  local pid=$! i
  for ((i = 0; i < 1800; i++)); do
    grep -q 'every module ready' "$log" && break
    kill -0 "$pid" 2>/dev/null || break
    sleep 0.5
  done
  # A moment for the open file's diagnostics, then quit as ⌘Q does.
  for ((i = 0; i < 20; i++)); do grep -q '^startup .*first diagnostics' "$log" && break; sleep 0.5; done
  kill -TERM "$pid" 2>/dev/null; wait "$pid" 2>/dev/null || true
  local ms
  ms() { local t; t="$(grep '^startup ' "$log" | grep -m1 "$1" | awk '{print $2}')"; [[ -n "$t" ]] && awk -v t="$t" 'BEGIN { printf "%.1f", t / 1000 }' || echo -; }
  echo "$(printf '%-6s' "$name") window $(ms 'main window painted\|main window opened')   model $(ms 'project model read')   jdtls $(ms 'Java language server ready')   diag $(ms 'first diagnostics')   ready $(ms 'every module ready')"
}

echo "Project: $project${open:+ (opening $open)}"
echo "Output:  $out"
run() { if [[ -n "$app" ]]; then app_run "$@"; else engine_run "$@"; fi; }
if $cold; then
  rm -rf "$out/cache" "$out/home"
  run cold
fi
for ((n = 1; n <= warm; n++)); do
  run "warm$n"
done
