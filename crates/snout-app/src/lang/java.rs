//! What the Java engine needs around it: where its distribution is, which Gradle build a
//! file belongs to, which JDK that build needs, the JDKs installed, and downloading one
//! from Adoptium when none fits. All of it on the workspace's [`Host`]: blocking, so off the
//! UI thread.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use snout_host::{Cmd, Host, Stdio};

/// The engine distribution on `host`: `SNOUT_ENGINE_DIST`, else inside the app bundle
/// (`Contents/Resources/engine`) or beside the program serving the host, else
/// `engine/build/dist` of the workspace the binary was built in (development).
pub fn engine_dist(host: &dyn Host) -> Option<PathBuf> {
    let ok = |d: &Path| host.is_file(&d.join("runtime/bin/java")) && host.is_dir(&d.join("lib"));
    if let Some(d) = host.info().var("SNOUT_ENGINE_DIST").map(PathBuf::from) {
        return ok(&d).then_some(d);
    }
    // On a remote machine: the copy SnoutCode installed there (see `remote_engine`).
    if is_remote(host) {
        return super::remote_engine::installed(host);
    }
    let exe = host.info().exe.clone()?;
    for dir in exe.ancestors().skip(1) {
        for candidate in [dir.join("Resources/engine"), dir.join("engine/build/dist")] {
            if ok(&candidate) {
                return Some(candidate);
            }
        }
    }
    let built = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../engine/build/dist");
    ok(&built).then(|| host.canonicalize(&built).unwrap_or(built))
}

/// The command that runs `snout-engine lsp` on `project` with the distribution's own Java,
/// Gradle running on `jdk`, its Gradle daemons stopping after `idle_minutes` without a build.
pub fn engine_command(host: &dyn Host, dist: &Path, project: &Path, jdk: &Path, idle_minutes: u64) -> Cmd {
    let mut c = Cmd::new(dist.join("runtime/bin/java"))
        .arg("-cp")
        .arg(format!("{}/*", dist.join("lib").display()))
        .arg("com.snoutcode.engine.cli.SnoutEngine")
        .arg("lsp")
        .arg(project)
        .arg("--java-home")
        .arg(jdk)
        // The cache the app looks in for the model the engine writes (the same as the
        // engine's own default, unless HOME points elsewhere).
        .arg("--cache-dir")
        .arg(engine_cache(host))
        .arg("--daemon-idle-minutes")
        .arg(idle_minutes.max(1).to_string())
        .cwd(project);
    // The JDKs the app downloaded, for builds that want another Java than `jdk`.
    for extra in downloaded_jdks(host) {
        if extra.home != jdk {
            c = c.arg("--jdk").arg(extra.home);
        }
    }
    c
}

/// The JDKs the app downloaded, under the user's home on `host`.
pub fn downloaded_jdks(host: &dyn Host) -> Vec<Jdk> {
    jdks_in(host, &downloads_dir(host.info()))
}

/// SnoutCode's cache folder on `host`, where the engine keeps its builds, models and jdtls data.
pub fn engine_cache(host: &dyn Host) -> PathBuf {
    crate::project_model::cache_dir(host.info())
}

const SETTINGS: [&str; 2] = ["settings.gradle", "settings.gradle.kts"];
const BUILDS: [&str; 2] = ["build.gradle", "build.gradle.kts"];

/// Whether `dir` is the root of a Gradle build (it has a settings or build script).
pub fn is_gradle_root(host: &dyn Host, dir: &Path) -> bool {
    SETTINGS.iter().chain(BUILDS.iter()).any(|f| host.is_file(&dir.join(f)))
}

