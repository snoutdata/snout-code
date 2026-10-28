//! What the Java engine needs around it: where its distribution is, which Gradle build a
//! file belongs to, which JDK that build needs, the JDKs installed, and downloading one
//! from Adoptium when none fits.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// The engine distribution: `SNOUT_ENGINE_DIST`, else inside the app bundle
/// (`Contents/Resources/engine`), else `engine/build/dist` of the workspace the binary was
/// built in (development).
pub fn engine_dist() -> Option<PathBuf> {
    let ok = |d: &Path| d.join("runtime/bin/java").is_file() && d.join("lib").is_dir();
    if let Some(d) = std::env::var_os("SNOUT_ENGINE_DIST").map(PathBuf::from) {
        return ok(&d).then_some(d);
    }
    let exe = std::env::current_exe().ok()?;
    for dir in exe.ancestors().skip(1) {
        for candidate in [dir.join("Resources/engine"), dir.join("engine/build/dist")] {
            if ok(&candidate) {
                return Some(candidate);
            }
        }
    }
    let built = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../engine/build/dist");
    ok(&built).then(|| built.canonicalize().unwrap_or(built))
}

/// The command that runs `snout-engine lsp` on `project` with the distribution's own Java,
/// Gradle running on `jdk`.
pub fn engine_command(dist: &Path, project: &Path, jdk: &Path) -> Command {
    let mut c = Command::new(dist.join("runtime/bin/java"));
    c.arg("-cp")
        .arg(format!("{}/*", dist.join("lib").display()))
        .arg("com.snoutcode.engine.cli.SnoutEngine")
        .arg("lsp")
        .arg(project)
        .arg("--java-home")
        .arg(jdk)
        .current_dir(project);
    c
}

const SETTINGS: [&str; 2] = ["settings.gradle", "settings.gradle.kts"];
const BUILDS: [&str; 2] = ["build.gradle", "build.gradle.kts"];

/// Whether `dir` is the root of a Gradle build (it has a settings or build script).
pub fn is_gradle_root(dir: &Path) -> bool {
    SETTINGS.iter().chain(BUILDS.iter()).any(|f| dir.join(f).is_file())
}

/// The Gradle build `file` belongs to: the nearest folder above it with a settings script,
/// else the nearest with a build script, looking no higher than `stop`.
pub fn gradle_root(file: &Path, stop: Option<&Path>) -> Option<PathBuf> {
    let mut nearest_build = None;
    for dir in file.ancestors().skip(1) {
        if SETTINGS.iter().any(|f| dir.join(f).is_file()) {
            return Some(dir.to_owned());
        }
        if nearest_build.is_none() && BUILDS.iter().any(|f| dir.join(f).is_file()) {
            nearest_build = Some(dir.to_owned());
        }
        if stop.is_some_and(|s| dir == s) {
            break;
        }
    }
    nearest_build
}

/// An installed JDK.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Jdk {
    pub home: PathBuf,
    /// The feature release: 8, 11, 17, 21…
    pub major: u32,
    pub version: String,
}

/// The JDK at `home`, read from its `release` file.
pub fn jdk_at(home: &Path) -> Option<Jdk> {
    if !home.join("bin/java").is_file() {
        return None;
    }
    let release = std::fs::read_to_string(home.join("release")).ok()?;
    let version =
        release.lines().find_map(|l| l.strip_prefix("JAVA_VERSION=")).map(|v| v.trim().trim_matches('"').to_owned())?;
    let major = major_of(&version)?;
    Some(Jdk { home: home.to_owned(), major, version })
}

/// 17 for "17.0.2", 8 for "1.8.0_392".
pub fn major_of(version: &str) -> Option<u32> {
    let mut parts = version.split(['.', '_', '-', '+']);
    let first: u32 = parts.next()?.parse().ok()?;
    if first == 1 { parts.next()?.parse().ok() } else { Some(first) }
}

/// The JDK homes in a folder of JDKs: each entry, or its `Contents/Home` (macOS bundles).
fn jdks_in(dir: &Path) -> Vec<Jdk> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out = Vec::new();
    for e in entries.flatten() {
        let p = e.path();
        for home in [p.join("Contents/Home"), p.clone()] {
            if let Some(jdk) = jdk_at(&home) {
                out.push(jdk);
                break;
            }
        }
    }
    out
}

