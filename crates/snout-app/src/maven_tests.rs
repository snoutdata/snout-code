//! Maven builds, headless: finding a reactor's root, the Maven window over the engine's
//! goals (profiles turned on and off, goals run in a module with the window's choices),
//! the project model of a Maven build (its files, the stripe), Edit in build file in a POM,
//! the New Project wizard's Maven build, a Maven run's processes and the POM's colours.

use std::path::Path;

use gpui::{Entity, TestAppContext, VisualTestContext};
use serde_json::json;

use crate::lang::java;
use crate::run::config::MavenChoices;
use crate::run::{Launch, Runs, parse_maven_goals};
use crate::tests::{demo_repo, open, with_repo};
use crate::workspace::Workspace;

fn runs_of(ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> Entity<Runs> {
    cx.update(|_, cx| ws.read(cx).runs.clone())
}

fn goals() -> serde_json::Value {
    json!({ "builds": [{
        "build": "pantry", "rootDir": "/w/pantry", "mavenVersion": "3.9.16", "wrapper": false,
        "profiles": [ { "id": "dev", "active": true, "source": "/w/pantry/pom.xml" },
                      { "id": "prod", "active": false, "source": "/w/pantry/pom.xml" } ],
        "modules": [
            { "module": "pantry", "project": ":", "name": "pantry", "packaging": "pom",
              "lifecycle": ["clean", "install"], "plugins": [] },
            { "module": "pantry:web", "project": ":web", "name": "web", "packaging": "jar",
              "buildFile": "/w/pantry/web/pom.xml",
              "lifecycle": ["clean", "validate", "compile", "test", "package", "verify", "install", "site", "deploy"],
              "plugins": [ { "artifactId": "spring-boot-maven-plugin", "version": "3.3.5", "prefix": "spring-boot",
                             "goals": [ { "name": "run", "description": "Run an application in place." },
                                        { "name": "repackage" } ] } ] }
        ]
    }]})
}

#[test]
fn a_module_belongs_to_the_reactor_that_lists_it() {
    let dir = demo_repo("maven-reactor");
    std::fs::create_dir_all(dir.join("web/src/main/java")).unwrap();
    std::fs::write(dir.join("pom.xml"), "<project><modules>\n  <module>web</module>\n</modules></project>\n").unwrap();
    std::fs::write(dir.join("web/pom.xml"), "<project/>").unwrap();
    let host = snout_host::local();
    let file = dir.join("web/src/main/java/App.java");
    assert_eq!(java::gradle_root(&*host, &file, None), Some(dir.clone()), "the reactor, not the module");
    assert!(java::is_maven_root(&*host, &dir) && java::is_gradle_root(&*host, &dir));
    std::fs::write(dir.join("pom.xml"), "<project/>").unwrap();
    assert_eq!(java::gradle_root(&*host, &file, None), Some(dir.join("web")), "a POM that does not list it");
}

#[test]
fn profile_choices_remember_only_what_differs() {
    let mut m = MavenChoices::default();
    assert!(m.profile_on("dev", true) && !m.profile_on("prod", false));
    m.toggle("dev", true);
    assert_eq!(m.profiles, ["!dev"]);
    m.toggle("prod", false);
    assert_eq!(m.profiles, ["!dev", "prod"]);
    m.toggle("dev", true);
    assert_eq!(m.profiles, ["prod"], "back as the POM has it");
}

#[gpui::test]
fn the_maven_window_lists_profiles_lifecycle_and_goals_and_runs_one(cx: &mut TestAppContext) {
    let repo = demo_repo("maven-window");
    let (_, ws, cx) = open(cx, with_repo(&repo));
    let runs = runs_of(&ws, cx);
    cx.update(|_, cx| {
        runs.update(cx, |r, cx| {
            r.maven.builds = parse_maven_goals(&goals());
            cx.notify();
        })
    });
    let window = cx.update(|_, cx| ws.read(cx).maven.clone());
    cx.update(|_, cx| window.update(cx, |w, cx| w.reveal("spring-boot:run", cx)));
    let rows = cx.update(|_, cx| crate::run::maven_window::goal_rows(window.read(cx), cx));
    for want in ["Profiles", "[x] dev", "[ ] prod", "pantry", "pantry (root)", "web", "Plugins", "spring-boot:run"] {
        assert!(rows.iter().any(|r| r == want), "{want} in {rows:?}");
    }
    assert!(cx.update(|_, cx| ws.read(cx).shows_maven(cx)), "the stripe offers the Maven window");

    // Skip tests and a profile turned on go with a goal run in the module.
    cx.update(|_, cx| {
        runs.update(cx, |r, cx| {
            r.set_maven_skip_tests(true, cx);
            r.project.maven.toggle("prod", false);
        })
    });
    let launch =
        Launch::Maven { build: "pantry".into(), project: ":web".into(), module: "web".into(), goals: "package".into() };
    assert_eq!(launch.title(), "web [package]");
    let id = cx.update(|_, cx| runs.update(cx, |r, cx| r.adopt(launch, "run-9", cx)));
    let session = cx.update(|_, cx| runs.read(cx).session(id).map(|s| (s.title.clone(), s.place())));
    assert_eq!(session, Some(("web [package]".into(), crate::run::Place::Build)));
    let choices = cx.update(|_, cx| runs.read(cx).project.maven.clone());
    assert!(choices.skip_tests);
    assert_eq!(choices.profiles, ["prod"]);
}

#[test]
fn a_maven_models_files_are_maven_files() {
    let model = json!({
        "modelVersion": 2, "rootDir": "/w/pantry", "name": "pantry",
        "modules": [ { "id": "pantry:web", "name": "web", "build": "pantry", "tool": "maven", "gradlePath": ":web",
                       "contentRoot": "/w/pantry/web", "sourceRoots": [], "excludedDirs": ["/w/pantry/web/target"] } ],
        "gradleScripts": [ { "build": "pantry", "tool": "maven", "files": [
            { "path": "/w/pantry/pom.xml", "kind": "build" }, { "path": "/w/pantry/web/pom.xml", "kind": "build" } ] } ],
    });
    let m = crate::project_model::parse(&model).unwrap();
    assert_eq!(m.scripts_label(), "Maven Files");
    assert!(m.maven_only() && m.modules[0].maven);
    assert!(m.scripts.iter().all(|s| s.maven));
}

#[test]
fn edit_in_build_file_finds_the_poms_lines() {
    use crate::structure::locate::{pom_block_line, pom_level_line, pom_module_line};
    let pom = "<project>\n  <modules>\n    <module>stock</module>\n    <module>web</module>\n  </modules>\n  \
               <properties>\n    <maven.compiler.release>21</maven.compiler.release>\n  </properties>\n  \
               <dependencies>\n    <dependency>\n      <groupId>org.springframework.boot</groupId>\n      \
               <artifactId>spring-boot-starter-web</artifactId>\n    </dependency>\n    <dependency>\n      \
               <groupId>com.example</groupId>\n      <artifactId>stock</artifactId>\n    </dependency>\n  </dependencies>\n\
               </project>\n";
    assert_eq!(pom_module_line(pom, "web"), Some(3));
    assert_eq!(pom_level_line(pom), Some(6));
    assert_eq!(
        pom_block_line(pom, "dependency", Some("org.springframework.boot"), "spring-boot-starter-web"),
        Some(11)
    );
    assert_eq!(pom_block_line(pom, "dependency", None, "stock"), Some(15));
    assert_eq!(pom_block_line(pom, "dependency", Some("org.other"), "stock"), None);
}

#[test]
fn the_wizard_makes_a_maven_build() {
    use crate::new_project::generate::{Basics, Boot, Dsl, starter_url, write_boot_offline, write_java};
    let host = snout_host::local();
    let b = Basics {
        name: "hello".into(),
        group: "com.example".into(),
        artifact: "hello".into(),
        package: "com.example.hello".into(),
        java: 21,
        dsl: Dsl::Maven,
    };
    let dir = demo_repo("maven-wizard").join("hello");
    write_java(&*host, &dir, &b).unwrap();
    let pom = std::fs::read_to_string(dir.join("pom.xml")).unwrap();
    assert!(pom.contains("<maven.compiler.release>21</maven.compiler.release>") && pom.contains("junit-jupiter"));
    assert!(dir.join("src/main/java/com/example/hello/Main.java").is_file());
    assert!(!dir.join("build.gradle.kts").exists() && !dir.join("gradlew").exists());
    let boot = Boot { version: "3.3.5".into(), dependencies: vec!["web".into(), "lombok".into()] };
    let app = demo_repo("maven-wizard-boot").join("hello");
    write_boot_offline(&*host, &app, &b, &boot).unwrap();
    let pom = std::fs::read_to_string(app.join("pom.xml")).unwrap();
    assert!(pom.contains("<artifactId>spring-boot-starter-parent</artifactId>"), "{pom}");
    assert!(pom.contains("<artifactId>lombok</artifactId>\n      <optional>true</optional>"), "{pom}");
    assert!(pom.contains("spring-boot-maven-plugin"));
    assert!(starter_url("https://start.spring.io", &b, &boot).contains("type=maven-project"));
}

#[test]
fn maven_processes_are_told_apart() {
    use crate::processes::is_maven;
    assert!(is_maven(
        "/opt/jdk/bin/java -classpath /x/boot/plexus-classworlds-2.8.0.jar org.codehaus.plexus.classworlds.launcher.Launcher -B test"
    ));
    assert!(is_maven("/bin/sh /w/pantry/mvnw -B package"));
    assert!(!is_maven("/opt/jdk/bin/java -jar app.jar"));
}

#[test]
fn a_pom_is_coloured_as_xml() {
    use crate::editor::language_for;
    assert_eq!(language_for(Path::new("/w/pantry/pom.xml")), "html");
    assert_eq!(crate::claude_ide::tools::language_id(Path::new("/w/pantry/pom.xml")), "xml");
    assert!(gpui_component::highlighter::LanguageRegistry::singleton().language("html").is_some());
}
