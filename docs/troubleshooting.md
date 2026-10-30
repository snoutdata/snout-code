# Troubleshooting

What to check when Java support, a Gradle build or the app itself does not work.

## See what the Java engine is doing

The status bar, at the bottom right, shows the Java engine of the project:

- While it works, its stage shows as a background task with a progress bar. Click it to list
  everything running in the background, with each module's stage.
- When it is done, it shows `Java:` and its state. Click it to show the Problems tab.

| Status | What it means |
|---|---|
| Java: Importing Gradle model | The engine reads the Gradle builds. |
| Java: Compiling Scala | The engine compiles the Scala the Java code uses. |
| Java: Starting, Java: Indexing | The Java language server starts and reads the project. |
| Java: Ready | Java support works. |
| Java: Needs JDK 17 | No installed JDK can run the build. See [No JDK fits the build](#no-jdk-fits-the-build). |
| Java: Restarting | The engine stopped unexpectedly and starts again. |
| Java: Stopped | The engine was stopped from the [Processes](processes.md) window. |
| Java: The Java engine keeps stopping | It stopped five times in a row; SnoutCode no longer starts it by itself. |
| Java: Waiting for snout-engine | A development build of the engine is being built. |

When the engine is stopped, or SnoutCode gave up on it, **Restart** shows beside its status.
You can also run **Restart the Java Engine** from the command palette (⇧⌘P) at any time; it
starts the engine again, and the engine reads the Gradle model as it starts.

When the engine stops unexpectedly, SnoutCode starts it again at once, then after longer and
longer waits, up to 30 seconds. After five stops in a row it gives up and offers Restart.

## Java support unavailable

The status bar says "Java support unavailable" when SnoutCode cannot find its Java engine.
Click it to see why.

- The app from the disk image carries the engine. If you see this with it, install the app
  again.
- A build from source needs the engine built beside it. Run `./snout.sh` again: it builds the
  engine with JDK 21, and downloads one when none is installed (or set `SNOUT_JDK21` to a JDK
  21 folder). Then start SnoutCode again.
- `SNOUT_ENGINE_DIST` set in the environment points SnoutCode at another engine build.

The rest of SnoutCode works without the engine.

## No JDK fits the build

The Java engine itself runs on its own Java, but each Gradle build runs on a JDK of your Mac.
When none of the installed JDKs fits, a bar above the editor says "This project needs JDK 17.
Download it?".

- Click **Download** to download Eclipse Temurin into
  `~/Library/Application Support/SnoutCode/jdks`. The engine starts once it is there.
- If the download fails, the bar says why. Click **Try again**.
- **Not now** hides the bar until SnoutCode starts again.

To install a JDK yourself instead, put it where SnoutCode looks (see
[Getting started](getting-started.md#java-for-your-builds)), then open the project again.

## The Gradle version needs an older JDK

Each Gradle version runs on Java up to a certain version. A build whose Gradle is older than
its JDK fails to sync, often with an error about a class file it cannot read. SnoutCode picks a
JDK the build's Gradle runs on, at least as new as the Java level its scripts ask for:

| Gradle | Newest Java it runs on |
|---|---|
| 9.1 | 25 |
| 8.14 | 24 |
| 8.10 | 23 |
| 8.8 | 22 |
| 8.5 | 21 |
| 8.3 | 20 |
| 7.6 | 19 |
| 7.5 | 18 |
| 7.3 | 17 |
| 7.0 | 16 |
| 6.7 | 15 |
| 6.3 | 14 |
| 6.0 | 13 |
| 5.4 | 12 |
| 5.0 | 11 |

When no installed JDK fits, SnoutCode offers the one the build asks for, or else the newest
long term support release its Gradle runs on, not newer than 21. For example, a build on
Gradle 8.1 gets Java 17.

To choose the JDK of a build yourself, open Settings ▸ Build Tools ▸ Gradle and pick it under
**JDK per project** (see [Settings](settings-and-keymap.md#build-tools--gradle)). Upgrading the
build's Gradle wrapper also lets it run on a newer JDK.

## A build did not sync

When a Gradle sync fails, a bar above the editor says "The Gradle sync failed:" and why, with
**Try again** and **Not now**. In the Project view, a build that did not sync shows its Gradle
version and the reason; hover over it for the whole error. A build that synced before keeps
its last model, marked with a warning, so its modules stay. See [Gradle](gradle.md).

When a build script changes, the bar says "Gradle files changed. Sync?". Click **Sync**.

## Errors from Git and the tools

The Notifications tool window, on the right stripe, lists the errors from Git and the tools.

## Slow start

The splash should show within a fraction of a second, and the window right after it; the
project, the engine and the build tools' sync follow in the status bar. To see where a slow
start spends its time, start SnoutCode with `SNOUT_STARTUP_TRACE=1`:

```sh
SNOUT_STARTUP_TRACE=1 ./snout.sh --fg
SNOUT_STARTUP_TRACE=1 /Applications/SnoutCode.app/Contents/MacOS/snout-code
```

Each step prints how long after the start it was reached: `snout.sh` lines for the script
(checking the engine and Zig, building, starting the app), then `startup` lines from the
app, counted from the moment the process started: the splash opened and shown, the main
window opened and painted, the menus. A line saying the UI thread was busy marks work that
held up the window.

- The first start of a new build takes longer, around half a second more: macOS checks a
  program it has not run before.
- When `snout.sh` says "Building SnoutCode…", the sources changed since the last build (or
  another build changed the binary), and cargo builds before the app starts. If cargo says it
  is waiting for a lock, another build in the same folder runs; it goes on when that is done.

## Logs

SnoutCode writes its log to standard error. To read it:

- Start the app from a terminal, and the log shows there:

  ```sh
  /Applications/SnoutCode.app/Contents/MacOS/snout-code
  ```

- A build from source started with `./snout.sh` writes its log to `$TMPDIR/snout-code.log`.
  With `./snout.sh --fg` the log shows in the terminal. When it builds the engine, that
  build's output goes to `$TMPDIR/snout-engine-build.<number>.log`.

Lines about the Java engine start with `[java]`.

The Java engine keeps its files under `~/Library/Caches/SnoutCode`, one folder per project:

| Path | What |
|---|---|
| `jdtls/<hash>/model.json` | The Gradle model the engine read |
| `jdtls/<hash>/workspace/` | The Java language server's workspace, with its log |
| `daemons/<hash>/` | SnoutCode's own Gradle daemons and their logs |
| `build/<hash>/` | The build output of tasks and tests the engine runs |

## Start from a clean state

- **The engine's caches.** Quit SnoutCode and delete `~/Library/Caches/SnoutCode`. The next
  start reads the Gradle builds again.
- **Settings.** Your settings, recent projects and tabs are in
  `~/Library/Application Support/SnoutCode/settings.json`. A file SnoutCode cannot read is
  copied to `settings.json.unreadable`, and SnoutCode starts from the defaults. See
  [Settings](settings-and-keymap.md#where-settings-are-kept).

## Ghostty unavailable

Settings ▸ Tools ▸ Terminal shows **Ghostty is unavailable**, with its card dimmed, in a build
of SnoutCode made without the Ghostty terminal engine. Release builds (the `.dmg`) include it;
`./snout.sh` includes it when it finds Zig 0.15, and a plain `cargo build` only with the
`ghostty` feature. Terminals then run on the built-in engine, also when `settings.json` says
`"terminal_engine": "Ghostty"`.

To build with Ghostty, install Zig 0.15 (`brew install zig@0.15`) and build with the feature:

```sh
PATH="$(brew --prefix zig@0.15)/bin:$PATH" cargo build --release -p snout-app --features ghostty
```

The first such build fetches Ghostty's sources from GitHub, so it needs the network. See
[Building](../README.md#building).

## The app does not open on another Mac

A copy of SnoutCode that was signed only for the Mac that built it is blocked on other Macs.
Open System Settings > Privacy & Security and allow SnoutCode there once.
