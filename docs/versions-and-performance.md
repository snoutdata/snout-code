# Supported versions and performance

Which Gradle and Maven versions and which JDKs SnoutCode supports, what it shows for a build
outside them, and how long opening a project takes, with the scripts that check both.

## Supported versions

| | Supported | Notes |
|---|---|---|
| Gradle | 7.0 to 9.x | Through the build's wrapper. Gradle 7.0 to 7.2 run on Java 11 to 16 only, since they came before Java 17. |
| Maven | 3.6 to 3.9 | Through the build's wrapper (`mvnw`). A build without one runs on Apache Maven 3.9.16, which SnoutCode downloads. |
| Build JDK | 17, 21, 25 | The JDK the build's Gradle or Maven runs on and compiles with. 11 for Gradle 7.0 to 7.2. |

SnoutCode reads a Maven build itself, with Maven 3.9's model builder, whatever the wrapper's version; the wrapper's Maven runs the goals, tests and run configurations.

Each Gradle version runs on Java up to a certain version: 17 from Gradle 7.3, 21 from 8.5, 25 from 9.1. SnoutCode picks a JDK the build's Gradle runs on; see [The JDK each build runs on](gradle.md#the-jdk-each-build-runs-on).

### What was tested

Each cell is a small two module build (a library, an application that uses it, a JUnit 5 test, and code of the Java level: records and sealed types from 17, pattern switches from 21, a statement before `super()` from 25), opened through the Java engine. Every cell synced, had no errors in the editor, went from a call to its definition in the other module, completed a method, ran its tests through Gradle or Maven, and ran its main class.

| Gradle | JDK 11 | JDK 17 | JDK 21 | JDK 25 |
|---|---|---|---|---|
| 7.0.2 | works | | | |
| 7.6.6 | | works | | |
| 8.0.2 | | works | | |
| 8.5 | | | works | |
| 8.14.3 | | works | works | |
| 9.0.0 | | works | works | |
| 9.1.0 | | | | works |
| 9.8.2 | | works | works | works |

| Maven (wrapper) | JDK 17 | JDK 21 | JDK 25 |
|---|---|---|---|
| 3.6.3 | works | | |
| 3.8.8 | | works | |
| 3.9.16 | works | | works |

Outside the range:

| Version | Result |
|---|---|
| Gradle 6.9.4, JDK 11 | Everything worked; SnoutCode says the version is outside the range. |
| Maven 3.5.4, JDK 17 | The build synced and the editor worked, but its tests did not run: current Maven plugins need Maven 3.6.3. |
| Maven 4.0.0 RC 5, JDK 21 | Everything worked; SnoutCode says Maven 4 is not supported yet. |

### A build outside the range

A build on a Gradle version older than 7.0 or newer than 9.x, or with a Maven wrapper older than 3.6 or of Maven 4, still opens, and syncs if it can. SnoutCode says why it may not work in full:

- once, in the status line and in the Notifications tool window, for example "scala/legacy: Gradle 6.0.1 is older than SnoutCode supports (Gradle 7.0 to 9.x): it may sync, but parts of the build can be missing or wrong. Update the build's wrapper to Gradle 7 or newer.";
- in the Project view, where the build carries a warning badge and, opened, shows the same note above its files. A build that did not sync shows it under its reason.

The fix is in the build: update its wrapper (`./gradlew wrapper --gradle-version 8.14.3`, or `mvn wrapper:wrapper -Dmaven=3.9.16`).

### Check the range yourself

`scripts/compat-matrix.sh` runs the cells above, or the ones you name, and prints a line for each:

```sh
JDK11=/path/to/jdk11 JDK17=/path/to/jdk17 JDK21=/path/to/jdk21 JDK25=/path/to/jdk25 \
  scripts/compat-matrix.sh g-8.14.3-j21 m-3.9.16-j17
```

It writes everything under a folder of its own (`--out`): the sample builds, a Gradle user home, a Maven local repository and the Maven wrappers' downloads. Nothing goes to `~/.gradle` or `~/.m2`.

