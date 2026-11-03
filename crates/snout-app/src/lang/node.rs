//! Node, for TypeScript and Playwright projects: the version a project asks for (`.nvmrc`,
//! `.node-version`, `package.json`'s `volta` and `engines`), the Node installs a host has
//! (the login shell's `PATH`, nvm, Volta, fnm, asdf, mise, Homebrew, SnoutCode's own), and
//! the one that fits. Nothing here needs Node installed: when none fits, the app offers to
//! download one.

use std::path::{Path, PathBuf};

use serde_json::Value;
use snout_host::{Cmd, Host};

/// One Node install.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    /// The `node` program.
    pub bin: PathBuf,
    /// `22.11.0`.
    pub version: String,
    pub major: u32,
}

impl Node {
    /// The folder of `node`, where `npm` and `npx` lie beside it.
    pub fn bin_dir(&self) -> &Path {
        self.bin.parent().unwrap_or(Path::new("."))
    }
}

/// What a project asks of Node.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Wanted {
    /// An exact version (`22.11.0`), from Volta or a full `.nvmrc`.
    pub exact: Option<String>,
    /// A major version (`22`).
    pub major: Option<u32>,
    /// The oldest major that fits (`>=18`, `^20`).
    pub min_major: Option<u32>,
    /// Any long-term support release (`lts/*`, `lts/jod`): an even major.
    pub lts: bool,
    /// Where it was read from, for messages (`.nvmrc`).
    pub source: Option<String>,
}

impl Wanted {
    pub fn fits(&self, node: &Node) -> bool {
        if let Some(exact) = &self.exact {
            return &node.version == exact;
        }
        if let Some(major) = self.major {
            return node.major == major;
        }
        if let Some(min) = self.min_major
            && node.major < min
        {
            return false;
        }
        !self.lts || node.major.is_multiple_of(2)
    }

    /// The same, less strict: an exact version also takes its major.
    fn loosened(&self) -> Wanted {
        let major = self.exact.as_deref().and_then(major_of).or(self.major);
        Wanted { exact: None, major, ..self.clone() }
    }

    /// The major version to download when none fits: the one asked for, else the newest
    /// long-term support release SnoutCode knows.
    pub fn download_major(&self) -> u32 {
        self.exact.as_deref().and_then(major_of).or(self.major).unwrap_or(LTS_MAJOR).max(self.min_major.unwrap_or(0))
    }
}

/// The newest long-term support major, downloaded when a project asks for nothing.
pub const LTS_MAJOR: u32 = 22;

/// 22 for `v22.11.0`, `22.11`, `22`.
pub fn major_of(version: &str) -> Option<u32> {
    version.trim().trim_start_matches('v').split('.').next()?.parse().ok()
}

/// Read a version spec as `.nvmrc`, Volta and `engines` write them.
pub fn parse_spec(spec: &str) -> Wanted {
    let s = spec.trim();
    let lower = s.to_ascii_lowercase();
    if lower.starts_with("lts") {
        return Wanted { lts: true, ..Default::default() };
    }
    if matches!(lower.as_str(), "node" | "stable" | "latest" | "current" | "*" | "") {
        return Wanted::default();
    }
    // A range: `>=18`, `>=18.0.0 <23`, `^20.9`, `~20`, `18.x || 20.x`. Its lowest major fits.
    if s.contains(['>', '<', '^', '~', '|', 'x', '*', ' ']) {
        let lowest = s
            .split("||")
            .filter_map(|alt| {
                alt.split_whitespace().find(|p| !p.starts_with('<')).map(|p| p.trim_start_matches(['>', '=', '^', '~']))
            })
            .filter_map(major_of)
            .min();
        return Wanted { min_major: lowest, ..Default::default() };
    }
    let v = s.trim_start_matches('v');
    match v.split('.').count() {
        3 => Wanted { exact: Some(v.to_owned()), ..Default::default() },
        _ => Wanted { major: major_of(v), ..Default::default() },
    }
}

/// What the project at `dir` asks of Node, looking in `dir` and the folders above it up to
/// `stop`: `.nvmrc`, `.node-version`, then `package.json`'s `volta.node` and `engines.node`.
pub fn wanted(host: &dyn Host, dir: &Path, stop: Option<&Path>) -> Wanted {
    let mut at = Some(dir);
    while let Some(d) = at {
        for name in [".nvmrc", ".node-version"] {
            if let Ok(text) = host.read_to_string(&d.join(name))
                && let Some(line) = text.lines().map(str::trim).find(|l| !l.is_empty() && !l.starts_with('#'))
            {
                return Wanted { source: Some(name.to_owned()), ..parse_spec(line) };
            }
        }
        if let Ok(text) = host.read_to_string(&d.join("package.json"))
            && let Ok(json) = serde_json::from_str::<Value>(&text)
        {
            if let Some(v) = json.pointer("/volta/node").and_then(Value::as_str) {
                return Wanted { source: Some("package.json volta".into()), ..parse_spec(v) };
            }
            if let Some(v) = json.pointer("/engines/node").and_then(Value::as_str) {
                return Wanted { source: Some("package.json engines".into()), ..parse_spec(v) };
            }
        }
        if stop.is_some_and(|s| s == d) {
            break;
        }
        at = d.parent();
    }
    Wanted::default()
}

