# snout-engine

The JVM side of SnoutCode. Three layers:

- **Gradle sync**: one Tooling API call per Gradle build that returns the whole build model as JSON, so the editor knows the source roots, classpaths, compiler settings and run targets of a Java, Scala, Kotlin or Groovy project without running a build. A project is any folder: one Gradle build, or many side by side, synced together into one model.
- **Java semantics**: `snout-engine lsp` runs Eclipse JDT LS (jdtls) as a language server, fed by that model instead of jdtls's own Gradle import (Buildship).
- **Running and debugging**: the same LSP stream carries the engine's own `snout/` methods: Gradle tasks, tests, run configurations, launching and debugging (through java-debug in jdtls), and recompiling Scala as it changes.

## Layout

- `sync-plugin`: model builders that run inside the user's Gradle daemon. Compiled to Java 8 bytecode, and version-sensitive Gradle APIs are reached through reflection, so one jar serves Gradle 5 through 8. The `BuildAction` that walks the build lives here too, because the Tooling API ships it into the daemon.
- `sync-cli`: the command line, `snout-engine`: the Tooling API client, the Scala bridge and the `lsp` launcher.
- `jdtls-importer`: an OSGi bundle for jdtls that builds JDT projects from the model.
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

Each build syncs on its own Gradle daemon, several at once, and on a JDK its Gradle version runs on, chosen from the JDKs found on the machine (`JAVA_HOME`, this JVM, the macOS and Linux JDK folders, SDKMAN, `~/.jdks`, Gradle's provisioned toolchains, SnoutCode's cache) and those given with `--jdk`:

- Gradle older than 6.7 gets Java 11.
- Newer Gradle gets the Java its build script names (a toolchain, `jvmToolchain`, a source or target compatibility, a `--release`) when that is newer than 17, else 17.
- Never newer than the Java the Gradle version supports; when the wanted Java is missing, the nearest installed one it supports.
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
| `--offline` | Run Gradle offline. |
| `--verbose` | Show Gradle's output, and a line per build, on stderr. |

The command prints a one line summary on stderr and exits with 1 if a build failed to sync; the model is written all the same.

A sync only configures the builds. No task runs, so nothing is compiled and no `build/` directory appears.

## Java semantics: `snout-engine lsp`

```sh
snout-engine lsp /path/to/folder --java-home /path/to/jdk17
```

The command speaks LSP on stdin and stdout, so the app spawns it and talks to it like any language server. Before it starts jdtls it:

1. syncs the folder's builds as `sync` does, with every build directory moved out of the project (an init script sets each project's `layout.buildDirectory`) and Gradle's project caches with them;
2. runs the **Scala bridge**, for each build that has Scala, several builds at once: jdtls cannot read Scala source, so for each source set with Scala files it runs that source set's Scala compile task, and nothing else. The task compiles the Java of a joint source set too, so the bridge keeps a mirror with only the classes whose `SourceFile` is a `.scala` file, hard linked and grouped by the source root that holds the file, and records them in the model. `ScalaBridge.compile` is the one entry point, for recompiling later;
3. writes the model where the importer reads it, and starts jdtls with the importer bundle and java-debug's;
4. relays LSP between the app and jdtls, answering the `snout/` methods itself (see [The engine's methods](#the-engines-methods)).

All of it lives under the cache directory (`~/Library/Caches/SnoutCode` on macOS), keyed by a hash of the folder:

| Path | Contents |
| --- | --- |
| `build/<hash>/` | Gradle build directories, including the compiled Scala |
| `gradle/<hash>/<build>-<hash>/` | Gradle's project cache of each build |
| `jdtls/<hash>/model.json` | The model the importer reads |
| `jdtls/<hash>/config/` | jdtls's OSGi configuration, with the bundle added |
| `jdtls/<hash>/workspace/` | jdtls's workspace: the JDT projects, their output folders, indexes and log |
| `jdtls/<hash>/runs/` | Argument files of the processes the engine launches, while they run |

A build whose sync fails keeps its entry from the last model that synced it, marked `stale`, so a broken build file does not cost its modules. A build that never synced has no modules; the importer still claims the folder, so jdtls's Gradle importer never runs Buildship on it.

| Option | Meaning |
| --- | --- |
| `--java-home <dir>`, `--jdk <dir>`, `--parallel <n>` | As for `sync`. jdtls's Java 21 is too new for Gradle before 8.5. |
| `--cache-dir <dir>` | Use another cache directory. |
| `--jdtls-java-home <dir>` | Java 21 or newer for jdtls. Defaults to the JVM running snout-engine. |
| `--jdtls <dir>`, `--bundle <jar>` | Use another jdtls installation or importer bundle. |
| `--debug-bundle <jar>` | Use another java-debug bundle. |
| `--no-debugger` | Do not load java-debug into jdtls. |
| `--no-scala-bridge` | Do not compile Scala. |
| `--max-heap <size>` | jdtls's heap limit (default `2G`). |
| `--jvm-arg <arg>` | An extra JVM argument for jdtls; repeatable. |
| `--gradle-user-home`, `--gradle-version`, `--no-sources`, `--offline`, `--verbose` | As for `sync`. |

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
- Any other folder that more than one project compiles gets a JDT project of its own, named after its tree (`core` for `core/src/main/java` and `core/src/test/java`). Its classpath satisfies every user: the union of their libraries, the newest version of each, test only where every user has it test only, plus the projects they depend on that do not lead back to a user. Its JDK and compiler settings are those of the user with the newest language level. It exports the Scala bridge classes of its folders, taken from one user's compile.
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
| `snout/runTask` | `{ build?, project?, task, args? }` | `{ runId, tasks }` at once. `task` is a path, or a name in `project`; `args` are more command line arguments (`--info`, `-Pkey=value`, task options). |
| `snout/cancelTask` | `{ runId }` | `{ cancelled }`. Cancels a Gradle run through the Tooling API, or destroys a launched process. Also accepted as a notification. |
| `snout/runTests` | `{ module, filter?: { kind: "all" \| "package" \| "class" \| "method", package?, class?, method? }, rerunFailed?, task?, debug? }` | `{ runId, tasks }` at once. Runs the module's `test` task (or `task`, or every test task when there is no `test`), even when it is up to date. `rerunFailed` runs the tests that failed in the module's last run. `debug` as for `snout/run`. |
| `snout/testsInFile` | `{ uri \| path, text? }` | `{ language, module, classes: [ { name, simpleName, range, methods: [ { name, range } ] } ] }`: the test classes (nested ones as `Outer$Inner`) and methods of a Java or Scala file, with the LSP range of each name, for gutter run icons. `text` is the editor's unsaved text. |
| `snout/runConfigurations` | `{ module? }` | `{ configurations: [ { id, kind, name, module, build, ... } ] }`, see below. |
| `snout/run` | `{ config, debug?, dryRun? }` | `{ runId, ... }` at once; `config` is a configuration or its `id`. With `debug`: `debug: { dapPort, attach, listening }`. With `dryRun`: what would run, and nothing starts (below). |
| `snout/scalaChanged` | `{ files? }` | `{ runId, millis, builds: [ { build, tasks, millis, compileMillis, error } ] }` when the compile is done. Also accepted as a notification, which queues the compile. |

### Notifications to the app

| Method | Parameters |
| --- | --- |
| `snout/taskOutput` | `{ runId, stream: "stdout" \| "stderr", line }`, each line of Gradle's or the launched process's output. |
| `snout/taskProgress` | `{ runId, task, event: "started" \| "finished", outcome?: "success" \| "up-to-date" \| "from-cache" \| "skipped" \| "no-source" \| "failed", millis?, failure? }`, from the Tooling API's task events. |
| `snout/testEvent` | `{ runId, task, event: "started" \| "finished" \| "output", kind: "suite" \| "test", id, parent, name, displayName, className, outcome?: "passed" \| "failed" \| "skipped", durationMillis?, message?, exception?, stackTrace?, tests?, failedTests?, skippedTests?, destination?: "stdout" \| "stderr", text? }`. `id` and `parent` are Gradle's descriptor ids, unique within a run; suites include Gradle's own (`Gradle Test Run :app:test`, `Gradle Test Executor 1`) above each class. |
| `snout/buildProblems` | `{ runId, problems: [ { file, line, column, severity: "error" \| "warning" \| "info", message } ] }`, once per run, read from the output: javac, scalac (Zinc's `[Error] file:line:col:` and the older form) and kotlinc. Lines and columns are 1-based; column 0 is unknown. |
| `snout/debugReady` | `{ runId, dapPort, attach }`, when the debuggee's JDWP agent listens. |
| `snout/taskFinished` | `{ runId, outcome: "success" \| "failed" \| "cancelled", millis, exitCode?, error?, tests?: { passed, failed, skipped } }`, last. |

### Running Gradle

Every run goes through the Tooling API on the daemon JDK and caches the sync chose, with the build directory init script, so outputs (classes, test results, reports) land under the cache's `build/<hash>/`, and a run init script. That script adds a `TestListener` and `TestOutputListener` to every `Test` task, which print each event as a JSON line after a marker; the engine takes those lines out of the output and sends them as `snout/testEvent`. It also applies a run's test filters (as `--tests` would, through the task's command line include patterns) and makes test tasks run when up to date, so a rerun reports its tests again.

### Run configurations

| `kind` | From | Fields |
| --- | --- | --- |
| `main` | A Spring Boot app's main class (the build's setting, `bootRun`'s, the jar manifest's, or the one `@SpringBootApplication` class); the `application` plugin's; every other class with a `main` method in a main source set: Java, and Scala objects with `def main(args: Array[String])` or `extends App` | `mainClass`, `workingDir`, `args`, `jvmArgs`, `springBoot` |
| `gradle` | `bootRun`, the `application` plugin's `run` | `task`, `springBoot` |
| `tests` | Each test task | `task` |

A `main` configuration runs as jdtls compiled it: java-debug resolves the class's JDT project, classpath (the project's output folders, its libraries and the Scala bridge folders) and Java (`vscode.java.resolveClasspath`, `vscode.java.resolveJavaExecutable`), and the engine adds the resource folders of the module and of the modules it depends on, since jdtls does not copy resources. The JVM gets its classpath in an argument file (on Java 8, in `CLASSPATH`). `dryRun` answers `{ mainClass, projectName, java, classpath, modulepath, workingDir, command }`. A `gradle` or `tests` configuration runs its task as `snout/runTask` and `snout/runTests` do; `dryRun` answers `{ rootDir, tasks, args, javaHome }`.

### Debugging

java-debug runs inside jdtls as a bundle. For `snout/run` with `debug`, the engine asks it for a DAP session (`vscode.java.startDebugSession`), which answers with a port, and starts the JVM with a JDWP agent that waits on a free local port: a `main` configuration with `-agentlib:jdwp=...,suspend=y`, a Gradle `JavaExec` or `Test` task through the run init script. The app connects a DAP client to `dapPort` on 127.0.0.1, sends `initialize` and then `attach` with the `attach` arguments it got (`hostName`, `port`, `projectName`), sets its breakpoints and sends `configurationDone`. For a `main` configuration the answer comes once the JVM listens (`listening`); for Gradle tasks, attach after `snout/debugReady`.

java-debug resolves sources, evaluates expressions and replaces code in the running JVM from jdtls's projects. Hot code replace: after the app saves an edited file and jdtls has built it, java-debug sends a `hotcodereplace` event (`changeType: "BUILD_COMPLETE"`), and a `redefineClasses` request applies the changed classes. With `vscode.java.updateDebugSettings` (through `workspace/executeCommand`, `{ "hotCodeReplace": "auto" }`) it applies them without asking.

### Scala, incrementally

The Scala bridge compiles Scala once, before jdtls starts. Afterwards, `snout/scalaChanged` (the app sends it when a `.scala` file is saved, created or deleted) recompiles the builds whose Scala source folders hold the files, every build with Scala when none are given, one compile at a time. It runs Gradle's own Scala compile task again: the build directories, and with them Zinc's analysis, persist under the cache, so the compile is incremental, recompiling the changed files and those that depend on what changed. The class mirrors relink only what changed, and the engine then asks jdtls (`snout.refreshClassFolders`, a command of the importer bundle) to refresh the class folders: JDT reads them again and rebuilds the Java that uses them. Problems of the compile come as `snout/buildProblems`.

Zinc falls back to a full compile when a change invalidates more than half of a source set's files.

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
  "failedBuilds": [ { "build", "rootDir", "gradleVersion", "javaHome", "reason", "error", "stale" } ],
  "gradleBuilds": [ {
    "id": "<build id>", "status": "synced" | "failed" | "stale", "rootDir", "gradleVersion",
    "gradleJavaHome", "javaVersion", "syncMillis", "error", "reason" (failed and stale only),
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

A Scala or Groovy source set that also compiles `.java` files (joint compilation) lists those directories under `javaSourceDirs`, and its roots name both languages.

## Limits

- `apply from` scripts are found through Gradle's internal script listener. Where it is missing (Gradle 5) the plugin scans the build file for literal `apply from` paths instead; `appliedScripts.source` says which.
- Spring Boot fills in an unset main class by scanning compiled classes, a task that sync does not run. `mainClassCandidates` comes from scanning the sources instead.
- `--project-cache-dir` applies to each build's root; included builds keep their own `.gradle`.
- The daemon JDK is chosen from the build scripts' text before Gradle runs, so a toolchain set in a convention plugin or an applied script is not seen; pass `--java-home` for such a build.

## Credits

Parts of `sync-plugin` are adapted from IntelliJ IDEA Community Edition, under the Apache License 2.0. The distribution carries Eclipse JDT LS 1.61.0, under the Eclipse Public License 2.0; 1.61.0 is the last release that runs on Java 21. It also carries java-debug 0.53.1 (microsoft/java-debug), under the Eclipse Public License 1.0. See [NOTICE](NOTICE).
- `snout-engine lsp` reads the model once, when it starts. After a build file changes, restart it.
- The engine does not watch Scala files itself; the app sends `snout/scalaChanged`.
- A `main` configuration runs jdtls's compile of the Java, which does not run annotation processors, with the resource folders as they are: no `processResources` filtering.
- jdtls does not run annotation processors for these projects. Generated source folders are used when a Gradle build has filled them.
- The JDT projects sit outside the root folder, which jdtls treats as an unmanaged project in a few places: if the client changes jdtls's default JDK, their compiler level follows it until the next start.
