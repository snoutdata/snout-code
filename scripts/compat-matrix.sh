#!/usr/bin/env bash
# Check the Gradle and Maven versions and build JDKs SnoutCode supports: for each cell, a small
# sample build (scripts/compat/sample.py) opened through snout-engine with the headless client
# (engine/lsp-driver): the sync, jdtls with no errors, a definition and a completion across
# modules, the tests run through the build tool, and the main class run. One line per cell.
#
#   JDK11=… JDK17=… JDK21=… JDK25=… scripts/compat-matrix.sh [--out <dir>] [cell…]
#
# The JDK variables name JDK homes; a cell whose JDK is not given is skipped. Cells are
# `g-<gradle version>-j<java>` or `m-<maven version>-j<java>` (default: the list below, the
# range docs/supported-versions.md states and a few versions outside it). Everything goes under
# --out (default: a new temporary folder): the samples, a Gradle user home, a Maven local
# repository, the Maven wrappers' downloads and HOME for the Maven runs, so nothing is written
# to ~/.gradle or ~/.m2. The engine is engine/build/dist (or SNOUT_ENGINE_DIST).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out=""
cells=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --out) out="$2"; shift ;;
    -h|--help) sed -n '2,14p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) cells+=("$1") ;;
  esac
  shift
done
[[ ${#cells[@]} -gt 0 ]] || cells=(
  g-7.0.2-j11 g-7.6.6-j17 g-8.0.2-j17 g-8.5-j21 g-8.14.3-j17 g-8.14.3-j21
  g-9.0.0-j17 g-9.0.0-j21 g-9.1.0-j25 g-9.8.2-j17 g-9.8.2-j21 g-9.8.2-j25
  m-3.6.3-j17 m-3.8.8-j21 m-3.9.16-j17 m-3.9.16-j25
  g-6.9.4-j11 m-3.5.4-j17 m-4.0.0-rc-5-j21
)
[[ -n "$out" ]] || out="$(mktemp -d "${TMPDIR:-/tmp}/compat-matrix.XXXXXX")"
mkdir -p "$out"
out="$(cd "$out" && pwd -P)"
DIST="${SNOUT_ENGINE_DIST:-$ROOT/engine/build/dist}"
[[ -x "$DIST/bin/snout-engine" ]] || { echo "No engine at $DIST: run ./snout.sh once, or set SNOUT_ENGINE_DIST" >&2; exit 1; }
DRIVER="$ROOT/engine/lsp-driver/build/install/lsp-driver/bin/lsp-driver"
if [[ ! -x "$DRIVER" ]]; then
  (cd "$ROOT/engine" && ./gradlew --no-daemon --quiet :lsp-driver:installDist) >&2
fi
home="$out/home"
mkdir -p "$home"
maven_env=(HOME="$home" MAVEN_USER_HOME="$home/.m2" MAVEN_OPTS="-Duser.home=$home -Dmaven.repo.local=$out/m2")

# The Maven wrapper's scripts (only-script: mvnw downloads the Maven its properties name),
# made once with a Maven of our own.
wrapper() {
  [[ -x "$out/wrapper/mvnw" ]] && return 0
  local v=3.9.16 base=https://repo.maven.apache.org/maven2/org/apache/maven/apache-maven
  mkdir -p "$out/tools" "$out/wrapper"
  if [[ ! -x "$out/tools/apache-maven-$v/bin/mvn" ]]; then
    curl -fsSL -o "$out/tools/maven.tgz" "$base/$v/apache-maven-$v-bin.tar.gz"
    curl -fsSL -o "$out/tools/maven.tgz.sha512" "$base/$v/apache-maven-$v-bin.tar.gz.sha512"
    local want; want="$(cut -d' ' -f1 <"$out/tools/maven.tgz.sha512")"
    [[ "$(shasum -a 512 "$out/tools/maven.tgz" | cut -d' ' -f1)" == "$want" ]] || { echo "Maven's checksum does not match" >&2; return 1; }
    tar -xzf "$out/tools/maven.tgz" -C "$out/tools"
  fi
  printf '<project><modelVersion>4.0.0</modelVersion><groupId>x</groupId><artifactId>x</artifactId><version>1</version><packaging>pom</packaging></project>\n' >"$out/wrapper/pom.xml"
  (cd "$out/wrapper" && env "${maven_env[@]}" JAVA_HOME="$1" "$out/tools/apache-maven-$v/bin/mvn" -B -ntp -q -N \
    org.apache.maven.plugins:maven-wrapper-plugin:3.3.2:wrapper -Dtype=only-script -Dmaven=$v)
}

echo "Output: $out"
for cell in "${cells[@]}"; do
  if [[ ! "$cell" =~ ^([gm])-(.+)-j([0-9]+)$ ]]; then echo "$cell: not a cell (g-<version>-j<java>, m-<version>-j<java>)" >&2; continue; fi
  tool=gradle; [[ ${BASH_REMATCH[1]} == m ]] && tool=maven
  version="${BASH_REMATCH[2]}" level="${BASH_REMATCH[3]}"
  jdk_var="JDK$level"; jdk="${!jdk_var:-}"
  [[ -d "$jdk/Contents/Home" ]] && jdk="$jdk/Contents/Home"
  if [[ -z "$jdk" || ! -x "$jdk/bin/java" ]]; then printf '%-28s skipped: no %s\n' "$cell" "$jdk_var"; continue; fi
  dir="$out/cells/$cell"
  rm -rf "$dir"; mkdir -p "$dir"
  python3 -I "$ROOT/scripts/compat/sample.py" "$dir/sample" "$tool" "$version" "$level"
  if [[ $tool == maven ]]; then
    wrapper "$jdk"
    cp "$out/wrapper/mvnw" "$dir/sample/"
    mkdir -p "$dir/sample/.mvn/wrapper"
    printf 'wrapperVersion=3.3.2\ndistributionType=only-script\ndistributionUrl=https://repo.maven.apache.org/maven2/org/apache/maven/apache-maven/%s/apache-maven-%s-bin.zip\n' \
      "$version" "$version" >"$dir/sample/.mvn/wrapper/maven-wrapper.properties"
  fi
  python3 -I - "$dir/script.json" "$jdk" "$dir/cache" "$out/gradle" "$home" "$out/m2" <<'PY'
import json, sys
path, jdk, cache, gradle, home, m2 = sys.argv[1:]
app = "app/src/main/java/demo/app/App.java"
json.dump({
    "engineArgs": ["--java-home", jdk, "--cache-dir", cache, "--gradle-user-home", gradle, "--daemon-idle-minutes", "1"],
    "env": {"HOME": home, "MAVEN_USER_HOME": home + "/.m2", "MAVEN_OPTS": "-Duser.home=%s -Dmaven.repo.local=%s" % (home, m2)},
    "open": [app, "app/src/test/java/demo/app/AppTest.java"],
    "checks": [
        {"name": "definition", "kind": "definition", "file": app, "find": "greeter.greet", "offset": 9,
         "expect": "lib/src/main/java/demo/lib/Greeter.java"},
        {"name": "completion", "kind": "completion", "file": app, "find": "greeter.greet", "offset": 8, "expect": "greet"},
        {"name": "tests", "kind": "runTests", "module": "sample:app", "filter": {"kind": "all"}},
        {"name": "run", "kind": "run", "mainClass": "demo.app.App", "expect": "Hello, SnoutCode!"},
    ],
}, open(path, "w"), indent=1)
PY
  (cd "$dir" && JAVA_HOME="$DIST/runtime" "$DRIVER" --engine "$DIST/bin/snout-engine" --project "$dir/sample" \
    --script script.json --report report.json --stderr engine.log >/dev/null 2>driver.log) || true
  python3 -I "$ROOT/scripts/compat/summary.py" "$cell" "$dir"
done
