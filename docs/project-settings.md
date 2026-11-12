# Project Settings

Project Settings is everything about the open project, in one window: the JDK it runs on, its modules and their source folders, every dependency and where it comes from, the frameworks SnoutCode found, the settings the project sets for itself, and the JDKs on your machine. Your own settings, the same in every project, are in [Settings](settings-and-keymap.md).

![The Project page of the ledger project](images/project-structure-project.png)

## Open Project Settings

Any of these opens it:

- File ▸ Project Settings… (⌘;)
- **Project Settings…** at the top of the right-click menu of the project's folder, the first row of the Project view
- **Project Settings…** in the command palette (⇧⌘P)

Project Settings is there when the open folder is a project: a Gradle or Maven build, or a Node project (a `package.json`). With a plain folder open, the menu item is off and the project's folder does not offer it.

⌘; is the shortcut of the command **Project Settings…**. You can give it other keys in Settings, under Keymap (see [Settings and keymap](settings-and-keymap.md#keymap)).

The pages are listed on the left, under **Structure** (Project, Modules, Dependencies, Facets), **Settings for this project** (Editor, Version Control, Gradle, Spring, Terminal) and **Platform Settings** (SDKs). Type in **Search** to find a page by what it holds, for example "jdk" or "conflicts".

## What you can change here, and what Gradle owns

Two kinds of values show in the window:

- **Values SnoutCode owns** are changed in place: the JDK the project runs on, and the JDKs you add or remove. The window edits a copy of them. **Apply** saves the changes and keeps the window open, **OK** saves them and closes it, and **Cancel** (or Esc) closes it without changing anything. The project's JDK is saved in the project's own settings, `.snout/settings.json` (see [Settings for this project](#settings-for-this-project)). The JDKs you add are saved in your own settings.
- **Values Gradle owns** carry a lock: the project's name, its Java level, the Gradle version, the modules, their folders and their dependencies. SnoutCode never writes them, because the next sync would read them from the build again. Click **Edit in build file** beside one instead. The window closes and the file that declares the value opens at its line: a `build.gradle(.kts)`, the `settings.gradle(.kts)`, the version catalog `gradle/libs.versions.toml`, or `gradle/wrapper/gradle-wrapper.properties`. When the line cannot be found (a value set in a convention plugin, for example), the file opens at its top.

Once you save a build file, the bar above the editor says **Gradle files changed. Sync?** Click **Sync** to read the build again (see [Sync after you change a build script](gradle.md#sync-after-you-change-a-build-script)). **Sync Gradle** at the top of the Project, Modules and Dependencies pages syncs at any time; the pages follow when the sync is done.

The window reads what the last Gradle sync wrote. Until a project has been synced, its Gradle pages say **No Gradle model yet** and offer to sync it.

## Project

- **Name** and **Location**: the root project's name (from `rootProject.name` in the settings script) and the project's folder.
- **JDK**: the JDK every Gradle build of the project runs on, and the Java engine with it. **Each build's own (for its Java level)** lets SnoutCode pick one for each build (see [The JDK each build runs on](gradle.md#the-jdk-each-build-runs-on)). The list holds the JDKs of the SDKs page. A JDK set here shows the badge **Set for this project**. The Java engine restarts on it the next time it starts.
- **Language level**: the Java level the modules compile at (their `--release`, else their source compatibility), as one level or a range.
- **Gradle version**: the version each build's wrapper downloads. Its **Edit in build file** opens the `distributionUrl` line of the wrapper's properties.
- **Gradle JVM**: the JDK the last sync ran each build's Gradle on.

A project of several builds lists the Gradle rows once per build.

For a Maven build, the section is **Maven** and its rows are **Maven version** (the wrapper's, or the Maven SnoutCode downloads; its **Edit in build file** opens the wrapper's `distributionUrl`, else the root POM) and **Maven JVM** (the JDK Maven runs on and compiles with). A project with both lists them under **Gradle and Maven**.

## Modules

![The modules of ledger, and the source sets of one](images/project-structure-modules.png)

The tree on the left holds every Gradle project of every build the project holds: the root project, its subprojects, included builds (marked **included build**) and `buildSrc`. Under each module are its source sets, `main`, `test` and any others. A test source set is marked **tests**. Click the arrow beside a module to fold its source sets away.

Select a module to see:

- its Gradle path, folder and build file (click the build file to open it),
- its **Language level** (with its toolchain, when one is set) and its **Target compatibility**, the level of the bytecode it produces,
- a card for each source set, with its Java level and:
  - **Content roots**: its source, resource, test and generated folders, each with its icon. A folder the build names that does not exist yet says **not there yet**.
  - **Output folders**: where it compiles its classes and copies its resources.

Select a source set to see its card alone. **Edit in build file** at the top opens the line of the settings script that includes the module.

![The Modules page in the dark theme](images/project-structure-modules-dark.png)

## Dependencies

![What the app module of ledger runs with: its projects and what they bring in](images/project-structure-dependencies.png)

Pick a module at the top, then a scope:

| Scope | What it lists |
|---|---|
| **Compile** | What the main code compiles against (`compileClasspath`) |
| **Runtime** | What the main code runs with (`runtimeClasspath`) |
| **Test** | What the tests compile and run with (the test source set's `runtimeClasspath`) |
| **Annotation processor** | The annotation processors javac runs on the main code |

The tree starts with the dependencies the module declares, each with the configuration that declares it (`implementation`, `testRuntimeOnly`). Click the arrow beside one to see what it brings in, as Gradle resolved it. A line under the scopes counts what is declared and what is on the classpath in all.

- **Projects**: a dependency on another project of the build reads **project :core** with a **project** badge. Click its name to show that module on the Modules page, or the arrow at the end of the row to see its own dependencies.
- **Version conflicts**: when one dependency asks for a version and another one is used, the row shows the version asked for struck through, an arrow and the version used, with a **Conflict** badge. Hover it to read why: a newer version asked for elsewhere won, or a dependency management plugin or a platform chose it. The version that won says **won over** and the versions it replaced. A version a platform or a BOM gives (the dependency names none) says **managed**.
- **Unresolved**: a dependency Gradle could not find is a red row with Gradle's reason.

### Maven modules

A Maven module's pages show what Maven resolves, by scope:

| Scope | What it lists |
|---|---|
| **Compile** | The `compile`, `provided` and `system` dependencies the main code compiles against |
| **Runtime** | The `compile` and `runtime` dependencies it runs with |
| **Test** | Every scope, as the tests compile and run with it |

Each declared dependency names its scope. A dependency on another module of the reactor reads **project :stock**. Where dependency management (a parent POM, an imported BOM) chose another version than the one a POM asked for, the row shows both. **Edit in build file** and **Show in build file** open the POM line that declares the dependency, the `<module>` that lists the module, the plugin, or the property that sets the Java level, in the module's POM or the parent POM in the build that sets it.

![The web module of the pantry Maven build](images/maven-structure-dependencies.png)

**Filter** finds dependencies by group, name or version anywhere in the tree, and shows each one under what brings it in. **Conflicts only** lists just the dependencies whose version lost a conflict.

![The conflicts of the shop's test classpath](images/project-structure-conflicts.png)

At the end of a row:

- **Show in build file** (the page with a magnifier) opens the build script at the line that declares the dependency. For a dependency brought in by another, it opens the declaration of the one that brings it in. A dependency declared through the version catalog (`libs.guava`) is found by its alias.
- **Show in version catalog** (the open book) opens `gradle/libs.versions.toml` at the dependency's entry, when the catalog declares it.

## Settings for this project

A project can set some settings for itself, over yours: they are kept in `.snout/settings.json` in the project's folder. Commit the file and everyone who opens the project in SnoutCode gets the same values. The pages under **Settings for this project** show them:

![The Editor page of Project Settings](images/settings-and-keymap-project.png)

*The shop sets its own tab size and soft wrap; whitespace and parameter hints are yours.*

Each setting says where its value comes from:

- **Set for this project**: the project's file sets it. The round arrow beside it resets it to your setting (the file no longer sets it).
- **Your setting**: the project leaves it to you. Change it here to set it for the project.

**Edit as JSON** at the top of these pages opens the file in a tab.

A project can set these, by their names in the file:

| Setting | In the file | Values |
| --- | --- | --- |
| Editor ▸ Tab size | `tab_size` | 1 to 8 |
| Editor ▸ Soft wrap long lines | `soft_wrap` | `true` or `false` |
| Editor ▸ Show whitespace | `show_whitespace` | `true` or `false` |
| Editor ▸ Show parameter names of literal arguments | `param_hints` | `true` or `false` |
| Editor ▸ Show hover | `show_hover` | `"after_delay"` or `"while_command_held"` |
| Editor ▸ Hover delay | `hover_delay_ms` | 0 to 3000 |
| Version Control ▸ Fetch every | `fetch_minutes` | 5 to 120 |
| Project ▸ JDK | `jdk` | the JDK's home folder |
| Gradle ▸ Stop idle daemons after | `daemon_idle_minutes` | 1 to 240 |
| Spring ▸ Active profiles | `spring_profiles` | `"any"`, `"config"`, or a list of names |
| Terminal ▸ Shell ▸ Program | `terminal_shell` | a program; `""` is the login shell |
| Terminal ▸ Shell ▸ Arguments | `terminal_shell_args` | a list, such as `["-l"]` |
| Terminal ▸ Shell ▸ Environment | `terminal_env` | `{ "NAME": "value" }`, in place of yours |

A project's file looks like this:

```json
{
  "tab_size": 2,
  "soft_wrap": true,
  "jdk": "/Library/Java/JavaVirtualMachines/temurin-21.jdk/Contents/Home",
  "spring_profiles": ["dev"]
}
```

SnoutCode reads the file again whenever it changes, within a second, whoever changed it: you in an editor, a `git pull`, a teammate's commit. When it cannot be read (it is not JSON, or a value has the wrong type), SnoutCode keeps the values it read last, says what is wrong in the status bar and over these pages, and reads it again once you fix it. Keys SnoutCode does not know are kept when it writes the file.

On a project opened over [Remote SSH](remote-ssh.md), the file is on the remote machine, in the project's folder there, and SnoutCode reads and writes it there.

## SDKs

![The JDKs on this Mac](images/project-structure-sdks.png)

The JDKs SnoutCode can run Gradle and the Java engine on. Each says where it comes from:

- **Detected**: found where JDKs are installed (macOS's JavaVirtualMachines folders, IntelliJ IDEA's JDKs, SDKMAN, asdf, mise, jenv, Homebrew, Gradle's provisioned toolchains).
- **Downloaded**: downloaded by SnoutCode.
- **Added**: added by you.

The one the project runs on says **This project's**. Click a JDK to see its version and home under the list; **Use for This Project** makes it the project's JDK, as the Project page's picker does.

- **Add from Folder…** asks for the folder of a JDK: its home (the folder holding `bin/java`), a macOS `.jdk` bundle, or a folder that holds one JDK. **Apply** or **OK** keeps it, in your `settings.json` under `sdks`. A JDK you added is offered by the Project page, by Settings, and when SnoutCode picks a JDK for a build.
- The bin at the end of an added JDK's row takes it off the list. Its files stay where they are.
- **Download JDK…** opens the Download JDK dialog (below).

### Download a JDK

**Download JDK…** on the SDKs page asks for:

- **Version**: every Java version that can be downloaded for this machine, the newest first. The long-term support versions say so; the newest of them is picked.
- **Vendor**: the vendors that publish that version, with the exact build: Eclipse Temurin, Amazon Corretto, Azul Zulu, BellSoft Liberica, GraalVM Community, Oracle OpenJDK, Oracle Java, Microsoft Build of OpenJDK, SapMachine, IBM Semeru, JetBrains Runtime and Alibaba Dragonwell.
- **Location**: SnoutCode's JDK folder, where it is unpacked.

The list comes from the foojay catalogue, the one IntelliJ IDEA and Gradle's toolchains use. **Download** fetches the JDK from its vendor, checks its SHA-256 when the vendor publishes one, and unpacks it. The page shows the progress, and the JDK is listed when it is done. The bar offering a missing JDK opens the same dialog on the version the project needs, from **Use Another JDK…** (see [When no installed JDK fits](gradle.md#when-no-installed-jdk-fits)).

## Facets

![Spring and Claude Code in the shop project](images/project-structure-facets.png)

What SnoutCode found for the frameworks and tools of the project. The page is read only; its buttons open the page where each is set.

- **Spring**: **Detected** when a module applies the Spring Boot plugin or the Spring model has beans. It lists those modules with their Spring Boot version, the beans in the Spring model, the **Active profiles** the beans are evaluated under, the profiles the project knows, and its configuration files (`application*.properties`, `application*.yml` and `bootstrap*` in the resources). Click a configuration file to open it. **Spring Settings** opens the Spring page of Project Settings, where the active profiles are chosen (see [Spring](spring.md)).
- **Claude Code**: whether the `claude` program was found, and how many Claude Code sessions are connected to SnoutCode. **Claude Code Settings** opens Settings ▸ Tools ▸ Agents (see [Coding agents](agents.md)).

## Remote SSH

On a remote machine (see [Remote SSH](remote-ssh.md)), Project Settings shows the project there: its model, its build files, its own settings file and the JDKs of that machine. **Add from Folder…** browses the remote machine's folders, and **Download JDK…** downloads to it. The JDKs you add there are kept in your settings under the machine's name, apart from those of your Mac.
