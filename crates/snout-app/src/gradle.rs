//! What the engine knows of a Gradle build: the model it writes when it imports the build
//! (`~/Library/Caches/SnoutCode/jdtls/<build>/model.json`), read for the Gradle tool window
//! and the run configurations. The tasks are those the applied plugins add, and the test
//! tasks the model lists.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde_json::Value;

/// A project of the build and its tasks (`:app:test`, …).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
    /// Gradle's path: `:` for the root, `:app` for a subproject.
    pub path: String,
    pub name: String,
    pub tasks: Vec<String>,
}

impl Project {
    /// The task's full path, as `gradle` takes it.
    pub fn task_path(&self, task: &str) -> String {
        if self.path == ":" { format!(":{task}") } else { format!("{}:{task}", self.path) }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Build {
    pub root: PathBuf,
    pub projects: Vec<Project>,
}

/// The tasks a plugin adds, by the plugin's class.
const PLUGIN_TASKS: &[(&str, &[&str])] = &[
    ("org.gradle.language.base.plugins.LifecycleBasePlugin", &["assemble", "build", "check", "clean"]),
    ("org.gradle.api.plugins.JavaPlugin", &["classes", "compileJava", "jar", "javadoc"]),
    ("org.gradle.api.plugins.ApplicationPlugin", &["run", "installDist", "distZip"]),
    ("org.gradle.api.plugins.HelpTasksPlugin", &["dependencies", "help", "projects", "tasks"]),
];

/// The build in `json` (the engine's model).
pub fn parse(json: &Value) -> Build {
    let mut out = Build {
        root: json.get("rootDir").and_then(Value::as_str).map(PathBuf::from).unwrap_or_default(),
        projects: Vec::new(),
    };
    let builds = json.get("builds").and_then(Value::as_array).cloned().unwrap_or_default();
    for build in builds.iter().take(1) {
        for p in build.get("projects").and_then(Value::as_array).into_iter().flatten() {
            let classes: Vec<&str> = p
                .get("plugins")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|pl| pl.get("class").and_then(Value::as_str))
                .collect();
            let mut tasks: Vec<String> = PLUGIN_TASKS
                .iter()
                .filter(|(class, _)| classes.contains(class))
                .flat_map(|(_, tasks)| tasks.iter().map(|t| (*t).to_owned()))
                .collect();
            for t in p.get("testTasks").and_then(Value::as_array).into_iter().flatten() {
                if let Some(name) = t.get("name").and_then(Value::as_str) {
                    tasks.push(name.to_owned());
                }
            }
            tasks.sort();
            tasks.dedup();
            out.projects.push(Project {
                path: p.get("path").and_then(Value::as_str).unwrap_or(":").to_owned(),
                name: p.get("name").and_then(Value::as_str).unwrap_or_default().to_owned(),
                tasks,
            });
        }
    }
    out
}

/// The model files the engine wrote, under `home`.
fn models(home: &Path) -> Vec<PathBuf> {
    let dir = home.join("Library/Caches/SnoutCode/jdtls");
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    entries.flatten().map(|e| e.path().join("model.json")).filter(|p| p.is_file()).collect()
}

/// The build at `root` as the engine last imported it, and when it did.
pub fn load(home: &Path, root: &Path) -> Option<(Build, SystemTime)> {
    let want = root.canonicalize().unwrap_or_else(|_| root.to_owned());
    for file in models(home) {
        let Ok(text) = std::fs::read_to_string(&file) else { continue };
        let Ok(json) = serde_json::from_str::<Value>(&text) else { continue };
        let Some(dir) = json.get("rootDir").and_then(Value::as_str) else { continue };
        let dir = Path::new(dir);
        if dir.canonicalize().unwrap_or_else(|_| dir.to_owned()) == want {
            let when = std::fs::metadata(&file).and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
            return Some((parse(&json), when));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tasks_come_from_the_plugins_and_the_test_tasks() {
        let json = serde_json::json!({
            "rootDir": "/w/demo",
            "builds": [{ "projects": [
                { "path": ":", "name": "demo", "plugins": [{ "class": "org.gradle.api.plugins.HelpTasksPlugin" }], "testTasks": [] },
                { "path": ":app", "name": "app", "plugins": [
                    { "class": "org.gradle.language.base.plugins.LifecycleBasePlugin" },
                    { "id": "org.gradle.java", "class": "org.gradle.api.plugins.JavaPlugin" }
                ], "testTasks": [{ "name": "test", "path": ":app:test" }] }
            ]}]
        });
        let build = parse(&json);
        assert_eq!(build.root, Path::new("/w/demo"));
        assert_eq!(build.projects[0].tasks, ["dependencies", "help", "projects", "tasks"]);
        let app = &build.projects[1];
        assert!(app.tasks.contains(&"build".to_owned()) && app.tasks.contains(&"test".to_owned()));
        assert_eq!(app.task_path("test"), ":app:test");
        assert_eq!(build.projects[0].task_path("help"), ":help");
    }
}
