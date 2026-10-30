#!/usr/bin/env bash
# Start SnoutCode with no display of its own and check that it works: under Xvfb (X11) or
# sway's headless backend (Wayland), on a small Maven project, with a scratch home and no Java
# on PATH. Runs in a container (scripts/test-linux.sh, scripts/linux/install-test.sh); as root,
# it runs the app as a user of its own, as a desktop would (sway refuses root).
#
#   smoke.sh [--x11|--wayland] [--bin <snout-code>] [--out <dir>] [--jdk <home>] [--wait <s>]
#
# Without --jdk the machine has no JDK: the app must open the project and offer to download
# one for the build. With --jdk that JDK is where SnoutCode keeps the JDKs it downloads (as if
# its Download button had fetched it), and the Java engine must start on the project. Under X11
# it also opens the File menu of the in-window menu bar and the Settings. Screenshots and the
# app's log go to <out>.
set -euo pipefail

MODE=x11
BIN=snout-code
OUT=/tmp/snout-smoke
JDK=""
WAIT=180
ARGS=("$@")
while [[ $# -gt 0 ]]; do
  case "$1" in
    --x11) MODE=x11 ;;
    --wayland) MODE=wayland ;;
    --bin) BIN="$2"; shift ;;
    --out) OUT="$2"; shift ;;
    --jdk) JDK="$2"; shift ;;
    --wait) WAIT="$2"; shift ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
  shift
done
mkdir -p "$OUT"
OUT="$(cd "$OUT" && pwd)"

if [[ $(id -u) == 0 ]]; then
  id smoke >/dev/null 2>&1 || useradd -m smoke
  chmod 777 "$OUT"
  exec su smoke -s /bin/bash -c "$(printf '%q ' "$0" "${ARGS[@]}")"
fi

TAG="$MODE${JDK:+-jdk}"
LOG="$OUT/snout-code-$TAG.log"

# A scratch home with no Java: neither JAVA_HOME nor java on PATH.
export HOME="$(mktemp -d /tmp/snout-home.XXXXXX)"
export XDG_RUNTIME_DIR="$HOME/run"
mkdir -p -m 700 "$XDG_RUNTIME_DIR"
unset JAVA_HOME
PATH="$(echo "$PATH" | tr ':' '\n' | grep -v -e jdk -e java | paste -sd: -)"
export PATH
if [[ -n "$JDK" ]]; then
  mkdir -p "$HOME/.local/share/SnoutCode/jdks"
  ln -s "$JDK" "$HOME/.local/share/SnoutCode/jdks/jdk-21"
fi

# A Maven project with one class, which the app opens at it.
PROJECT="$HOME/hello"
mkdir -p "$PROJECT/src/main/java/demo"
cat >"$PROJECT/pom.xml" <<'POM'
<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0">
  <modelVersion>4.0.0</modelVersion>
  <groupId>demo</groupId>
  <artifactId>hello</artifactId>
  <version>1.0</version>
  <properties>
    <maven.compiler.release>21</maven.compiler.release>
  </properties>
</project>
POM
FILE="$PROJECT/src/main/java/demo/Hello.java"
cat >"$FILE" <<'JAVA'
package demo;

public class Hello {
    public static void main(String[] args) {
        System.out.println("Hello from SnoutCode on Linux");
    }
}
JAVA
(cd "$PROJECT" && git init -q && git add -A && git -c user.email=t@example.com -c user.name=T commit -qm init) || true

pids=()
cleanup() {
  for p in "${pids[@]}"; do kill "$p" 2>/dev/null || true; done
  pkill -u "$(id -u)" -f 'com.snoutcode.engine.cli.SnoutEngine' 2>/dev/null || true
}
trap cleanup EXIT

shot() {
  if [[ $MODE == x11 ]]; then import -window root "$OUT/$1.png"; else grim "$OUT/$1.png"; fi
  echo "screenshot $OUT/$1.png"
}
fail() {
  echo "FAIL ($TAG): $1"
  grep -v 'UI thread was busy' "$LOG" | tail -40
  shot "failed-$TAG" || true
  exit 1
}