/// The Gradle build `file` belongs to: the nearest folder above it with a settings script,
/// else the nearest with a build script, looking no higher than `stop`.
pub fn gradle_root(host: &dyn Host, file: &Path, stop: Option<&Path>) -> Option<PathBuf> {
    let mut nearest_build = None;
    for dir in file.ancestors().skip(1) {
        if SETTINGS.iter().any(|f| host.is_file(&dir.join(f))) {
            return Some(dir.to_owned());
        }
        if nearest_build.is_none() && BUILDS.iter().any(|f| host.is_file(&dir.join(f))) {
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
pub fn jdk_at(host: &dyn Host, home: &Path) -> Option<Jdk> {
    if !host.is_file(&home.join("bin/java")) {
        return None;
    }
    let release = host.read_to_string(&home.join("release")).ok()?;
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
pub fn jdks_in(host: &dyn Host, dir: &Path) -> Vec<Jdk> {
    let Ok(entries) = host.read_dir(dir) else { return Vec::new() };
    let mut out = Vec::new();
    for e in entries {
        let p = e.path;
        for home in [p.join("Contents/Home"), p.clone()] {
            if let Some(jdk) = jdk_at(host, &home) {
                out.push(jdk);
                break;
            }
        }
    }
    out
}

/// Where SnoutCode keeps the JDKs it downloads on a host: in its folder of Application
/// Support on a Mac, in `~/.snout-code/jdks` on a remote machine.
pub fn downloads_dir(info: &snout_host::HostInfo) -> PathBuf {
    downloads_under(info.is_mac(), &info.home)
}

fn downloads_under(mac: bool, home: &Path) -> PathBuf {
    if mac { home.join("Library/Application Support/SnoutCode/jdks") } else { home.join(".snout-code/jdks") }
}

/// Every JDK `host` has where JDKs usually are. On a Mac: what `java_home` lists, the
/// user's and the system's JavaVirtualMachines. On Linux: `JAVA_HOME` (of the login shell),
/// the `java` on `PATH`, and the distributions' folders (`/usr/lib/jvm`, …). On both:
/// SDKMAN, Gradle's toolchains, and SnoutCode's own.
pub fn discover(host: &dyn Host, home: &Path) -> Vec<Jdk> {
    let mut found = Vec::new();
    if !host.info().is_mac() {
        let mut homes: Vec<PathBuf> = host.info().var("JAVA_HOME").map(PathBuf::from).into_iter().collect();
        if let Some(java) = snout_host::which(host, &["java"], &[])
            && let Ok(real) = host.canonicalize(&java)
            && let Some(home) = real.parent().and_then(Path::parent)
        {
            homes.push(home.to_owned());
        }
        found.extend(homes.iter().filter_map(|h| jdk_at(host, h)));
        for dir in ["/usr/lib/jvm", "/usr/java", "/usr/local/java", "/opt/java", "/opt/jdk"] {
            found.extend(jdks_in(host, Path::new(dir)));
        }
    } else if let Ok(out) = host.run(&Cmd::new("/usr/libexec/java_home").arg("-V"), None) {
        // It lists on stderr: `    17.0.2 (arm64) "Vendor" - "Name" /path/to/home`.
        for line in String::from_utf8_lossy(&out.stderr).lines() {
            if let Some(at) = line.find(" /")
                && let Some(jdk) = jdk_at(host, Path::new(line[at + 1..].trim()))
            {
                found.push(jdk);
            }
        }
    }
    let mac = host.info().is_mac();
    let mac_dirs = [home.join("Library/Java/JavaVirtualMachines"), PathBuf::from("/Library/Java/JavaVirtualMachines")];
    for dir in mac_dirs.into_iter().filter(|_| mac).chain([
        home.join(".sdkman/candidates/java"),
        home.join(".gradle/jdks"),
        downloads_under(mac, home),
    ]) {
        found.extend(jdks_in(host, &dir));
    }
    let mut seen = std::collections::HashSet::new();
    found.retain(|j| seen.insert(host.canonicalize(&j.home).unwrap_or_else(|_| j.home.clone())));
    found
}

/// [`discover`], and the JDKs added by hand at `added` (their homes) that it did not find.
pub fn discover_with(host: &dyn Host, home: &Path, added: &[PathBuf]) -> Vec<Jdk> {
    let mut found = discover(host, home);
    for h in added {
        if let Some(jdk) = jdk_at(host, h)
            && !found.iter().any(|j| j.home == jdk.home)
        {
            found.push(jdk);
        }
    }
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
        self.wanted_on(cfg!(target_arch = "aarch64"))
    }

    /// [`Self::wanted`] on a Mac with Apple silicon (`arm`) or not. Temurin publishes no
    /// Java 8 for Apple silicon: there a Java 8 build gets 11, the oldest it publishes, which
    /// builds and runs Java 8 code.
    pub fn wanted_on(&self, arm: bool) -> u32 {
        // A long-term support release: the newest the build's Gradle runs on.
        let lts = |max: u32| [25, 21, 17, 11, 8].into_iter().find(|v| *v <= max.min(21)).unwrap_or(8);
        let want = self.declared.or(self.gradle_max.map(lts)).unwrap_or(21);
        if arm && want < 11 && self.gradle_max.is_none_or(|m| m >= 11) { 11 } else { want }
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
pub fn requirement(host: &dyn Host, root: &Path) -> Requirement {
    let mut scripts: Vec<PathBuf> = SETTINGS.iter().chain(BUILDS.iter()).map(|f| root.join(f)).collect();
    if let Ok(entries) = host.read_dir(root) {
        for e in entries {
            if e.is_dir {
                scripts.extend(BUILDS.iter().map(|f| e.path.join(f)));
            }
        }
    }
    let declared = scripts.iter().filter_map(|p| host.read_to_string(p).ok()).flat_map(|s| declared_levels(&s)).max();
    let wrapper = host.read_to_string(&root.join("gradle/wrapper/gradle-wrapper.properties")).unwrap_or_default();
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

/// Whether `host` is another machine than the one the app runs on.
pub fn is_remote(host: &dyn Host) -> bool {
    host.info().pid != std::process::id()
}

/// The text at `url`, fetched on `host` with curl (else wget).
fn fetch_text(host: &dyn Host, url: &str) -> anyhow::Result<Vec<u8>> {
    let curl = Cmd::new("curl").args(["-fsSL", "--max-time", "60", url]);
    let wget = Cmd::new("wget").args(["-q", "-T", "60", "-O", "-", url]);
    let mut last = anyhow::anyhow!("neither curl nor wget is installed");
    for cmd in [curl, wget] {
        match host.run(&cmd, None) {
            Ok(out) if out.status.success() => return Ok(out.stdout),
            Ok(out) if out.status.code == Some(127) => {}
            Ok(out) => last = anyhow::anyhow!("{}", String::from_utf8_lossy(&out.stderr).trim().to_owned()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => last = e.into(),
        }
    }
    Err(last)
}

/// Fetch `url` into `out` on `host` with curl (else wget), counting its bytes in `progress`.
fn fetch_file(host: &dyn Host, url: &str, out: &Path, progress: &DownloadProgress) -> anyhow::Result<()> {
    let curl = Cmd::new("curl").args(["-fsSL", "-o"]).arg(out).arg(url);
    let wget = Cmd::new("wget").args(["-q", "-O"]).arg(out).arg(url);
    let mut last = anyhow::anyhow!("neither curl nor wget is installed");
    for cmd in [curl, wget] {
        let mut child = match host.spawn(&cmd.stdout(Stdio::Null).stderr(Stdio::Null)) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.into()),
        };
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if let Ok(m) = host.metadata(out) {
                progress.done.store(m.len, Ordering::Relaxed);
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        };
        if status.success() {
            return Ok(());
        }
        let _ = host.remove_file(out);
        if status.code == Some(127) {
            continue;
        }
        last = anyhow::anyhow!("the download failed ({status})");
        break;
    }
    Err(last)
}

/// The SHA-256 of `path` on `host`, in hex.
pub fn sha256_on(host: &dyn Host, path: &Path) -> anyhow::Result<String> {
    for cmd in [Cmd::new("sha256sum").arg(path), Cmd::new("shasum").args(["-a", "256"]).arg(path)] {
        if let Ok(out) = host.run(&cmd, None)
            && out.status.success()
        {
            let hash = String::from_utf8_lossy(&out.stdout).split_whitespace().next().unwrap_or_default().to_owned();
            return Ok(hash.to_ascii_lowercase());
        }
    }
    anyhow::bail!("neither sha256sum nor shasum could read {}", path.display())
}

/// Copy the local file `from` to `to` on `host`, in pieces, counting them in `progress`.
pub fn upload(host: &dyn Host, from: &Path, to: &Path, progress: &DownloadProgress) -> anyhow::Result<()> {
    use std::io::Read as _;
    const PIECE: usize = 4 << 20;
    let mut file = std::fs::File::open(from)?;
    progress.total.store(file.metadata()?.len(), Ordering::Relaxed);
    progress.done.store(0, Ordering::Relaxed);
    let _ = host.remove_file(to);
    host.write(to, b"")?;
    let mut buf = vec![0u8; PIECE];
    loop {
        let mut n = 0;
        while n < PIECE {
            let got = file.read(&mut buf[n..])?;
            if got == 0 {
                break;
            }
            n += got;
        }
        if n == 0 {
            return Ok(());
        }
        host.append(to, &buf[..n])?;
        progress.done.fetch_add(n as u64, Ordering::Relaxed);
    }
}

/// Download Eclipse Temurin `major` for `host` from api.adoptium.net into `dir` there, check
/// its SHA-256, unpack it, and return the new JDK. A remote machine fetches it itself; one
/// that cannot reach the internet gets it fetched on this Mac and copied over. Blocking;
/// run it off the main thread.
pub fn download_temurin(host: &dyn Host, major: u32, dir: &Path, progress: &DownloadProgress) -> anyhow::Result<Jdk> {
    use anyhow::{Context as _, bail};
    let arch = if host.info().arch == "aarch64" { "aarch64" } else { "x64" };
    let os = if host.info().is_mac() { "mac" } else { host.info().os.as_str() };
    let api = format!(
        "https://api.adoptium.net/v3/assets/latest/{major}/hotspot?architecture={arch}&image_type=jdk&os={os}&vendor=eclipse"
    );
    let remote = is_remote(host);
    let local = snout_host::local();
    let answer = match fetch_text(host, &api) {
        Ok(a) => a,
        Err(_) if remote => fetch_text(&*local, &api).context("ask Adoptium")?,
        Err(e) => return Err(e.context("ask Adoptium")),
    };
    let assets: serde_json::Value = serde_json::from_slice(&answer).context("read Adoptium's answer")?;
    let package = assets.get(0).and_then(|a| a.pointer("/binary/package")).context("no JDK for this machine")?;
    let link = package.get("link").and_then(|v| v.as_str()).context("no download link")?;
    let sha = package.get("checksum").and_then(|v| v.as_str()).unwrap_or_default().to_ascii_lowercase();
    let size = package.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
    progress.total.store(size, Ordering::Relaxed);

    host.create_dir_all(dir)?;
    let part = dir.join(format!(".temurin-{major}.tar.gz.part"));
    if let Err(e) = fetch_file(host, link, &part, progress) {
        if !remote {
            return Err(e);
        }
        // The remote machine cannot fetch it: this Mac does, and copies it over.
        let tmp = tempfile::Builder::new().prefix("snout-temurin").tempdir()?;
        let file = tmp.path().join("jdk.tar.gz");
        fetch_file(&*local, link, &file, progress).context("download the JDK on this Mac")?;
        if !sha.is_empty() && sha256_on(&*local, &file)? != sha {
            bail!("the download is damaged (its checksum does not match)");
        }
        upload(host, &file, &part, progress).context("copy the JDK to the remote machine")?;
    }
    progress.done.store(size, Ordering::Relaxed);
    if !sha.is_empty() && sha256_on(host, &part)? != sha {
        let _ = host.remove_file(&part);
        bail!("the download is damaged (its checksum does not match)");
    }
    let unpack = dir.join(format!(".temurin-{major}.unpack"));
    let _ = host.remove_dir_all(&unpack);
    host.create_dir_all(&unpack)?;
    let ok = host.run(&Cmd::new("tar").arg("-xzf").arg(&part).arg("-C").arg(&unpack), None)?.status.success();
    let _ = host.remove_file(&part);
    if !ok {
        bail!("the download could not be unpacked");
    }
    let top = host.read_dir(&unpack)?.into_iter().find(|e| e.is_dir).map(|e| e.path).context("an empty archive")?;
    let name = top.file_name().map(|n| n.to_owned()).context("an unnamed folder")?;
    let dest = dir.join(name);
    let _ = host.remove_dir_all(&dest);
    host.rename(&top, &dest)?;
    let _ = host.remove_dir_all(&unpack);
    [dest.join("Contents/Home"), dest.clone()].iter().find_map(|h| jdk_at(host, h)).context("the download holds no JDK")
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
        // Java 8 on Apple silicon: 11, which Temurin has for it.
        let eight = Requirement { declared: Some(8), gradle_max: Some(23) };
        assert_eq!((eight.wanted_on(false), eight.wanted_on(true)), (8, 11));
        assert!(eight.fits(11));
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
        assert_eq!(gradle_root(&*snout_host::local(), &src.join("App.java"), None), Some(dir.clone()));
        assert_eq!(requirement(&*snout_host::local(), &dir), Requirement { declared: Some(17), gradle_max: Some(19) });
        assert_eq!(gradle_root(&*snout_host::local(), &std::env::temp_dir().join("nowhere/X.java"), None), None);
        let _ = std::fs::remove_dir_all(dir);
    }
}
