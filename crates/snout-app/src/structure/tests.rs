//! Tests of the Project Structure window: the model it reads, where it finds declarations,
//! and the window itself in a test window.

use std::path::{Path, PathBuf};

use gpui::{Modifiers, TestAppContext, VisualTestContext};

use super::data::{BuildKind, Edge, NodeKind, Scope, Structure, parse};
use super::locate::{self, Target};
use super::{Page, StructureEvent};
use crate::settings::Settings;
use crate::tests::{demo_repo, scratch_dir, with_repo};

fn fixture(root: &Path) -> Structure {
    let text = include_str!("../../tests/fixtures/structure-model.json").replace("$ROOT", &root.to_string_lossy());
    parse(&serde_json::from_str(&text).unwrap()).unwrap()
}

/// The fixture's scripts, catalog and wrapper on disk under `root`.
fn write_scripts(root: &Path) {
    let files: &[(&str, &str)] = &[
        ("settings.gradle.kts", "rootProject.name = \"demo\"\n\ninclude(\"app\", \"core\")\nincludeBuild(\"tools\")\n"),
        (
            "build.gradle.kts",
            "subprojects {\n    apply(plugin = \"java\")\n    // sourceCompatibility in a comment\n    java {\n        sourceCompatibility = JavaVersion.VERSION_21\n    }\n}\n",
        ),
        (
            "app/build.gradle.kts",
            "plugins {\n    java\n    id(\"org.springframework.boot\") version \"3.3.0\"\n}\n\ndependencies {\n    implementation(project(\":core\"))\n    implementation(\"org.springframework:spring-web:6.1.0\")\n}\n",
        ),
        (
            "core/build.gradle.kts",
            "dependencies {\n    implementation(libs.guava)\n    testImplementation(libs.guava.testlib)\n}\n",
        ),
        (
            "gradle/libs.versions.toml",
            "[versions]\nguava = \"33.0.0-jre\"\n\n[libraries]\nguava-testlib = { module = \"com.google.guava:guava-testlib\", version.ref = \"guava\" }\nguava = { module = \"com.google.guava:guava\", version.ref = \"guava\" }\n",
        ),
        (
            "gradle/wrapper/gradle-wrapper.properties",
            "distributionBase=GRADLE_USER_HOME\ndistributionUrl=https\\://services.gradle.org/distributions/gradle-8.10.2-bin.zip\n",
        ),
        (
            "tools/build.gradle",
            "plugins { id 'java-library' }\njava { toolchain { languageVersion = JavaLanguageVersion.of(17) } }\n",
        ),
        ("app/src/main/resources/application.yml", "server:\n  port: 8080\n"),
        ("app/src/main/resources/application-dev.properties", "debug=true\n"),
        ("app/src/main/resources/banner.txt", "demo\n"),
    ];
    for (path, text) in files {
        let f = root.join(path);
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(f, text).unwrap();
    }
}

#[test]
fn reads_builds_modules_source_sets_and_graphs() {
    let s = fixture(Path::new("/w/demo"));
    let names: Vec<(&str, usize, BuildKind)> = s.modules.iter().map(|m| (m.id.as_str(), m.depth, m.kind)).collect();
    assert_eq!(
        names,
        [
            ("demo", 0, BuildKind::Root),
            ("demo:app", 1, BuildKind::Root),
            ("demo:core", 1, BuildKind::Root),
            ("demo/tools", 1, BuildKind::Included),
            ("demo/buildSrc", 1, BuildKind::BuildSrc),
        ],
        "the root project first, its subprojects, then the included build and buildSrc a level down"
    );
    assert_eq!(s.builds[0].gradle_version.as_deref(), Some("8.10.2"));
    assert_eq!(s.builds[0].java_version.as_deref(), Some("21.0.2"));
    let core = s.module("demo:core").unwrap();
    assert!(!s.module("demo").unwrap().is_jvm());
    assert_eq!(core.level(), Some("21"));
    let main = core.main().unwrap();
    let kinds: Vec<_> = main.roots.iter().map(|r| r.1).collect();
    use crate::project_model::RootKind;
    assert_eq!(kinds, [RootKind::Main, RootKind::Generated, RootKind::Resources], "sources, then resources");
    assert!(!main.roots[2].2, "a resource folder not made yet says so");
    assert_eq!(main.outputs.len(), 2);
    assert!(core.test().unwrap().test);
    let app = s.module("demo:app").unwrap();
    assert_eq!((app.level(), app.target_level()), (Some("21"), Some("17")));
    assert_eq!(app.spring_boot.as_deref(), Some("3.3.0"));
    let tools = s.module("demo/tools").unwrap();
    assert_eq!(
        (tools.level(), tools.toolchain.as_deref(), tools.note().as_deref()),
        (Some("17"), Some("17"), Some("included build"))
    );
    assert_eq!(s.module("demo/buildSrc").unwrap().note().as_deref(), Some("buildSrc"));
    // The graph: a project, a library whose dependency lost a conflict, an unresolved one.
    let g = app.graph(Scope::Compile).unwrap();
    assert!(app.graph(Scope::Runtime).is_none());
    assert!(matches!(g.roots[2], Edge::Unresolved { .. }));
    let conflicts = g.conflicts();
    assert_eq!(conflicts.len(), 1);
    assert_eq!((conflicts[0].from, conflicts[0].to, conflicts[0].requested.as_str()), (Some(1), 3, "2.15.0"));
    assert_eq!(g.lost_versions(3), ["2.15.0"]);
    assert_eq!(g.brought_by(), [Some(0), Some(1), Some(0), Some(1)]);
    let paths = g.paths_to(|_, n| n.haystack().contains("guava"));
    assert_eq!(paths, [vec![0, 2]], "guava through the project that brings it in");
    let NodeKind::Project { build, path } = &g.nodes[0].kind else { panic!("a project") };
    assert_eq!(s.project_module(app, build, path).unwrap().id, "demo:core", "a project link finds its module");
}

