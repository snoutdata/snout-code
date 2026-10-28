# snout-engine

The JVM side of SnoutCode. Two layers:

- **Gradle sync**: one Tooling API call that returns the whole build model as JSON, so the editor knows the source roots, classpaths, compiler settings and run targets of a Java, Scala, Kotlin or Groovy project without running a build.
- **Java semantics**: `snout-engine lsp` runs Eclipse JDT LS (jdtls) as a language server, fed by that model instead of jdtls's own Gradle import (Buildship).

## Layout

- `sync-plugin`: model builders that run inside the user's Gradle daemon. Compiled to Java 8 bytecode, and version-sensitive Gradle APIs are reached through reflection, so one jar serves Gradle 5 through 8. The `BuildAction` that walks the build lives here too, because the Tooling API ships it into the daemon.
- `sync-cli`: the command line, `snout-engine`: the Tooling API client, the Scala bridge and the `lsp` launcher.
- `jdtls-importer`: an OSGi bundle for jdtls that builds JDT projects from the model.
- `lsp-driver`: a headless LSP client. It runs the end-to-end test and can check a real project.

## Build

Needs JDK 21: jdtls runs on it, and the importer compiles against jdtls. The wrapper downloads Gradle, and the build downloads the pinned jdtls release from download.eclipse.org into `jdtls-importer/build/jdtls` (checked against its SHA-256; never committed).

```sh
cd engine
./gradlew build          # compiles, runs the unit, integration and end-to-end tests
./gradlew installDist    # puts the CLI, jdtls and the bundle in sync-cli/build/install/snout-engine
```

## Run

```sh
sync-cli/build/install/snout-engine/bin/snout-engine sync /path/to/project --out model.json
```

| Option | Meaning |
| --- | --- |
| `--out <file>` | Write the model to a file instead of stdout. |
| `--java-home <dir>` | JDK for the Gradle daemon. Defaults to the JVM running snout-engine. Gradle 6 and older cannot run on JDK 17; give them a JDK 11. |
| `--project-cache-dir <dir>` | Keep Gradle's `.gradle` project cache out of the project directory. |
| `--gradle-user-home <dir>` | Use another Gradle user home (default `~/.gradle`). |
| `--gradle-version <v>` | Use this Gradle version instead of the build's wrapper. |
| `--no-sources` | Skip the source jar lookup. |
| `--offline` | Run Gradle offline. |
| `--verbose` | Show Gradle's output on stderr. |

The command prints a one line summary on stderr and exits with 1 if a build or project failed to sync.

A sync only configures the build. No task runs, so nothing is compiled and no `build/` directory appears.

## Java semantics: `snout-engine lsp`

```sh
snout-engine lsp /path/to/project --java-home /path/to/jdk17
```

The command speaks LSP on stdin and stdout, so the app spawns it and talks to it like any language server. Before it starts jdtls it:

1. syncs the model, with every build directory moved out of the project (an init script sets each project's `layout.buildDirectory`) and Gradle's project cache with it;
2. runs the **Scala bridge**: jdtls cannot read Scala source, so for each source set with Scala files it runs that source set's Scala compile task, and nothing else. The task compiles the Java of a joint source set too, so the bridge keeps a mirror with only the classes whose `SourceFile` is a `.scala` file, hard linked and grouped by the source root that holds the file, and records them in the model. `ScalaBridge.compile` is the one entry point, for recompiling later;
3. writes the model where the importer reads it, and starts jdtls with the importer bundle.

All of it lives under the cache directory (`~/Library/Caches/SnoutCode` on macOS), keyed by a hash of the project directory:

| Path | Contents |
| --- | --- |
| `build/<hash>/` | Gradle build directories, including the compiled Scala |
| `gradle/<hash>/` | Gradle's project cache |
| `jdtls/<hash>/model.json` | The model the importer reads |
| `jdtls/<hash>/config/` | jdtls's OSGi configuration, with the bundle added |
| `jdtls/<hash>/workspace/` | jdtls's workspace: the JDT projects, their output folders, indexes and log |

If the sync fails, jdtls starts with the last good model. With none, the importer still claims the build, so jdtls's Gradle importer never runs Buildship on it.

| Option | Meaning |
| --- | --- |
| `--java-home <dir>` | JDK for the Gradle daemon, as for `sync`. jdtls's Java 21 is too new for Gradle before 8.5. |
| `--cache-dir <dir>` | Use another cache directory. |
| `--jdtls-java-home <dir>` | Java 21 or newer for jdtls. Defaults to the JVM running snout-engine. |
| `--jdtls <dir>`, `--bundle <jar>` | Use another jdtls installation or importer bundle. |
| `--no-scala-bridge` | Do not compile Scala. |
| `--max-heap <size>` | jdtls's heap limit (default `2G`). |
| `--jvm-arg <arg>` | An extra JVM argument for jdtls; repeatable. |
| `--gradle-user-home`, `--gradle-version`, `--no-sources`, `--offline`, `--verbose` | As for `sync`. |

### The importer

`com.snoutcode.engine.jdtls` contributes to jdtls's `org.eclipse.jdt.ls.core.importers` extension point with order 200, ahead of the Gradle (300) and Maven (400) importers. It applies when the root folder the client opens is, contains or lies inside the model's root build, and it reports every project directory of the model as resolved, so jdtls skips Buildship for them.

Each Gradle project with Java sources becomes one JDT project, the way Buildship maps them:

- The project lives in jdtls's workspace directory, not the repository: no `.project`, `.classpath`, `.settings` or `bin` appears in the user's files.
- Each source root is a linked folder, including folders outside the project directory. A root nested in another is excluded from the outer one.
- Each source set compiles to `bin/<sourceSet>` inside the workspace project. Test source sets, and libraries only they use, carry JDT's `test` attribute.
- Libraries come from the compile classpaths, with their source jars attached. Project dependencies, including those on included builds, become project references.
- The JDK is the compile task's toolchain, or the Gradle daemon's, registered with jdtls when it does not know it. Source, target, `--release`, `-parameters`, `--enable-preview` and the encoding come from the main Java compile task.
- The Scala bridge folders are libraries, with their Scala source root attached, so going to a Scala type shows its Scala source.

A build support for the same bundle owns these projects: jdtls would otherwise delete them on start, since they lie outside every root folder.

### Checking a real project

```sh
./gradlew :lsp-driver:installDist
lsp-driver/build/install/lsp-driver/bin/lsp-driver --engine sync-cli/build/install/snout-engine/bin/snout-engine \
    --project /path/to/project --script script.json --report report.json
```

The script names the engine's arguments, files to open, and definition, completion and references checks at text positions; see the `Driver` class. The report has the time to the first diagnostics, the error count and error kinds across all Java files, each check's result, and the peak memory of jdtls.

## How sync works

1. `snout-engine` writes a small init script (once per plugin location, under the system temp directory) that puts the plugin jar on the init script classpath and applies `SnoutSyncPlugin`.
2. Gradle applies the plugin to every build of the composite: the root build, included builds and buildSrc. The plugin registers one `ToolingModelBuilder` for `SnoutProjectModel` and starts recording which scripts each project loads.
3. `SyncAction` runs in the daemon: it walks the root build, every included build and (Gradle 8 and later) buildSrc, and asks each project for its model. For older Gradle, the CLI syncs buildSrc as a separate build.
4. Each project model comes back as a JSON string; the CLI assembles one document.

## The model

```
{
  "modelVersion": 1, "rootDir", "gradleVersion", "gradleJavaHome", "syncMillis",
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
        "unresolved": [ { "requested", "reason" } ]
      } ],
      "sourceJarsFound",
      "testTasks": [ { "name", "path", "framework": "junit-platform" | "junit4" | "testng",
                       "sourceSets", "testClassesDirs", "junitPlatform" } ],
      "springBoot": { "pluginVersion", "mainClass", "bootJarMainClass", "jarManifestMainClass",
                      "bootRun": { "task", "mainClass", "args", "jvmArgs", "workingDir",
                                   "systemProperties" },
                      "mainClassCandidates": [classes annotated @SpringBootApplication] },
      "errors": [sections that failed to read], "buildMillis"
    } ] } ]
}
```

A Scala or Groovy source set that also compiles `.java` files (joint compilation) lists those directories under `javaSourceDirs`, and its roots name both languages.

## Limits

- `apply from` scripts are found through Gradle's internal script listener. Where it is missing (Gradle 5) the plugin scans the build file for literal `apply from` paths instead; `appliedScripts.source` says which.
- Spring Boot fills in an unset main class by scanning compiled classes, a task that sync does not run. `mainClassCandidates` comes from scanning the sources instead.
- `--project-cache-dir` applies to the root build; included builds keep their own `.gradle`.

## Credits

Parts of `sync-plugin` are adapted from IntelliJ IDEA Community Edition, under the Apache License 2.0. The distribution carries Eclipse JDT LS 1.61.0, under the Eclipse Public License 2.0; 1.61.0 is the last release that runs on Java 21. See [NOTICE](NOTICE).
- `snout-engine lsp` reads the model once, when it starts. After a build file changes, restart it. Recompiling Scala when Scala files change is not wired up yet; `ScalaBridge.compile` is the place.
- jdtls does not run annotation processors for these projects. Generated source folders are used when a Gradle build has filled them.
- The JDT projects sit outside the root folder, which jdtls treats as an unmanaged project in a few places: if the client changes jdtls's default JDK, their compiler level follows it until the next start.
