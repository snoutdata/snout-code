//! What kind of project a folder is, for the shapes repositories really have. The kind decides
//! what SnoutCode starts by itself, so a new rule or a new kind adds its rows here.

use std::path::Path;

use snout_host::Host;

use crate::project_kind::Kind as K;
use crate::tests::scratch_dir;

use Kind::*;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    Java,
    JavaAndPlaywright,
    Node,
    NodeAndPlaywright,
    Playwright,
    Neither,
}

fn kind_of(host: &dyn Host, dir: &Path) -> Kind {
    match K::of(host, dir) {
        K::Java { playwright: false } => Java,
        K::Java { playwright: true } => JavaAndPlaywright,
        K::Node { playwright: false } => Node,
        K::Node { playwright: true } => NodeAndPlaywright,
        K::Playwright => Playwright,
        K::Plain => Neither,
    }
}

/// A scratch folder holding `files`; a `package.json` among them is a valid, plain one. A
/// file written `name=text` holds that text.
fn folder(name: &str, files: &[&str]) -> std::path::PathBuf {
    let dir = scratch_dir(&format!("kind-{name}"));
    for file in files {
        let (file, text) = file.split_once('=').unwrap_or((file, ""));
        let path = dir.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let text = if text.is_empty() && file.ends_with("package.json") { r#"{ "name": "x" }"# } else { text };
        std::fs::write(&path, text).unwrap();
    }
    dir
}

/// Checks every row and names all the wrong ones at once.
fn check<'a>(rows: impl IntoIterator<Item = (&'a str, &'a [&'a str], &'a str, Kind)>) {
    let host = snout_host::local();
    let wrong: Vec<String> = rows
        .into_iter()
        .filter_map(|(name, files, project, want)| {
            let dir = folder(name, files);
            let got = kind_of(&*host, &dir.join(project));
            let _ = std::fs::remove_dir_all(dir);
            (got != want).then(|| format!("{name}: {got:?}, should be {want:?}"))
        })
        .collect();
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

/// Rows whose project is the folder itself.
fn check_roots(rows: &[(&str, &[&str], Kind)]) {
    check(rows.iter().map(|&(name, files, want)| (name, files, "", want)));
}

#[test]
fn java_projects() {
    check_roots(&[
        ("gradle-groovy", &["settings.gradle"], Java),
        ("gradle-kotlin", &["settings.gradle.kts", "build.gradle.kts"], Java),
        ("gradle-single", &["build.gradle"], Java),
        ("maven", &["pom.xml"], Java),
        ("maven-modules", &["pom.xml", "api/pom.xml", "api/src/main/java/A.java"], Java),
        ("android-app", &["settings.gradle", "app/src/main/java/Main.java"], Java),
        ("java-with-front-end-folder", &["settings.gradle", "frontend/package.json"], Java),
    ]);
}

#[test]
fn java_builds_with_a_package_json_beside_them() {
    check_roots(&[
        ("spring-and-npm", &["pom.xml", "package.json", "src/main/java/App.java"], Java),
        ("modules-and-npm", &["settings.gradle", "package.json", "app/src/main/kotlin/App.kt"], Java),
        ("scala-and-npm", &["build.gradle", "package.json", "src/main/scala/App.scala"], Java),
        ("groovy-tests-and-npm", &["build.gradle", "package.json", "src/test/groovy/T.groovy"], Java),
    ]);
}

#[test]
fn folders_of_services_with_no_language_at_the_root() {
    check_roots(&[
        ("services", &["orders/pom.xml", "billing/build.gradle"], Java),
        ("three-levels-down", &["a/b/c/pom.xml"], Java),
        ("four-levels-down", &["a/b/c/d/pom.xml"], Neither),
    ]);
}

#[test]
fn node_projects_stay_node_whatever_is_inside() {
    check_roots(&[
        ("node", &["package.json"], Node),
        ("workspaces", &["package.json", "packages/a/package.json"], Node),
        ("cypress", &["package.json", "cypress.config.ts", "cypress/e2e/a.cy.ts"], Node),
        (
            "cypress-with-maven-module",
            &["package.json", "cypress.config.ts", "tools/report/pom.xml", "tools/report/src/main/java/Report.java"],
            Node,
        ),
        (
            "react-native",
            &["package.json", "android/build.gradle", "android/app/src/main/java/Main.java", "ios/Podfile"],
            Node,
        ),
        (
            "node-with-java-service",
            &["package.json", "services/api/pom.xml", "services/api/src/main/java/A.java"],
            Node,
        ),
        ("gradle-only-installed", &["package.json", "node_modules/x/android/build.gradle"], Node),
    ]);
}

#[test]
fn a_build_file_that_compiles_no_java_does_not_make_node_java() {
    check_roots(&[
        ("pom-packages-node", &["pom.xml", "package.json", "src/index.ts"], Node),
        ("gradle-packages-node", &["build.gradle.kts", "package.json"], Node),
        ("pom-and-react-native", &["build.gradle", "package.json", "android/app/src/main/java/Main.java"], Node),
        ("sources-only-in-output", &["pom.xml", "package.json", "target/src/main/java/A.java"], Node),
        ("sources-only-installed", &["pom.xml", "package.json", "node_modules/x/src/main/java/A.java"], Node),
    ]);
}