#[test]
fn finds_declarations_in_scripts_catalogs_settings_and_the_wrapper() {
    let catalog = locate::parse_catalog(
        "[versions]\nguava = \"33\"\n[libraries]\njunit-bom = { module = \"org.junit:junit-bom\", version.ref = \"junit\" }\nlang = \"org.apache.commons:commons-lang3:3.14.0\"\nx = { group = \"g\", name = \"n\" }\n",
    );
    let found: Vec<(&str, &str, &str, u32)> =
        catalog.libraries.iter().map(|l| (l.alias.as_str(), l.group.as_str(), l.name.as_str(), l.line)).collect();
    assert_eq!(
        found,
        [
            ("junit-bom", "org.junit", "junit-bom", 3),
            ("lang", "org.apache.commons", "commons-lang3", 4),
            ("x", "g", "n", 5)
        ]
    );
    assert_eq!(locate::accessor("junit-bom"), "libs.junit.bom");
    let script = "dependencies {\n    // \"org.junit:junit-bom:5\" not here\n    testImplementation(platform(libs.junit.bom))\n    implementation 'org.apache.commons:commons-lang3:3.14.0'\n    implementation group: 'g', name: 'n', version: '1'\n    implementation(project(\":core\"))\n    api(projects.webApi)\n}\n";
    assert_eq!(locate::library_line(script, "org.junit", "junit-bom", Some(&catalog)), Some(2));
    assert_eq!(locate::library_line(script, "org.apache.commons", "commons-lang3", None), Some(3));
    assert_eq!(locate::library_line(script, "g", "n", None), Some(4));
    assert_eq!(locate::library_line(script, "org.apache.commons", "commons", None), None, "a prefix is not a match");
    assert_eq!(locate::project_line(script, ":core"), Some(5));
    assert_eq!(locate::project_line(script, ":web-api"), Some(6), "as projects.webApi");
    assert_eq!(locate::project_line(script, ":webApi"), Some(6));
    assert_eq!(
        locate::level_line("plugins {}\njava {\n    toolchain { languageVersion = JavaLanguageVersion.of(21) }\n}\n"),
        Some(2)
    );
    assert_eq!(locate::level_line("kotlin { jvmToolchain(17) }\n"), Some(0));
    assert_eq!(locate::level_line("tasks.withType<JavaCompile> { options.release.set(17) }\n"), Some(0));
    assert_eq!(locate::level_line("val releaseNotes = 1\n"), None);
    assert_eq!(locate::wrapper_line("a=b\ndistributionUrl=x\n"), Some(1));
    let settings = "rootProject.name = 'demo'\ninclude 'app', ':core'\nincludeBuild('tools')\n";
    assert_eq!(locate::include_line(settings, ":core", None), Some(1));
    assert_eq!(locate::include_line(settings, ":", Some("tools")), Some(2));
    assert_eq!(locate::include_line(settings, ":", None), Some(0));
    assert_eq!(
        locate::plugin_line("plugins {\n    id(\"org.springframework.boot\")\n}\n", "org.springframework.boot"),
        Some(1)
    );
}