/// The `node` program's name on `host`.
fn node_name(host: &dyn Host) -> &'static str {
    if host.info().is_windows() { "node.exe" } else { "node" }
}

/// The Node at `bin`, asked for its version.
pub fn node_at(host: &dyn Host, bin: &Path) -> Option<Node> {
    if !host.is_file(bin) {
        return None;
    }
    let out = host.run(&Cmd::new(bin).arg("--version"), None).ok()?;
    let version = String::from_utf8_lossy(&out.stdout).trim().trim_start_matches('v').to_owned();
    let major = major_of(&version)?;
    Some(Node { bin: bin.to_owned(), version, major })
}

/// The Node installs in a version manager's folder of versions: `<dir>/<version>/<sub>/node`.
fn nodes_in(host: &dyn Host, dir: &Path, sub: &str) -> Vec<PathBuf> {
    let Ok(entries) = host.read_dir(dir) else { return Vec::new() };
    let name = node_name(host);
    entries.into_iter().filter(|e| e.is_dir).map(|e| e.path.join(sub).join(name)).filter(|p| host.is_file(p)).collect()
}

/// Where SnoutCode keeps the Node versions it downloads on a host.
pub fn downloads_dir(info: &snout_host::HostInfo) -> PathBuf {
    if info.pid == std::process::id() {
        let home = info.home.clone();
        snout_host::dirs::AppDirs::for_os(&info.os, &home, |n: &str| info.var(n).map(str::to_owned)).data.join("node")
    } else {
        info.home.join(".snout-code/node")
    }
}

/// Every Node `host` has where Node usually is, newest first, each install once.
pub fn discover(host: &dyn Host) -> Vec<Node> {
    let info = host.info();
    let home = info.home.clone();
    let name = node_name(host);
    let mut bins: Vec<PathBuf> = Vec::new();
    bins.extend(snout_host::which(host, &["node"], &[]));
    let nvm = info.var("NVM_DIR").map(PathBuf::from).unwrap_or_else(|| home.join(".nvm"));
    bins.extend(nodes_in(host, &nvm.join("versions/node"), "bin"));
    bins.extend(nodes_in(host, &home.join(".volta/tools/image/node"), "bin"));
    for fnm in [home.join(".local/share/fnm/node-versions"), home.join("Library/Application Support/fnm/node-versions")]
    {
        bins.extend(nodes_in(host, &fnm, "installation/bin"));
    }
    bins.extend(nodes_in(host, &home.join(".asdf/installs/nodejs"), "bin"));
    bins.extend(nodes_in(host, &home.join(".local/share/mise/installs/node"), "bin"));
    if info.is_windows() {
        if let Some(pf) = info.var("ProgramFiles") {
            bins.push(Path::new(pf).join("nodejs").join(name));
        }
    } else {
        for dir in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"] {
            bins.push(Path::new(dir).join(name));
        }
    }
    let ours = downloads_dir(info);
    bins.extend(nodes_in(host, &ours, if info.is_windows() { "" } else { "bin" }));
    let mut seen = std::collections::HashSet::new();
    let mut found: Vec<Node> = bins
        .into_iter()
        .filter(|b| seen.insert(host.canonicalize(b).unwrap_or_else(|_| b.clone())))
        .filter_map(|b| node_at(host, &b))
        .collect();
    found.sort_by_key(|n| std::cmp::Reverse(version_key(&n.version)));
    found
}

fn version_key(v: &str) -> Vec<u32> {
    v.split('.').map(|p| p.parse().unwrap_or(0)).collect()
}

/// The Node to use for `wanted` among `all` (newest first): the newest that fits, else the
/// newest of the major an exact version names. None when nothing fits.
pub fn choose<'a>(wanted: &Wanted, all: &'a [Node]) -> Option<&'a Node> {
    all.iter().find(|n| wanted.fits(n)).or_else(|| all.iter().find(|n| wanted.loosened().fits(n)))
}

/// How a project installs its packages, from its lockfile.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PackageManager {
    #[default]
    Npm,
    Pnpm,
    Yarn,
    Bun,
}

impl PackageManager {
    pub fn of(host: &dyn Host, dir: &Path) -> Self {
        let has = |f: &str| host.is_file(&dir.join(f));
        if has("pnpm-lock.yaml") {
            PackageManager::Pnpm
        } else if has("yarn.lock") {
            PackageManager::Yarn
        } else if has("bun.lockb") || has("bun.lock") {
            PackageManager::Bun
        } else {
            PackageManager::Npm
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            PackageManager::Npm => "npm",
            PackageManager::Pnpm => "pnpm",
            PackageManager::Yarn => "yarn",
            PackageManager::Bun => "bun",
        }
    }

    /// The command that runs a package's program (`npx playwright`, `pnpm exec playwright`).
    pub fn exec(self) -> &'static [&'static str] {
        match self {
            PackageManager::Npm => &["npx", "--no-install"],
            PackageManager::Pnpm => &["pnpm", "exec"],
            PackageManager::Yarn => &["yarn"],
            PackageManager::Bun => &["bunx"],
        }
    }
}
