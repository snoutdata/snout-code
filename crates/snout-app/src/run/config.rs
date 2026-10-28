//! Run configurations: those the engine finds in the build (`snout/runConfigurations`: Spring
//! Boot apps, main classes, `bootRun` and `run`, each module's tests) and those the user
//! makes or changes, kept with the project's settings outside the repository, with the
//! configuration picked last and the breakpoints.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// A main class, launched as jdtls compiled it.
    #[default]
    Main,
    /// A Gradle task (`bootRun`, `run`).
    Gradle,
    /// A module's tests.
    Tests,
}

impl Kind {
    pub const ALL: [Kind; 3] = [Kind::Main, Kind::Gradle, Kind::Tests];

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "main" => Some(Kind::Main),
            "gradle" => Some(Kind::Gradle),
            "tests" => Some(Kind::Tests),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Main => "main",
            Kind::Gradle => "gradle",
            Kind::Tests => "tests",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Kind::Main => "Application",
            Kind::Gradle => "Gradle",
            Kind::Tests => "Tests",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RunConfig {
    pub id: String,
    pub kind: Kind,
    pub name: String,
    pub module: String,
    pub build: Option<String>,
    pub main_class: Option<String>,
    pub task: Option<String>,
    pub spring_boot: bool,
    /// Program arguments (for a Gradle configuration, Gradle's own).
    pub args: Vec<String>,
    /// VM options.
    pub jvm_args: Vec<String>,
    pub env: BTreeMap<String, String>,
    /// Spring profiles to activate, separated by commas.
    pub profiles: String,
    pub working_dir: Option<String>,
    /// Made or changed by the user (not as the build has it).
    pub user: bool,
}

fn strings(v: &Value, key: &str) -> Vec<String> {
    v.get(key).and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(str::to_owned).collect()
}

impl RunConfig {
    /// A configuration as `snout/runConfigurations` lists it.
    pub fn from_engine(v: &Value) -> Option<Self> {
        let s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_owned);
        let kind = Kind::parse(&s("kind")?)?;
        Some(Self {
            id: s("id")?,
            kind,
            name: s("name").unwrap_or_default(),
            module: s("module").unwrap_or_default(),
            build: s("build"),
            main_class: s("mainClass"),
            task: s("task"),
            spring_boot: v.get("springBoot").and_then(Value::as_bool).unwrap_or(false),
            args: strings(v, "args"),
            jvm_args: strings(v, "jvmArgs"),
            env: v
                .get("env")
                .and_then(Value::as_object)
                .map(|m| m.iter().filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_owned()))).collect())
                .unwrap_or_default(),
            profiles: String::new(),
            working_dir: s("workingDir"),
            user: false,
        })
    }

    /// The configuration as `snout/run` takes it: profiles become the JVM's (or, for a
    /// Gradle task, the application's) `spring.profiles.active`.
    pub fn to_engine(&self) -> Value {
        let mut jvm_args = self.jvm_args.clone();
        let mut args = self.args.clone();
        let profiles = self.profiles.trim();
        if !profiles.is_empty() {
            match self.kind {
                Kind::Main => jvm_args.push(format!("-Dspring.profiles.active={profiles}")),
                Kind::Gradle => args.push(format!("--args=--spring.profiles.active={profiles}")),
                Kind::Tests => jvm_args.push(format!("-Dspring.profiles.active={profiles}")),
            }
        }
        let mut v = json!({
            "id": self.id, "kind": self.kind.as_str(), "name": self.name, "module": self.module,
            "args": args, "jvmArgs": jvm_args, "springBoot": self.spring_boot, "env": self.env,
        });
        let o = v.as_object_mut().expect("an object");
        if let Some(b) = &self.build {
            o.insert("build".into(), json!(b));
        }
        if let Some(c) = &self.main_class {
            o.insert("mainClass".into(), json!(c));
        }
        if let Some(t) = &self.task {
            o.insert("task".into(), json!(t));
        }
        if let Some(d) = &self.working_dir {
            o.insert("workingDir".into(), json!(d));
        }
        v
    }

    /// Where the selector files it.
    pub fn group(&self) -> &'static str {
        match self.kind {
            Kind::Main if self.spring_boot => "Spring Boot",
            Kind::Main => "Application",
            Kind::Gradle => "Gradle",
            Kind::Tests => "Tests",
        }
    }
}

/// Split a command line into arguments: whitespace separates them, quotes keep them whole.
pub fn split_args(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut any = false;
    for c in line.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => cur.push(c),
            (None, '"' | '\'') => {
                quote = Some(c);
                any = true;
            }
            (None, c) if c.is_whitespace() => {
                if any || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                    any = false;
                }
            }
            (None, c) => cur.push(c),
        }
    }
    if any || !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Arguments back into one line, quoting those with spaces.
pub fn join_args(args: &[String]) -> String {
    args.iter()
        .map(|a| if a.is_empty() || a.contains(char::is_whitespace) { format!("\"{a}\"") } else { a.clone() })
        .collect::<Vec<_>>()
        .join(" ")
}