/// Where SnoutCode keeps the JDKs it downloads.
pub fn downloads_dir(home: &Path) -> PathBuf {
    home.join("Library/Application Support/SnoutCode/jdks")
}

/// Every JDK this Mac has where JDKs usually are: what `java_home` lists, the user's and
/// the system's JavaVirtualMachines, SDKMAN, Gradle's toolchains, and SnoutCode's own.
pub fn discover(home: &Path) -> Vec<Jdk> {
    let mut found = Vec::new();
    if let Ok(out) = Command::new("/usr/libexec/java_home").arg("-V").output() {
        // It lists on stderr: `    17.0.2 (arm64) "Vendor" - "Name" /path/to/home`.
        for line in String::from_utf8_lossy(&out.stderr).lines() {
            if let Some(at) = line.find(" /")
                && let Some(jdk) = jdk_at(Path::new(line[at + 1..].trim()))
            {
                found.push(jdk);
            }
        }
    }
    for dir in [
        home.join("Library/Java/JavaVirtualMachines"),
        PathBuf::from("/Library/Java/JavaVirtualMachines"),
        home.join(".sdkman/candidates/java"),
        home.join(".gradle/jdks"),
        downloads_dir(home),
    ] {
        found.extend(jdks_in(&dir));
    }
    let mut seen = std::collections::HashSet::new();
    found.retain(|j| seen.insert(j.home.canonicalize().unwrap_or_else(|_| j.home.clone())));
    found
}

/// What a build asks of the JDK running Gradle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Requirement {
    /// The newest Java level its scripts ask for (source compatibility, toolchain, release).
    pub declared: Option<u32>,
    /// The newest Java its Gradle version runs on.
    pub gradle_max: Option<u32>,
}

impl Requirement {
    /// The JDK to download when none fits.
    pub fn wanted(&self) -> u32 {
        self.declared.or(self.gradle_max.map(|m| m.min(21))).unwrap_or(21)
    }

    pub fn fits(&self, major: u32) -> bool {
        self.declared.is_none_or(|d| major >= d) && self.gradle_max.is_none_or(|m| major <= m)
    }
}

/// The newest Java each Gradle release runs on, from Gradle's compatibility matrix.
pub fn gradle_max_java(gradle: &str) -> Option<u32> {
    let mut parts = gradle.split(['.', '-']).map(|p| p.parse::<u32>().unwrap_or(0));
    let v = (parts.next()?, parts.next().unwrap_or(0));
    let table = [
        ((9, 1), 25),
        ((8, 14), 24),
        ((8, 10), 23),
        ((8, 8), 22),
        ((8, 5), 21),
        ((8, 3), 20),
        ((7, 6), 19),
        ((7, 5), 18),
        ((7, 3), 17),
        ((7, 0), 16),
        ((6, 7), 15),
        ((6, 3), 14),
        ((6, 0), 13),
        ((5, 4), 12),
        ((5, 0), 11),
    ];
    table.iter().find(|(at, _)| v >= *at).map(|(_, j)| *j)
}

/// The Java levels a build script names, in the forms Gradle scripts use.
pub fn declared_levels(script: &str) -> Vec<u32> {
    let keys = [
        "JavaVersion.VERSION_",
        "sourceCompatibility",
        "targetCompatibility",
        "JavaLanguageVersion.of(",
        "jvmToolchain(",
        "release.set(",
        "options.release",
        "jvmTarget",
    ];
    let mut out = Vec::new();
    for key in keys {
        let mut rest = script;
        while let Some(at) = rest.find(key) {
            rest = &rest[at + key.len()..];
            let tail: String = rest.chars().take(24).take_while(|c| *c != '\n').collect();
            let digits: String = tail
                .trim_start_matches(|c: char| !c.is_ascii_digit())
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '_')
                .collect();
            if let Some(level) = major_of(&digits.replace('_', ".")).filter(|l| (6..=40).contains(l)) {
                out.push(level);
            }
        }
    }
    out
}