#[test]
fn playwright_projects() {
    check_roots(&[
        ("playwright", &["package.json", "playwright.config.ts"], Playwright),
        ("playwright-with-maven-module", &["package.json", "playwright.config.ts", "tools/pom.xml"], Playwright),
        ("playwright-pom-packages-it", &["package.json", "playwright.config.ts", "pom.xml"], Playwright),
        (
            "spring-with-playwright",
            &["pom.xml", "package.json", "playwright.config.ts", "src/main/java/App.java"],
            JavaAndPlaywright,
        ),
    ]);
}

/// An app with Playwright tests stays a Node project: what it depends on, the tool that builds
/// it or a script that serves it says it is more than its tests.
#[test]
fn an_app_with_playwright_tests_is_a_node_project() {
    check_roots(&[
        (
            "vite-app",
            &["package.json", "playwright.config.ts", "vite.config.ts", "index.html", "src/main.tsx"],
            NodeAndPlaywright,
        ),
        (
            "app-by-its-dependencies",
            &[
                r#"package.json={ "dependencies": { "react": "^19.0.0" }, "devDependencies": { "@playwright/test": "^1.62.1" } }"#,
                "playwright.config.ts",
            ],
            NodeAndPlaywright,
        ),
        (
            "app-by-its-scripts",
            &[
                r#"package.json={ "scripts": { "dev": "node server.js", "test": "playwright test" } }"#,
                "playwright.config.ts",
            ],
            NodeAndPlaywright,
        ),
        (
            "workspace-with-e2e",
            &[r#"package.json={ "workspaces": ["apps/*"] }"#, "playwright.config.ts", "apps/web/package.json"],
            NodeAndPlaywright,
        ),
        (
            "app-known-by-its-dependency-alone",
            &[
                r#"package.json={ "dependencies": { "express": "^5.0.0" }, "devDependencies": { "@playwright/test": "^1.62.1" } }"#,
            ],
            NodeAndPlaywright,
        ),
    ]);
}

/// A QA suite stays a Playwright project, whatever helpers its tests use and scripts run them.
#[test]
fn a_test_suite_with_helpers_is_still_a_playwright_project() {
    check_roots(&[
        (
            "suite-with-helpers",
            &[
                r#"package.json={ "dependencies": { "dotenv": "^16.0.0", "@faker-js/faker": "^9.0.0" }, "devDependencies": { "@playwright/test": "^1.62.1" }, "scripts": { "test": "playwright test", "build": "tsc --noEmit", "start": "playwright test --ui" } }"#,
                "playwright.config.ts",
                "tests/login.spec.ts",
            ],
            Playwright,
        ),
        ("config-without-a-package", &["playwright.config.js", "tests/a.spec.js"], Playwright),
    ]);
}

#[test]
fn a_playwright_project_can_be_known_by_its_dependency_alone() {
    let host = snout_host::local();
    let dir = folder("playwright-declared", &[]);
    let manifest = r#"{ "devDependencies": { "@playwright/test": "^1.50.0" } }"#;
    std::fs::write(dir.join("package.json"), manifest).unwrap();
    assert_eq!(kind_of(&*host, &dir), Playwright);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn other_languages_projects_are_no_java_whatever_is_inside() {
    let rows: Vec<(String, Vec<String>)> =
        ["pyproject.toml", "requirements.txt", "go.mod", "Cargo.toml", "composer.json", "Gemfile", "mix.exs"]
            .iter()
            .map(|f| (format!("other-{f}"), vec![(*f).to_owned(), "tools/pom.xml".to_owned()]))
            .collect();
    let refs: Vec<(String, Vec<&str>)> =
        rows.iter().map(|(n, f)| (n.clone(), f.iter().map(String::as_str).collect())).collect();
    check(refs.iter().map(|(n, f)| (n.as_str(), f.as_slice(), "", Neither)));
    check_roots(&[("flutter", &["pubspec.yaml", "android/build.gradle", "android/app/src/main/java/M.java"], Neither)]);
}

#[test]
fn folders_of_no_kind() {
    check_roots(&[
        ("empty", &[], Neither),
        ("notes", &["readme.md", "docs/a.md"], Neither),
        ("java-files-without-a-build", &["src/main/java/App.java"], Neither),
        ("builds-only-in-output", &["build/x/pom.xml", "out/y/pom.xml", "target/z/pom.xml"], Neither),
        ("builds-only-hidden", &[".cache/x/pom.xml"], Neither),
    ]);
}

#[test]
fn a_project_inside_something_larger() {
    check([
        ("module-of-repo", &[".git/HEAD", "settings.gradle", "app/x.txt"][..], "app", Java),
        ("nested-module-of-repo", &[".git/HEAD", "pom.xml", "a/b/x.txt"][..], "a/b", Java),
        ("pom-above-the-repo", &["pom.xml", "repo/.git/HEAD", "repo/x.txt"][..], "repo", Neither),
        ("pom-above-a-folder-of-no-repo", &["pom.xml", "notes/x.txt"][..], "notes", Neither),
        ("node-in-a-java-repo", &[".git/HEAD", "settings.gradle", "web/package.json"][..], "web", Node),
        ("submodule-below-a-build", &["pom.xml", "lib/.git", "lib/x.txt"][..], "lib", Neither),
    ]);
}
