# SnoutCode on Windows

SnoutCode runs on Windows as it does on a Mac or a Linux PC: the same window, tool windows,
Java engine and Git panel. This page covers installing it, and what is different on Windows:
the menus in the window, the keys, the terminals, and where SnoutCode keeps its files.

## What you need

- Windows 10 (version 1809 or later) or Windows 11, on x64 or arm64.
- Git for Windows, for the Git panel (`git` on your `PATH`).
- Nothing else. The installer carries the Java engine with a Java runtime of its own, so you do
  not need Java installed to start SnoutCode or to get Java support in the editor. Your builds
  get a JDK as on a Mac (see [Java for your builds](getting-started.md#java-for-your-builds)).

## Install

Download `SnoutCode-<version>-windows-x64-setup.exe` (or `-arm64-setup.exe` for a Windows on
Arm PC) and run it.

### The SmartScreen warning

The installer is not signed yet, so Windows does not know who made it and Microsoft Defender
SmartScreen stops it the first time: "Windows protected your PC". Click **More info**, check
that the file is the one you downloaded, then click **Run anyway**. Some browsers ask too, and
keep the download only when you choose **Keep**. A signed installer will come later; it will
start without the warning.

### What the installer does

By default it installs for you alone, with no administrator rights, in
`%LOCALAPPDATA%\Programs\SnoutCode`. Its first page offers to install for all users instead,
in `Program Files`, which asks for administrator rights. It adds SnoutCode to the Start menu,
and on request a desktop shortcut.

| Where | What |
|---|---|
| `snout-code.exe` | the program |
| `engine\` | the Java engine, with its Java runtime in `runtime\` |
| `server\` | the Linux servers Remote SSH installs on remote machines |
| `unins000.exe` | the uninstaller |

To remove SnoutCode, use **Settings ▸ Apps ▸ Installed apps ▸ SnoutCode ▸ Uninstall** (or Add
or Remove Programs). Your settings and the JDKs SnoutCode downloaded stay; delete the folders in
[Where SnoutCode keeps its files](#where-snoutcode-keeps-its-files) to remove them too.

For a silent install, as a script or a deployment tool would run it:

```bat
SnoutCode-1.0.0-windows-x64-setup.exe /VERYSILENT /SUPPRESSMSGBOXES /CURRENTUSER
"%LOCALAPPDATA%\Programs\SnoutCode\unins000.exe" /VERYSILENT /SUPPRESSMSGBOXES
```

`/ALLUSERS` in place of `/CURRENTUSER` installs for every user (as an administrator).

### Open a project from the command line

```bat
"%LOCALAPPDATA%\Programs\SnoutCode\snout-code.exe" C:\projects\shop
"%LOCALAPPDATA%\Programs\SnoutCode\snout-code.exe" C:\projects\shop\src\main\java\shop\App.java:42
```

What SnoutCode writes to its log goes to the command prompt it was started from.

## The window

Windows has no menu bar shared by every app, so SnoutCode draws its menus in the window's
title bar, left of the project's name: File, Edit, View, Code, Navigate, Refactor, Run, Build,
Git, Tools, Window and Help. They are the macOS menus, item for item, and each item shows its
shortcut. **Settings…** and **Exit** are at the end of File, and **About SnoutCode** at the
end of Help.

The minimize, maximize and close buttons are at the right of the title bar, and Windows's snap
layouts open from the maximize button. Drag the title bar to move the window and double-click
it to maximize. Closing the window quits, asking first as [Quitting](getting-started.md#quitting)
describes; the Java engine, its language server, Gradle and the programs you run stop with
it, even when SnoutCode ends some other way.

**Show in Explorer** (the Mac's Reveal in Finder) opens the folder in File Explorer with the
file selected. **Move to Trash** moves files to the Recycle Bin. A file on a network share has
no Recycle Bin, so deleting it there is for good, as in Explorer.

## Keys

The keys are the Linux ones: Ctrl takes the place of ⌘ (⌘S is Ctrl+S, ⇧⌘F is Ctrl+Shift+F),
and where Ctrl and a key already meant something else, SnoutCode follows IntelliJ IDEA's keymap
for Windows. The table in [SnoutCode on Linux](linux.md#keys) lists those: Run is Shift+F10,
Debug Shift+F9, Go to Class… Ctrl+N, New File… Alt+Insert, Show Project Alt+1, Redo Ctrl+Y.
Shortcuts show the Windows way, as "Ctrl+Shift+F".

On keyboard layouts with an AltGr key, Windows reads AltGr as Ctrl+Alt: a shortcut with
Ctrl+Alt and a key that types a character with AltGr (Ctrl+Alt+L on a Polish layout, say) can
reach SnoutCode instead of typing. Give that command other keys in Settings ▸ Keymap.

In the Terminal tab and the agent console, the Ctrl keys go to the shell, and the terminal's
own commands take Shift as well: Ctrl+Shift+C copies, Ctrl+Shift+V pastes, Ctrl+Shift+F finds.

## The terminal

The Terminal tab runs PowerShell (PowerShell 7 when it is installed, else Windows PowerShell),
on Windows's pseudo console. Settings ▸ Tools ▸ Terminal ▸ **Shell** takes another one:
`cmd.exe`, Git for Windows's `C:\Program Files\Git\bin\bash.exe`, or `wsl.exe` for a Linux
shell. The terminal engine is SnoutCode's built-in one.

**Open in External Terminal** opens Windows Terminal at the folder, or where it is not
installed PowerShell in a console window of its own. Settings ▸ Tools ▸ Terminal ▸ **Open in**
lists Windows Terminal, PowerShell, Command Prompt, WezTerm and Alacritty, or a command of your
own with `{dir}` in it, which the Command Prompt runs.

## Java, Gradle and Maven

SnoutCode looks for JDKs in `JAVA_HOME`, the `java` on your `PATH`, the folders the JDK
installers use under `Program Files` (Java, Eclipse Adoptium, Microsoft, Zulu, Amazon
Corretto, BellSoft, Semeru), `%USERPROFILE%\.gradle\jdks`, SDKMAN, and its own folder below.
A JDK it downloads comes from Eclipse Temurin, for your PC's architecture.

Gradle builds run as on a Mac, through Gradle itself; a Maven build uses the project's
`mvnw.cmd` when it has one. Projects SnoutCode creates get both `gradlew` and `gradlew.bat`.

The Java engine and the debugger take connections only from this PC (the loopback address), so
Windows Firewall does not ask whether Java may use the network when you run or debug.

## Remote SSH

[Remote SSH](remote-ssh.md) uses the OpenSSH client that comes with Windows (`ssh.exe`;
Settings ▸ System ▸ Optional features ▸ OpenSSH Client, if it is missing) with your
`%USERPROFILE%\.ssh\config` and keys, and the ssh agent's keys when its service runs. Windows's
OpenSSH cannot share one connection between several ssh commands, so each of SnoutCode's
connects on its own: a password or passphrase may be asked for more than once. Keys in the
agent avoid that.

## Claude Code

Claude Code finds SnoutCode in `%USERPROFILE%\.claude\ide` as it does on a Mac, and its hooks
report each edit through `curl`, which Windows has. Claude Code on Windows runs its hooks with
Git for Windows's bash, so install Git for Windows (you need it for the Git panel anyway).

## Where SnoutCode keeps its files

| What | Where |
|---|---|
| Settings (`settings.json`), keymap, layout, recent projects | `%APPDATA%\SnoutCode` |
| The JDKs SnoutCode downloads | `%LOCALAPPDATA%\SnoutCode\jdks` |
| The Java engine's caches: builds, models, jdtls data | `%LOCALAPPDATA%\SnoutCode\cache` |

Settings are in the roaming profile, so they follow you to other PCs of a domain; what can be
downloaded or made again stays on this one.

## Build it from source

Install Rust (with the MSVC toolchain and the Windows SDK, as the Visual Studio Build Tools
give them), Git, a JDK 21 and Inno Setup 6, then in PowerShell:

```powershell
scripts\package-windows.ps1
```

It builds `snout-code.exe` and the Java engine with its jlinked Java 21 runtime, and writes the
installer to `target\windows-package\`. `-ServerDist target\snout-server-dist` puts in the Linux
servers for Remote SSH, which `scripts/build-server-linux.sh` builds (with Docker). The
[Windows workflow](../.github/workflows/windows.yml) runs clippy, the tests and the app on a
demo project, builds the installers for x64 and arm64, and installs each one silently, opens a
project in the installed app (which must start its Java engine), then uninstalls it, checking
that nothing but the user's settings is left.

## When something goes wrong

- **The window does not open**: start SnoutCode from a command prompt with
  `set SNOUT_LOG=info` first, and read what it says about the graphics adapter. SnoutCode draws
  with Direct3D 11; on a PC without a working GPU driver it draws with Windows's Microsoft
  Basic Render Driver, more slowly.
- **A shortcut does nothing**: another program (a clipboard manager, a screen recorder) may
  hold it system wide. Give the command other keys in Settings ▸ Keymap.
- See [Troubleshooting](troubleshooting.md) for the logs and the Java engine's status.
