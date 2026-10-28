# snout-engine

The JVM side of SnoutCode. Three layers:

- **Gradle sync**: one Tooling API call per Gradle build that returns the whole build model as JSON, so the editor knows the source roots, classpaths, compiler settings and run targets of a Java, Scala, Kotlin or Groovy project without running a build. A project is any folder: one Gradle build, or many side by side, synced together into one model.
- **Java semantics**: `snout-engine lsp` runs Eclipse JDT LS (jdtls) as a language server, fed by that model instead of jdtls's own Gradle import (Buildship).
- **Running and debugging**: the same LSP stream carries the engine's own `snout/` methods: Gradle tasks, tests, run configurations, launching and debugging (through java-debug in jdtls), and recompiling Scala as it changes.
- **Spring**: a second jdtls bundle reads the Spring model from JDT's ASTs and bindings (beans, injection points, endpoints, configuration properties), warns about missing and ambiguous beans, and gives the `application*.properties` and `application*.yml` files completion, hover, definition and validation.

## Layout

- `sync-plugin`: model builders that run inside the user's Gradle daemon. Compiled to Java 8 bytecode, and version-sensitive Gradle APIs are reached through reflection, so one jar serves Gradle 5 through 8. The `BuildAction` that walks the build lives here too, because the Tooling API ships it into the daemon.
- `sync-cli`: the command line, `snout-engine`: the Tooling API client, the Scala bridge and the `lsp` launcher.
- `jdtls-importer`: an OSGi bundle for jdtls that builds JDT projects from the model.
- `spring`: an OSGi bundle for jdtls with the Spring support (see [Spring](#spring)). Its resolver, properties and YAML documents, configuration metadata and XML bean files are plain Java with their own tests.
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

Each build syncs on its own Gradle daemon, several at once, and on a JDK its Gradle version runs on, chosen from the JDKs found on the machine (`JAVA_HOME`, this JVM, the macOS and Linux JDK folders, SDKMAN, `~/.jdks`, Gradle's provisioned toolchains, the JDKs the app downloads to `~/Library/Application Support/SnoutCode/jdks`, the engine's build JDK in `~/Library/Caches/SnoutCode/build-jdk`, and the `jdks` and `build-jdk` folders of `--cache-dir`) and those given with `--jdk`:

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
3. writes the model where the importer reads it, and starts jdtls with the importer bundle, the Spring bundle and java-debug's;
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
| `--spring-bundle <jar>` | Use another Spring bundle. |
| `--no-spring` | No Spring support: the bundle is not loaded and the `snout/spring*` methods fail. |
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
| `snout/springBeans` | `{ module? }` | The beans of each Spring module's application context, for a bean list: `{ modules: [ { module, project, applications, scanPackages, scansEverything, xmlFiles, libraryBeans, beans: [Bean] } ] }`. See [Spring](#spring). |
| `snout/springEndpoints` | `{ module? }` | The request mappings of the controllers in each module's context, sorted by path: `{ endpoints: [Endpoint] }`. |
| `snout/springInjections` | `{ uri \| path }` | For gutter icons and "go to bean" or "go to usages", per line of a Java or Scala file: `{ uri, lines: [ { line, injections: [Injection], beans: [Bean with injectedInto] } ] }`. |

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

The bridge gives the Scala compiler daemon a 3 GB heap when the build sets none (`scalaCompileOptions.forkOptions.memoryMaximumSize`): Gradle's default is too small for a source set of thousands of files.

### Spring

The `com.snoutcode.engine.spring` bundle builds the Spring model inside jdtls from JDT's ASTs and bindings; the engine starts it once jdtls is ready (one scan of the workspace) and then asks it to read again each Java file the app opens, edits or saves (after 400 ms without changes), XML bean files that change, and the Scala bridge's classes after each Scala compile. A project without Spring on its classpath costs one type lookup.

What it reads, per JDT project (test source folders excluded):

- **Beans**: classes annotated `@Component` or any annotation meta-annotated with it (`@Service`, `@Repository`, `@Controller`, `@RestController`, `@Configuration`, `@ControllerAdvice`, the project's own), and `@Named`; `@Bean` methods; `@ConfigurationProperties` classes; `<bean>` definitions of the XML files `@ImportResource` names; Spring Data repository interfaces; MyBatis mappers (`@Mapper`, or interfaces in `@MapperScan` packages). `@Primary`, `@Qualifier` (and custom qualifier annotations), `@Profile` and every `@Conditional...` are recorded and never evaluated.
- **The application context** of each module: its project and everything on its classpath. Component scans start at its `@SpringBootApplication` (or `@EnableAutoConfiguration`) classes: their packages or `scanBasePackages`, `@ComponentScan`s of the classes in the context, `<context:component-scan>` in its XML files; `@Import`ed classes (through meta-annotations, so `@EnableWebSecurity` brings Spring Security's configurations), nested configuration classes. A module with no application class and no scan counts every component as scanned.
- **Library beans**: the `@Bean` methods of the auto-configurations every jar lists (`AutoConfiguration.imports`, `spring.factories`), of the configuration classes in their packages (the ones conditions and import selectors choose among), and of the library configurations the sources import; plus what the container supplies itself (`ApplicationContext`, `Environment`, the servlet request and others).
- **Injection points**: `@Autowired` and `@Inject` fields, setters and methods, constructor injection (the `@Autowired` constructor, or a bean's only constructor), `@Bean` method parameters, `@Resource` (by name, then type) and `@Value`. A collection, map, array, `Optional`, `ObjectProvider` or `Provider` takes the element type; `required = false` and `@Nullable` make it optional.
- **Endpoints**: `@RequestMapping` and its variants (`@GetMapping` and the others, or any annotation meta-annotated with it) on the controllers' methods and those they inherit, with the class's mapping joined to each method's: paths, HTTP methods, `produces`, `consumes`, `params`, `headers`.
- **The Scala bridge's classes** go through the same code: their annotations come from the class files, and their positions from the Scala file their `SourceFile` attribute names.

Resolution follows Spring's: candidates by type (generic arguments compared when both sides have concrete ones), narrowed by qualifiers, then the `@Primary` one, then the bean named like the field or parameter. A bean with a profile or a condition, and every library bean, may be absent, so it never makes an injection ambiguous, and one unconditional candidate wins over conditional ones. Code shared by several applications (a source folder each service compiles) is checked in the context of each application that holds it, keeping to the applications that scan its class: a bean is missing when every one lacks it, and ambiguous when one has several.

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
            primary?, qualifiers?, profiles?, conditions?,
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

The engine reaches the bundle through `workspace/executeCommand`, one JSON argument each: `snout.spring.scan {}`, `snout.spring.update { uris }` (answer: the diagnostics that changed), `snout.spring.beans { module? }`, `snout.spring.endpoints { module? }`, `snout.spring.injections { uri }`, `snout.spring.properties { action: "complete" | "hover" | "definition" | "validate", uri, text, line?, character? }` and `snout.spring.inlayHints { uri, range }`.

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
- Spring: conditions and profiles are not evaluated; test sources, Kotlin, Lombok-generated constructors and beans that `ImportBeanDefinitionRegistrar`s or `BeanFactoryPostProcessor`s register are not seen; Scala positions come from the text, so a member is found by its name.
- A `main` configuration runs jdtls's compile of the Java, which does not run annotation processors, with the resource folders as they are: no `processResources` filtering.
- jdtls does not run annotation processors for these projects. Generated source folders are used when a Gradle build has filled them.
- The JDT projects sit outside the root folder, which jdtls treats as an unmanaged project in a few places: if the client changes jdtls's default JDK, their compiler level follows it until the next start.
