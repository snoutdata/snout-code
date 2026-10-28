# snout-engine

The JVM side of SnoutCode. This first layer is **Gradle sync**: one Tooling API call that returns the whole build model as JSON, so the editor knows the source roots, classpaths, compiler settings and run targets of a Java, Scala, Kotlin or Groovy project without running a build.

## Layout

- `sync-plugin`: model builders that run inside the user's Gradle daemon. Compiled to Java 8 bytecode, and version-sensitive Gradle APIs are reached through reflection, so one jar serves Gradle 5 through 8. The `BuildAction` that walks the build lives here too, because the Tooling API ships it into the daemon.
- `sync-cli`: the Tooling API client, `snout-engine`.

## Build

Needs JDK 17 or newer. The wrapper downloads Gradle.

```sh
cd engine
./gradlew build          # compiles, runs the unit and integration tests
./gradlew installDist    # puts the CLI in sync-cli/build/install/snout-engine
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

## How it works

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

Parts of `sync-plugin` are adapted from IntelliJ IDEA Community Edition, under the Apache License 2.0. See [NOTICE](NOTICE).