/// What the build at `root` needs: the levels its scripts declare (its own and its
/// subprojects', one level down) and its Gradle wrapper's version.
pub fn requirement(root: &Path) -> Requirement {
    let mut scripts: Vec<PathBuf> = SETTINGS.iter().chain(BUILDS.iter()).map(|f| root.join(f)).collect();
    if let Ok(entries) = std::fs::read_dir(root) {
        for e in entries.flatten() {
            if e.path().is_dir() {
                scripts.extend(BUILDS.iter().map(|f| e.path().join(f)));
            }
        }
    }
    let declared =
        scripts.iter().filter_map(|p| std::fs::read_to_string(p).ok()).flat_map(|s| declared_levels(&s)).max();
    let wrapper = std::fs::read_to_string(root.join("gradle/wrapper/gradle-wrapper.properties")).unwrap_or_default();
    let gradle = wrapper
        .lines()
        .find_map(|l| l.strip_prefix("distributionUrl="))
        .and_then(|url| url.rsplit('/').next())
        .and_then(|name| name.strip_prefix("gradle-"))
        .map(|v| v.trim_end_matches(".zip").trim_end_matches("-bin").trim_end_matches("-all").to_owned());
    Requirement { declared, gradle_max: gradle.as_deref().and_then(gradle_max_java) }
}

/// The JDK to run the build on: the declared level exactly if installed, else the oldest
/// newer one Gradle still runs on; with nothing declared, the newest Gradle runs on.
pub fn choose<'a>(req: &Requirement, jdks: &'a [Jdk]) -> Option<&'a Jdk> {
    let mut fit: Vec<&Jdk> = jdks.iter().filter(|j| req.fits(j.major)).collect();
    match req.declared {
        Some(d) => {
            fit.sort_by_key(|j| (j.major != d, j.major));
            fit.first().copied()
        }
        None => fit.into_iter().max_by_key(|j| j.major),
    }
}

/// How a download is going: bytes so far, and the total once known.
#[derive(Clone, Default)]
pub struct DownloadProgress {
    pub done: Arc<AtomicU64>,
    pub total: Arc<AtomicU64>,
}

impl DownloadProgress {
    pub fn percent(&self) -> Option<u32> {
        let total = self.total.load(Ordering::Relaxed);
        (total > 0).then(|| ((self.done.load(Ordering::Relaxed) * 100) / total).min(100) as u32)
    }
}

