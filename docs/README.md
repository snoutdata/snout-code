# SnoutCode user guide

SnoutCode is a native IDE for Java, Gradle, Maven and Spring Boot on macOS, Linux and Windows, with Git and coding agents (Claude Code, Codex, your own) built in. These pages show how to use each part of it.

## Start here

- [Getting started](getting-started.md): install SnoutCode, the first launch, and the Java it needs (none, to start).
- [SnoutCode on Linux](linux.md): the .deb and the Arch Linux package, the menus in the window, the Linux keys, and where settings live.
- [SnoutCode on Windows](windows.md): the installer and the SmartScreen warning, the menus in the window, the keys, the terminals, and where settings live.
- [Projects](projects.md): open, close and reopen projects, the Project view, and the New Project wizard.
- [Settings and keymap](settings-and-keymap.md): every setting, a project's own settings (`.snout/settings.json`), settings as JSON with completion and checks, changing shortcuts, and what SnoutCode restores when it starts.
- [Keyboard shortcuts](keyboard-shortcuts.md): every shortcut, by area.
- [Tool windows and menus](tool-windows.md): the tool windows around the editor, their headers and empty states, and the Build and Tools menus.

## Writing code

- [Editor](editor.md): tabs, editing, finding and replacing text across files, finding files, Search Everywhere and the command palette.
- [Java](java.md): completion, navigation, find references, rename, quick fixes, formatting, problems, and the engine's readiness.
- [Spring](spring.md): beans and injections, endpoints, the Spring window, and `application.yml` and `application.properties` help.

## Building and running

- [Gradle](gradle.md): syncing builds, the Gradle window, tasks, build problems, JDKs and Gradle daemons.
- [Maven](maven.md): Maven builds without a Maven install: reactors, parents, BOMs and profiles, the Maven window, goals, tests, and POM editing.
- [Project Structure](project-structure.md): the project's JDK, modules, source sets, dependencies and their conflicts, the JDKs on your machine, and the facets SnoutCode found.
- [Run and debug](run-and-debug.md): run configurations, the Run window, tests, and the debugger.
- [Processes](processes.md): what SnoutCode runs on your machine, and how to stop it.

## Working with others and with agents

- [Git](git.md): the Git view, its panes and dialogs.
- [Annotate with Git Blame](blame.md): who changed each line and when, beside it; a line's commit, earlier revisions, and who wrote each class and method.
- [Coding agents](agents.md): Claude Code, Codex or your own agent in the console; watching it write in its colour, sending it the selection, who wrote which lines, and reviewing its changes by prompt in the Agent Changes window.
- [Terminal](terminal.md): the Terminal tab and the console as terminals: keys, scrollback, find, links, and the terminal engine (Built-in or Ghostty).
- [Remote SSH](remote-ssh.md): keep the window on your Mac and work on a Linux machine over SSH.

## When something goes wrong

- [Troubleshooting](troubleshooting.md): logs, the Java engine's status, and common problems, such as Ghostty unavailable.
