# Getting started

Install SnoutCode, open your first project, and get Java support running.

## What you need

- A Mac, a Linux PC (x86_64 or aarch64, X11 or Wayland), or a Windows 10 or 11 PC (x64 or
  arm64). On Linux, install the .deb or the Arch Linux package as
  [SnoutCode on Linux](linux.md) describes; on Windows, run the installer as
  [SnoutCode on Windows](windows.md) describes. The rest of this page is the same there, with
  Ctrl for ⌘.
- Git on your `PATH`. A project is a folder inside a Git repository.
- Nothing else to start. The app carries its Java engine with a Java runtime of its own, so
  you do not need to install Java to open the app or to get Java support in the editor.

Your Gradle build runs on a JDK of its own, chosen to suit the build. When no installed JDK
fits, SnoutCode offers to download one (see [Java for your builds](#java-for-your-builds)).

## Install the app

On Linux, see [Install](linux.md#install) in SnoutCode on Linux; on Windows,
[Install](windows.md#install) in SnoutCode on Windows. On a Mac:

1. Open the SnoutCode disk image (`.dmg`).
2. Drag SnoutCode into Applications.
3. Open SnoutCode from Applications.

A copy that was signed only for the Mac that built it has to be allowed once on any other
Mac: open System Settings > Privacy & Security and allow SnoutCode there.

### Build it from source

To build the app and its disk image yourself, run this from a clone of the repository:

```sh
scripts/package-mac.sh
```

It puts `SnoutCode.app`, a `.dmg` and the `.zip` apps update from in `target/release/bundle/`,
for the architecture of the Mac that builds it. Building the Java engine needs JDK 21: the script uses `SNOUT_JDK21`, else
`JAVA_HOME` when it is 21, else one that `/usr/libexec/java_home` finds.

While you work on SnoutCode itself, `./snout.sh` builds the app and the engine when their
sources changed, and starts the app:

```sh
./snout.sh                     # reopen the project you had open last
./snout.sh ~/projects/shop     # open a folder as the project
./snout.sh --pick              # choose a folder with a dialog
./snout.sh --fg                # stay in the foreground, with the log in the terminal
```

When nothing changed since the last build, the app starts at once. When something did,
`snout.sh` prints "Building SnoutCode…" and cargo's progress, and starts the app once the build
is done: it never starts an older build than the sources. A SnoutCode already running carries on
while the new one builds.

When no JDK 21 is installed, `snout.sh` downloads one to build the engine with. The app starts
first and shows the engine build as a background task in the status bar.

## First launch

A small splash shows at once, and the window follows it a moment later. What takes longer
comes after the window, with its progress in the status bar: opening the project's
repository, the Java engine, reading the Gradle or Maven build, indexing. The first time, no
project is open, and the Welcome page shows:

- **Open Project…** opens a folder on this Mac.
- **Clone Repository…** copies a repository from a Git URL and opens it.
- **New Project…** makes a Java or Spring Boot project with Gradle or Maven.
- **Recent Projects** lists the projects you opened, with their branch and when you last
  opened them. Type in the search box to filter them.

![The Welcome page](images/getting-started-welcome.png)

*The Welcome page, with three recent projects.*

You can also drag a folder from Finder onto the window to open it.

After that, SnoutCode opens the project you had open last, with its editor tabs as you left
them. See [Projects](projects.md) for opening, switching and closing projects.

### The menus

On Linux and Windows the menus are in the window's title bar (see [The window](linux.md#the-window)
on Linux, [The window](windows.md#the-window) on Windows). On a
Mac, the menu bar holds every command: File, Edit, View, Code, Navigate, Refactor, Run,
Repository (Git), Window and Help. Each item shows an icon, an SF Symbol as macOS's own menus
use, so commands are quick to spot; its shortcut, when it has one, is at the right. Items that
cannot act now (Save with no editor open, Stage with nothing selected) are greyed out.

### Open files and folders from Finder

- **Open With**: right-click a file, choose **Open With** and then **SnoutCode**. To make it the
  app for that kind of file, choose **Get Info**, pick SnoutCode under **Open with** and click
  **Change All**. SnoutCode never takes a file type over by itself.
- **Open in SnoutCode**: right-click a file or a folder and choose it under **Quick Actions** or
  **Services**. If it is not listed, turn it on in System Settings under Keyboard, Keyboard
  Shortcuts, Services, Files and Folders.
- **The Dock**: drop files or a folder on the SnoutCode icon.

A file opens in a tab of the window with every panel collapsed, so only the editor shows, and the project open there stays. Open a panel again from the bars at the sides. When SnoutCode is not running, a file in a Git repository opens that repository as the project. A file in no repository, or several files at once, open on their own with no project behind them, so nothing else starts and a quick look at a text file stays quick. The next plain start brings your last project back. A folder always opens as the project.

A file that is not text shows a message in its tab instead of its contents.

### Open a project from the command line

The app's program takes folders and files, as Finder hands them over (above). Add `:line` or
`:line:column` to a file to put the caret there.

```sh
/Applications/SnoutCode.app/Contents/MacOS/snout-code ~/projects/shop
/Applications/SnoutCode.app/Contents/MacOS/snout-code ~/projects/shop/src/main/java/shop/App.java:42
```

## Quitting

Press ⌘Q (SnoutCode ▸ Quit SnoutCode), or close the window: closing the last window quits.
SnoutCode asks first:

![Quit SnoutCode?](images/getting-started-quit.png)

*Quitting with a file unsaved and a program running.*

- With unsaved files, it names them: **Save All and Quit** (⏎) saves them first, **Discard
  and Quit** (⌘⌫) drops the changes, **Cancel** (Esc) stays.
- It names what would stop: a program running or being debugged, a Gradle build or tests in
  progress, Claude Code editing a file, and on a remote machine the connection and what the
  project runs there.
- When nothing would be lost or stopped, tick **Don't ask again** to quit at once from then
  on. Settings ▸ Appearance ▸ **Confirm before quitting** turns the question back on. It is
  always asked when something would be lost or stopped.

The window goes the moment you quit. The Java engine, its language server and Gradle daemons,
and the programs and terminals SnoutCode started are stopped right after, in the background,
so you can open SnoutCode again at once. Each is asked to stop and usually has within a
second; one still there after 12 seconds is ended.

## Java support

Java support starts by itself for a Gradle project. While the Java engine starts, the status
bar, at the bottom right, shows what it is doing as a background task with a thin progress
bar. Click it to list everything running in the background. The editor works meanwhile.

Once the engine is done, the status bar shows `Java: Ready`. Click it to show the Problems
tab. See [Java](java.md) for what the engine gives you.

## Java for your builds

SnoutCode runs each Gradle build on a JDK that suits it: the Java level its build scripts ask
for, within what its Gradle version can run on. It looks for JDKs in these places:

- the JDKs `/usr/libexec/java_home` lists
- `~/Library/Java/JavaVirtualMachines` and `/Library/Java/JavaVirtualMachines`
- `~/.sdkman/candidates/java`
- `~/.gradle/jdks`
- `~/Library/Application Support/SnoutCode/jdks`, where SnoutCode puts the JDKs it downloads

On Linux it looks in `JAVA_HOME`, the `java` on your `PATH` and `/usr/lib/jvm` instead of the
first two, and keeps its downloads in `~/.local/share/SnoutCode/jdks` (see
[SnoutCode on Linux](linux.md#where-snoutcode-keeps-its-files)). On Windows it looks in
`JAVA_HOME`, the `java` on your `PATH` and the JDK installers' folders under `Program Files`,
and keeps its downloads in `%LOCALAPPDATA%\SnoutCode\jdks` (see
[SnoutCode on Windows](windows.md#java-gradle-and-maven)).

When none fits, a bar above the editor says so: "This project needs JDK 17. Download it?"

- Click **Download** to download Eclipse Temurin of that version into SnoutCode's JDK folder. The bar shows the progress. The Java engine
  starts once the download is done, as does any other build waiting for the same JDK.
- Click **Not now** to hide the bar. It does not offer that version again until SnoutCode
  starts again.

If the download fails, the bar says why and offers **Try again**.

Temurin publishes no Java 8 for Apple silicon, so on such a Mac a build that asks for Java 8
gets Java 11, which builds and runs Java 8 code.

To choose the JDK of a build yourself, see [Settings](settings-and-keymap.md#build-tools--gradle).

## Next steps

- [Projects](projects.md): open, switch and close projects, and find your way in them.
- [Editor](editor.md): tabs, search and navigation.
- [Run and debug](run-and-debug.md): run your application and its tests.
- [Keyboard shortcuts](keyboard-shortcuts.md): every default shortcut.
- [Troubleshooting](troubleshooting.md): when something does not work.
