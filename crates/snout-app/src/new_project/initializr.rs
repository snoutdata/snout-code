//! What start.spring.io offers (its client metadata): Spring Boot versions, Java versions,
//! and the dependencies in their groups; or, without the network, what the offline
//! template knows.

use std::path::Path;
use std::process::Command;

use anyhow::{Context as _, bail};
use serde_json::Value;

use super::generate::{KNOWN, OFFLINE_BOOT};

/// The service the wizard asks.
pub const START_SPRING_IO: &str = "https://start.spring.io";

#[derive(Clone, Debug, PartialEq)]
pub struct Dependency {
    pub id: String,
    pub name: String,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Catalog {
    /// Spring Boot versions: id and label, newest first, and the one to start on.
    pub boot: Vec<(String, String)>,
    pub boot_default: String,
    /// Java versions, newest first, and the one to start on.
    pub java: Vec<u32>,
    pub java_default: u32,
    /// The dependencies, in their groups.
    pub groups: Vec<(String, Vec<Dependency>)>,
    /// Made from the offline template's list, not start.spring.io's.
    pub offline: bool,
}

impl Catalog {
    /// start.spring.io's client metadata, read.
    pub fn parse(v: &Value) -> Option<Self> {
        let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
        let boot: Vec<(String, String)> = v
            .pointer("/bootVersion/values")?
            .as_array()?
            .iter()
            .map(|b| (s(b, "id"), s(b, "name")))
            .filter(|(id, _)| !id.is_empty())
            .collect();
        let java: Vec<u32> =
            v.pointer("/javaVersion/values")?.as_array()?.iter().filter_map(|j| s(j, "id").parse().ok()).collect();
        let groups = v
            .pointer("/dependencies/values")?
            .as_array()?
            .iter()
            .map(|g| {
                let deps = g
                    .get("values")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .map(|d| Dependency { id: s(d, "id"), name: s(d, "name"), description: s(d, "description") })
                    .filter(|d| !d.id.is_empty())
                    .collect();
                (s(g, "name"), deps)
            })
            .collect();
        let boot_default = v.pointer("/bootVersion/default").and_then(Value::as_str).map(str::to_owned);
        let java_default = v.pointer("/javaVersion/default").and_then(Value::as_str).and_then(|j| j.parse().ok());
        Some(Self {
            boot_default: boot_default.or_else(|| boot.first().map(|b| b.0.clone()))?,
            java_default: java_default.or_else(|| java.first().copied())?,
            boot,
            java,
            groups,
            offline: false,
        })
    }

    /// What the offline template knows, on `boot` (start.spring.io's last default, when it
    /// once answered).
    pub fn offline(boot: Option<String>) -> Self {
        let boot = boot.unwrap_or_else(|| OFFLINE_BOOT.to_owned());
        let mut groups: Vec<(String, Vec<Dependency>)> = Vec::new();
        for k in KNOWN {
            let d = Dependency { id: k.id.into(), name: k.name.into(), description: k.description.into() };
            match groups.iter_mut().find(|(g, _)| g == k.group) {
                Some((_, list)) => list.push(d),
                None => groups.push((k.group.into(), vec![d])),
            }
        }
        Self {
            boot: vec![(boot.clone(), boot.clone())],
            boot_default: boot,
            java: vec![25, 21, 17],
            java_default: 17,
            groups,
            offline: true,
        }
    }

    pub fn dependency(&self, id: &str) -> Option<&Dependency> {
        self.groups.iter().flat_map(|(_, d)| d.iter()).find(|d| d.id == id)
    }
}

/// Ask `base` for its metadata (blocking). The answer is kept in `cache`, so a later offline
/// start uses the Spring Boot version it named.
pub fn fetch(base: &str, cache: Option<&Path>) -> anyhow::Result<Catalog> {
    let out = Command::new("curl")
        .args([
            "-fsSL",
            "--max-time",
            "20",
            "--connect-timeout",
            "8",
            "-H",
            "Accept: application/vnd.initializr.v2.2+json",
        ])
        .arg(base)
        .output()
        .context("run curl")?;
    if !out.status.success() {
        let why = String::from_utf8_lossy(&out.stderr).trim().trim_start_matches("curl: ").to_owned();
        bail!("{why}");
    }
    let v: Value = serde_json::from_slice(&out.stdout).context("an answer that is not JSON")?;
    let catalog = Catalog::parse(&v).context("an answer without versions or dependencies")?;
    if let Some(cache) = cache {
        if let Some(dir) = cache.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(cache, serde_json::json!({ "bootDefault": catalog.boot_default }).to_string());
    }
    Ok(catalog)
}

/// The Spring Boot version start.spring.io named last time it answered.
pub fn cached_boot(cache: &Path) -> Option<String> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(cache).ok()?).ok()?;
    v.get("bootDefault").and_then(Value::as_str).map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_metadata_and_falls_back_to_the_known_list() {
        let v = serde_json::json!({
            "bootVersion": { "default": "4.1.2", "values": [
                { "id": "4.2.0-RC1", "name": "4.2.0 (RC1)" }, { "id": "4.1.2", "name": "4.1.2" } ] },
            "javaVersion": { "default": "17", "values": [ { "id": "25" }, { "id": "21" }, { "id": "17" } ] },
            "dependencies": { "values": [ { "name": "Web", "values": [
                { "id": "web", "name": "Spring Web", "description": "Build web apps." } ] } ] }
        });
        let c = Catalog::parse(&v).unwrap();
        assert_eq!((c.boot_default.as_str(), c.java_default), ("4.1.2", 17));
        assert_eq!(c.boot.len(), 2);
        assert_eq!(c.dependency("web").unwrap().name, "Spring Web");
        assert!(Catalog::parse(&serde_json::json!({})).is_none());
        let off = Catalog::offline(Some("4.0.9".into()));
        assert!(off.offline);
        assert_eq!(off.boot_default, "4.0.9");
        assert!(off.dependency("data-jpa").is_some());
        // A service that is not there: the reason, in curl's words.
        let e = fetch("http://127.0.0.1:9", None).unwrap_err().to_string();
        assert!(!e.is_empty());
    }
}