/// Download Eclipse Temurin `major` for this Mac from api.adoptium.net into `dir`, check
/// its SHA-256, unpack it, and return the new JDK. Blocking; run it off the main thread.
pub fn download_temurin(major: u32, dir: &Path, progress: &DownloadProgress) -> anyhow::Result<Jdk> {
    use anyhow::{Context as _, bail};
    let arch = if cfg!(target_arch = "aarch64") { "aarch64" } else { "x64" };
    let api = format!(
        "https://api.adoptium.net/v3/assets/latest/{major}/hotspot?architecture={arch}&image_type=jdk&os=mac&vendor=eclipse"
    );
    let out = Command::new("curl").args(["-fsSL", "--max-time", "60", &api]).output().context("run curl")?;
    if !out.status.success() {
        bail!("Adoptium did not answer: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    let assets: serde_json::Value = serde_json::from_slice(&out.stdout).context("read Adoptium's answer")?;
    let package = assets.get(0).and_then(|a| a.pointer("/binary/package")).context("no JDK for this Mac")?;
    let link = package.get("link").and_then(|v| v.as_str()).context("no download link")?;
    let sha = package.get("checksum").and_then(|v| v.as_str()).unwrap_or_default().to_owned();
    let size = package.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
    progress.total.store(size, Ordering::Relaxed);

    std::fs::create_dir_all(dir)?;
    let part = dir.join(format!(".temurin-{major}.tar.gz.part"));
    let mut curl = Command::new("curl").args(["-fsSL", "-o"]).arg(&part).arg(link).spawn().context("run curl")?;
    let status = loop {
        if let Some(status) = curl.try_wait()? {
            break status;
        }
        if let Ok(m) = std::fs::metadata(&part) {
            progress.done.store(m.len(), Ordering::Relaxed);
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    };
    if !status.success() {
        let _ = std::fs::remove_file(&part);
        bail!("the download failed");
    }
    progress.done.store(size, Ordering::Relaxed);
    if !sha.is_empty() {
        let out = Command::new("shasum").args(["-a", "256"]).arg(&part).output()?;
        let actual = String::from_utf8_lossy(&out.stdout).split_whitespace().next().unwrap_or_default().to_owned();
        if !actual.eq_ignore_ascii_case(&sha) {
            let _ = std::fs::remove_file(&part);
            bail!("the download is damaged (its checksum does not match)");
        }
    }
    let unpack = dir.join(format!(".temurin-{major}.unpack"));
    let _ = std::fs::remove_dir_all(&unpack);
    std::fs::create_dir_all(&unpack)?;
    let ok = Command::new("tar").arg("-xzf").arg(&part).arg("-C").arg(&unpack).status()?.success();
    let _ = std::fs::remove_file(&part);
    if !ok {
        bail!("the download could not be unpacked");
    }
    let top =
        std::fs::read_dir(&unpack)?.flatten().map(|e| e.path()).find(|p| p.is_dir()).context("an empty archive")?;
    let name = top.file_name().map(|n| n.to_owned()).context("an unnamed folder")?;
    let dest = dir.join(name);
    let _ = std::fs::remove_dir_all(&dest);
    std::fs::rename(&top, &dest)?;
    let _ = std::fs::remove_dir_all(&unpack);
    [dest.join("Contents/Home"), dest.clone()].iter().find_map(|h| jdk_at(h)).context("the download holds no JDK")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_java_levels_from_build_scripts() {
        let groovy = "java {\n\tsourceCompatibility = '17'\n}\ntargetCompatibility = JavaVersion.VERSION_1_8\n";
        assert_eq!(declared_levels(groovy), [8, 17]);
        let kts = "kotlin { jvmToolchain(21) }\njava { toolchain { languageVersion = JavaLanguageVersion.of(11) } }";
        let mut l = declared_levels(kts);
        l.sort();
        assert_eq!(l, [11, 21]);
        assert_eq!(declared_levels("sourceCompatibility = 1.8"), [8]);
        assert!(declared_levels("version = '1.0.3'").is_empty());
    }

    #[test]
    fn knows_which_java_each_gradle_runs_on() {
        assert_eq!(gradle_max_java("8.1.1"), Some(19));
        assert_eq!(gradle_max_java("8.10.2"), Some(23));
        assert_eq!(gradle_max_java("7.3"), Some(17));
        assert_eq!(gradle_max_java("6.9.4"), Some(15));
        assert_eq!(gradle_max_java("4.10"), None);
        assert_eq!(major_of("1.8.0_392"), Some(8));
        assert_eq!(major_of("17.0.21"), Some(17));
    }

    fn jdk(major: u32) -> Jdk {
        Jdk { home: PathBuf::from(format!("/j/{major}")), major, version: format!("{major}.0.1") }
    }

    #[test]
    fn chooses_the_declared_jdk_or_the_nearest_newer_gradle_runs_on() {
        let jdks = [jdk(11), jdk(17), jdk(21), jdk(25)];
        let bamboo = Requirement { declared: Some(17), gradle_max: Some(19) };
        assert_eq!(choose(&bamboo, &jdks).unwrap().major, 17);
        assert_eq!(choose(&bamboo, &[jdk(11), jdk(21)]), None, "21 is too new for Gradle 8.1");
        assert_eq!(bamboo.wanted(), 17);
        let loose = Requirement { declared: Some(11), gradle_max: Some(23) };
        assert_eq!(choose(&loose, &[jdk(17), jdk(21)]).unwrap().major, 17);
        let none = Requirement { declared: None, gradle_max: Some(23) };
        assert_eq!(choose(&none, &jdks).unwrap().major, 21);
        assert_eq!(Requirement::default().wanted(), 21);
    }

    #[test]
    fn finds_the_gradle_build_of_a_file_and_what_it_needs() {
        let dir = std::env::temp_dir().join(format!("snout-gradle-root-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let src = dir.join("app/src/main/java/demo");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(dir.join("gradle/wrapper")).unwrap();
        std::fs::write(dir.join("settings.gradle"), "include 'app'\n").unwrap();
        std::fs::write(dir.join("app/build.gradle"), "java { sourceCompatibility = '17' }\n").unwrap();
        std::fs::write(
            dir.join("gradle/wrapper/gradle-wrapper.properties"),
            "distributionUrl=https\\://services.gradle.org/distributions/gradle-8.1.1-bin.zip\n",
        )
        .unwrap();
        assert_eq!(gradle_root(&src.join("App.java"), None), Some(dir.clone()));
        assert_eq!(requirement(&dir), Requirement { declared: Some(17), gradle_max: Some(19) });
        assert_eq!(gradle_root(&std::env::temp_dir().join("nowhere/X.java"), None), None);
        let _ = std::fs::remove_dir_all(dir);
    }
}