## Performance

### What happens when a project opens

1. **The window** opens before anything else, in under half a second.
2. **The build model.** On the first open the Java engine syncs every build in the project: Gradle configures each one (no task runs), and Maven builds are read in the engine. Builds of one Gradle version and JDK sync one after another on one daemon; up to four such groups run at once. When most builds are done and the last few are slow, to configure or to fail, the engine goes on without them, and they join the project when they are done. On a later open, when no build file changed, the engine takes the last model at once.
3. **The Java language server** (jdtls) starts and imports the model. On a later open it reads its workspace and search indexes as it saved them. Completion, navigation and search work from here.
4. **Scala.** A module whose Java uses Scala classes waits until the engine has compiled that Scala, through the build's own Gradle; its diagnostics wait with it. A Scala folder that several builds compile is compiled once, first, then the builds that use it, the open files' builds and the longest ones first. On a later open, Scala that did not change is not compiled again.
5. **Every module ready.** The status bar shows each stage as it goes.

### A large build, measured

A large private project: 21 Gradle builds side by side (Gradle 5.2 to 8.1, 13 of them sync, 8 fail), about 2,800 Java and 740 Scala files, 540 libraries, and one Java and Scala folder that 11 of the builds compile. Times in seconds from the app's start, on an Apple silicon Mac with 18 cores, Gradle's dependency caches present:

| Open | Phase | Before | After |
|---|---|---|---|
| Warm (reopened) | Java language server ready | 5.1 to 6.8 | 2.6 to 2.9 |
| | First diagnostics in the open file | 6.4 to 6.7 | 4.1 to 4.9 |
| Cold (no SnoutCode caches) | Build model | 64 | 17 to 21 |
| | Java language server ready | 67 | 19 to 22 |
| | First diagnostics in the open file | | 52 to 59 |
| | Every module ready | 118 | 65 to 69 |

What made the difference:

- On the first open, the app synced the builds a second time beside the engine's own sync, and the two took turns at Gradle's locks. Now the engine's sync is the only one.
- A tab restored at startup started an engine of its own for its build, beside the project's. Now every file of the project goes to the project's engine.
- Quitting stopped jdtls without saving its workspace, so every start refreshed every source folder to recover it. jdtls now saves its workspace however it is stopped.
- The model file is no longer written again when it did not change, so the app does not read it again.
- The Scala compile that runs first after a shared folder gets the daemon that compiled it, whose Scala compiler is loaded: the largest build there compiled in 11 to 13 seconds instead of 26.
- A build that fails to sync now says why: "no variant of the build plugin ... fits Gradle 6.0.1", or "Android Gradle plugin requires Java 11 to run", instead of a message about the Tooling API.

On a cold open, most of the time goes to compiling Scala through Gradle (about 21 seconds for the shared folder, then about 27 for the 11 builds that use it) and to the Gradle sync (13 to 21 seconds, depending on the builds that sync at the same time). A project without Scala is ready when the language server is.

### Measure it yourself

`scripts/bench-open.sh` opens a project cold, then warm, and prints the time to each phase:

```sh
scripts/bench-open.sh ~/src/shop --open app/src/main/java/shop/App.java
scripts/bench-open.sh ~/src/shop --app target/release/snout-code   # the real app
```

It keeps the caches, logs and reports in a folder of its own (`--out`), never in SnoutCode's caches; with `--app` the app runs with `HOME` pointed into that folder, so your settings and projects are left alone. Run it on a copy of a project you must not write to: an included build keeps its own `.gradle` folder. `--help` lists the options.

### Make it faster

- Open the folder with the builds you work on, rather than one that holds many more.
- A build that fails to sync costs its time on every first open: fix it (the Project view says why), or open a folder without it.
- Install the JDK the build asks for, so it does not wait for a download.
- Keep SnoutCode's caches (`~/Library/Caches/SnoutCode`): deleting them makes the next open a cold one.
