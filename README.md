# SnoutCode

**[snoutcode.com](https://snoutcode.com)** · **[Read the user guide](docs/README.md)**

A native IDE for Java, Gradle, Maven and Spring Boot, for TypeScript and JavaScript projects on Node, and for end-to-end tests written with Playwright in TypeScript, on macOS, Linux and Windows, written in Rust. It has Git and Claude Code built in, and can work on a Linux machine over SSH while the window stays on your desktop.

A Java project gets SnoutCode's Java engine. A `package.json` project opens as a Node project, and a repository of Playwright tests as a project of its own: both get TypeScript editing and the Node and packages they need (a Playwright project its browsers too), and never start Java. A repository with both gets each for its own files.

![SnoutCode on a Spring Boot web shop: the Project view, the editor and the Spring window's beans](docs/images/spring-beans.png)

![The debugger stopped at a breakpoint, with the values inline and the variables and watches below, in dark mode](docs/images/run-and-debug-debugger-dark.png)

## Features

Each item links to its page in the user guide.

- **[Projects](docs/projects.md):** open any folder as the project, with one Gradle or Maven build or many side by side. The Project view shows the modules as the build sees them, or the plain files. File ▸ New Project… makes a Java application or a Spring Boot application, with Gradle or Maven, or a Playwright project of end-to-end tests.
- **[Editor](docs/editor.md):** tabs and split editors, find and replace, Find and Replace in Files, Go to File, Search Everywhere and the command palette.
- **[Java](docs/java.md):** completion that adds imports, go to definition (into libraries too), find references, rename, quick fixes, organize imports, formatting, parameter hints and problems. Scala classes in the build are compiled so Java sees them.
- **[TypeScript and JavaScript](docs/typescript.md):** completion with auto imports, hover, go to definition, find references, rename, quick fixes and problems, from a TypeScript server on the project's own TypeScript. It runs on the Node the project asks for (`.nvmrc`, `.node-version`, `volta`, `engines`), found among nvm, Volta, fnm, asdf, mise and Homebrew installs, or downloaded from nodejs.org when none fits.
- **[Playwright projects](docs/playwright.md):** a tests-only repository that tests an application through its URL opens without Java. A bar installs its packages (npm, pnpm, Yarn or Bun, by its lockfile) and the browsers its Playwright version runs, with the output in the Build window. New projects test in Chromium, Firefox and WebKit and read the application's address from `BASE_URL`.
- **[Writing Playwright tests](docs/playwright.md#writing-tests):** Record at Cursor runs Playwright's recorder against the environment picked and writes its steps into the spec as you act in the browser: into the test the caret is in, as a new test, or as a whole spec. Pick Locator puts the locator of each element clicked at the caret. UI Mode and the Inspector are a click away.
- **[Running Playwright tests](docs/playwright.md#running-tests):** run all of them, a project, a file or the test at the caret (run icons in the gutter), or rerun the failed ones. The Tests window follows the run live: progress and time left, each worker's step, a grid of tests by browser with their last ten results and flaky rate, and a failure's error at its line, its screenshot comparison with Accept, its steps, and from its trace a filmstrip, failed requests and console errors. Environments set `BASE_URL` and variables per run; every run is kept, with what broke since the last one.
- **[Node projects](docs/node.md):** a `package.json` project opens without Java. Its scripts, and those of every package of a workspace or folder with its own `package.json`, are run configurations (npm, pnpm, Yarn or Bun, by its lockfile) with a run icon beside each in `package.json`; any script file runs as Current File, TypeScript through the project's `tsx` or Node's own type stripping. Debug stops at breakpoints in TypeScript or JavaScript in every Node process the run starts, and a Node configuration attaches to a Node started elsewhere with `--inspect`.
- **[Vitest and Jest](docs/node.md#unit-tests-vitest-and-jest):** run icons at each `test`, `it` and `describe`; runs followed live in the Tests window, with each failure's message and line, Rerun failed, and a switch between the packages that have tests; Debug stops at breakpoints in the test and the code it calls.
- **[Debugging Playwright tests](docs/playwright.md#debugging-tests):** breakpoints in specs, page objects and helpers, in TypeScript or JavaScript; the test stops there with its browser showing, its stack, variables and watches in the Debug window, and stepping as for Java. The debugger is js-debug, downloaded the first time.
- **[Gradle](docs/gradle.md):** sync, the Gradle window and its tasks, build problems, a JDK that suits each build's Gradle version, with a download offered when none is installed.
- **[Maven](docs/maven.md):** Maven builds with no Maven install: reactors, parent POMs, BOM imports and profiles, the Maven window with its phases, goals and profiles, tests through Surefire, and POM completion from the local repository.
- **[Run and debug](docs/run-and-debug.md):** run configurations, run icons in the gutter, tests with their results in a tree, and a debugger with breakpoints, stepping, variables and expressions.
- **[Spring](docs/spring.md):** beans and where they are injected, endpoints, the Spring window, and help in `application.properties` and `application.yml`.
- **[Git](docs/git.md):** changes, diffs, commit, branches, history, pull, fetch and push.
- **[Coding agents](docs/agents.md):** Claude Code, Codex, opencode, Amp or your own agent in a docked console; each edit typed in live with the agent's caret and colour, every line it wrote marked, and its changes reviewed by prompt.
- **[The API for agents](docs/editor-api.md):** an agent in the console works the whole window, opening files, switching projects, starting runs and reading their results, asking the language engines what the code is, using Git and showing you reports, through an Agent Skill, a [UTCP](https://www.utcp.io) manual and a plain HTTP API. What changes something asks you first, and the Agent Log lists every call.
- **[Terminal](docs/terminal.md):** your shell in the bottom dock, with scrollback, find and links, on the built-in terminal engine or Ghostty's.
- **[Remote SSH](docs/remote-ssh.md):** the project, Git, the Java engine, builds, runs and terminals on a Linux machine you reach with `ssh`.
- **[Processes](docs/processes.md):** what SnoutCode runs on your machine, and stopping it.
- **[Updates](docs/updates.md):** SnoutCode downloads a new version in the background and installs it when you restart or quit, from a feed it checks is signed.

The Java features come from SnoutCode's Java engine (`engine/`, see [engine/README.md](engine/README.md)). The macOS app, the Linux packages and the Windows installer carry the engine with its own Java runtime, so you need no Java installed to start.

The TypeScript features come from [vtsls](https://github.com/yioneko/vtsls), a language server around TypeScript's own `tsserver`. SnoutCode installs it on first use into its data folder with the project's Node, so neither Node nor vtsls needs to be installed beforehand.

## Linux

SnoutCode runs on Linux (x86_64 and aarch64, X11 and Wayland) and ships as a `.deb` for Debian and Ubuntu and as an Arch Linux package. The menus are drawn in the window's title bar, Ctrl takes the place of ⌘ (with IntelliJ IDEA's Linux keys where Ctrl was taken), and settings live in `~/.config/SnoutCode`. See [docs/linux.md](docs/linux.md).

## Windows

SnoutCode runs on Windows 10 and 11 (x64 and arm64) and ships as an installer that installs for the current user, with no administrator rights, and adds SnoutCode to the Start menu. The installer is not signed yet, so SmartScreen warns the first time. The menus and keys are the Linux ones, the Terminal tab runs PowerShell, and settings live in `%APPDATA%\SnoutCode`. See [docs/windows.md](docs/windows.md).

### Remote SSH

The window stays on your Mac while the project, Git, the Java engine, builds, runs, terminals and Claude Code are on a Linux machine you reach with `ssh`. SnoutCode installs its small server there by itself and reconnects when the link drops. See [docs/remote-ssh.md](docs/remote-ssh.md).

## Building

### Requirements

- **Rust 1.95 or newer.** Install it with [rustup](https://rustup.rs).
- **Git** on `PATH`. Push, pull and fetch use your normal Git credentials.
- **JDK 21**, to build the Java engine. `snout.sh` and `scripts/package-mac.sh` look for one (`SNOUT_JDK21`, then `JAVA_HOME` if it is 21, then `/usr/libexec/java_home` on a Mac or `/usr/lib/jvm` on Linux) and otherwise `snout.sh` downloads Temurin 21 to `~/Library/Caches/SnoutCode` (`~/.cache/SnoutCode` on Linux).
- **Docker**, to build the Linux server for Remote SSH (`scripts/build-server-linux.sh`) and the Linux packages (`scripts/package-linux.sh`).
- **On Linux**, a C toolchain and the libraries GPUI builds against; see [Build it from source](docs/linux.md#build-it-from-source).
- **On Windows**, the MSVC toolchain with the Windows SDK, and Inno Setup 6 for the installer; see [Build it from source](docs/windows.md#build-it-from-source).
- **Zig 0.15**, only for the Ghostty terminal engine (`brew install zig@0.15`; it is keg-only, and the scripts find it there). `scripts/package-mac.sh` needs it; `snout.sh` uses it when it finds it and otherwise builds with the built-in engine only; plain `cargo build` leaves it out.
- **Optional:** [Claude Code](https://docs.claude.com/en/docs/claude-code), for the console.

### Build and run

```sh
git clone https://github.com/snoutdata/snout-code.git
cd snout-code
./snout.sh                  # build what changed, then start SnoutCode
./snout.sh ~/projects/shop  # open a folder as the project
./snout.sh --fg             # stay in the foreground, with logs in the terminal
```

`snout.sh` builds the release binary (`target/release/snout-code`) and the Java engine when they are missing or older than their sources. When nothing changed it starts the app at once, without running cargo; when something did, it says "Building SnoutCode…" and shows cargo's progress before the app starts. In the background, logs go to `$TMPDIR/snout-code.log`. `SNOUT_STARTUP_TRACE=1 ./snout.sh` times the way to the first window (see [Troubleshooting](docs/troubleshooting.md#slow-start)).

### The Ghostty terminal engine

The terminals can run on Ghostty's terminal emulation (libghostty-vt) as well as the built-in one; see [docs/terminal.md](docs/terminal.md). It is the `ghostty` cargo feature of `snout-app`, off by default so that building and running need no Zig:

```sh
PATH="$(brew --prefix zig@0.15)/bin:$PATH" cargo build --release -p snout-app --features ghostty
cargo test --workspace --features snout-app/ghostty   # the terminal tests run on both engines
```

The first build fetches Ghostty's sources (a pinned commit) from GitHub and builds `libghostty-vt.a` with Zig. A build without the feature offers Ghostty in Settings ▸ Terminal as unavailable, and its terminals run on the built-in engine.

### macOS app

`scripts/package-mac.sh` builds `SnoutCode.app` and a disk image in `target/release/bundle/`, for this Mac's architecture, with the Ghostty terminal engine (`SNOUT_NO_GHOSTTY=1` leaves it out). Open the `.dmg` and drag SnoutCode into Applications.

Without signing settings the app is signed ad hoc: it opens as is on the Mac that built it, and a copy on another Mac has to be allowed once in System Settings > Privacy & Security. To sign with a Developer ID and notarize, set `SNOUT_APPLE_DIR` and `SNOUT_NOTARY_ISSUER` in the environment or in a gitignored `.signing.env`; the comment at the top of the script lists what the folder holds.

### Linux packages

`scripts/package-linux.sh` builds, in Docker, a `.deb` and an Arch Linux package for this machine's architecture (`x86_64`, `aarch64` or `all` as arguments) in `target/linux-packages/`, with the Java engine and its own Java 21 runtime. `scripts/test-linux.sh` runs clippy, the tests and the app under X11 and Wayland in the same image, and `scripts/linux/install-test.sh` installs the packages in clean Debian, Ubuntu and Arch containers and starts the app. The [Linux workflow](.github/workflows/linux.yml) does all three on GitHub for both architectures.

### Windows installer

`scripts/package-windows.ps1`, run on Windows, builds the installer (`SnoutCode-<version>-windows-<x64|arm64>-setup.exe`, made with Inno Setup) in `target\windows-package\`, with the Java engine and its own Java 21 runtime. It is unsigned for now; `-Sign` signs it with Azure Trusted Signing once that is set up. The [Windows workflow](.github/workflows/windows.yml) runs clippy, the tests and the app on a demo project, and builds, installs, starts and uninstalls the installer, for x64 and arm64.

## How it is built

```
crates/
  snout-app/           the SnoutCode app, on GPUI and GPUI Kit (binary: snout-code)
  snout-core/          Git operations, through the git CLI
  snout-lang/          the language server client (the Java engine, vtsls)
  snout-host/          the machine a workspace lives on: files, processes, terminals
  snout-remote/        the app's side of Remote SSH
  snout-remote-proto/  the wire protocol between the app and snout-server
  snout-server/        the remote side of Remote SSH
engine/                the Java engine: Gradle and Maven sync, Eclipse JDT LS, debugging, Spring
```

`snout-core` runs the `git` CLI instead of using a library, so authentication, hooks and settings such as `pull.rebase` behave exactly as they do in a terminal.

## Tests

```sh
cargo test --workspace
cargo clippy --workspace --all-targets
cargo fmt --all --check
```

The GUI tests render the app headlessly. Set `SNOUT_SCREENSHOT=/path/to/dir` to save PNGs of the renders. The engine has its own tests: `cd engine && ./gradlew build`.

The TypeScript tests run the real vtsls when the machine has Node, and say they are skipped when it has none. The Playwright runs in the tests go through a stand-in for Playwright's test runner (`tests/fixtures/playwright/fake-cli.js`) that drives SnoutCode's own reporter and plays a recorder, and a stand-in for its API (`fake-api.js`) for the locator picker, so they need Node but no browsers. A few tests need the network or a real installed project (a Node download from nodejs.org, a package install, the real Playwright listing a new project's tests, a real run of the project at `SNOUT_PW_PROJECT`); they are ignored by default and run with `cargo test -p snout-app --lib playwright_tests -- --ignored`. Debugging goes through a stand-in for js-debug (`fake-js-debug.js`); `node_tests -- --ignored` debugs an npm script, a TypeScript file and an attach under the real js-debug.

## The website

The site at [snoutcode.com](https://snoutcode.com) is in [`web/`](web/README.md): a static React + Vite site, prerendered at build time, with no backend.

## Contributing

Issues and pull requests are welcome. Please run the three commands under [Tests](#tests) before opening a pull request.

## License

[MIT](LICENSE), copyright (c) 2026 SNOUTDATA INC.