/// `A=1; B=two` into a map (`;` or new lines between them).
pub fn parse_env(text: &str) -> BTreeMap<String, String> {
    text.split([';', '\n'])
        .filter_map(|kv| {
            let (k, v) = kv.split_once('=')?;
            let k = k.trim();
            (!k.is_empty()).then(|| (k.to_owned(), v.trim().to_owned()))
        })
        .collect()
}

pub fn env_text(env: &BTreeMap<String, String>) -> String {
    env.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("; ")
}

/// A line breakpoint; the line from 0.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Breakpoint {
    pub path: PathBuf,
    pub line: u32,
    #[serde(default)]
    pub condition: Option<String>,
}

/// What the app keeps for one project, outside its folder.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectSettings {
    /// The user's configurations, and the build's as the user changed them (same id).
    pub configurations: Vec<RunConfig>,
    /// The configuration the title bar shows; none for Current File.
    pub selected: Option<String>,
    pub breakpoints: Vec<Breakpoint>,
}

impl ProjectSettings {
    /// The saved settings, as far as this build can read them (see [`crate::settings::lenient`]).
    pub fn load(file: &Path) -> Self {
        std::fs::read_to_string(file)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .map(crate::settings::lenient)
            .unwrap_or_default()
    }

    pub fn save(&self, file: &Path) -> std::io::Result<()> {
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        let tmp = file.with_extension("json.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(tmp, file)
    }
}

/// Where a project's settings live under the app's settings folder `support`.
pub fn project_file(support: &Path, root: &Path) -> PathBuf {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in root.to_string_lossy().bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    let name = root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    support.join("projects").join(format!("{name}-{h:016x}")).join("project.json")
}

/// The build's configurations with the user's changes over them, then the user's own.
pub fn merged(engine: &[RunConfig], user: &[RunConfig]) -> Vec<RunConfig> {
    let mut out: Vec<RunConfig> =
        engine.iter().map(|c| user.iter().find(|u| u.id == c.id).cloned().unwrap_or_else(|| c.clone())).collect();
    out.extend(user.iter().filter(|u| !engine.iter().any(|c| c.id == u.id)).cloned());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_engines_configurations_and_sends_them_back_with_profiles() {
        let v = json!({ "id": "main:shop:app:demo.App", "kind": "main", "name": "App", "module": "shop:app",
            "build": "shop", "mainClass": "demo.App", "springBoot": true, "args": ["--x"], "jvmArgs": [],
            "workingDir": "/w/shop/app" });
        let mut c = RunConfig::from_engine(&v).unwrap();
        assert_eq!(c.group(), "Spring Boot");
        c.profiles = "dev, local".into();
        c.env.insert("PORT".into(), "8081".into());
        let back = c.to_engine();
        assert_eq!(back["jvmArgs"], json!(["-Dspring.profiles.active=dev, local"]));
        assert_eq!(back["mainClass"], "demo.App");
        assert_eq!(back["env"]["PORT"], "8081");
        assert_eq!(back["args"], json!(["--x"]));
        let task = RunConfig::from_engine(&json!({ "id": "g", "kind": "gradle", "task": ":app:bootRun" })).unwrap();
        let mut task = task;
        task.profiles = "dev".into();
        assert_eq!(task.to_engine()["args"], json!(["--args=--spring.profiles.active=dev"]));
    }

    #[test]
    fn arguments_and_environment_round_trip() {
        assert_eq!(split_args(r#"--name "A B" -x 'c d' """#), ["--name", "A B", "-x", "c d", ""]);
        assert_eq!(join_args(&["--name".into(), "A B".into()]), r#"--name "A B""#);
        let env = parse_env("A=1; B = two ;bad");
        assert_eq!(env.get("B").map(String::as_str), Some("two"));
        assert_eq!(env_text(&env), "A=1; B=two");
    }

    #[test]
    fn the_users_changes_replace_the_builds_configuration() {
        let a = RunConfig { id: "a".into(), name: "A".into(), ..Default::default() };
        let b = RunConfig { id: "b".into(), name: "B".into(), ..Default::default() };
        let a2 = RunConfig { id: "a".into(), name: "A (mine)".into(), user: true, ..Default::default() };
        let c = RunConfig { id: "user:1".into(), name: "C".into(), user: true, ..Default::default() };
        let names: Vec<String> = merged(&[a, b], &[a2, c]).into_iter().map(|c| c.name).collect();
        assert_eq!(names, ["A (mine)", "B", "C"]);
    }

    #[test]
    fn project_settings_save_and_load() {
        let dir = std::env::temp_dir().join(format!("snout-project-settings-{}", std::process::id()));
        let file = project_file(&dir, Path::new("/w/shop"));
        assert!(file.to_string_lossy().contains("/projects/shop-"));
        let s = ProjectSettings {
            selected: Some("a".into()),
            breakpoints: vec![Breakpoint { path: "/w/A.java".into(), line: 3, condition: Some("i > 2".into()) }],
            ..Default::default()
        };
        s.save(&file).unwrap();
        assert_eq!(ProjectSettings::load(&file), s);
        let _ = std::fs::remove_dir_all(dir);
    }
}
