# snout-engine

The JVM side of SnoutCode. Three layers:

- **Maven sync**: a Maven build (a reactor) read in this process with Maven's own model builder and resolver into the same model, so no Maven installation is needed to import one (see [Maven](#maven)).
- **Gradle sync**: one Tooling API call per Gradle build that returns the whole build model as JSON, so the editor knows the source roots, classpaths, compiler settings and run targets of a Java, Scala, Kotlin or Groovy project without running a build. A project is any folder: one Gradle build, or many side by side, synced together into one model.
- **Java semantics**: `snout-engine lsp` runs Eclipse JDT LS (jdtls) as a language server, fed by that model instead of jdtls's own Gradle import (Buildship).
- **Running and debugging**: the same LSP stream carries the engine's own `snout/` methods: Gradle tasks, tests, run configurations, launching and debugging (through java-debug in jdtls), and recompiling Scala as it changes.
- **Spring**: a second jdtls bundle reads the Spring model from JDT's ASTs and bindings (beans, injection points, endpoints, configuration properties), warns about missing and ambiguous beans, and gives the `application*.properties` and `application*.yml` files completion, hover, definition and validation.

## Layout

- `sync-plugin`: model builders that run inside the user's Gradle daemon. Compiled to Java 8 bytecode, and version-sensitive Gradle APIs are reached through reflection, so one jar serves Gradle 5 through 8. The `BuildAction` that walks the build lives here too, because the Tooling API ships it into the daemon.
- `sync-cli`: the command line, `snout-engine`: the Tooling API client, the Scala bridge and the `lsp` launcher.
- `jdtls-importer`: an OSGi bundle for jdtls that builds JDT projects from the model.
- `spring`: an OSGi bundle for jdtls with the Spring support (see [Spring](#spring)). Its resolver, properties and YAML documents, configuration metadata and XML bean files are plain Java with their own tests.
- `junit-listener`: `snout-junit-listener.jar`, a JUnit Platform listener that Maven test runs load to send their test events to the engine (Java 8 bytecode).
- `lsp-driver`: a headless LSP client. It runs the end-to-end test and can check a real project.

## Build

Needs JDK 21: jdtls runs on it, and the importer compiles against jdtls. The wrapper downloads Gradle, and the build downloads the pinned jdtls release from download.eclipse.org into `jdtls-importer/build/jdtls`, and the pinned java-debug bundle from Maven Central into `jdtls-importer/build/downloads` (each checked against its SHA-256; never committed).

The end-to-end test also runs the fixture's Gradle on Java 17, so the JDT projects get a JDK other than jdtls's. Gradle finds a local JDK 17 (or one given with `-Porg.gradle.java.installations.paths=<dir>`) and otherwise downloads one through the Foojay toolchain resolver.

```sh
cd engine
./gradlew build          # compiles, runs the unit, integration and end-to-end tests
./gradlew installDist    # puts the CLI, jdtls and the bundle in sync-cli/build/install/snout-engine
```

## Run

```sh
sync-cli/build/install/snout-engine/bin/snout-engine sync /path/to/folder --out model.json
```

The folder is the project. It may be one Gradle build or hold many:

- A folder that is a build, or lies inside one, is that build: the nearest folder at or above it with a `settings.gradle(.kts)`, else the nearest with a `build.gradle(.kts)`. The search upward stops below the user's home.
- Any other folder holds every build under it: each folder with a settings script, or with a build script and no settings script above it. A build owns everything below it (subprojects, included builds, `buildSrc`), so the search does not descend into one. It skips `build`, `.gradle`, `node_modules`, `out`, `target` and hidden folders, and stops 8 levels down.

Each build syncs on its own Gradle daemon (one of the engine's, see [Gradle daemons](#gradle-daemons)), several at once, and on a JDK its Gradle version runs on, chosen from the JDKs found on the machine (`JAVA_HOME`, this JVM, the macOS and Linux JDK folders, SDKMAN, `~/.jdks`, Gradle's provisioned toolchains, the JDKs the app downloads to `~/Library/Application Support/SnoutCode/jdks`, the engine's build JDK in `~/Library/Caches/SnoutCode/build-jdk`, and the `jdks` and `build-jdk` folders of `--cache-dir`) and those given with `--jdk`:

- Gradle older than 6.7 gets Java 11.
- Newer Gradle gets the Java its build script names (a toolchain, `jvmToolchain`, a source or target compatibility, a `--release`) when that is newer than 17, else 17.
- Never newer than the Java the Gradle version supports (Gradle's compatibility matrix: 21 from Gradle 8.5, 17 from 7.3, 16 from 7.0, 15 from 6.7, 11 from 5.0), nor older (Java 8, and 17 from Gradle 9); when the wanted Java is missing, the nearest installed one it supports.
- When no installed JDK runs the build's Gradle version, the build is never synced on one that cannot work: its `failedBuilds` entry says `needsJdk`, the Java major to install (the newest LTS its Gradle runs on, not newer than it wants: 17 for Gradle 8.1.1, 11 for 7.0.2 and for anything before 6.7), with the reason "Needs JDK 17, which isn't installed". The engine's methods refuse such a build the same way. A sync that fails because its Gradle cannot run on the JDK it got after all (a class file it cannot read, Groovy failing inside Gradle) gets a `needsJdk` too.
- `--java-home` is preferred whenever the build's Gradle runs on it and the script asks for nothing newer.

A build that fails does not fail the others: its entry in the model carries the error, the Gradle version and a short reason.

| Option | Meaning |
| --- | --- |
| `--out <file>` | Write the model to a file instead of stdout. |
| `--java-home <dir>` | Preferred JDK for the Gradle daemons, as above. |
| `--jdk <dir>` | Another JDK to choose from; repeatable. |
| `--parallel <n>` | How many builds sync at once (default: half the processors, at most 4). |
| `--project-cache-dir <dir>` | Keep Gradle's `.gradle` project caches out of the builds, one folder per build under this one. |
| `--gradle-user-home <dir>` | Use another Gradle user home (default `~/.gradle`). |
| `--gradle-version <v>` | Use this Gradle version for every build instead of their wrappers. |
| `--no-sources` | Skip the source jar lookup. |
| `--offline` | Run Gradle offline (and Maven builds' resolution). |
| `--maven-profiles <list>` | Maven profiles to turn on, and with `!` off, in every Maven build: `prod,!dev`. |
| `--verbose` | Show Gradle's output, and a line per build, on stderr. |
| `--daemon-idle-minutes <n>` | Stop the engine's Gradle daemons after this many minutes without a build (default 15, see [Gradle daemons](#gradle-daemons)). |

The command prints a one line summary on stderr and exits with 1 if a build failed to sync; the model is written all the same.

A sync only configures the builds. No task runs, so nothing is compiled and no `build/` directory appears.

## Java semantics: `snout-engine lsp`

```sh
snout-engine lsp /path/to/folder --java-home /path/to/jdk17
```

The command speaks LSP on stdin and stdout, so the app spawns it and talks to it like any language server. It:

1. takes the model of the last sync when the builds did not change since (see [Starting from the cache](#starting-from-the-cache)), else syncs the folder's builds as `sync` does, with every build directory moved out of the project (an init script sets each project's `layout.buildDirectory`) and Gradle's project caches with them;
2. writes the model where the importer reads it, with the class folders of the **Scala bridge** in it (empty until compiled), and starts jdtls with the importer bundle, the Spring bundle and java-debug's;
3. meanwhile runs the Scala bridge, in the background: jdtls cannot read Scala source, so for each source set with Scala files it runs that source set's Scala compile task, and nothing else, several builds at once, the builds of the files open in the editor first. A Scala folder that several builds compile is compiled once (see [Shared Scala, compiled once](#shared-scala-compiled-once)). The task compiles the Java of a joint source set too, so the bridge keeps a mirror with only the classes whose `SourceFile` is a `.scala` file, hard linked and grouped by the source root that holds the file, and records them in the model. As each compile finishes, jdtls reads its classes and builds what uses them, and the modules it serves are ready (see [Readiness](#readiness));
4. relays LSP between the app and jdtls, answering the `snout/` methods itself (see [The engine's methods](#the-engines-methods)).

All of it lives under the cache directory (`~/Library/Caches/SnoutCode` on macOS), keyed by a hash of the folder:

| Path | Contents |
| --- | --- |
| `build/<hash>/` | Gradle build directories: what tests and tasks compile, and the bridge's Scala of builds that share no Scala folder |
| `bridge/<hash>/` | The bridge's build directories for builds that compile a shared Scala folder, and the shared folders' own compiles under `shared/` |
| `gradle/<hash>/<build>-<hash>/` | Gradle's project cache of each build |
| `jdtls/<hash>/model.json` | The model the importer reads |
| `jdtls/<hash>/model-inputs.json` | What the model was synced from, to tell whether the next start can take it |
| `jdtls/<hash>/scala-bridge.json` | The fingerprint of each Scala compile's inputs at its last good compile |
| `jdtls/<hash>/config/` | jdtls's OSGi configuration, with the bundle added |
| `jdtls/<hash>/workspace/` | jdtls's workspace: the JDT projects, their output folders, indexes and log |
| `jdtls/<hash>/runs/` | Argument files of the processes the engine launches, while they run |
| `daemons/<hash>/` | The registry and logs of the engine's own Gradle daemons (see [Gradle daemons](#gradle-daemons)) |

A build whose sync fails keeps its entry from the last model that synced it, marked `stale`, so a broken build file does not cost its modules. A build that never synced has no modules; the importer still claims the folder, so jdtls's Gradle importer never runs Buildship on it.

| Option | Meaning |
| --- | --- |
| `--java-home <dir>`, `--jdk <dir>`, `--parallel <n>` | As for `sync`. jdtls's Java 21 is too new for Gradle before 8.5. |
| `--cache-dir <dir>` | Use another cache directory. |
| `--jdtls-java-home <dir>` | Java 21 or newer for jdtls. Defaults to the JVM running snout-engine. |
| `--jdtls <dir>`, `--bundle <jar>` | Use another jdtls installation or importer bundle. |
| `--debug-bundle <jar>` | Use another java-debug bundle. |
| `--no-debugger` | Do not load java-debug into jdtls. |
| `--spring-bundle <jar>` | Use another Spring bundle. |
| `--no-spring` | No Spring support: the bundle is not loaded and the `snout/spring*` methods fail. |
| `--no-scala-bridge` | Do not compile Scala. |
| `--max-heap <size>` | jdtls's heap limit, or `auto` (the default): 512 MB, plus 150 MB per thousand Java and Scala source files and 0.5 MB per library, rounded up to 256 MB, from 1 GB to 8 GB (at most a quarter of the memory). The parallel collector, unless a `--jvm-arg` names another. |
| `--jvm-arg <arg>` | An extra JVM argument for jdtls; repeatable. |
| `--gradle-user-home`, `--gradle-version`, `--no-sources`, `--offline`, `--verbose`, `--daemon-idle-minutes` | As for `sync`. |

### Gradle daemons

The engine runs Gradle on daemons of its own, never on one the user started from a terminal, and stops them when it ends:

- **Private.** Each folder has a daemon registry of its own (`daemons/<hash>/` under the cache directory, set through the Tooling API's connector), and a Gradle client looks for daemons only in its registry. So `./gradlew` in a terminal never runs on the engine's daemons, nor the engine on the user's, and `./gradlew --stop` leaves the engine's alone. Nothing else about a daemon changes: it runs on the JDK chosen for its build (above), with the build's `org.gradle.jvmargs` and the shared Gradle user home and caches.
- **Stopped on exit.** When the engine ends (the app shuts it down, closes its input, quits or dies, or a signal), it stops the daemons of its registry: each gets SIGTERM, which a daemon answers in well under a second, and those still running after 3 seconds are killed. The daemons are found by their logs (`<Gradle version>/daemon-<pid>.out.log`), and a process counts only when it runs Gradle's daemon and started before its log was last written (a daemon writes to it as it stops, so a later process that got its pid does not count). `GradleConnector.disconnect()` cannot do this: it reaches only connections still open (the engine closes each after its build), and does nothing on Gradle before 6.8.
- **Short lived.** The engine's daemons stop after 15 minutes without a build (or as many as `--daemon-idle-minutes` says; the app's Settings ▸ Gradle sets it), not Gradle's 3 hours, so they go even when the engine is killed outright. A daemon that dies (killed, or out of memory) is simply replaced: the next build starts a fresh one in the registry.

`snout-engine sync` uses the same registry and idle time, but leaves its daemons running, so that `lsp` on the same folder starts on warm daemons.

### Starting from the cache

The model is kept with a record of what it was synced from: the scripts the sync saw each build load (settings, build and applied scripts, version catalogs, the wrapper's properties), each build's and included build's settings and build scripts and `gradle.properties` (whether they exist or not), its `buildSrc` folder, the Gradle user home's `gradle.properties` and init scripts, the builds the folder holds with each one's Gradle version and JDK, and the engine's options and sync plugin. Files count by content, `buildSrc` by its files' sizes and times. When nothing of it changed, the start skips the sync and takes the model as it is; the stderr line says `the builds did not change since the last sync`.

What a sync may give differently with the same inputs is synced again in the background, once the Scala bridge is done: a build that failed for a reason that may pass (the network, a daemon), dynamic or snapshot versions, libraries no longer on disk. When their model changed, the engine writes it, imports the projects into jdtls again (`snout.reimport`, a command of the importer bundle), compiles their Scala and sends `snout/modelChanged`.

jdtls's workspace (`jdtls/<hash>/workspace`) persists too, with its search indexes: a start reads them, and writes none again unless their libraries or sources changed.

Only one jdtls can hold a workspace. When an engine dies outright (killed, or crashed with the app), its jdtls can outlive it with the workspace's lock, so the next engine of the folder stops it before it starts its own: a jdtls on the same workspace (`-data` on its command line) whose engine is gone, or whose engine the same app is replacing (after five seconds for it to end by itself). One that another app's engine runs is left alone. When jdtls dies, the engine ends too (with jdtls's exit code), and the app starts it again.

### Shared Scala, compiled once

Independent builds often compile one folder of Java and Scala into each of them (services that list a `core/src/main/java` in their Scala source sets). JDT needs one copy of its classes, owned by the one JDT project the importer makes for the folder (see [Source folders that several projects compile](#source-folders-that-several-projects-compile)), so the bridge compiles it once, as a **unit**:

- A unit is the Scala source folders that the same compiles share, for builds on one Scala version (`2.13`, `3`), on Gradle 6.1 or newer.
- It compiles in one of its builds, the representative (the user with the newest Java, then the longest classpath), as a task an init script registers there, made from that build's Scala compile: its compiler, options and JDK. Its classpath is the representative's, with each library another user has in a newer version, or that the representative lacks, put in: the union, as the importer gives the folder's JDT project. Java and Scala in it compile jointly, so each sees the other.
- Each user then compiles only its own Scala: the init script leaves the shared folders out of its Scala compile, and puts the unit's classes on the classpath of its compile tasks (but the Java compile of the same source set, which runs before the Scala and never saw them). Its own Java and Scala still compile jointly.
- These builds compile into the bridge's own build directories (`bridge/<hash>/`), so a test or task run, which compiles the folder into the build as its script says, never mixes with them.
- The unit's classes go to jdtls once, as the shared JDT project's library; the users' own mirrors leave the shared folders out.
- A unit that fails to compile before it ever worked (a folder that needs something of each build) falls back to compiling the folder in each build, and does so on later starts while its inputs stay the same.

Each unit and build has a fingerprint of its inputs: its sources' paths, sizes and times, its build's model (classpath, compiler settings), and the units it uses. A compile whose fingerprint is the one of its last good compile does not run; when nothing changed, a start runs no Gradle for Scala at all.

### Readiness

The engine tells the app how far each module is with `snout/readiness` notifications (see [Notifications to the app](#notifications-to-the-app)), one per change, and the whole project's as `module: null`. A module is:

- `syncing`: only the project as a whole, while the Gradle sync runs;
- `compilingScala`: its Scala, or a shared folder it compiles, is compiling for jdtls; its percent moves with the time the last compile took;
- `importing`: jdtls is importing it, or reading its new Scala classes and building;
- `ready`: jdtls has it all.

A module without Scala is ready when jdtls is (its `ServiceReady`), one with Scala when jdtls has built against its new classes. Until then, jdtls's diagnostics for its files are held back, since they would report the Scala types not compiled yet as missing; the latest go out when it is ready. Shared source folders are modules of their own here, named as in `sharedSourceRoots` and marked `shared`.

### The importer

`com.snoutcode.engine.jdtls` contributes to jdtls's `org.eclipse.jdt.ls.core.importers` extension point with order 200, ahead of the Gradle (300) and Maven (400) importers. It applies when the root folder the client opens is, contains or lies inside the model's folder, and it reports every build and project directory of the model as resolved, failed builds included, so jdtls skips Buildship for them.

Each Gradle project with Java sources becomes one JDT project, the way Buildship maps them:

- The project lives in jdtls's workspace directory, not the repository: no `.project`, `.classpath`, `.settings` or `bin` appears in the user's files.
- Each source root is a linked folder, including folders outside the project directory. A root nested in another is excluded from the outer one.
- Each source set compiles to `bin/<sourceSet>` inside the workspace project. Test source sets, and libraries only they use, carry JDT's `test` attribute.
- Libraries come from the compile classpaths, with their source jars attached. Project dependencies, including those on included builds, become project references.
- The JDK is the compile task's toolchain, or the Gradle daemon's, registered with jdtls when it does not know it. Source, target, `--release`, `-parameters`, `--enable-preview` and the encoding come from the main Java compile task.
- The Scala bridge folders are libraries, with their Scala source root attached, so going to a Scala type shows its Scala source.

#### Source folders that several projects compile

Independent builds often compile one folder into each of them, as services compile a shared `core/src/main/java` that is not a Gradle project. JDT cannot hold the same files in several projects: every copy is indexed, searched and compiled again, so find references lists each use once per copy, and going to a definition picks one copy at random. So each source folder has one owner in the workspace:

- A folder inside a project that compiles it belongs to that project (`app` compiling `../lib/src/main/java` gets it from `lib`).
- Any other folder that more than one project compiles gets a JDT project of its own, named after its tree (`core` for `core/src/main/java` and `core/src/test/java`). Its classpath satisfies every user: the union of their libraries, the newest version of each, test only where every user has it test only, plus the projects they depend on that do not lead back to a user. Its JDK and compiler settings are those of the user with the newest language level. It exports the Scala bridge classes of its folders: the one compile of them (see [Shared Scala, compiled once](#shared-scala-compiled-once)), else one user's.
- The users drop the folder and their Scala bridge classes for it, and depend on its owner instead.

A folder only one project compiles stays linked into that project.

A build support for the same bundle owns these projects: jdtls would otherwise delete them on start, since they lie outside every root folder.

## The engine's methods

`snout-engine lsp` sits between the app and jdtls. Every message passes through unchanged, except requests and notifications whose method starts with `snout/`, which the engine answers itself. The engine's own requests to jdtls use string ids `snout-<n>` and their responses never reach the app. Requests run on a pool, so a slow one holds up nothing else.

Builds are named by their id in the model (`gradleBuilds[].id`); `build` may be left out when the folder holds one build. Modules are named by their id in `modules[].id` (`shop`, `shop:app`, `shop/tools:lib`). Task paths are as the root build runs them: `:app:test`, or `:tools:lib:test` for a task of the included build `tools`. Paths are absolute; a file may be given as a `file:` URI or a path.

### Requests

| Method | Parameters | Result |
| --- | --- | --- |
| `snout/gradleTasks` | `{ build?, refresh? }` | `{ builds: [ { build, rootDir, millis, error?, errors, projects: [ { project, name, module, projectDir, groups: [ { name, tasks: [ { name, path, description } ] } ] } ] } ] }`. Gradle's own groups, sorted, with `other` last. Configures the build once (every task realized); cached until `refresh`. |
| `snout/runTask` | `{ build?, project?, task, args? }` | `{ runId, tasks }` at once. `task` is a path, or a name in `project`; `args` are more command line arguments (`--info`, `-Pkey=value`, task options). For a Maven build, `task` is the goals and phases (`clean package`), `project` the module path (`:web`, `:` or none for the whole build), and `profiles` (`["prod", "!dev"]`), `offline` and `skipTests` go with them; see [Maven](#maven). |
| `snout/mavenGoals` | `{ build?, refresh? }` | `{ builds: [ { build, rootDir, mavenVersion, wrapper, millis, error?, profiles: [ { id, active, activeByDefault, source } ], modules: [ { module, project, name, packaging, projectDir, buildFile, lifecycle, plugins: [ { groupId, artifactId, version, prefix, goals: [ { name, description, phase } ] } ] } ] } ] }`: each Maven build's modules, with the phases, the plugins their packaging binds and their POMs declare, each plugin's goals from its descriptor (resolved once), and the profiles. Cached until `refresh`. |
| `snout/cancelTask` | `{ runId }` | `{ cancelled }`. Cancels a Gradle run through the Tooling API, or destroys a launched process. Also accepted as a notification. |
| `snout/runTests` | `{ module, filter?: { kind: "all" \| "package" \| "class" \| "method", package?, class?, method? }, rerunFailed?, task?, debug? }` | `{ runId, tasks }` at once. Runs the module's `test` task (or `task`, or every test task when there is no `test`), even when it is up to date. `rerunFailed` runs the tests that failed in the module's last run. `debug` as for `snout/run`. |
| `snout/testsInFile` | `{ uri \| path, text? }` | `{ language, module, classes: [ { name, simpleName, range, methods: [ { name, range } ] } ] }`: the test classes (nested ones as `Outer$Inner`) and methods of a Java or Scala file, with the LSP range of each name, for gutter run icons. `text` is the editor's unsaved text. |
| `snout/runConfigurations` | `{ module? }` | `{ configurations: [ { id, kind, name, module, build, ... } ] }`, see below. |
| `snout/run` | `{ config, debug?, dryRun? }` | `{ runId, ... }` at once; `config` is a configuration or its `id`. A `main` configuration answers `{ runId, pid, mainClass, projectName, java, workingDir, command }`, the command as launched (with its argument file). With `debug`: `debug: { dapPort, attach, listening }`. With `dryRun`: what would run, and nothing starts (below). |
| `snout/scalaChanged` | `{ files? }` | `{ runId, millis, builds: [ { build, tasks, millis, compileMillis, upToDate, error } ] }` when the compile is done and jdtls has the classes. Also accepted as a notification, which queues the compile. |
| `snout/readiness` | `{}` | `{ stage, percent, modules: [ { module, stage, percent, shared? } ] }`: what the `snout/readiness` notifications said last. |
| `snout/springBeans` | `{ module? }` | The beans of each Spring module's application context, for a bean list: `{ modules: [ { module, project, applications, scanPackages, scansEverything, xmlFiles, libraryBeans, beans: [Bean] } ] }`. See [Spring](#spring). |
| `snout/springEndpoints` | `{ module? }` | The request mappings of the controllers in each module's context, sorted by path: `{ endpoints: [Endpoint] }`. |
| `snout/springInjections` | `{ uri \| path }` | For gutter icons and "go to bean" or "go to usages", per line of a Java or Scala file: `{ uri, lines: [ { line, injections: [Injection], beans: [Bean with injectedInto] } ] }`. |
| `snout/springProfiles` | `{ module? }` | The profiles each Spring module knows, and the ones active now: `{ mode, chosen, modules: [ { module, configActive, active?, known: [ { name, sources } ], inactiveBeans } ] }`. See [Profiles](#profiles). |
| `snout/springSetProfiles` | `{ mode: "any" \| "config" \| "chosen", profiles? }` | How the beans are evaluated from now on; resolves again, publishes the Spring diagnostics that change and checks the open application files again. Answers as `snout/springProfiles`. |

### Notifications to the app

| Method | Parameters |
| --- | --- |
| `snout/taskOutput` | `{ runId, stream: "stdout" \| "stderr", line }`, each line of Gradle's or the launched process's output. |
| `snout/taskProgress` | `{ runId, task, event: "started" \| "finished", outcome?: "success" \| "up-to-date" \| "from-cache" \| "skipped" \| "no-source" \| "failed", millis?, failure? }`, from the Tooling API's task events. |
| `snout/testEvent` | `{ runId, task, event: "started" \| "finished" \| "output", kind: "suite" \| "test", id, parent, name, displayName, className, outcome?: "passed" \| "failed" \| "skipped", durationMillis?, message?, exception?, stackTrace?, tests?, failedTests?, skippedTests?, destination?: "stdout" \| "stderr", text? }`. `id` and `parent` are Gradle's descriptor ids, unique within a run; suites include Gradle's own (`Gradle Test Run :app:test`, `Gradle Test Executor 1`) above each class. |
| `snout/buildProblems` | `{ runId, problems: [ { file, line, column, severity: "error" \| "warning" \| "info", message } ] }`, once per run, read from the output: javac, scalac (Zinc's `[Error] file:line:col:` and the older form) and kotlinc. Lines and columns are 1-based; column 0 is unknown. |
| `snout/debugReady` | `{ runId, dapPort, attach }`, when the debuggee's JDWP agent listens. |
| `snout/taskFinished` | `{ runId, outcome: "success" \| "failed" \| "cancelled", millis, exitCode?, error?, tests?: { passed, failed, skipped } }`, last. |
| `snout/readiness` | `{ module, stage: "syncing" \| "compilingScala" \| "importing" \| "ready", percent, shared?, millis }` for a module as it moves on (see [Readiness](#readiness)); `{ module: null, stage, percent, modules, ready, millis }` for the whole project: its least advanced module's stage, their percent on average, how many there are and are ready. `millis` counts from the engine's start. |
| `snout/modelChanged` | `{}`, when a background sync changed the model the engine wrote. |

### Maven

A folder with a `pom.xml` (and no Gradle script) is a Maven build: the outermost POM of the reactor that holds the folder (each POM above lists the one below among its `<modules>`), or every such build under it. Gradle and Maven builds may sit side by side in one folder; each gets an entry in `gradleBuilds`, a Maven one with `tool: "maven"` and `mavenVersion`.

**Sync.** `MavenSync` reads the reactor with Maven's model builder (`maven-model-builder`) and resolves each module's dependencies with Maven's resolver (`maven-resolver-provider`, `maven-resolver-supplier`), both 3.9.16 and Apache 2.0, in the engine's process: no Maven runs. It reads `~/.m2/settings.xml` (mirrors, proxies, servers, active profiles and their repositories, `localRepository`, offline), `.mvn/maven.config` (`-D` and `-P`), the `maven.repo.local` system property or `-Dmaven.repo.local` in `MAVEN_OPTS`, and the user's home from `-Duser.home` in `MAVEN_OPTS`, else `HOME`. Profiles are those the POMs activate, `settings.xml`'s, `.mvn/maven.config`'s and `--maven-profiles`; modules of a profile that is on join the reactor. Parents and imported BOMs come from the reactor first, then the repositories. Each module becomes a project with `main` and `test` source sets in the Gradle model's form: source, generated (the compiler's annotation processor folders, Build Helper's added sources, and the folders already under `target/generated-sources`) and resource roots; `compileClasspath` and `runtimeClasspath` in Maven's order (`main`: `compile`, `provided`, `system` / `compile`, `runtime`; `test`: every scope), a module of the reactor (or its test jar) as a `project` entry; the compiler plugin's `release`, `source`, `target`, `encoding`, `-parameters`, `--enable-preview` and annotation processor path; a toolchain's JDK; `dependencyGraph`s from the resolver's tree, with the version a POM asked for where dependency management chose another; `springBoot` for Spring Boot's plugin; and under `maven` the coordinates, packaging, modules, profiles and plugins. A `pom` module has no source sets. The build's JDK (`gradleJavaHome`) is one of the newest Java level the POMs name: the preferred one when it is that, else one installed of that level, else the preferred one when newer, else the nearest newer one; with none, `needsJdk` is the LTS at or above it. The resync triggers are every POM of the reactor and the parents in the folder, `.mvn/maven.config`, `.mvn/extensions.xml`, `.mvn/jvm.config`, the wrapper's properties and `settings.xml`.

**Runs.** Goals run with the build's wrapper (`mvnw` and `.mvn/wrapper/maven-wrapper.properties`), else Apache Maven 3.9.16, downloaded once from Maven Central into `<cache>/maven/apache-maven-3.9.16` and checked against its SHA-512; with `-B`, `-ntp`, `-Dmaven.repo.local` and `-s` as the sync read them, on the build's JDK (`JAVA_HOME`). A run in one module of a reactor (`project`) compiles the modules it depends on first (`-pl <them> compile`, `test-compile` for a test jar), then runs the goals in the module alone with m2e's workspace reader (`org.eclipse.m2e.workspace.cli`, unmodified, from the bundled jdtls) through `-Dmaven.ext.class.path`, its state file mapping each module of the reactor to its POM and class folders; without that jar, `-pl <module> -am`. Output lines go out as `snout/taskOutput`; each `--- prefix:version:goal (execution) @ module ---` line as `snout/taskProgress`; compiler problems (`[ERROR] /file.java:[line,col] message`) as `snout/buildProblems`.

**Tests.** `snout/runTests` on a Maven module runs `test` in it, with surefire's `-Dtest` for the filter (`Class`, `Class#method`, `pkg/**/*`; a rerun of failed tests `Class#a+b`), and `snout-junit-listener.jar` on the test classpath (`-Dmaven.test.additionalClasspath`): JUnit Platform loads it, and it sends each event to a local socket the engine opened (`SNOUT_TEST_EVENTS`), in the form of `snout/testEvent`. A run that sent none (JUnit 4 alone) is read from surefire's XML reports when it ends. With `debug`, the test JVM waits for the debugger (`-Dmaven.surefire.debug`, `-DforkCount=1`), and `snout/debugReady` follows once its port is taken; `spring-boot:run` gets its JDWP agent through `-Dspring-boot.run.jvmArguments`.

**Configurations.** A module with Spring Boot's plugin gets its main class (`mainClass` of the plugin, the `start-class` property, else the `@SpringBootApplication` class) as a `main` configuration, and `{ kind: "maven", goals: "spring-boot:run" }`. A `maven` configuration runs its `goals` in its module as `snout/runTask` does; `dryRun` answers `{ commands, javaHome }`.

### Running Gradle

Every run goes through the Tooling API on the daemon JDK and caches the sync chose, with the build directory init script, so outputs (classes, test results, reports) land under the cache's `build/<hash>/`, and a run init script. That script adds a `TestListener` and `TestOutputListener` to every `Test` task, which print each event as a JSON line after a marker; the engine takes those lines out of the output and sends them as `snout/testEvent`. It also applies a run's test filters (as `--tests` would, through the task's command line include patterns) and makes test tasks run when up to date, so a rerun reports its tests again.

### Run configurations

| `kind` | From | Fields |
| --- | --- | --- |
| `main` | A Spring Boot app's main class (the build's setting, `bootRun`'s, the jar manifest's, or the one `@SpringBootApplication` class); the `application` plugin's; every other class with a `main` method in a main source set: Java, and Scala objects with `def main(args: Array[String])` or `extends App` | `mainClass`, `workingDir`, `args`, `jvmArgs`, `springBoot` |
| `gradle` | `bootRun`, the `application` plugin's `run` | `task`, `springBoot` |
| `maven` | Spring Boot's Maven plugin: `spring-boot:run` | `goals`, `springBoot` |
| `tests` | Each test task | `task` |

A `main` configuration runs as jdtls compiled it: java-debug resolves the class's JDT project, classpath (the project's output folders, its libraries and the Scala bridge folders) and Java (`vscode.java.resolveClasspath`, `vscode.java.resolveJavaExecutable`), and the engine adds the resource folders of the module and of the modules it depends on, since jdtls does not copy resources, and makes the libraries those of the module's runtime classpath as Gradle resolved it (with `runtimeOnly` libraries, those of the projects it depends on, and without `compileOnly` ones), since jdtls gives the compile classpath. The JVM gets its classpath in an argument file (on Java 8, in `CLASSPATH`). `dryRun` answers `{ mainClass, projectName, java, classpath, modulepath, workingDir, command }`. A `gradle` or `tests` configuration runs its task as `snout/runTask` and `snout/runTests` do; `dryRun` answers `{ rootDir, tasks, args, javaHome }`.

### Debugging

java-debug runs inside jdtls as a bundle. For `snout/run` with `debug`, the engine asks it for a DAP session, which answers with a port: through the importer bundle's `snout.startDebugSession`, which serves java-debug's session on a port of the loopback address (java-debug's own `vscode.java.startDebugSession` listens on every address, and Windows Firewall would ask about it), and starts the JVM with a JDWP agent that waits on a free local port: a `main` configuration with `-agentlib:jdwp=...,suspend=y`, a Gradle `JavaExec` or `Test` task through the run init script. The app connects a DAP client to `dapPort` on 127.0.0.1, sends `initialize` and then `attach` with the `attach` arguments it got (`hostName`, `port`, `projectName`), sets its breakpoints and sends `configurationDone`. For a `main` configuration the answer comes once the JVM listens (`listening`); for Gradle tasks, attach after `snout/debugReady`.

java-debug resolves sources, evaluates expressions and replaces code in the running JVM from jdtls's projects. Hot code replace: after the app saves an edited file and jdtls has built it, java-debug sends a `hotcodereplace` event (`changeType: "BUILD_COMPLETE"`), and a `redefineClasses` request applies the changed classes. With `vscode.java.updateDebugSettings` (through `workspace/executeCommand`, `{ "hotCodeReplace": "auto" }`) it applies them without asking.

### Scala, incrementally

The Scala bridge compiles Scala at the start, while jdtls starts. Afterwards, `snout/scalaChanged` (the app sends it when a `.scala` file is saved, created or deleted) recompiles the builds whose Scala source folders hold the files, every build with Scala when none are given, one change at a time; a file of a shared folder recompiles its unit, then every build that uses it. It runs Gradle's own Scala compile tasks again: the build directories, and with them Zinc's analysis, persist under the cache, so the compile is incremental, recompiling the changed files and those that depend on what changed. The class mirrors relink only what changed, and the engine then asks jdtls (`snout.refreshClassFolders`, a command of the importer bundle) to refresh the class folders: JDT reads them again and rebuilds the Java that uses them. Problems of the compile come as `snout/buildProblems`.

Zinc falls back to a full compile when a change invalidates more than half of a source set's files.

The bridge gives the Scala compiler daemon a 3 GB heap when the build sets none (`scalaCompileOptions.forkOptions.memoryMaximumSize`): Gradle's default is too small for a source set of thousands of files.

### Spring

The `com.snoutcode.engine.spring` bundle builds the Spring model inside jdtls from JDT's ASTs and bindings; the engine starts it once jdtls is ready (one scan of the workspace) and then asks it to read again each Java file the app opens, edits or saves (after 400 ms without changes), XML bean files that change, and the Scala bridge's classes after each Scala compile. A project without Spring on its classpath costs one type lookup.

What it reads, per JDT project (test source folders excluded):

- **Beans**: classes annotated `@Component` or any annotation meta-annotated with it (`@Service`, `@Repository`, `@Controller`, `@RestController`, `@Configuration`, `@ControllerAdvice`, the project's own), and `@Named`; `@Bean` methods; `@ConfigurationProperties` classes; `<bean>` definitions of the XML files `@ImportResource` names; Spring Data repository interfaces; MyBatis mappers (`@Mapper`, or interfaces in `@MapperScan` packages). `@Primary`, `@Qualifier` (and custom qualifier annotations), `@Profile` and every `@Conditional...` are recorded; conditions are never evaluated, and profiles only when the app asks (see [Profiles](#profiles)).
- **The application context** of each module: its project and everything on its classpath. Component scans start at its `@SpringBootApplication` (or `@EnableAutoConfiguration`) classes: their packages or `scanBasePackages`, `@ComponentScan`s of the classes in the context, `<context:component-scan>` in its XML files; `@Import`ed classes (through meta-annotations, so `@EnableWebSecurity` brings Spring Security's configurations), nested configuration classes. A module with no application class and no scan counts every component as scanned.
- **Library beans**: the `@Bean` methods of the auto-configurations every jar lists (`AutoConfiguration.imports`, `spring.factories`), of the configuration classes in their packages (the ones conditions and import selectors choose among), and of the library configurations the sources import; plus what the container supplies itself (`ApplicationContext`, `Environment`, the servlet request and others).
- **Injection points**: `@Autowired` and `@Inject` fields, setters and methods, constructor injection (the `@Autowired` constructor, or a bean's only constructor), `@Bean` method parameters, `@Resource` (by name, then type) and `@Value`. A collection, map, array, `Optional`, `ObjectProvider` or `Provider` takes the element type; `required = false` and `@Nullable` make it optional.
- **Endpoints**: `@RequestMapping` and its variants (`@GetMapping` and the others, or any annotation meta-annotated with it) on the controllers' methods and those they inherit, with the class's mapping joined to each method's: paths, HTTP methods, `produces`, `consumes`, `params`, `headers`.
- **The Scala bridge's classes** go through the same code: their annotations come from the class files, and their positions from the Scala file their `SourceFile` attribute names.

Resolution follows Spring's: candidates by type (generic arguments compared when both sides have concrete ones), narrowed by qualifiers, then the `@Primary` one, then the bean named like the field or parameter. A bean with a condition (or a profile, while profiles are not evaluated), and every library bean, may be absent, so it never makes an injection ambiguous, and one unconditional candidate wins over conditional ones. Code shared by several applications (a source folder each service compiles) is checked in the context of each application that holds it, keeping to the applications that scan its class: a bean is missing when every one lacks it, and ambiguous when one has several.

#### Profiles

`snout/springSetProfiles` sets how profiles are evaluated, for every module, until it is set again (the engine starts with `any`):

- `any`: not at all. A bean with `@Profile` may be absent, like one with a condition.
- `config`: the profiles the module's application files make active: `spring.profiles.active` of their documents for every profile.
- `chosen`: `profiles`, as `--spring.profiles.active` would, whatever the files say.

Either way the files' `spring.profiles.include` and the members of their groups (`spring.profiles.group.<name>`, recursively) are added, and when nothing is active, the profiles of `spring.profiles.default` (else `default`). The application files are `application.properties`, `.yml` and `.yaml` and `application-<profile>.*` in the module's resource folders; a file is split into documents at YAML's `---` and a properties file's `#---`, and a document with `spring.config.activate.on-profile` (or the older `spring.profiles`) is for that profile expression.

While profiles are evaluated, a bean's `@Profile` is an expression (a name, `!`, `&`, `|`, parentheses; several values: any one of them) checked against the active profiles of the application's module. A bean it does not match is listed with `active: false` (in `springBeans`; in `springInjections` when no context takes it) and is no candidate for any injection, so the injections and the warnings follow; one it matches counts as unconditional. `springBeans` gives each module its `activeProfiles`. `@Value` hints take the general documents and those of the active profiles, in Spring's order. An application file validates with a hint (severity 4, `spring.inactiveProfile`) on a profile file or document the active profiles leave out, and completes the known profiles as values of `spring.profiles.active`, `include`, `default`, `group.*` and `spring.config.activate.on-profile`. Saving an application file evaluates the profiles again.

It warns (severity 2, `source: "spring"`) when a required injection has no candidate (`spring.missingBean`, and says so when the only candidate is a component outside the scanned packages) or several (`spring.ambiguousBean`), and only in a context something starts. It does not warn about an interface with annotations and no implementation (Feign clients, mappers: a library registers a proxy), or a library class annotated as a component.

**Diagnostics and quick fixes.** The engine adds a file's Spring diagnostics to the ones jdtls publishes for it, and publishes the file again when only the Spring ones change. A diagnostic's `data.fixes` holds its quick fixes, `[ { title, edits: [TextEdit] } ]`, which the engine also adds to jdtls's answer to `textDocument/codeAction` for that range: "Inject 'name' with @Qualifier" for each candidate of an ambiguous injection (with the import), and "Make it optional: @Autowired(required = false)" for a missing one.

**Inlay hints.** The engine adds to jdtls's `textDocument/inlayHint` answer the value of each `@Value("${key}")` in range, after the string: `= value` from the module's `application.properties` or `.yml` and the files of the profiles `spring.profiles.active` names, else `= default (default)`.

**Application properties and YAML.** The engine handles `textDocument/didOpen`, `didChange` (whole or ranged), `didClose`, `completion`, `hover` and `definition` for files named `application*.properties`, `application*.yml`, `application*.yaml` (and `bootstrap*`) itself; jdtls never sees them, and any other request for them is answered with `null`. The app sends them on the same connection as the Java files. What a file knows comes from the module whose resource folder holds it:

- the metadata of every jar on its classpath (`META-INF/spring-configuration-metadata.json` and `additional-spring-configuration-metadata.json`), the `@ConfigurationProperties` classes in its sources (with their Javadoc and field defaults), and an `additional-spring-configuration-metadata.json` in its resources;
- the keys its code reads: `@Value` placeholders, `@ConditionalOnProperty`, `getProperty("key")` calls in Java and Scala, placeholders in its XML bean files and in the file itself, and Logback's `<springProperty source>`.

Completion offers keys (in YAML, relative to the keys above the line, inserting `rest.of.key: `) and values: booleans, an enum's constants, the metadata's hints. Hover shows a key's type, description, default, deprecation and declaring class. Definition goes to the property's field (or setter, or class) in the sources or in the library (a `jdt://` URI jdtls opens), and to the code that reads the key. Validation (published as the file changes, `source: "spring"`) warns about unknown keys (`spring.unknownProperty`), values that do not fit the type (`spring.invalidValue`: numbers, booleans, enums, durations, data sizes, each element of a list) and deprecated keys (`spring.deprecatedProperty`). A value with a `${...}` placeholder is not checked; nor are keys inside a map or a list of objects.

#### Shapes

```
Bean      { name, aliases?, type, kind: "component" | "service" | "repository" | "controller" | "restController"
                                     | "configuration" | "bean" | "xml" | "dataRepository" | "mapper" | "properties",
            stereotype?, declaringType?, method? (a @Bean method), uri, range, language: "java" | "scala" | "xml",
            primary?, qualifiers?, profiles?, conditions?, active? (false: the active profiles leave it out),
            injectedInto: count (springBeans) | [ { uri, range, declaringType, kind, member?, name? } ] (springInjections) }
Endpoint  { module, path, paths, methods: ["GET", ...] (empty: any), produces?, consumes?, params?, headers?,
            handler: { class, declaringClass? (when inherited), method }, uri, range, language }
Injection { kind: "field" | "constructor" | "setter" | "method" | "beanMethod",
            annotation: "autowired" | "inject" | "resource" | "value" | "implicit",
            declaringType, member?, name?, type?, wrapper?: "collection" | "map" | "array" | "optional" | "provider",
            required, qualifiers?, value? (@Value's expression), range,
            status: "resolved" | "missing" | "ambiguous" | "multiple" | "possible" | "optional" | "value"
                  | "framework" | "unresolved" | "unknown",
            message?, beans: [ { name, type, kind, declaringType?, method?, uri?, range? } ] }
```

`applications` are `[ { class, scanPackages, uri, range } ]`. A library bean has `kind: "library"` and no `uri`; `springBeans` lists only the module's own and counts the library's in `libraryBeans`. `possible` is a `@Bean` method declared as a supertype of the injected type, which may still fit; `multiple` a collection; `framework` what the container supplies; `unknown` a type the model cannot decide on.

#### The bundle's commands

The engine reaches the bundle through `workspace/executeCommand`, one JSON argument each: `snout.spring.scan {}`, `snout.spring.update { uris }` (answer: the diagnostics that changed), `snout.spring.beans { module? }`, `snout.spring.endpoints { module? }`, `snout.spring.injections { uri }`, `snout.spring.properties { action: "complete" | "hover" | "definition" | "validate", uri, text, line?, character? }`, `snout.spring.inlayHints { uri, range }`, `snout.spring.profiles { module? }`, `snout.spring.setProfiles { mode, profiles? }` (answer: `{ diagnostics, profiles }`) and `snout.spring.refreshProfiles {}` (after an application file is saved).

### Checking a real project

```sh
./gradlew :lsp-driver:installDist
lsp-driver/build/install/lsp-driver/bin/lsp-driver --engine sync-cli/build/install/snout-engine/bin/snout-engine \
    --project /path/to/project --script script.json --report report.json
```

The script names the engine's arguments, files to open, and definition, completion and references checks at text positions; see the `Driver` class. The report has the time to the first diagnostics, the error count and error kinds across all Java files, each check's result, and the peak memory of jdtls.

## How sync works

For each build the folder holds:

1. `snout-engine` writes a small init script (once per plugin location, under the system temp directory) that puts the plugin jar on the init script classpath and applies `SnoutSyncPlugin`.
2. Gradle applies the plugin to every build of the composite: the root build, included builds and buildSrc. The plugin registers one `ToolingModelBuilder` for `SnoutProjectModel` and starts recording which scripts each project loads.
3. `SyncAction` runs in the daemon: it walks the root build, every included build and (Gradle 8 and later) buildSrc, and asks each project for its model. For older Gradle, the CLI syncs buildSrc as a separate build.
4. Each project model comes back as a JSON string; the CLI assembles one document per build, and then the project view across all of them.

## The model

Format version 2. The top holds the project view, which the app shows as the Project tool window; `gradleBuilds` holds each build's full sync, which the importer and the Scala bridge read. Paths are absolute.

```
{
  "modelVersion": 2, "pluginModelVersion": 1, "rootDir": "<folder>", "name", "syncMillis",
  "modules": [ {
    "id": "<build id>[/<included build>][<Gradle path>]",   e.g. "billing", "shop:app", "shop/tools"
    "name", "build": "<build id>", "buildKind": "root" | "included" | "buildSrc", "includedBuild",
    "gradlePath", "contentRoot", "buildFile",
    "sourceRoots": [ { "dir", "type": "main" | "test" | "resources" | "test-resources" | "generated",
                       "sourceSet", "test" (generated only), "languages", "exists",
                       "shared": true when no module owns it, "owner": "<module id>" when another does } ],
    "excludedDirs": [ "<contentRoot>/build", "<contentRoot>/.gradle", output folders inside it ],
    "dependencies": { "compile" | "provided" | "runtime" | "test": [ { "kind": "library" | "module", "id" } ] },
    "jdk": { "javaHome", "version", "languageLevel" }
  } ],
  "sharedSourceRoots": [ { "dir", "type", "languages", "exists",
                           "name", "contentRoot": the folder the root belongs to, e.g. "<folder>/core",
                           "modules": [module ids that compile it] } ],
  "libraries": [ { "id": "group:name:version" or the file, "group", "name", "version", "file", "sources" } ],
  "gradleScripts": [ { "build": "<build id>",
                       "files": [ { "path", "kind": "settings" | "build" | "applied" | "properties"
                                                  | "versionCatalog" | "wrapper" } ] } ],
  "failedBuilds": [ { "build", "rootDir", "gradleVersion", "javaHome", "reason", "error", "stale",
                      "needsJdk": the Java major to install, when no installed JDK runs the build's Gradle } ],
  "gradleBuilds": [ {
    "id": "<build id>", "status": "synced" | "failed" | "stale", "rootDir", "gradleVersion",
    "gradleJavaHome", "javaVersion", "syncMillis", "error", "reason", "needsJdk" (failed and stale only),
    "builds": [ { "name", "kind": "root" | "included" | "buildSrc", "rootDir", "errors": [],
      "projects": [ {
        "path", "name", "projectDir", "buildFile", "buildDir", "group", "version",
        "build": { (root project only) "gradleVersion", "gradleJvm", "settingsFile",
                   "includedBuilds", "buildSrc", "resyncTriggers": [files], "scriptTracking" },
        "plugins": [ { "id", "class" } ],
        "appliedScripts": { "scripts": [files applied with `apply from`], "source" },
        "java": { "sourceCompatibility", "targetCompatibility", "toolchain" },
        "sourceSets": [ {
          "name",
          "sources": { "java" | "scala" | "kotlin" | "groovy": { "srcDirs", "includes", "excludes",
                       "filter", "compileTask", "outputDir" }, "resources": { ... } },
          "javaSourceDirs": [every directory whose .java files belong to this source set],
          "roots": [ { "dir", "kind": "source" | "resource" | "generated", "languages",
                       "compiledBy", "exists" } ],
          "generatedSources": [ { "dir", "kind": "annotationProcessor", "task", "exists" } ],
          "output": { "classesDirs", "resourcesDir" },
          "compile": { "<language>": { "task", "type", "sourceCompatibility", "targetCompatibility",
                       "release", "encoding", "compilerArgs", "scalaParameters", "toolchain" } },
          "annotationProcessorPath": [files],
          "compileClasspath" | "runtimeClasspath": [ entries, in Gradle's order:
              { "kind": "module", "group", "name", "version", "file", "sources" }
              { "kind": "project", "build", "path", "files" }
              { "kind": "sourceSetOutput", "sourceSet", "files" }
              { "kind": "file", "file" } ],
          "unresolved": [ { "requested", "reason" } ],
          "dependencyGraph": { "compileClasspath" | "runtimeClasspath" | "annotationProcessor": {
              "roots": [Edge], "nodes": [ { "kind": "module" | "project" | "other", "group", "name",
                  "version" | "build", "path" | "id", "reason": "conflict" | "forced" | "rule" | "constraint",
                  "deps": [Edge] } ] } },
          "scalaBridge": { (lsp only) "compiledDir", "folders": [ { "classesDir", "sourceDir", "classes" } ] }
        } ],
        "sourceJarsFound",
        "testTasks": [ { "name", "path", "framework": "junit-platform" | "junit4" | "testng",
                         "sourceSets", "testClassesDirs", "junitPlatform" } ],
        "springBoot": { "pluginVersion", "mainClass", "bootJarMainClass", "jarManifestMainClass",
                        "bootRun": { "task", "mainClass", "args", "jvmArgs", "workingDir",
                                     "systemProperties" },
                        "mainClassCandidates": [classes annotated @SpringBootApplication] },
        "application": { (the application plugin) "mainClass", "mainModule", "applicationName", "jvmArgs", "runTask" },
        "errors": [sections that failed to read], "buildMillis"
      } ] } ]
  } ]
}
```

- A build's id is its path inside the folder (`services/billing`), or the folder's name when the build is the folder itself.
- A module is one Gradle project of any build: root, included or `buildSrc`.
- Source roots are typed by their source set: a source set that a test task runs, or named `test` or ending in `Test` or `Tests`, is test code. A folder that is both a source and a resource root (resources read from `src/main/java`) is listed once, as source.
- Dependency scopes are an IDE's: `compile` is on the main compile and runtime classpaths, `provided` on compile only, `runtime` on runtime only, `test` only on the tests' classpaths. A dependency on another project's output is a `module` reference; everything else is a `library`, listed once in `libraries` however many modules use it.
- A source root belongs to the module with the deepest content root that holds it, when that module compiles it. A root that belongs to no module is a shared source root, listed once with every module that compiles it; its `name` is its folder inside the project, which groups `core/src/main/java` and `core/src/main/resources` under `core`.
- `reason` is a few words for a list of failed builds; `error` is Gradle's full message chain.

A source set's `dependencyGraph` is each classpath's resolution as Gradle did it, for the Project Structure window: the root's edges are the dependencies the configuration and those it extends declare (`declaredIn` names the one that declares it, `implementation` or `testRuntimeOnly`), every component selected is one node, and an `Edge` is `{ "to": node, "requested"?: the version asked for when another won }` or `{ "unresolved": "group:name:version", "reason" }`. Constraints are not edges. A node's `reason` says why it was selected when that is more than being asked for: `conflict` (a newer version asked for elsewhere won). A graph stops at 4000 nodes; `-Dsnout.sync.dependencyGraphs=false` leaves them out.

A Scala or Groovy source set that also compiles `.java` files (joint compilation) lists those directories under `javaSourceDirs`, and its roots name both languages.

## Limits

- `apply from` scripts are found through Gradle's internal script listener. Where it is missing (Gradle 5) the plugin scans the build file for literal `apply from` paths instead; `appliedScripts.source` says which.
- Spring Boot fills in an unset main class by scanning compiled classes, a task that sync does not run. `mainClassCandidates` comes from scanning the sources instead.
- `--project-cache-dir` applies to each build's root; included builds keep their own `.gradle`.
- The daemon JDK is chosen from the build scripts' text before Gradle runs, so a toolchain set in a convention plugin or an applied script is not seen; pass `--java-home` for such a build.

## Credits

Parts of `sync-plugin` are adapted from IntelliJ IDEA Community Edition, under the Apache License 2.0. The distribution carries Eclipse JDT LS 1.61.0, under the Eclipse Public License 2.0; 1.61.0 is the last release that runs on Java 21. It also carries java-debug 0.53.1 (microsoft/java-debug), under the Eclipse Public License 1.0. See [NOTICE](NOTICE).
- `snout-engine lsp` syncs when it starts, or takes the last model when the builds did not change. After a build file changes, restart it.
- The engine does not watch Scala files itself; the app sends `snout/scalaChanged`.
- Spring: conditions and profiles are not evaluated; test sources, Kotlin, Lombok-generated constructors and beans that `ImportBeanDefinitionRegistrar`s or `BeanFactoryPostProcessor`s register are not seen; Scala positions come from the text, so a member is found by its name.
- A `main` configuration runs jdtls's compile of the Java, which does not run annotation processors, with the resource folders as they are: no `processResources` filtering.
- jdtls does not run annotation processors for these projects. Generated source folders are used when a Gradle build has filled them.
- The JDT projects sit outside the root folder, which jdtls treats as an unmanaged project in a few places: if the client changes jdtls's default JDK, their compiler level follows it until the next start.