#[test]
fn finds_a_modules_declarations_on_disk() {
    let root = scratch_dir("structure-locate");
    write_scripts(&root);
    let s = fixture(&root);
    let host = snout_host::local();
    let find = |id: &str, t: Target| locate::find(&*host, &s, s.module(id).unwrap(), &t).unwrap();
    let app = root.join("app/build.gradle.kts");
    let core = root.join("core/build.gradle.kts");
    let lib = |g: &str, n: &str| Target::Library { group: g.into(), name: n.into() };
    assert_eq!(
        find("demo:app", lib("org.springframework", "spring-web")),
        locate::Place { file: app.clone(), line: Some(7) }
    );
    assert_eq!(find("demo:app", Target::Project { path: ":core".into() }).line, Some(6));
    assert_eq!(
        find("demo:core", lib("com.google.guava", "guava")),
        locate::Place { file: core.clone(), line: Some(1) }
    );
    assert_eq!(find("demo:core", lib("com.google.guava", "guava-testlib")).line, Some(2));
    // Not in the module's script: the build's root script sets it for every subproject.
    assert_eq!(find("demo:core", Target::Level), locate::Place { file: root.join("build.gradle.kts"), line: Some(4) });
    assert_eq!(find("demo:core", Target::GradleVersion).line, Some(1));
    assert_eq!(
        find("demo:core", Target::Include),
        locate::Place { file: root.join("settings.gradle.kts"), line: Some(2) }
    );
    assert_eq!(find("demo/tools", Target::Include).line, Some(3));
    assert_eq!(
        find("demo/tools", Target::Level),
        locate::Place { file: root.join("tools/build.gradle"), line: Some(1) }
    );
    assert_eq!(find("demo:app", Target::Plugin("org.springframework.boot".into())).line, Some(2));
    // Not declared anywhere: the module's own script, at its top.
    assert_eq!(find("demo:app", lib("x", "y")), locate::Place { file: app, line: None });
    let place = locate::catalog_place(&*host, &s, s.module("demo:core").unwrap(), "com.google.guava", "guava").unwrap();
    assert_eq!(place, locate::Place { file: root.join("gradle/libs.versions.toml"), line: Some(5) });
    let config = super::spring_config_files(&*host, &s);
    let names: Vec<String> = config.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
    assert_eq!(
        names,
        ["application-dev.properties", "application.yml"],
        "the Spring configuration files, not the rest"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn the_pages_search_finds_them_by_title_and_keyword() {
    assert!(Page::Sdks.matches("jdk"));
    assert!(Page::Dependencies.matches("conflicts"));
    assert!(Page::Modules.matches("buildSrc"));
    assert!(Page::Sdks.matches("platform"));
    assert!(!Page::Facets.matches("transitive"));
    assert!(Page::ALL.iter().all(|p| p.matches("")));
}

#[test]
fn added_jdks_are_kept_per_machine() {
    let mut s = Settings::default();
    super::set_added_jdks(&mut s, None, vec![PathBuf::from("/opt/jdk-21")]);
    super::set_added_jdks(&mut s, Some("build-box"), vec![PathBuf::from("/usr/lib/jvm/jdk-17")]);
    assert_eq!(super::added_jdks(&s, None), [PathBuf::from("/opt/jdk-21")]);
    assert_eq!(super::added_jdks(&s, Some("build-box")), [PathBuf::from("/usr/lib/jvm/jdk-17")]);
    super::set_added_jdks(&mut s, Some("build-box"), Vec::new());
    assert!(s.remote_sdks.is_empty());
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let at = cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} is drawn")).center();
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
}

/// A JDK's folder as the app reads one: `bin/java` and a `release` file.
fn fake_jdk(dir: &Path, version: &str) {
    std::fs::create_dir_all(dir.join("bin")).unwrap();
    std::fs::write(dir.join("bin/java"), "").unwrap();
    std::fs::write(dir.join("release"), format!("JAVA_VERSION=\"{version}\"\n")).unwrap();
}

#[gpui::test]
fn the_window_shows_the_pages_and_keeps_what_snoutcode_owns(cx: &mut TestAppContext) {
    let repo = demo_repo("structure-window");
    write_scripts(&repo);
    let file = scratch_dir("structure-window-store").join("settings.json");
    let jdk = scratch_dir("structure-window-jdk").join("jdk-23");
    fake_jdk(&jdk, "23.0.1");
    let settings = with_repo(&repo);
    cx.update(|cx| crate::init(&settings, cx));
    let (model, ws, cx) = {
        let model = cx.update(|cx| {
            gpui::AppContext::new(cx, |cx| {
                crate::model::Model::new(settings.clone(), Some(crate::settings::Store::at(file.clone())), cx)
            })
        });
        let mut ws = None;
        let (_, vcx) = cx.add_window_view(|window, cx| {
            let view = gpui::AppContext::new(cx, |cx| crate::workspace::Workspace::new(model.clone(), window, cx));
            ws = Some(view.clone());
            gpui_component::Root::new(view, window, cx)
        });
        vcx.run_until_parked();
        (model, ws.unwrap(), vcx)
    };
    // ⌘; puts it up.
    cx.simulate_keystrokes(&crate::keymap::platform_keys("cmd-;"));
    let view = cx.update(|_, cx| ws.read(cx).project_structure.clone()).expect("the window is up");
    assert!(cx.debug_bounds("project-structure").is_some());
    cx.update(|_, cx| {
        view.update(cx, |v, cx| {
            v.set_structure(fixture(&repo), Vec::new());
            cx.notify()
        })
    });
    cx.run_until_parked();
    // Modules: the tree, and a source set's roots.
    click(cx, "structure-page-modules");
    for row in
        ["structure-module-app", "structure-module-core-main", "structure-module-tools", "structure-module-buildSrc"]
    {
        assert!(cx.debug_bounds(row).is_some(), "{row} is in the tree");
    }
    click(cx, "structure-module-core-test");
    cx.update(|_, cx| {
        let sel = view.read(cx).selected.clone();
        assert_eq!(sel, Some(super::ModuleSel::SourceSet("demo:core".into(), "test".into())));
    });
    // Dependencies: the module's declared ones; opening the project shows what it brings in.
    click(cx, "structure-page-dependencies");
    cx.update(|_, cx| view.update(cx, |v, cx| v.show_dependencies_of("demo:app".into(), cx)));
    cx.run_until_parked();
    assert!(cx.debug_bounds("structure-dep-2").is_some(), "the unresolved one is a row");
    assert!(cx.debug_bounds("structure-dep-3").is_none());
    click(cx, "structure-dep-fold-1");
    assert!(cx.debug_bounds("structure-dep-conflict-2").is_some(), "the version that lost shows");
    // The filter finds a transitive one through what brings it in.
    cx.update(|w, cx| {
        let f = view.read(cx).filter.clone();
        f.update(cx, |i, cx| i.set_value("guava", w, cx))
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("structure-dep-1").is_some() && cx.debug_bounds("structure-dep-2").is_none());
    // A project link goes to its module.
    click(cx, "structure-dep-link-0");
    cx.update(|_, cx| {
        let v = view.read(cx);
        assert_eq!((v.page, v.selected.clone()), (Page::Modules, Some(super::ModuleSel::Module("demo:core".into()))));
    });
    // SDKs: add one from a folder (its bundle folder will do), then pick it for the project.
    click(cx, "structure-page-sdks");
    cx.update(|_, cx| view.update(cx, |v, cx| v.add_jdk(jdk.clone(), cx)));
    cx.run_until_parked();
    cx.update(|_, cx| {
        let v = view.read(cx);
        assert_eq!(v.added, std::slice::from_ref(&jdk));
        assert!(v.all_jdks().iter().any(|j| j.home == jdk && j.major == 23));
        assert!(v.changed(cx));
    });
    cx.update(|_, cx| view.update(cx, |v, cx| v.set_jdk(Some(jdk.clone()), cx)));
    click(cx, "structure-ok");
    cx.run_until_parked();
    assert!(cx.update(|_, cx| ws.read(cx).project_structure.is_none()), "OK closes it");
    cx.update(|_, cx| {
        let m = model.read(cx);
        assert_eq!(m.project_settings.jdk.as_ref(), Some(&jdk));
        assert_eq!(m.user_settings().sdks, std::slice::from_ref(&jdk));
    });
    let written = std::fs::read_to_string(crate::project_settings::file_of(&repo)).unwrap();
    assert!(written.contains("jdk-23"), "the project's JDK is in its file: {written}");
    assert_eq!(
        crate::settings::Store::at(file.clone()).load().sdks,
        std::slice::from_ref(&jdk),
        "the JDK added is the user's"
    );
    // Edit in build file: the line that sets the level, in a tab.
    cx.dispatch_action(crate::menus::ProjectStructure);
    cx.run_until_parked();
    let view = cx.update(|_, cx| ws.read(cx).project_structure.clone()).expect("up again");
    cx.update(|_, cx| {
        view.update(cx, |v, cx| {
            v.set_structure(fixture(&repo), Vec::new());
            cx.notify()
        })
    });
    let opened = std::rc::Rc::new(std::cell::RefCell::new(None));
    let o = opened.clone();
    cx.update(|_, cx| {
        cx.subscribe(&view, move |_, e: &StructureEvent, _| {
            if let StructureEvent::Open(place) = e {
                *o.borrow_mut() = Some(place.clone());
            }
        })
        .detach()
    });
    cx.run_until_parked();
    click(cx, "structure-edit-level");
    let place = opened.borrow().clone().expect("a place is shown");
    assert_eq!(place, locate::Place { file: repo.join("build.gradle.kts"), line: Some(4) });
    cx.update(|_, cx| {
        assert!(ws.read(cx).project_structure.is_none(), "the window gives way to the file");
        assert!(ws.read(cx).editors(cx).iter().any(|e| e.read(cx).path == repo.join("build.gradle.kts")));
    });
    let _ = std::fs::remove_dir_all(&repo);
    let _ = std::fs::remove_dir_all(file.parent().unwrap());
    let _ = std::fs::remove_dir_all(jdk.parent().unwrap());
}
