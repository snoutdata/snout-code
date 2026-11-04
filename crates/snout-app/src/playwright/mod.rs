//! Playwright projects: a folder with a `playwright.config.*`, or a `package.json` that
//! depends on `@playwright/test`. Such a project opens as a project of its own kind: Node,
//! TypeScript and the Tests window, and never the Java engine.

use std::path::{Path, PathBuf};

use serde_json::Value;
use snout_host::Host;

use crate::lang::node::PackageManager;

pub mod env_dialog;
pub mod gutter;
pub mod history;
pub mod report;
pub mod run;
pub mod setup;
pub mod tests;
pub mod trace;
pub mod window;

/// The names Playwright looks for its configuration under, in its order.
pub const CONFIG_NAMES: [&str; 6] = [
    "playwright.config.ts",
    "playwright.config.js",
    "playwright.config.mts",
    "playwright.config.mjs",
    "playwright.config.cts",
    "playwright.config.cjs",
];

/// The package the test runner comes in.
pub const TEST_PACKAGE: &str = "@playwright/test";

/// What SnoutCode knows of a Playwright project without running anything.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
    pub root: PathBuf,
    /// Its `playwright.config.*`, when it has one.
    pub config: Option<PathBuf>,
    /// The version `package.json` asks for (`^1.48.0`).
    pub declared: Option<String>,
    /// The version installed in `node_modules` (`1.48.2`): none until the packages are.
    pub installed: Option<String>,
    pub package_manager: PackageManager,
}

impl Project {
    /// Whether its packages are installed (`node_modules` holds the test runner).
    pub fn installed(&self) -> bool {
        self.installed.is_some()
    }
}

/// The configuration file in `dir`, if there is one.
pub fn config_in(host: &dyn Host, dir: &Path) -> Option<PathBuf> {
    CONFIG_NAMES.iter().map(|n| dir.join(n)).find(|p| host.is_file(p))
}

/// The version of `@playwright/test` that `dir/package.json` lists, in any of its dependencies.
pub fn declared_version(host: &dyn Host, dir: &Path) -> Option<String> {
    let text = host.read_to_string(&dir.join("package.json")).ok()?;
    let json: Value = serde_json::from_str(&text).ok()?;
    ["devDependencies", "dependencies", "optionalDependencies"]
        .iter()
        .find_map(|k| json.get(k)?.get(TEST_PACKAGE)?.as_str().map(str::to_owned))
}

/// The version of the test runner installed for `dir`: in its `node_modules`, or one above
/// it (a workspace hoists packages to its root), up to `stop`.
pub fn installed_version(host: &dyn Host, dir: &Path, stop: Option<&Path>) -> Option<String> {
    let mut at = Some(dir);
    while let Some(d) = at {
        let manifest = d.join("node_modules").join(TEST_PACKAGE).join("package.json");
        if let Ok(text) = host.read_to_string(&manifest)
            && let Ok(json) = serde_json::from_str::<Value>(&text)
            && let Some(v) = json.get("version").and_then(Value::as_str)
        {
            return Some(v.to_owned());
        }
        if stop.is_some_and(|s| s == d) {
            break;
        }
        at = d.parent();
    }
    None
}

/// Whether `dir` is a Playwright project's folder.
pub fn is_root(host: &dyn Host, dir: &Path) -> bool {
    config_in(host, dir).is_some() || declared_version(host, dir).is_some()
}

/// The Playwright project at `dir`, if it is one.
pub fn project_at(host: &dyn Host, dir: &Path) -> Option<Project> {
    let config = config_in(host, dir);
    let declared = declared_version(host, dir);
    if config.is_none() && declared.is_none() {
        return None;
    }
    Some(Project {
        root: dir.to_owned(),
        config,
        declared,
        installed: installed_version(host, dir, None),
        package_manager: PackageManager::of(host, dir),
    })
}

/// The Playwright project `file` belongs to: the nearest folder above it, up to `stop`, that
/// is one.
pub fn root_of(host: &dyn Host, file: &Path, stop: Option<&Path>) -> Option<PathBuf> {
    let mut at = file.parent();
    while let Some(d) = at {
        if is_root(host, d) {
            return Some(d.to_owned());
        }
        if stop.is_some_and(|s| s == d) {
            break;
        }
        at = d.parent();
    }
    None
}

/// Whether `path` names a test file as Playwright's default `testMatch` has them
/// (`*.spec.ts`, `*.test.js`, …).
pub fn is_test_file(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    let Some((stem, ext)) = name.rsplit_once('.') else { return false };
    matches!(ext, "ts" | "tsx" | "js" | "jsx" | "mts" | "mjs" | "cts" | "cjs")
        && (stem.ends_with(".spec") || stem.ends_with(".test"))
}
