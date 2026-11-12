//! What kind of project a folder is. This is the one place that decides it: the setup bar, the
//! Project view's line, Project Settings and the agents all ask [`Kind::of`], so a folder is
//! never one kind in one place and another kind elsewhere. `project_kind_tests.rs` holds the
//! shapes repositories really have.
//!
//! Playwright is a test tool, not a kind of its own, except in a repository that holds nothing
//! else (a QA suite): a web app with `playwright.config.ts` beside its `vite.config.ts` is a
//! Node project that has Playwright tests.

use std::path::Path;

use serde_json::Value;
use snout_host::Host;

/// What a folder is, as its root says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A Gradle or Maven build ([`crate::project_model::declares_java`]); `playwright`: it
    /// also has Playwright tests at its root.
    Java { playwright: bool },
    /// A Node package (a `package.json`): an app, a library, a workspace; `playwright`: it
    /// also has Playwright tests.
    Node { playwright: bool },
    /// A Playwright suite and nothing else: its packages are the test runner and what the
    /// tests use, and its scripts run the tests.
    Playwright,
    /// None of these: a plain folder.
    Plain,
}

impl Kind {
    /// What `dir` is. Reads a few files at its root (and, to tell a Java build, folders a few
    /// levels down), so callers on the UI thread ask it in the background.
    pub fn of(host: &dyn Host, dir: &Path) -> Kind {
        let playwright = crate::playwright::is_root(host, dir);
        if crate::project_model::declares_java(host, dir) {
            return Kind::Java { playwright };
        }
        let manifest = host
            .read_to_string(&dir.join("package.json"))
            .ok()
            .map(|text| serde_json::from_str::<Value>(&text).unwrap_or(Value::Null));
        match (manifest, playwright) {
            (Some(json), true) if is_an_app(host, dir, &json) => Kind::Node { playwright: true },
            (_, true) => Kind::Playwright,
            (Some(_), false) => Kind::Node { playwright: false },
            (None, false) => Kind::Plain,
        }
    }

    /// Whether it is a project at all (Project Settings and the rest are on), not a plain folder.
    pub fn is_project(self) -> bool {
        self != Kind::Plain
    }

    /// Whether it has Playwright tests, as a suite of its own or beside an app.
    pub fn has_playwright(self) -> bool {
        matches!(self, Kind::Playwright | Kind::Java { playwright: true } | Kind::Node { playwright: true })
    }

    /// Whether it is a Node package itself: a Node project or a Playwright suite.
    pub fn is_node_package(self) -> bool {
        matches!(self, Kind::Node { .. } | Kind::Playwright)
    }
}

/// Packages a Playwright suite may need at run time without being an app: the runner itself,
/// and what tests commonly use to read settings, make data, call an API or report.
const TEST_HELPERS: [&str; 12] = [
    "@playwright/test",
    "playwright",
    "playwright-core",
    "dotenv",
    "@faker-js/faker",
    "cross-env",
    "allure-playwright",
    "monocart-reporter",
    "axios",
    "zod",
    "uuid",
    "@axe-core/playwright",
];

/// The configuration files of the tools that build or serve an app.
const APP_FILES: [&str; 16] = [
    "index.html",
    "vite.config.ts",
    "vite.config.js",
    "vite.config.mjs",
    "next.config.js",
    "next.config.mjs",
    "next.config.ts",
    "nuxt.config.ts",
    "svelte.config.js",
    "astro.config.mjs",
    "angular.json",
    "webpack.config.js",
    "remix.config.js",
    "vue.config.js",
    "nest-cli.json",
    "wrangler.toml",
];

/// The scripts an app has and a test suite does not. Not `build`: a suite may type check its
/// tests with one.
const APP_SCRIPTS: [&str; 5] = ["dev", "start", "serve", "preview", "deploy"];

/// Whether the package at `dir`, which has Playwright tests, is more than its tests: it depends
/// on something a test suite would not, a tool that builds or serves an app is set up beside it,
/// or a script serves or starts something other than Playwright.
fn is_an_app(host: &dyn Host, dir: &Path, json: &Value) -> bool {
    let runtime = json.get("dependencies").and_then(Value::as_object);
    if runtime.is_some_and(|deps| deps.keys().any(|k| !TEST_HELPERS.contains(&k.as_str()))) {
        return true;
    }
    if APP_FILES.iter().any(|f| host.is_file(&dir.join(f))) {
        return true;
    }
    json.get("workspaces").is_some()
        || json.get("scripts").and_then(Value::as_object).is_some_and(|scripts| {
            scripts.iter().any(|(name, command)| {
                APP_SCRIPTS.contains(&name.as_str()) && command.as_str().is_some_and(|c| !c.contains("playwright"))
            })
        })
}