if [[ $MODE == x11 ]]; then
  export DISPLAY=:$((90 + RANDOM % 9))
  Xvfb "$DISPLAY" -screen 0 1600x1000x24 -nolisten tcp >/dev/null 2>&1 &
  pids+=($!)
  for _ in $(seq 50); do xdpyinfo >/dev/null 2>&1 && break; sleep 0.1; done
  unset WAYLAND_DISPLAY
  # Mesa's software Vulkan presents nothing to Xvfb's windows (they stay black): draw with
  # OpenGL there. A desktop's X server and GPU drivers need no such thing.
  export VK_ICD_FILENAMES=/nonexistent
else
  printf 'output HEADLESS-1 resolution 1600x1000\ndefault_border none\nxwayland disable\n' >"$HOME/sway.conf"
  WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 WLR_RENDERER=pixman sway -c "$HOME/sway.conf" \
    >"$OUT/sway-$TAG.log" 2>&1 &
  pids+=($!)
  WAYLAND_DISPLAY=""
  for _ in $(seq 100); do
    WAYLAND_DISPLAY="$(ls "$XDG_RUNTIME_DIR" 2>/dev/null | grep -m1 '^wayland-[0-9]*$' || true)"
    [[ -n "$WAYLAND_DISPLAY" ]] && break
    sleep 0.1
  done
  [[ -n "$WAYLAND_DISPLAY" ]] || { echo "sway did not start:"; cat "$OUT/sway-$TAG.log"; exit 1; }
  export WAYLAND_DISPLAY
  unset DISPLAY
fi

echo "== SnoutCode on $MODE${JDK:+ with a JDK}: $(command -v "$BIN" || echo "$BIN")"
SNOUT_LOG=warn SNOUT_WINDOW_ORIGIN=0,0 SNOUT_NO_FETCH=1 SNOUT_STARTUP_TRACE=1 "$BIN" "$FILE" >"$LOG" 2>&1 &
app=$!
pids+=($app)
deadline=$((SECONDS + WAIT))
wait_for() {
  until grep -q "$1" "$LOG"; do
    kill -0 "$app" 2>/dev/null || fail "the app exited"
    ((SECONDS < deadline)) || fail "$2"
    sleep 0.5
  done
}

wait_for 'main window painted' "no window after $WAIT s"
echo "window painted ($(grep 'main window painted' "$LOG" | awk '{print $2}') ms after start)"
wait_for '\[project\] read' "the project was not read"
grep '\[project\] read' "$LOG"

if [[ -z "$JDK" ]]; then
  wait_for '\[java\] no JDK fits' "no JDK download was offered"
  grep '\[java\] no JDK fits' "$LOG"
  sleep 2
  shot "no-jdk-$TAG"
else
  wait_for '\[java\] started the engine' "the Java engine did not start in $WAIT s"
  grep '\[java\] started the engine' "$LOG"
  engine="$(pgrep -u "$(id -u)" -af 'com.snoutcode.engine.cli.SnoutEngine' | head -1)" || true
  [[ -n "$engine" ]] || fail "no engine process"
  echo "engine: ${engine%% -cp *}"
  for _ in $(seq 90); do grep -q '\[java\] readiness: .*[Rr]eady\|\[java\] first diagnostics' "$LOG" && break; sleep 1; done
  grep '\[java\] readiness\|\[java\] first diagnostics' "$LOG" | tail -2 || true
  sleep 3
  shot "main-$TAG"
  if [[ $MODE == x11 ]]; then
    # The window is at the screen's top left; File is the first menu of its title bar.
    xdotool mousemove 31 18 click 1
    sleep 1.5
    shot "menu-$TAG"
    xdotool key Escape
    sleep 0.5
    xdotool key ctrl+comma
    sleep 2
    shot "settings-$TAG"
    xdotool key Escape
  fi
fi

kill -0 "$app" 2>/dev/null || fail "the app exited"
echo "PASS ($TAG): SnoutCode opened $PROJECT and $([[ -n "$JDK" ]] && echo 'started the Java engine' || echo 'offered to download a JDK')"
